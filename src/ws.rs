use axum::prelude::*;
use axum::service::ServiceExt;
use axum::ws::{ws, Message, WebSocket};
use futures::stream::{SplitSink, SplitStream};
use futures::{sink::SinkExt, stream::StreamExt};
use hyper::StatusCode;
use log::{debug, error, info};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{
    mpsc::{channel, Sender},
    oneshot,
    oneshot::error::TryRecvError,
    Mutex,
};
use tokio::time::{interval, sleep, Duration};
use tower::ServiceBuilder;
use tower_http::services::ServeDir;
use tower_http::{
    add_extension::AddExtensionLayer,
    trace::{DefaultMakeSpan, TraceLayer},
};

use serde::{Deserialize, Serialize};

use crate::data::SharedState;
use libwebrtc::errors::LibWebrtcError;
use libwebrtc::peerconnection::{PeerConnection, RTCConfiguration};
use libwebrtc::peerconnection_factory::PeerConnectionFactory;
use libwebrtc::peerconnection_observer::{
    IceConnectionState, PeerConnectionObserver, PeerConnectionObserverTrait,
};
use libwebrtc::rust_video_track_source::RustTrackVideoSource;
use libwebrtc::sdp::{SdpType, SessionDescription};
use libwebrtc::stats_collector::{DummyRTCStatsCollector, RTCStatsCollectorCallback};

type SafePeerConnectionFactory = Arc<Mutex<PeerConnectionFactory>>;

#[derive(Clone)]
struct ChannelPeerConnectionObserver {
    sender: Sender<String>,
}

impl PeerConnectionObserverTrait for ChannelPeerConnectionObserver {
    fn on_standardized_ice_connection_change(&mut self, state: IceConnectionState) {
        info!("new ice connection state: {:?}", state);
    }

    fn on_ice_candidate(&mut self, candidate_sdp: String, sdp_mid: String, sdp_mline_index: u32) {
        if let Err(e) = self.sender.blocking_send(candidate_sdp.clone()) {
            error!("could not send sdp candidate: {:?}", e);
        } else {
            info!("on ice candidate: {:?}", candidate_sdp);
        }
    }
}

impl ChannelPeerConnectionObserver {
    fn new(sender: Sender<String>) -> Box<Self> {
        Box::new(Self { sender })
    }

    fn drop_ref(obs: *mut Self) {
        unsafe { Box::from_raw(obs) };

        // drop here
        debug!("peerconnection observer dropped");
    }
}

#[derive(Debug, Clone)]
struct ChannelPCObsPtr<T: PeerConnectionObserverTrait> {
    pub _ptr: *mut T,
}

unsafe impl<T: PeerConnectionObserverTrait> Send for ChannelPCObsPtr<T> {}
unsafe impl<T: PeerConnectionObserverTrait> Sync for ChannelPCObsPtr<T> {}

/// Incoming websocket requests
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum MessageRequest {
    Sdp { r#type: String, sdp: String },
}

/// Outgoing websocket responses
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum MessageResponse {
    Sdp { r#type: String, sdp: String },
}

/// Create a new SDP
fn new_sdp<T: ToString, U: ToString>(kind: T, sdp: &U) -> Result<String, LibWebrtcError> {
    serde_json::to_string(&MessageResponse::Sdp {
        r#type: kind.to_string(),
        sdp: sdp.to_string(),
    })
    .map_err(|_e| LibWebrtcError::Generic("could not generate SDP"))
}

/// Log error, drop the channel peer connection observer reference,
/// and convert into a LibWebrtcError
fn error_drop_ref<T: ToString>(
    message: &'static str,
    error: T,
    holder: &ChannelPCObsPtr<ChannelPeerConnectionObserver>,
) -> LibWebrtcError {
    error!("{}", format!("{}: {}", message, error.to_string()));
    ChannelPeerConnectionObserver::drop_ref(holder._ptr);
    LibWebrtcError::Generic(message)
}

/// Create a peer connection
async fn create_peer_connection(
    holder: &ChannelPCObsPtr<ChannelPeerConnectionObserver>,
    shared_state: SharedState,
    video_source: &RustTrackVideoSource,
) -> Result<PeerConnection, LibWebrtcError> {
    let observer = { PeerConnectionObserver::new(holder._ptr).unwrap() };
    let pc = shared_state
        .lock()
        .await
        .peer_connection_factory
        .create_peer_connection(&observer, RTCConfiguration::default())?;

    // possible observer leak
    shared_state
        .lock()
        .await
        .peer_connection_factory
        .create_and_add_video_track(&pc, &video_source);

    Ok(pc)
}

/// Do the heavy lifting
async fn send_receive_offer(
    send: Arc<Mutex<SplitSink<WebSocket, Message>>>,
    recv: Arc<Mutex<SplitStream<WebSocket>>>,
    shared_state: SharedState,
    video_source: &RustTrackVideoSource,
    video_time: u32,
    iteration: u32,
) -> Result<(), LibWebrtcError> {
    let (tx, mut rx) = channel::<String>(10);
    let holder = ChannelPCObsPtr {
        _ptr: Box::into_raw(ChannelPeerConnectionObserver::new(tx.clone())),
    };
    let mut pc = create_peer_connection(&holder, shared_state, &video_source).await?;

    // create and send the offer SDP
    let offer = pc
        .create_offer()
        .map_err(|e| error_drop_ref("could not create offer", e, &holder))?;

    info!("sending offer");

    send.lock()
        .await
        .send(Message::text(new_sdp("offer", &offer)?))
        .await
        .map_err(|e| error_drop_ref("could not send offer", e, &holder))?;

    pc.set_local_description(offer)?;

    info!("set local description");

    // send the candidate response
    let sender = send.clone();

    tokio::spawn(async move {
        while let Some(cand) = rx.recv().await {
            if let Ok(sdp) = new_sdp("candidate", &cand) {
                let msg = Message::text(sdp);

                if let Err(e) = sender.lock().await.send(msg).await {
                    error!("couldn't send candidate response: {}", e);
                };
            }
        }
    });

    // check for an answer and candidate request
    let recv = recv.clone();

    while let Some(Ok(msg)) = recv.lock().await.next().await {
        let request = serde_json::from_slice::<MessageRequest>(msg.as_bytes())
            .map_err(|e| error_drop_ref("could not deserialize answer", e, &holder))?;

        match request {
            MessageRequest::Sdp { r#type, sdp } => {
                info!("incoming SDP: {:?} for video {}", r#type, iteration);

                // first, we recive an answer to our offer
                if r#type == "answer" {
                    let sdp = SessionDescription::from_string(SdpType::Answer, sdp.clone())
                        .map_err(|e| error_drop_ref("could not create answer sdp", e, &holder))?;

                    pc.set_remote_description(sdp).map_err(|e| {
                        error_drop_ref("could not set remote description", e, &holder)
                    })?;

                    info!("set remote description for video {}", iteration);

                    // break;
                };

                // next, we receive a candidate request
                if r#type == "candidate" {
                    info!("received candidate for video {}: {}", iteration, &sdp);
                    pc.add_ice_candidate_from_sdp(sdp)?;

                    // end listening for messages, we're done
                    break;
                }
            }
        }
    }

    // Stats task
    let mut pc_stats = pc.clone();
    let (_close_tx, mut close_rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        let stats: RTCStatsCollectorCallback = DummyRTCStatsCollector {}.into();
        let mut interval = interval(Duration::from_secs(1));
        interval.tick().await;

        loop {
            info!("collecting stats for video {}", iteration);
            match close_rx.try_recv() {
                Err(TryRecvError::Closed) | Ok(()) => return,
                Err(TryRecvError::Empty) => {}
            };
            let _ = pc_stats.get_stats(&stats);
            interval.tick().await;
        }
    });

    // pause in each thread to let the video stream
    // TODO: implement something better (channels?)
    sleep(Duration::from_secs(video_time.into())).await;

    // drop the reference
    ChannelPeerConnectionObserver::drop_ref(holder._ptr);

    // info!("closing websocket");

    // if let Err(e) = close_tx.send(()) {
    //     error!("could not send close_tx: {:?}", e)
    // };

    Ok(())
}

async fn handle_websocket(
    websocket: WebSocket,
    shared_state: SharedState,
    video_source: RustTrackVideoSource,
) -> Result<(), LibWebrtcError> {
    let delay = 3;
    let video_time = 60;

    log::info!("sending all offers in {} seconds", delay);
    log::info!("video will play for {} seconds", video_time);

    sleep(Duration::from_secs(delay)).await;

    let (send, recv) = websocket.split();
    let send = Arc::new(Mutex::new(send));
    let recv = Arc::new(Mutex::new(recv));

    for n in 1..=18 {
        // all of these clones are cheap
        let send = send.clone();
        let recv = recv.clone();
        let shared_state = shared_state.clone();
        let video_source = video_source.clone();

        sleep(Duration::from_millis(100)).await;

        tokio::spawn(async move {
            if let Err(e) = send_receive_offer(
                send.clone(),
                recv.clone(),
                shared_state.clone(),
                &video_source,
                video_time,
                n,
            )
            .await
            {
                error!("error sending and receiving an offer: {:?}", e);
            }
        });
    }

    Ok(())
}

async fn ws_connect_entry(
    websocket: WebSocket,
    extract::Extension(shared_state): extract::Extension<SharedState>,
    extract::Extension(video_source): extract::Extension<RustTrackVideoSource>,
) {
    if let Err(e) = handle_websocket(websocket, shared_state, video_source).await {
        error!("could not handle websocker: {:?}", e);
    }
}

pub(crate) async fn serve(shared_state: SharedState) {
    let static_directory = "static";

    let static_file_service =
        axum::service::get(ServeDir::new(static_directory).append_index_html_on_directories(true))
            .handle_error(|error: std::io::Error| {
                Ok::<_, std::convert::Infallible>((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Unhandled internal error: {}", error),
                ))
            });

    let video_source = RustTrackVideoSource::default();
    let (width, height) = (720, 480);
    video_source.start_gstreamer_thread_launch(
        & format!(
            "filesrc location=static/file.mp4 ! qtdemux name=demux demux.video_0 ! avdec_h264 ! videoconvert ! videoscale ! video/x-raw,format=I420,width={},height={}",
            width,
            height,
        ),
        width,
        height,
    );

    let app = axum::routing::nest("/", static_file_service)
        .route("/ws", ws(ws_connect_entry))
        .layer(AddExtensionLayer::new(shared_state))
        .layer(AddExtensionLayer::new(video_source))
        .layer(
            ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(DefaultMakeSpan::default().include_headers(true)),
                )
                .into_inner(),
        );

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    info!("Starting ws service on {:?}", &addr);

    hyper::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("could not start server");
}

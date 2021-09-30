use axum::prelude::{extract::Extension, *};
use axum::service::ServiceExt;
use axum::ws::{ws, Message, WebSocket};
use futures::stream::{SplitSink, SplitStream};
use futures::{sink::SinkExt, stream::StreamExt};
use hyper::StatusCode;
use log::{error, info};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tower::ServiceBuilder;
use tower_http::services::ServeDir;
use tower_http::{
    add_extension::AddExtensionLayer,
    trace::{DefaultMakeSpan, TraceLayer},
};

use serde::{Deserialize, Serialize};

use crate::data::SharedState;
use crate::peer_connection::{
    ChannelPCObsPtr, ChannelPeerConnectionObserver, PeerConnection, PeerConnectionQueueInner,
};

use libwebrtc::errors::LibWebrtcError;
use libwebrtc::rust_video_track_source::RustTrackVideoSource;
use libwebrtc::sdp::{SdpType, SessionDescription};

/// Incoming websocket requests
#[derive(Deserialize, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum MessageRequest {
    Offer { sdp: String },
    Answer { sdp: String },
    Candidate { sdp: String },
    CreatePeerConnection { name: String },
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
    shared_state: &SharedState,
    video_source: &RustTrackVideoSource,
    peer_connection_queue_inner: &PeerConnectionQueueInner,
) -> Result<PeerConnection, LibWebrtcError> {
    let PeerConnectionQueueInner {
        id,
        session_id: _,
        name,
    } = peer_connection_queue_inner;
    // create the peer connection from the peer connection factory
    let peer_connection = PeerConnection::new(
        &shared_state.lock().await.peer_connection_factory,
        id.into(),
        name.into(),
    )
    .await
    .map_err(|_e| LibWebrtcError::Generic("could not create peer connection"))?;

    // add the video track.  Chinmany note: possible observer leak
    shared_state
        .lock()
        .await
        .peer_connection_factory
        .create_and_add_video_track(&peer_connection.webrtc_peer_connection, &video_source);

    Ok(peer_connection)
}

/// use channels to send and receive offers, as well as candidate requests
async fn send_receive_offer(
    send: Arc<Mutex<SplitSink<WebSocket, Message>>>,
    recv: Arc<Mutex<SplitStream<WebSocket>>>,
    shared_state: SharedState,
    video_source: &RustTrackVideoSource,
    video_time_s: u32,
    peer_connection: &PeerConnectionQueueInner,
) -> Result<(), LibWebrtcError> {
    let mut pc = create_peer_connection(&shared_state, &video_source, &peer_connection).await?;
    let holder = &pc.holder;

    // create the offer SDP
    let offer = pc
        .webrtc_peer_connection
        .create_offer()
        .map_err(|e| error_drop_ref("could not create offer", e, &holder))?;

    info!("sending offer");

    // send the offer SDP
    send.lock()
        .await
        .send(Message::text(new_sdp("offer", &offer)?))
        .await
        .map_err(|e| error_drop_ref("could not send offer", e, &holder))?;

    pc.webrtc_peer_connection.set_local_description(offer)?;

    info!("set local description");

    // check for an answer and candidate request
    while let Some(Ok(msg)) = recv.lock().await.next().await {
        let request = serde_json::from_slice::<MessageRequest>(msg.as_bytes())
            .map_err(|e| error_drop_ref("could not deserialize answer", e, &holder))?;

        match request {
            // first, we recive an answer to our offer
            MessageRequest::Answer { sdp } => {
                let sdp = SessionDescription::from_string(SdpType::Answer, sdp.clone())
                    .map_err(|e| error_drop_ref("could not create answer sdp", e, &holder))?;

                pc.webrtc_peer_connection
                    .set_remote_description(sdp)
                    .map_err(|e| error_drop_ref("could not set remote description", e, &holder))?;

                info!("set remote description for video {}", &peer_connection.name);
            }
            // next, we receive a candidate request
            MessageRequest::Candidate { sdp } => {
                info!(
                    "received candidate for video {:?}: {}",
                    &peer_connection, &sdp
                );
                pc.webrtc_peer_connection.add_ice_candidate_from_sdp(sdp)?;

                // end listening for messages, we're done
                break;
            }
            // we don't care about other message types here
            _ => {
                error!("invalid message")
            }
        }
    }

    // send the candidate response
    while let Some(cand) = pc.receiver.recv().await {
        if let Ok(sdp) = new_sdp("candidate", &cand) {
            let msg = Message::text(sdp);

            if let Err(e) = send.lock().await.send(msg).await {
                error!("couldn't send candidate response: {}", e);
            };

            break;
        }
    }

    // drop the reference
    ChannelPeerConnectionObserver::drop_ref(holder._ptr);

    log::warn!("adding pc to session: {}", &peer_connection.session_id);

    // add the peer connection to the session
    shared_state
        .lock()
        .await
        .data
        .sessions
        .get_mut(&peer_connection.session_id)
        .ok_or_else(|| LibWebrtcError::Generic("invalid session"))?
        .add_peer_connection(pc)
        .await
        .map_err(|_e| LibWebrtcError::Generic("could not create peer connection"))?;

    // pause in each thread to let the video stream
    // TODO: implement something better (channels?)
    sleep(Duration::from_secs(video_time_s.into())).await;

    Ok(())
}

async fn handle_websocket(
    websocket: WebSocket,
    shared_state: SharedState,
    video_source: RustTrackVideoSource,
) -> Result<(), LibWebrtcError> {
    let video_time_s = 180;
    let poll_websocket_ms = 200;

    let (send, recv) = websocket.split();
    let send = Arc::new(Mutex::new(send));
    let recv = Arc::new(Mutex::new(recv));

    // establish an initial connection with the browser, and spawn
    loop {
        // check the peer connection queue for a new connection
        let peer_connection = shared_state
            .clone()
            .lock()
            .await
            .peer_connection_queue
            .pop_front();

        // if there is a new peer connection, perform the sdp handshake on a
        // separate thread to avoid blocking the main thread
        if let Some(peer_connection) = peer_connection {
            let send = send.clone();
            let recv = recv.clone();
            let shared_state = shared_state.clone();
            let video_source = video_source.clone();

            info!("new peer connection: {:?}", &peer_connection);

            tokio::spawn(async move {
                let offer = send_receive_offer(
                    send,
                    recv,
                    shared_state,
                    &video_source,
                    video_time_s,
                    &peer_connection,
                );

                if let Err(e) = offer.await {
                    error!("error sending and receiving an offer & candidate: {:?}", e);
                }
            });
        }

        sleep(Duration::from_millis(poll_websocket_ms)).await;

        // TODO: listen for ws disconnection and break out of loop
    }
}

// main websocket entry point
async fn ws_connect_entry(
    websocket: WebSocket,
    Extension(shared_state): Extension<SharedState>,
    Extension(video_source): Extension<RustTrackVideoSource>,
) {
    if let Err(e) = handle_websocket(websocket, shared_state, video_source).await {
        error!("could not handle websocker: {:?}", e);
    }
}

// stream a pre-encoded file from gstreamer to avoid encoding overhead
fn file_video_source() -> RustTrackVideoSource {
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

    video_source
}

// starts the server
pub(crate) async fn serve(shared_state: SharedState) {
    let video_source = file_video_source();
    let static_directory = "static";
    let static_file_service =
        axum::service::get(ServeDir::new(static_directory).append_index_html_on_directories(true))
            .handle_error(|error: std::io::Error| {
                Ok::<_, std::convert::Infallible>((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!(
                        "Unhandled internal error creating static file service: {}",
                        error
                    ),
                ))
            });

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

    let addr = SocketAddr::from(([127, 0, 0, 1], 3001));

    info!("Starting ws service on {:?}", &addr);

    hyper::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("could not start server");
}

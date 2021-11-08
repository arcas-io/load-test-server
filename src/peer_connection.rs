use crate::error::{Result, ServerError};
use crate::metrics::{write_video_rx_stats, write_video_tx_stats};

use core::fmt;
use cxx::{SharedPtr, UniquePtr};
use lazy_static::__Deref;
use libwebrtc::ffi::rtp_transceiver::C_RtpTransceiverDirection;
use libwebrtc::ffi::sdp::SdpType;
use libwebrtc::ffi::stats_collector::Rs_VideoSenderStats;
use libwebrtc::peerconnection::{
    IceServer, PeerConnection as WebRtcPeerConnection, RTCConfiguration,
};
use libwebrtc::peerconnection_factory::PeerConnectionFactory;
use libwebrtc::peerconnection_observer::{PeerConnectionObserver, PeerConnectionObserverTrait};
use libwebrtc::rtp_transceiver::RtpTransceiverInit;
use libwebrtc::rust_video_track_source::RustTrackVideoSource;
use libwebrtc::sdp::SessionDescription;
use libwebrtc::stats_collector::{DummyRTCStatsCollector, RTCStatsCollectorCallback};
use libwebrtc::video_track::VideoTrack;
use libwebrtc_sys::ffi::{
    create_arcas_video_track_source, ArcasICECandidate, ArcasICEServer, ArcasPeerConnection,
    ArcasPeerConnectionConfig, ArcasPeerConnectionFactory, ArcasPeerConnectionObserver,
    ArcasRTCConfiguration, ArcasRTPVideoTransceiver, ArcasSDPSemantics, ArcasSDPType,
    ArcasSessionDescription, ArcasVideoSenderStats, ArcasVideoTrack, ArcasVideoTrackSource,
};
use libwebrtc_sys::peer_connection::PeerConnectionObserverImpl;
use libwebrtc_sys::{
    peer_connection, ArcasRustCreateSessionDescriptionObserver, ArcasRustRTCStatsCollectorCallback,
    ArcasRustSetSessionDescriptionObserver,
};
use log::debug;
use std::pin::Pin;
use std::sync::mpsc::{self, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{info, warn};

// TODO: temp allowing dead code, only used in tests currently
#[allow(dead_code)]
pub(crate) struct PeerConnection<'a> {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) webrtc_peer_connection: UniquePtr<ArcasPeerConnection<'a>>,
    pub(crate) receiver: Receiver<UniquePtr<ArcasICECandidate>>,
    observer: SharedPtr<ArcasPeerConnectionObserver>,
}

impl<'a> fmt::Debug for PeerConnection<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "id={}, name={}", self.id, self.name)
    }
}

#[derive(Clone)]
pub(crate) struct ChannelPeerConnectionObserver {
    pub(crate) sender: Sender<UniquePtr<ArcasICECandidate>>,
}

impl peer_connection::PeerConnectionObserverImpl for ChannelPeerConnectionObserver {
    fn on_ice_candidate(&self, candidate: UniquePtr<ArcasICECandidate>) {
        info!(
            "XXX candidate generated: {} {} = {}",
            candidate.sdp_mid(),
            candidate.sdp_mline_index(),
            candidate.to_string(),
        );

        if self.sender.blocking_send(candidate).is_err() {
            warn!("could not pass sdp candidate");
        }
    }

    fn on_signaling_state_change(&self, state: libwebrtc_sys::ffi::ArcasRTCSignalingState) {
        info!("XXX signaling state: {:?}", state);
    }

    fn on_add_stream(&self, stream: UniquePtr<libwebrtc_sys::ffi::ArcasMediaStream>) {}

    fn on_remove_stream(&self, stream: UniquePtr<libwebrtc_sys::ffi::ArcasMediaStream>) {}

    fn on_datachannel(&self, data_channel: UniquePtr<libwebrtc_sys::ffi::ArcasDataChannel>) {}

    fn on_renegotiation_needed(&self) {}

    fn on_renegotiation_needed_event(&self, event: u32) {}

    fn on_ice_connection_change(&self, state: libwebrtc_sys::ffi::ArcasIceConnectionState) {
        info!("XXX ice change = {:?}", state);
    }

    fn on_connection_change(&self, state: libwebrtc_sys::ffi::ArcasPeerConnectionState) {
        info!("XXX: on connection change= {:?}", state);
    }

    fn on_ice_gathering_change(&self, state: libwebrtc_sys::ffi::ArcasIceGatheringState) {}

    fn on_ice_candidate_error(
        &self,
        host_candidate: String,
        url: String,
        error_code: i32,
        error_text: String,
    ) {
        info!("XXX candidate error = {}", error_text);
    }

    fn on_ice_candidate_error_address_port(
        &self,
        address: String,
        port: i32,
        url: String,
        error_code: i32,
        error_text: String,
    ) {
    }

    fn on_ice_candidates_removed(&self, removed: Vec<String>) {}

    fn on_ice_connection_receiving_change(&self, receiving: bool) {}

    fn on_ice_selected_candidate_pair_change(
        &self,
        event: libwebrtc_sys::ffi::ArcasCandidatePairChangeEvent,
    ) {
    }

    fn on_add_track(&self, receiver: UniquePtr<libwebrtc_sys::ffi::ArcasRTPReceiver>) {}

    fn on_track(&self, transceiver: UniquePtr<libwebrtc_sys::ffi::ArcasRTPTransceiver>) {}

    fn on_remove_track(&self, receiver: UniquePtr<libwebrtc_sys::ffi::ArcasRTPReceiver>) {}

    fn on_interesting_usage(&self, pattern: i32) {}
}

impl<'a> PeerConnection<'a> {
    pub(crate) fn new(
        peer_connection_factory: &ArcasPeerConnectionFactory<'a>,
        video_source: Pin<&mut ArcasVideoTrackSource>,
        id: String,
        name: String,
    ) -> Result<PeerConnection<'a>> {
        let (tx, rx) = tokio::sync::mpsc::channel::<UniquePtr<ArcasICECandidate>>(10);
        let observer = libwebrtc_sys::ffi::create_peer_connection_observer(Box::new(
            peer_connection::PeerConnectionObserverProxy::new(Box::new(
                ChannelPeerConnectionObserver { sender: tx },
            )),
        ));
        debug!("created pc observer");

        let config = Self::rtc_config();
        let webrtc_peer_connection = peer_connection_factory.create_peer_connection(
            config,
            // We have multiple references to this same pointer so we must
            // clone such that rust has one and C++ has another similar to an Arc.
            observer.clone(),
        );
        debug!("created peerconnection");

        // let track = peer_connection_factory.create_video_track("test".into(), video_source);
        // webrtc_peer_connection.add_video_track(track, vec!["0".into()]);

        let pc = PeerConnection {
            id,
            name,
            webrtc_peer_connection,
            receiver: rx,
            observer,
        };

        Ok(pc)
    }

    fn rtc_config() -> UniquePtr<ArcasRTCConfiguration<'static>> {
        let ice = ArcasICEServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            username: "".to_owned(),
            password: "".to_owned(),
        };
        let out = libwebrtc_sys::ffi::create_rtc_configuration(ArcasPeerConnectionConfig {
            ice_servers: vec![ice],
            sdp_semantics: ArcasSDPSemantics::kUnifiedPlan,
        });

        out
    }

    /// Send the callback to the rust ffi bindings and just listen for the first message.
    ///
    /// If the message fails, just return an empty vec.
    pub(crate) fn get_stats(&self) -> Vec<ArcasVideoSenderStats> {
        let (sender, receiver) = channel();
        let sender = Arc::new(Mutex::new(sender));
        let _ = self.webrtc_peer_connection.get_stats(Box::new(
            libwebrtc_sys::ArcasRustRTCStatsCollectorCallback::new(Box::new(
                move |_, _, video_send, _| {
                    sender.lock().unwrap().send(video_send).unwrap();
                },
            )),
        ));

        receiver.recv().unwrap_or_default()
    }

    pub(crate) fn create_offer(&mut self) -> Result<UniquePtr<ArcasSessionDescription>> {
        let (tx, rx) = mpsc::channel();

        // TODO: Actually return results.
        self.webrtc_peer_connection.create_offer(Box::new(
            ArcasRustCreateSessionDescriptionObserver::new(
                Box::new(move |session_description| {
                    tx.send(session_description)
                        .expect("Can send set desc message");
                }),
                Box::new(move |_err| assert!(false, "Failed to set description")),
            ),
        ));

        let sdp = rx.recv().expect("Can get offer");
        Ok(sdp)
    }

    pub(crate) fn create_answer(&mut self) -> Result<UniquePtr<ArcasSessionDescription>> {
        let (tx, rx) = mpsc::channel();

        // TODO: Actually return results.
        self.webrtc_peer_connection.create_answer(Box::new(
            ArcasRustCreateSessionDescriptionObserver::new(
                Box::new(move |session_description| {
                    tx.send(session_description)
                        .expect("Can send set desc message");
                }),
                Box::new(move |_err| assert!(false, "Failed to set description")),
            ),
        ));

        let sdp = rx.recv().expect("Can get offer");
        Ok(sdp)
    }

    pub(crate) fn set_local_description(
        &mut self,
        sdp_type: ArcasSDPType,
        sdp: String,
    ) -> Result<()> {
        let (set_tx, set_rx) = mpsc::channel();
        let observer = ArcasRustSetSessionDescriptionObserver::new(
            Box::new(move || {
                set_tx.send(1).expect("Can send set desc message");
            }),
            Box::new(move |_err| assert!(false, "Failed to set description")),
        );
        let sdp_create_result = libwebrtc_sys::ffi::create_arcas_session_description(sdp_type, sdp);
        if !sdp_create_result.ok {
            return Err(ServerError::CouldNotParseSdp(
                sdp_create_result.error.description,
            ));
        }

        let cc_observer = Box::new(observer);
        self.webrtc_peer_connection
            .set_local_description(cc_observer, sdp_create_result.session);
        set_rx.recv().expect("Can set description");

        Ok(())
    }

    pub(crate) fn set_remote_description(
        &mut self,
        sdp_type: ArcasSDPType,
        sdp: String,
    ) -> Result<()> {
        let (set_tx, set_rx) = mpsc::channel();
        let observer = ArcasRustSetSessionDescriptionObserver::new(
            Box::new(move || {
                set_tx.send(1).expect("Can send set desc message");
            }),
            Box::new(move |_err| assert!(false, "Failed to set description")),
        );
        let sdp_create_result = libwebrtc_sys::ffi::create_arcas_session_description(sdp_type, sdp);
        if !sdp_create_result.ok {
            return Err(ServerError::CouldNotParseSdp(
                sdp_create_result.error.description,
            ));
        }
        let cc_observer = Box::new(observer);
        self.webrtc_peer_connection
            .set_remote_description(cc_observer, sdp_create_result.session);
        set_rx.recv().expect("Can set description");

        Ok(())
    }

    fn create_track(
        peer_connection_factory: &ArcasPeerConnectionFactory<'a>,
        video_source: Pin<&mut ArcasVideoTrackSource>,
        label: String,
    ) -> Result<UniquePtr<ArcasVideoTrack<'a>>> {
        let out = peer_connection_factory.create_video_track(label, video_source);
        Ok(out)
    }

    pub(crate) fn add_track(
        &self,
        peer_connection_factory: &ArcasPeerConnectionFactory<'a>,
        video_source: Pin<&mut ArcasVideoTrackSource>,
        label: String,
    ) -> Result<()> {
        let track = Self::create_track(peer_connection_factory, video_source, label)?;
        let stream_ids = vec!["0".to_owned()];
        Ok(self
            .webrtc_peer_connection
            .add_video_track(track, stream_ids))
    }

    pub(crate) fn add_transceiver(
        &self,
        peer_connection_factory: &ArcasPeerConnectionFactory<'a>,
        video_source: Pin<&mut ArcasVideoTrackSource>,
        label: String,
    ) -> Result<UniquePtr<ArcasRTPVideoTransceiver>> {
        let init = libwebrtc_sys::ffi::ArcasTransceiverInit {
            stream_ids: vec!["0".into()],
            direction: libwebrtc_sys::ffi::ArcasCxxRtpTransceiverDirection::kSendRecv,
        };
        let track = Self::create_track(peer_connection_factory, video_source, label)?;
        let transciever = self
            .webrtc_peer_connection
            .add_video_transceiver_with_track(track, init);
        Ok(transciever)
    }

    pub fn start_gstreamer_thread_launch(
        src: UniquePtr<ArcasVideoTrackSource>,
        launch: String,
        width: i32,
        height: i32,
    ) {
        debug!("creating media stream");
        let rx: std::sync::mpsc::Receiver<bytes::BytesMut> =
            media_pipeline::create_and_start_appsink_pipeline(launch.as_str()).unwrap();

        thread::spawn(move || {
            while let Ok(buf) = rx.recv() {
                unsafe {
                    src.push_i420_data(width, height, width, width / 2, width / 2, buf.as_ptr());
                }
            }
        });
    }

    // stream a pre-encoded file from gstreamer to avoid encoding overhead
    pub(crate) fn file_video_source() -> UniquePtr<ArcasVideoTrackSource> {
        let video_source = create_arcas_video_track_source();
        let (width, height) = (720, 480);
        Self::start_gstreamer_thread_launch(
            video_source.clone(),
            format!(
                "filesrc location=static/file.mp4 ! qtdemux name=demux demux.video_0 ! avdec_h264 ! videoconvert ! videoscale ! video/x-raw,format=I420,width={},height={}",
                width,
                height,
            ),
            width,
            height,
        );

        video_source
    }

    pub(crate) fn export_stats(&self, session_id: &str) {
        let session_id = session_id.to_string();
        let pc_id = self.id.clone();
        let (tx, rx) = channel();
        self.webrtc_peer_connection
            .get_stats(Box::new(ArcasRustRTCStatsCollectorCallback::new(Box::new(
                move |video_receiver_stats,
                      _audio_receiver_stats,
                      video_sender_stats,
                      _audio_sender_stats| {
                    // TODO: This is ***VERY*** inefficient. Find way to persist required
                    // metrics in peerconnection wrapper object
                    for stat in &video_receiver_stats {
                        write_video_rx_stats(stat, &pc_id, &session_id);
                    }

                    for stat in &video_sender_stats {
                        write_video_tx_stats(stat, &pc_id, &session_id);
                    }
                    tx.send(1).expect("Can send stats message");
                },
            ))));
        rx.recv().expect("Can get stats");
    }
}

#[cfg(test)]
pub(crate) mod tests {

    use super::*;
    use libwebrtc::rust_video_track_source::RustTrackVideoSource;
    use libwebrtc_sys::ffi::ArcasAPI;
    use nanoid::nanoid;
    use tokio::time::{sleep, Duration};

    pub(crate) fn peer_connection_params<'a>() -> (
        UniquePtr<ArcasAPI<'a>>,
        UniquePtr<ArcasPeerConnectionFactory<'a>>,
        UniquePtr<ArcasVideoTrackSource>,
    ) {
        let api = libwebrtc_sys::ffi::create_arcas_api();
        let factory = api.create_factory();
        let video_source = PeerConnection::file_video_source();
        (api, factory, video_source)
    }

    #[tokio::test]
    async fn it_creates_a_new_peer_connection() {
        let (_api, factory, mut video_source) = peer_connection_params();
        PeerConnection::new(
            factory.as_ref().unwrap(),
            video_source.as_mut().unwrap(),
            nanoid!(),
            "new".into(),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn it_gets_stats_for_a_peer_connection() {
        let (_api, factory, mut video_source) = peer_connection_params();
        let pc = PeerConnection::new(
            factory.as_ref().unwrap(),
            video_source.as_mut().unwrap(),
            nanoid!(),
            "new".into(),
        )
        .unwrap();
        pc.get_stats();
    }

    #[tokio::test]
    async fn it_exports_stats_for_a_peer_connection() {
        let session_id = nanoid!();
        let (_api, factory, _video_source) = peer_connection_params();
        let mut video_source = PeerConnection::file_video_source();
        let mut pc = PeerConnection::new(
            factory.as_ref().unwrap(),
            video_source.as_mut().unwrap(),
            nanoid!(),
            "new".into(),
        )
        .unwrap();
        pc.add_track(
            factory.as_ref().unwrap(),
            video_source.as_mut().unwrap(),
            "Testlabel".into(),
        )
        .unwrap();
        let offer = pc.create_offer().unwrap();
        pc.set_local_description(offer.get_type(), offer.to_string())
            .unwrap();

        let mut pc_recv = PeerConnection::new(
            factory.as_ref().unwrap(),
            video_source.as_mut().unwrap(),
            nanoid!(),
            "new_recv".into(),
        )
        .unwrap();
        pc_recv
            .set_remote_description(offer.get_type(), offer.to_string())
            .unwrap();
        let answer = pc_recv.create_answer().unwrap();
        pc_recv
            .set_local_description(answer.get_type(), answer.to_string())
            .unwrap();
        pc.set_remote_description(answer.get_type(), answer.to_string())
            .unwrap();

        let pc_cand = pc.receiver.recv().await.unwrap();
        let pc_recv_cand = pc_recv.receiver.recv().await.unwrap();

        pc.webrtc_peer_connection.add_ice_candidate(pc_recv_cand);
        pc_recv.webrtc_peer_connection.add_ice_candidate(pc_cand);

        sleep(Duration::from_millis(500)).await;
        pc.export_stats(&session_id.to_owned());
        pc_recv.export_stats(&session_id.to_owned());
        sleep(Duration::from_millis(200)).await;
    }

    #[test]
    fn it_creates_an_offer() {
        let (_api, factory, mut video_source) = peer_connection_params();
        let mut pc = PeerConnection::new(
            factory.as_ref().unwrap(),
            video_source.as_mut().unwrap(),
            nanoid!(),
            "new".into(),
        )
        .unwrap();
        pc.create_offer().unwrap();
    }

    #[test]
    fn it_creates_an_answer() {
        let (_api, factory, mut video_source) = peer_connection_params();
        let mut pc = PeerConnection::new(
            factory.as_ref().unwrap(),
            video_source.as_mut().unwrap(),
            nanoid!(),
            "new".into(),
        )
        .unwrap();
        let offer = pc.create_offer().unwrap();
        pc.set_remote_description(offer.get_type(), offer.to_string())
            .unwrap();
        pc.create_answer().unwrap();
    }

    #[test]
    fn it_sets_local_description() {
        let (_api, factory, mut video_source) = peer_connection_params();
        let mut pc = PeerConnection::new(
            factory.as_ref().unwrap(),
            video_source.as_mut().unwrap(),
            nanoid!(),
            "new".into(),
        )
        .unwrap();
        let offer = pc.create_offer().unwrap();
        pc.set_local_description(offer.get_type(), offer.to_string())
            .unwrap();
    }

    #[test]
    fn it_sets_remote_description() {
        let (_api, factory, mut video_source) = peer_connection_params();
        let mut pc = PeerConnection::new(
            factory.as_ref().unwrap(),
            video_source.as_mut().unwrap(),
            nanoid!(),
            "new".into(),
        )
        .unwrap();
        let offer = pc.create_offer().unwrap();
        pc.set_remote_description(offer.get_type(), offer.to_string())
            .unwrap();
    }

    #[test]
    fn it_adds_a_track() {
        let (_api, factory, mut video_source) = peer_connection_params();
        let pc = PeerConnection::new(
            factory.as_ref().unwrap(),
            video_source.as_mut().unwrap(),
            nanoid!(),
            "new".into(),
        )
        .unwrap();
        pc.add_track(
            factory.as_ref().unwrap(),
            video_source.as_mut().unwrap(),
            "Testlabel".into(),
        )
        .unwrap();
    }

    #[test]
    fn it_adds_a_transceiver() {
        let (api_, factory, mut video_source) = peer_connection_params();
        let pc = PeerConnection::new(
            factory.as_ref().unwrap(),
            video_source.as_mut().unwrap(),
            nanoid!(),
            "new".into(),
        )
        .unwrap();
        pc.add_transceiver(
            factory.as_ref().unwrap(),
            video_source.as_mut().unwrap(),
            "Testlabel".into(),
        )
        .unwrap();
    }

    // #[test]
    // fn it_does_all_the_things() {
    //     let (factory, video_source) = peer_connection_params();
    //     let pc = PeerConnection::new(factory.as_ref().unwrap(), video_source.as_mut().unwrap(), nanoid!(), "new".into()).unwrap();
    //     pc.add_transceiver().unwrap();
    // }
}

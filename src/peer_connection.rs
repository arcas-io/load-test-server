use crate::error::{Result, ServerError};

use core::fmt;
use libwebrtc::ffi::stats_collector::Rs_VideoSenderStats;
use libwebrtc::peerconnection::{
    IceServer, PeerConnection as WebRtcPeerConnection, RTCConfiguration,
};
use libwebrtc::peerconnection_factory::PeerConnectionFactory;
use libwebrtc::peerconnection_observer::{PeerConnectionObserver, PeerConnectionObserverTrait};
use libwebrtc::rust_video_track_source::RustTrackVideoSource;
use libwebrtc::stats_collector::{DummyRTCStatsCollector, RTCStatsCollectorCallback};
use log::debug;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{info, warn};

// TODO: temp allowing dead code, only used in tests currently
#[allow(dead_code)]
pub(crate) struct PeerConnection {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) webrtc_peer_connection: WebRtcPeerConnection,
    pub(crate) observer: PeerConnectionObserver,
    pub(crate) receiver: Receiver<String>,
}

impl fmt::Debug for PeerConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "id={}, name={}", self.id, self.name)
    }
}

#[derive(Clone)]
pub(crate) struct ChannelPeerConnectionObserver {
    pub(crate) sender: Sender<String>,
}

impl PeerConnectionObserverTrait for ChannelPeerConnectionObserver {
    fn on_ice_candidate(&mut self, candidate_sdp: &str, sdp_mid: &str, sdp_mline_index: u32) {
        info!("candidate generated: {} {}", sdp_mid, sdp_mline_index);

        if self.sender.blocking_send(candidate_sdp.to_owned()).is_err() {
            warn!("could not pass sdp candidate");
        }
    }
}

impl PeerConnection {
    pub(crate) fn new(
        peer_connection_factory: &PeerConnectionFactory,
        video_source: &RustTrackVideoSource,
        id: String,
        name: String,
    ) -> Result<PeerConnection> {
        debug!("Creating observer");
        let (tx, rx) = tokio::sync::mpsc::channel::<String>(10);
        let observer = PeerConnectionObserver::new(ChannelPeerConnectionObserver { sender: tx })
            .map_err(|e| ServerError::CreatePeerConnectionError(e.to_string()))?;
        debug!("created pc observer");

        let webrtc_peer_connection = peer_connection_factory
            .create_peer_connection(&observer, Self::rtc_config())
            .map_err(|e| ServerError::CreatePeerConnectionError(e.to_string()))?;
        debug!("created peerconnection");

        // add the video track
        peer_connection_factory.create_and_add_video_track(&webrtc_peer_connection, &video_source);

        Ok(PeerConnection {
            id,
            name,
            webrtc_peer_connection,
            observer,
            receiver: rx,
        })
    }

    fn rtc_config() -> RTCConfiguration {
        RTCConfiguration {
            ice_servers: vec![IceServer {
                username: None,
                password: None,
                hostname: None,
                urls: vec!["stun:stun.l.google.com:19302".to_string()],
            }],
            ..Default::default()
        }
    }

    /// Send the callback to the rust ffi bindings and just listen for the first message.
    ///
    /// If the message fails, just return an empty vec.
    pub(crate) fn get_stats(&self) -> Vec<Rs_VideoSenderStats> {
        let (sender, receiver) = channel();
        let sender = Arc::new(Mutex::new(sender));
        let stats_collector = DummyRTCStatsCollector::new(sender);
        let stats_callback: RTCStatsCollectorCallback = stats_collector.into();
        let _ = self.webrtc_peer_connection.get_stats(&stats_callback);

        receiver.recv().unwrap_or_default()
    }

    // stream a pre-encoded file from gstreamer to avoid encoding overhead
    pub(crate) fn file_video_source() -> RustTrackVideoSource {
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
}

#[cfg(test)]
pub(crate) mod tests {

    use super::*;
    use nanoid::nanoid;

    pub(crate) fn peer_connection_params() -> (PeerConnectionFactory, RustTrackVideoSource) {
        let factory = PeerConnectionFactory::new().unwrap();
        let video_source = PeerConnection::file_video_source();
        (factory, video_source)
    }

    #[tokio::test]
    async fn it_creates_a_new_peer_connection() {
        let (factory, video_source) = peer_connection_params();
        PeerConnection::new(&factory, &video_source, nanoid!(), "new".into()).unwrap();
    }

    #[tokio::test]
    async fn it_gets_stats_for_a_peer_connection() {
        let (factory, video_source) = peer_connection_params();
        let pc = PeerConnection::new(&factory, &video_source, nanoid!(), "new".into()).unwrap();
        let _stats = pc.get_stats();
    }
}

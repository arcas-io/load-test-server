use crate::error::{Result, ServerError};
use crate::metrics::MetricsStatsCollectorCallback;

use core::fmt;
use libwebrtc::ffi::stats_collector::Rs_VideoSenderStats;
use libwebrtc::peerconnection::{
    IceServer, PeerConnection as WebRtcPeerConnection, RTCConfiguration,
};
use libwebrtc::peerconnection_factory::PeerConnectionFactory;
use libwebrtc::peerconnection_observer::{PeerConnectionObserver, PeerConnectionObserverTrait};
use libwebrtc::stats_collector::{DummyRTCStatsCollector, RTCStatsCollectorCallback};
use log::debug;
use std::collections::VecDeque;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{info, warn};

pub(crate) struct PeerConnection {
    pub(crate) id: String,
    pub(crate) session_id: String,
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

#[derive(Debug)]
pub(crate) struct PeerConnectionQueueInner {
    pub(crate) id: String,
    pub(crate) session_id: String,
    pub(crate) name: String,
}

/// Queue to fill on the gRPC side to be consumed by the websocket
pub(crate) type PeerConnectionQueue = VecDeque<PeerConnectionQueueInner>;

#[derive(Clone)]
pub(crate) struct ChannelPeerConnectionObserver {
    pub(crate) sender: Sender<String>,
}

impl PeerConnectionObserverTrait for ChannelPeerConnectionObserver {
    fn on_ice_candidate(&mut self, candidate_sdp: &str, sdp_mid: &str, sdp_mline_index: u32) {
        info!("candidate generated: {} {}", sdp_mid, sdp_mline_index);

        match self.sender.blocking_send(candidate_sdp.to_owned()) {
            Err(_err) => {
                warn!("could not pass sdp candidate");
            }
            _ => {}
        }
    }
}

impl PeerConnection {
    pub(crate) fn new(
        peer_connection_factory: &PeerConnectionFactory,
        id: String,
        session_id: String,
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

        Ok(PeerConnection {
            id,
            session_id,
            name,
            webrtc_peer_connection,
            observer,
            receiver: rx,
        })
    }

    fn rtc_config() -> RTCConfiguration {
        let mut config = RTCConfiguration::default();
        config.ice_servers = vec![IceServer {
            username: None,
            password: None,
            hostname: None,
            urls: vec!["stun:stun.l.google.com:19302".to_string()],
        }];
        config
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
        let stats = receiver.recv().unwrap_or(vec![]);

        stats
    }

    pub(crate) fn export_stats(&self) {
        let collector =
            MetricsStatsCollectorCallback::new(self.id.clone(), self.session_id.clone());
        let _ = self.webrtc_peer_connection.get_stats(&(collector.into()));
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use libwebrtc::rust_video_track_source::RustTrackVideoSource;
    use nanoid::nanoid;

    #[tokio::test]
    async fn it_creates_a_new_peer_connection() {
        let factory = PeerConnectionFactory::new().unwrap();
        PeerConnection::new(&factory, nanoid!(), "session_id".to_owned(), "new".into()).unwrap();
    }

    #[tokio::test]
    async fn it_gets_stats_for_a_peer_connection() {
        let factory = PeerConnectionFactory::new().unwrap();
        let pc = PeerConnection::new(&factory, nanoid!(), "session_id".to_owned(), "new".into())
            .unwrap();
        let _stats = pc.get_stats();
    }

    #[tokio::test]
    async fn it_exports_stats_for_a_peer_connection() {
        let factory = PeerConnectionFactory::new().unwrap();
        let source = RustTrackVideoSource::default();
        source.start_gstreamer_thread(720, 480);
        let mut pc =
            PeerConnection::new(&factory, nanoid!(), "session_id".to_owned(), "new".into())
                .unwrap();
        let track = factory
            .create_video_track(&source, "video".to_owned())
            .unwrap();

        pc.webrtc_peer_connection
            .add_track(track, vec!["0".to_owned()]);
        let offer = pc.webrtc_peer_connection.create_offer().unwrap();
        pc.webrtc_peer_connection
            .set_local_description(offer.clone())
            .unwrap();

        let mut pc_recv =
            PeerConnection::new(&factory, nanoid!(), "session_id".to_owned(), "new".into())
                .unwrap();
        pc_recv
            .webrtc_peer_connection
            .set_remote_description(offer)
            .unwrap();
        let answer = pc_recv.webrtc_peer_connection.create_answer().unwrap();

        pc_recv
            .webrtc_peer_connection
            .set_local_description(answer.clone())
            .unwrap();

        pc.webrtc_peer_connection
            .set_remote_description(answer)
            .unwrap();

        let pc_cand = pc.receiver.recv().await.unwrap();
        let pc_recv_cand = pc_recv.receiver.recv().await.unwrap();

        pc.webrtc_peer_connection
            .add_ice_candidate_from_sdp(pc_recv_cand)
            .unwrap();
        pc_recv
            .webrtc_peer_connection
            .add_ice_candidate_from_sdp(pc_cand)
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        pc.export_stats();
        pc_recv.export_stats();
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        let metrics = prometheus::gather();
        // log metrics
        metrics.iter().for_each(|m| log::debug!("{:?}", m));
    }
}

use crate::error::{Result, ServerError};

use core::fmt;
use libwebrtc::ffi::stats_collector::Rs_VideoSenderStats;
use libwebrtc::peerconnection::{
    IceServer, PeerConnection as WebRtcPeerConnection, RTCConfiguration,
};
use libwebrtc::peerconnection_factory::PeerConnectionFactory;
use libwebrtc::peerconnection_observer::{PeerConnectionObserver, PeerConnectionObserverTrait};
use libwebrtc::stats_collector::{DummyRTCStatsCollector, RTCStatsCollectorCallback};
use std::collections::VecDeque;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{info, warn};

pub(crate) struct PeerConnection<T: PeerConnectionObserverTrait> {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) webrtc_peer_connection: WebRtcPeerConnection,
    pub(crate) observer: PeerConnectionObserver<T>,
    pub(crate) receiver: Receiver<String>,
}

impl<T: PeerConnectionObserverTrait> fmt::Debug for PeerConnection<T> {
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
    sender: Sender<String>,
}

impl ChannelPeerConnectionObserver {
    pub(crate) fn new(sender: Sender<String>) -> Self {
        Self { sender }
    }
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

impl<T> PeerConnection<T> {
    pub(crate) async fn new(
        peer_connection_factory: &PeerConnectionFactory,
        id: String,
        name: String,
    ) -> Result<PeerConnection> {
        let (tx, rx) = tokio::sync::mpsc::channel::<String>(10);
        let observer = PeerConnectionObserver::new(ChannelPeerConnectionObserver::new(tx))
            .map_err(|e| ServerError::CreatePeerConnectionError(e.to_string()))?;

        let webrtc_peer_connection = peer_connection_factory
            .create_peer_connection(&observer, Self::rtc_config())
            .map_err(|e| ServerError::CreatePeerConnectionError(e.to_string()))?;

        Ok(PeerConnection {
            id,
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
}

#[cfg(test)]
mod tests {

    use super::*;
    use nanoid::nanoid;

    async fn new_peer_connection() -> Result<PeerConnection> {
        let peer_connection_factory = PeerConnectionFactory::new().unwrap();
        PeerConnection::new(
            &peer_connection_factory,
            nanoid!(),
            "New Peer Connection".into(),
        )
        .await
    }

    #[tokio::test]
    async fn it_creates_a_new_peer_connection() {
        new_peer_connection().await.unwrap();
    }

    #[tokio::test]
    async fn it_gets_stats_for_a_peer_connection() {
        let peer_connection = new_peer_connection().await.unwrap();
        let stats = peer_connection.get_stats();
    }
}

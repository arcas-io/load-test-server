use crate::error::{Result, ServerError};

use core::fmt;
use libwebrtc::ffi::stats_collector::Rs_VideoSenderStats;
use libwebrtc::peerconnection::{
    IceServer, PeerConnection as WebRtcPeerConnection, RTCConfiguration,
};
use libwebrtc::peerconnection_factory::PeerConnectionFactory;
use libwebrtc::peerconnection_observer::{
    IceConnectionState, PeerConnectionObserver, PeerConnectionObserverTrait,
};
use libwebrtc::stats_collector::{DummyRTCStatsCollector, RTCStatsCollectorCallback};
use std::collections::VecDeque;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{debug, info, warn};

pub(crate) struct PeerConnection {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) webrtc_peer_connection: WebRtcPeerConnection,
    pub(crate) holder: ChannelPCObsPtr<ChannelPeerConnectionObserver>,
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

pub(crate) type PeerConnectionQueue = VecDeque<PeerConnectionQueueInner>;

#[allow(dead_code)]
pub(crate) struct ChannelPCObsPtr<T: PeerConnectionObserverTrait> {
    pub(crate) _ptr: *mut T,
}

unsafe impl<T: PeerConnectionObserverTrait> Send for ChannelPCObsPtr<T> {}
unsafe impl<T: PeerConnectionObserverTrait> Sync for ChannelPCObsPtr<T> {}

#[derive(Clone)]
pub(crate) struct ChannelPeerConnectionObserver {
    sender: Sender<String>,
}

impl ChannelPeerConnectionObserver {
    pub(crate) fn new(sender: Sender<String>) -> Box<Self> {
        Box::new(Self { sender })
    }

    #[allow(dead_code)]
    pub(crate) fn drop_ref(obs: *mut Self) {
        unsafe { Box::from_raw(obs) };

        // drop here

        debug!("peerconnection observer dropped");
    }
}

impl PeerConnectionObserverTrait for ChannelPeerConnectionObserver {
    fn on_standardized_ice_connection_change(&mut self, state: IceConnectionState) {
        info!("new state: {:?}", state);
    }

    fn on_ice_candidate(&mut self, candidate_sdp: String, sdp_mid: String, sdp_mline_index: u32) {
        info!("candidate generated: {} {}", sdp_mid, sdp_mline_index);

        match self.sender.blocking_send(candidate_sdp) {
            Err(_err) => {
                warn!("could not pass sdp candidate");
            }
            _ => {}
        }
    }
}

impl PeerConnection {
    pub(crate) async fn new(
        peer_connection_factory: &PeerConnectionFactory,
        id: String,
        name: String,
    ) -> Result<PeerConnection> {
        let (tx, rx) = tokio::sync::mpsc::channel::<String>(10);
        let holder = ChannelPCObsPtr {
            _ptr: Box::into_raw(ChannelPeerConnectionObserver::new(tx)),
        };
        let observer = PeerConnectionObserver::new(holder._ptr)
            .map_err(|e| ServerError::CreatePeerConnectionError(e.to_string()))?;

        let webrtc_peer_connection = peer_connection_factory
            .create_peer_connection(&observer, Self::rtc_config())
            .map_err(|e| ServerError::CreatePeerConnectionError(e.to_string()))?;

        Ok(PeerConnection {
            id,
            name,
            webrtc_peer_connection,
            holder,
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

    pub(crate) fn get_stats(&self) -> Vec<Rs_VideoSenderStats> {
        let (sender, receiver) = channel();
        let sender = Arc::new(Mutex::new(sender));
        let stats_collector = DummyRTCStatsCollector::new(sender);
        let stats_callback: RTCStatsCollectorCallback = stats_collector.into();
        let _ = self.webrtc_peer_connection.get_stats(&stats_callback);
        let stats = receiver.recv().unwrap();
        stats
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use nanoid::nanoid;

    #[tokio::test]
    async fn it_creates_a_new_peer_connection() {
        tracing_subscriber::fmt::init();
        let peer_connection_factory = PeerConnectionFactory::new().unwrap();
        let _peer_connection = PeerConnection::new(
            &peer_connection_factory,
            nanoid!(),
            "New Peer Connection".into(),
        )
        .await
        .unwrap();
    }
}

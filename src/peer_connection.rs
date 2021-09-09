use crate::error::{Result, ServerError};

use libwebrtc::peerconnection::{PeerConnection as WebRtcPeerConnection, RTCConfiguration};
use libwebrtc::peerconnection_factory::PeerConnectionFactory;
use libwebrtc::peerconnection_observer::{
    IceConnectionState, PeerConnectionObserver, PeerConnectionObserverTrait,
};
use nanoid::nanoid;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

#[derive(Debug)]
pub(crate) struct PeerConnection {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) webrtc_peer_connection: WebRtcPeerConnection,
}

#[allow(dead_code)]
struct ChannelPCObsPtr<T: PeerConnectionObserverTrait> {
    pub _ptr: *mut T,
}

unsafe impl<T: PeerConnectionObserverTrait> Send for ChannelPCObsPtr<T> {}
unsafe impl<T: PeerConnectionObserverTrait> Sync for ChannelPCObsPtr<T> {}

#[derive(Clone)]
struct ChannelPeerConnectionObserver {
    sender: Sender<String>,
}

impl ChannelPeerConnectionObserver {
    fn new(sender: Sender<String>) -> Box<Self> {
        Box::new(Self { sender })
    }

    #[allow(dead_code)]
    fn drop_ref(obs: *mut Self) {
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
        peer_connection_factory: Arc<Mutex<PeerConnectionFactory>>,
        name: String,
    ) -> Result<PeerConnection> {
        let (tx, mut _rx) = tokio::sync::mpsc::channel::<String>(10);
        let holder = ChannelPCObsPtr {
            _ptr: Box::into_raw(ChannelPeerConnectionObserver::new(tx)),
        };
        let observer = PeerConnectionObserver::new(holder._ptr)
            .map_err(|e| ServerError::CreatePeerConnectionError(e.to_string()))?;

        let webrtc_peer_connection = peer_connection_factory
            .lock()
            .await
            .create_peer_connection(&observer, RTCConfiguration::default())
            .map_err(|e| ServerError::CreatePeerConnectionError(e.to_string()))?;

        ChannelPeerConnectionObserver::drop_ref(holder._ptr);

        Ok(PeerConnection {
            id: nanoid!(),
            name,
            webrtc_peer_connection,
        })
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn it_creates_a_new_peer_connection() {
        tracing_subscriber::fmt::init();
        let peer_connection_factory = Arc::new(Mutex::new(PeerConnectionFactory::new().unwrap()));
        let _peer_connection =
            PeerConnection::new(peer_connection_factory, "New Peer Connection".into())
                .await
                .unwrap();
    }
}

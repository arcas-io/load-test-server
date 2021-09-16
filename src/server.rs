use crate::data::Data;
use crate::error::{Result, ServerError};
use libwebrtc::peerconnection_factory::PeerConnectionFactory;
use log::info;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Server;
use webrtc::web_rtc_server::WebRtcServer;

pub(crate) mod webrtc {
    tonic::include_proto!("webrtc");
}

#[derive(Debug)]
pub(crate) struct MyWebRtc {
    pub(crate) data: Arc<Mutex<Data>>,
    pub(crate) peer_connection_factory: Arc<Mutex<PeerConnectionFactory>>,
}

pub(crate) async fn serve(addr: &str) -> Result<()> {
    let addr = addr.parse()?;
    let data = Arc::new(Mutex::new(Data::new()));
    let peer_connection_factory = PeerConnectionFactory::new()
        .map_err(|e| ServerError::CreatePeerConnectionError(e.to_string()))?;
    let peer_connection_factory = Arc::new(Mutex::new(peer_connection_factory));
    let mywebrtc = MyWebRtc {
        data,
        peer_connection_factory,
    };
    let service = WebRtcServer::new(mywebrtc);

    info!("Starting gPRC service on {:?}", addr);

    Server::builder()
        .add_service(service)
        .serve(addr)
        .await
        .map_err(|e| ServerError::InternalError(e.to_string()))?;

    Ok(())
}

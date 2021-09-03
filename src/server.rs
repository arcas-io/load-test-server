use crate::error::{Result, ServerError};
use crate::session::SessionStorage;
use std::sync::{Arc, Mutex};
use tonic::transport::Server;
use tracing::info;
use webrtc::web_rtc_server::WebRtcServer;

pub(crate) mod webrtc {
    tonic::include_proto!("webrtc");
}

#[derive(Debug)]
pub(crate) struct MyWebRtc {
    pub(crate) sessions: Arc<Mutex<SessionStorage>>,
}

pub(crate) async fn serve(addr: &str) -> Result<()> {
    tracing_subscriber::fmt::init();

    let addr = addr.parse()?;
    let session_storage = SessionStorage::new();
    let mywebrtc = MyWebRtc {
        sessions: Arc::new(Mutex::new(session_storage)),
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

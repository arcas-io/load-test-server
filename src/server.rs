use crate::data::SharedState;
use crate::error::{Result, ServerError};
use log::info;
use tonic::transport::Server;
use webrtc::web_rtc_server::WebRtcServer;

pub(crate) mod webrtc {
    tonic::include_proto!("webrtc");
}

pub(crate) async fn serve(addr: &str, shared_state: SharedState) -> Result<()> {
    let addr = addr.parse()?;
    let service = WebRtcServer::new(shared_state);

    info!("Starting gPRC service on {:?}", addr);

    Server::builder()
        .add_service(service)
        .serve(addr)
        .await
        .map_err(|e| ServerError::InternalError(e.to_string()))?;

    Ok(())
}

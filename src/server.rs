use crate::data::Data;
use crate::error::{Result, ServerError};
use std::sync::{Arc, Mutex};
use tonic::transport::Server;
use tracing::info;
use webrtc::web_rtc_server::WebRtcServer;

pub(crate) mod webrtc {
    tonic::include_proto!("webrtc");
}

#[derive(Debug)]
pub(crate) struct MyWebRtc {
    pub(crate) data: Arc<Mutex<Data>>,
}

pub(crate) async fn serve(addr: &str) -> Result<()> {
    tracing_subscriber::fmt::init();

    let addr = addr.parse()?;
    let data = Arc::new(Mutex::new(Data::new()));
    let mywebrtc = MyWebRtc { data };
    let service = WebRtcServer::new(mywebrtc);

    info!("Starting gPRC service on {:?}", addr);

    Server::builder()
        .add_service(service)
        .serve(addr)
        .await
        .map_err(|e| ServerError::InternalError(e.to_string()))?;

    Ok(())
}

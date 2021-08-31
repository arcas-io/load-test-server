use crate::session::SessionStorage;
use std::sync::{Arc, Mutex};
use tonic::transport::Server;
use webrtc::web_rtc_server::WebRtcServer;

pub(crate) mod webrtc {
    tonic::include_proto!("webrtc");
}

#[derive(Debug, Default)]
pub(crate) struct MyWebRtc {
    pub(crate) sessions: Arc<Mutex<SessionStorage>>,
}

pub(crate) async fn run(addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let addr = addr.parse()?;
    let session_storage = SessionStorage::new();
    let mywebrtc = MyWebRtc {
        sessions: Arc::new(Mutex::new(session_storage)),
    };
    let service = WebRtcServer::new(mywebrtc);

    Server::builder().add_service(service).serve(addr).await?;

    Ok(())
}

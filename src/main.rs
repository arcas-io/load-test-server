mod data;
mod error;
mod handlers;
mod helpers;
mod peer_connection;
mod server;
mod session;
mod stats;

use crate::data::Data;
use crate::error::Result;
use crate::error::ServerError;
use crate::server::serve;
use data::SharedState;
use libwebrtc::peerconnection_factory::PeerConnectionFactory;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let peer_connection_factory = PeerConnectionFactory::new()
        .map_err(|e| ServerError::CreatePeerConnectionError(e.to_string()))?;
    let shared_state = SharedState {
        data: Data::new(),
        peer_connection_factory,
    };

    // run the gRPC server
    let addr = "[::1]:50051";
    serve(addr, shared_state).await
}

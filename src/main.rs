mod data;
mod error;
mod handlers;
mod helpers;
mod metrics;
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
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let peer_connection_factory = PeerConnectionFactory::new()
        .map_err(|e| ServerError::CreatePeerConnectionError(e.to_string()))?;
    let shared_state = SharedState {
        data: Arc::from(Data::new()),
        peer_connection_factory,
        peer_connection_queue: Arc::from(Mutex::from(VecDeque::new())),
    };

    shared_state.start_metrics_collection();

    // run the gRPC server
    let addr = "[::1]:50051";
    serve(addr, shared_state).await
}

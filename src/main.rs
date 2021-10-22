mod config;
mod data;
mod error;
mod handlers;
mod helpers;
mod metrics;
mod peer_connection;
mod server;
mod session;
mod stats;

use crate::config::CONFIG;
use crate::data::{Data, SharedState};
use crate::error::{Result, ServerError};
use crate::server::serve;
use libwebrtc::peerconnection_factory::PeerConnectionFactory;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let peer_connection_factory = PeerConnectionFactory::new()
        .map_err(|e| ServerError::CreatePeerConnectionError(e.to_string()))?;
    let shared_state = SharedState {
        data: Arc::from(Data::new()),
        peer_connection_factory,
    };

    shared_state.start_metrics_collection();

    // run the gRPC server
    let addr = format!("{}:{}", CONFIG.host, CONFIG.port);
    serve(&addr, shared_state).await
}

mod data;
mod error;
mod handlers;
mod helpers;
mod peer_connection;
mod server;
mod session;
mod stats;
mod ws;

use crate::data::{Data, SharedStateInner};
use crate::error::Result;
use crate::error::ServerError;
use crate::server::serve;
use libwebrtc::peerconnection_factory::PeerConnectionFactory;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let peer_connection_factory = PeerConnectionFactory::new()
        .map_err(|e| ServerError::CreatePeerConnectionError(e.to_string()))?;
    let shared_state = Arc::new(Mutex::new(SharedStateInner {
        data: Data::new(),
        peer_connection_factory,
        peer_connection_queue: VecDeque::new(),
    }));

    // run the ws server in a separate thread
    let ws_shared_state = shared_state.clone();
    tokio::spawn(async { ws::serve(ws_shared_state).await });

    // run the gRPC server
    let addr = "[::1]:50051";
    serve(addr, shared_state).await
}

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
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();
    let arcas_api = libwebrtc_sys::ffi::create_arcas_api();
    let peer_connection_factory = arcas_api.create_factory();

    let shared_state = SharedState {
        data: Arc::from(Data::new()),
        peer_connection_factory,
        arcas_api,
    };

    shared_state.start_metrics_collection();

    // run the gRPC server
    let addr = format!("{}:{}", CONFIG.host, CONFIG.port);
    serve(&addr, shared_state).await
}

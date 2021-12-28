mod config;
mod data;
mod error;
mod handlers;
mod helpers;
mod log;
mod metrics;
mod peer_connection;
pub(crate) mod server;
mod session;
mod stats;
pub mod webrtc_pool;

use crate::config::CONFIG;
use crate::data::{Data, SharedState};
use crate::error::Result;
use crate::log::LogLevel;
use crate::server::serve;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();
    LogLevel::set_log_level(&LogLevel::default());

    let shared_state = SharedState {
        data: Arc::from(Data::new()),
    };

    // start exporting stats
    shared_state.start_metrics_collection();

    // run the gRPC server
    let addr = format!("{}:{}", CONFIG.host, CONFIG.port);
    serve(&addr, shared_state).await
}

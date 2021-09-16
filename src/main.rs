mod data;
mod error;
mod handlers;
mod helpers;
mod peer_connection;
mod server;
mod session;
mod stats;
mod ws;

use crate::error::Result;
use crate::server::serve;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    // run the ws server in a separate thread
    tokio::spawn(async { ws::serve().await });

    // run the gRPC server
    let addr = "[::1]:50051";
    serve(addr).await
}

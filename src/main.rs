mod data;
mod error;
mod handlers;
mod helpers;
mod peer_connection;
mod server;
mod session;
mod stats;

use crate::error::Result;
use crate::server::serve;

#[tokio::main]
async fn main() -> Result<()> {
    let addr = "[::1]:50051";
    serve(addr).await
}

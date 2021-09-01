mod error;
mod server;
mod session;

use crate::error::Result;
use crate::server::serve;

#[tokio::main]
async fn main() -> Result<()> {
    let addr = "[::1]:50051";
    serve(addr).await
}

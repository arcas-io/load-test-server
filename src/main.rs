mod server;
mod session;

use crate::server::run;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051";
    run(addr).await
}

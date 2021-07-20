use offer_websocket::*;
use warp::Filter;

mod crypto;
mod dtls;
mod endpoint_read_write;
mod error;
mod mux;
mod offer_websocket;
mod sdp;
mod utils;

// actix_web also boots tokio
#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let routes = warp::path("offer")
        // The `ws()` filter will prepare the Websocket handshake.
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| {
            // And then our closure will be called when it completes...
            ws.on_upgrade(|websocket| handle_offer_websocket(websocket))
        });

    warp::serve(routes).run(([127, 0, 0, 1], 60023)).await;

    Ok(())
}

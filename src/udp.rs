
use std::net::SocketAddr;
use std::{io};
use tokio::net::UdpSocket;

pub struct UDPServer {
    pub socket: UdpSocket,
    pub buf: Vec<u8>,
    pub to_send: Option<(usize, SocketAddr)>,
}

impl UDPServer {
    pub async fn run(self) -> Result<(), io::Error> {
        let UDPServer {
            socket,
            mut buf,
            mut to_send,
        } = self;

        loop {
          println!("nuitbar? {}", socket.local_addr().unwrap());
            // First we check to see if there's a message we need to echo back.
            // If so then we try to send it back to the original source, waiting
            // until it's writable and we're able to do so.
            if let Some((size, peer)) = to_send {
                let amt = socket.send_to(&buf[..size], &peer).await?;

                println!("Echoed {}/{} bytes to {}", amt, size, peer);
            }

            // If we're here then `to_send` is `None`, so we take a look for the
            // next message we're going to echo back.
            to_send = Some(socket.recv_from(&mut buf).await?);
        }
    }
}
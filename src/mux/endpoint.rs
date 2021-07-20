use crate::mux::mux_func::MatchFunc;
use webrtc_util::{Buffer, Conn};

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::Mutex;

/// Endpoint implements net.Conn. It is used to read muxed packets.
pub struct Endpoint {
    pub(crate) id: usize,
    pub(crate) buffer: Buffer,
    pub(crate) match_fn: MatchFunc,
    pub(crate) next_conn: Arc<dyn Conn + Send + Sync>,
    pub(crate) endpoints: Arc<Mutex<HashMap<usize, Arc<Endpoint>>>>,
}

impl std::fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Endpoint").field("id", &self.id).finish()
    }
}

impl Endpoint {
    /// Close unregisters the endpoint from the Mux
    pub async fn close(&self) -> Result<()> {
        self.buffer.close().await;

        let mut endpoints = self.endpoints.lock().await;
        endpoints.remove(&self.id);

        Ok(())
    }
}

#[async_trait]
impl Conn for Endpoint {
    async fn connect(&self, _addr: SocketAddr) -> Result<()> {
        Err(io::Error::new(io::ErrorKind::Other, "Not applicable").into())
    }

    /// reads a packet of len(p) bytes from the underlying conn
    /// that are matched by the associated MuxFunc
    async fn recv(&self, buf: &mut [u8]) -> Result<usize> {
        match self.buffer.read(buf, None).await {
            Ok(n) => Ok(n),
            Err(err) => Err(io::Error::new(io::ErrorKind::Other, err.to_string()).into()),
        }
    }
    async fn recv_from(&self, _buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        Err(io::Error::new(io::ErrorKind::Other, "Not applicable").into())
    }

    /// writes bytes to the underlying conn
    async fn send(&self, buf: &[u8]) -> Result<usize> {
        self.next_conn.send(buf).await
    }

    async fn send_to(&self, _buf: &[u8], _target: SocketAddr) -> Result<usize> {
        Err(io::Error::new(io::ErrorKind::Other, "Not applicable").into())
    }
    async fn local_addr(&self) -> Result<SocketAddr> {
        self.next_conn.local_addr().await
    }
}

#[async_trait]
impl io::Read for Endpoint {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        futures::executor::block_on(async {
            match self.buffer.read(buf, None).await {
                Ok(n) => Ok(n),
                Err(err) => Err(io::Error::new(io::ErrorKind::Other, err.to_string()).into()),
            }
        })
    }
}

#[async_trait]
impl io::Write for Endpoint {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        futures::executor::block_on(async {
            self.next_conn
                .send(buf)
                .await
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to write for Endpoint"))
        })
    }

    fn flush(&mut self) -> std::result::Result<(), std::io::Error> {
        Ok(())
    }
}

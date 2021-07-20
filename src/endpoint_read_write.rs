use std::{io, io::ErrorKind, sync::Arc, task::Poll};

use async_trait::async_trait;
use futures_util::FutureExt;
use log::error;
use std::io::{Read, Write};
use tokio::io::{AsyncRead, AsyncWrite};
use webrtc_util::Conn;

use crate::mux::endpoint::Endpoint;

#[derive(Debug)]
pub struct EndpointReadWrite {
    pub conn: Arc<Endpoint>,
}

impl EndpointReadWrite {
    pub fn new(conn: Arc<Endpoint>) -> EndpointReadWrite {
        EndpointReadWrite { conn }
    }
}

impl AsyncRead for EndpointReadWrite {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf_out: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let buf: &mut [u8] = &mut [0; 1400];
        let poll = self.conn.recv(buf).poll_unpin(cx);
        match poll {
            Poll::Pending => Poll::Pending,
            Poll::Ready(read_result) => match read_result {
                Ok(bytes_read) => {
                    buf_out.put_slice(&buf[0..bytes_read]);
                    Poll::Ready(Ok(()))
                }
                Err(err) => {
                    error!("error forwarding connection read: {:?}", err);
                    Poll::Ready(Err(ErrorKind::Unsupported.into()))
                }
            },
        }
    }
}

impl AsyncWrite for EndpointReadWrite {
    fn is_write_vectored(&self) -> bool {
        false
    }

    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        match self.conn.send(buf).poll_unpin(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(write_result) => match write_result {
                Ok(bytes_written) => Poll::Ready(Ok(bytes_written)),
                Err(err) => {
                    error!("error writing to conn read write: {:?}", err);
                    Poll::Ready(Err(ErrorKind::Unsupported.into()))
                }
            },
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        log::info!("poll_flush {:?}", cx);
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        log::info!("poll_shutdown");
        Poll::Ready(Ok(()))
    }
}

#[async_trait]
impl io::Read for EndpointReadWrite {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        futures::executor::block_on(async {
            match self.conn.buffer.read(buf, None).await {
                Ok(n) => Ok(n),
                Err(err) => Err(io::Error::new(io::ErrorKind::Other, err.to_string()).into()),
            }
        })
    }
}

#[async_trait]
impl io::Write for EndpointReadWrite {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        futures::executor::block_on(async {
            self.conn
                .next_conn
                .send(buf)
                .await
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to write for Endpoint"))
        })
    }

    fn flush(&mut self) -> std::result::Result<(), std::io::Error> {
        Ok(())
    }
}

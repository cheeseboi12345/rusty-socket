//! A minimal websocket implementation

use smol::io::{AsyncRead, AsyncWrite};
use smol::prelude::*;
use std::pin::Pin;
use thiserror::Error;

mod frame;
mod handshake;
mod mask;

pub use frame::{Frame, OpCode};
use handshake::{client_handshake, server_handshake};

/// Represents errors that can occur in WebSocket operations.
#[derive(Debug, Error)]
pub enum Error {
    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// A WebSocket protocol error occurred.
    #[error("WebSocket protocol error: {0}")]
    Protocol(String),
    /// The WebSocket connection was closed.
    #[error("WebSocket connection closed")]
    ConnectionClosed,
}

/// A Result type alias for WebSocket operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Represents a WebSocket connection.
pub struct WebSocket<S> {
    stream: S,
    is_client: bool,
}

impl<S> WebSocket<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    /// Accepts a WebSocket connection as a server.
    ///
    /// # Errors
    ///
    /// Returns an error if the handshake fails.
    pub async fn accept(stream: S) -> Result<Self> {
        let mut ws = WebSocket { stream, is_client: false };
        server_handshake(&mut ws.stream).await?;
        Ok(ws)
    }

    /// Connects to a WebSocket server as a client.
    ///
    /// # Errors
    ///
    /// Returns an error if the handshake fails.
    pub async fn connect(stream: S) -> Result<Self> {
        let mut ws = WebSocket { stream, is_client: true };
        client_handshake(&mut ws.stream).await?;
        Ok(ws)
    }

    /// Sends a WebSocket frame.
    ///
    /// # Errors
    ///
    /// Returns an error if sending the frame fails.
    pub async fn send(&mut self, frame: Frame) -> Result<()> {
        let mut data = frame.to_bytes();
        if self.is_client {
            mask::mask_frame(&mut data);
        }
        self.stream.write_all(&data).await?;
        Ok(())
    }

    /// Receives a WebSocket frame.
    ///
    /// # Errors
    ///
    /// Returns an error if receiving the frame fails or if the frame is invalid.
    pub async fn receive(&mut self) -> Result<Frame> {
        let frame = Frame::read_from(&mut self.stream).await?;
        if !self.is_client && frame.is_masked() {
            return Err(Error::Protocol("Client frames must be masked".into()));
        }
        if self.is_client && !frame.is_masked() {
            return Err(Error::Protocol("Server frames must not be masked".into()));
        }
        Ok(frame)
    }

    /// Closes the WebSocket connection.
    ///
    /// # Errors
    ///
    /// Returns an error if closing the connection fails.
    pub async fn close(mut self) -> Result<()> {
        let close_frame = Frame::close(None);
        self.send(close_frame).await?;
        // Wait for the close frame from the other side
        loop {
            match self.receive().await {
                Ok(frame) if frame.is_close() => break,
                Ok(_) => continue,
                Err(Error::ConnectionClosed) => break,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncRead for WebSocket<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncWrite for WebSocket<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_close(cx)
    }
}

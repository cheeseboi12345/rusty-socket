//! WebSocket handshake implementation.

use smol::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use smol::prelude::*;
use std::collections::HashMap;

use crate::{Error, Result};

const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// Performs the server-side WebSocket handshake.
///
/// # Errors
///
/// Returns an error if the handshake fails.
pub async fn server_handshake<S>(stream: &mut S) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut headers = HashMap::new();
    let mut buf_reader = BufReader::new(stream);

    loop {
        let mut line = String::new();
        buf_reader.read_line(&mut line).await?;
        if line == "\r\n" {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(
                key.trim().to_lowercase(),
                value.trim().to_string(),
            );
        }
    }

    let key = headers.get("sec-websocket-key").ok_or_else(|| {
        Error::Protocol("Missing Sec-WebSocket-Key header".into())
    })?;

    let response_key = generate_accept_value(key);

    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {}\r\n\r\n",
        response_key
    );

    buf_reader.get_mut().write_all(response.as_bytes()).await?;
    Ok(())
}

/// Performs the client-side WebSocket handshake.
///
/// # Errors
///
/// Returns an error if the handshake fails.
pub async fn client_handshake<S>(stream: &mut S) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let key = generate_random_key();
    let request = format!(
        "GET / HTTP/1.1\r\n\
         Host: server.example.com\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {}\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n",
        key
    );

    stream.write_all(request.as_bytes()).await?;

    let mut headers = HashMap::new();
    let mut buf_reader = BufReader::new(stream);

    loop {
        let mut line = String::new();
        buf_reader.read_line(&mut line).await?;
        if line == "\r\n" {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(
                key.trim().to_lowercase(),
                value.trim().to_string(),
            );
        }
    }

    let accept = headers.get("sec-websocket-accept").ok_or_else(|| {
        Error::Protocol("Missing Sec-WebSocket-Accept header".into())
    })?;

    let expected_accept = generate_accept_value(&key);
    if *accept != expected_accept {
        return Err(Error::Protocol("Invalid Sec-WebSocket-Accept value".into()));
    }

    Ok(())
}

/// Generates the accept value for the WebSocket handshake.
fn generate_accept_value(key: &str) -> String {
    use sha1::{Sha1, Digest};
    let mut hasher = Sha1::new();
    hasher.update(key.as_bytes());
    hasher.update(WEBSOCKET_GUID.as_bytes());
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &hasher.finalize())
}

/// Generates a random key for the WebSocket handshake.
fn generate_random_key() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let random_bytes: Vec<u8> = (0..16).map(|_| rng.gen()).collect();
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &random_bytes)
}

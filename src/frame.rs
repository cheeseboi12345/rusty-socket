//! WebSocket frame implementation.

use smol::io::{AsyncRead, AsyncReadExt};
use std::convert::TryFrom;

use crate::{Error, Result};

/// Represents the opcode of a WebSocket frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    /// Indicates a continuation frame.
    Continuation,
    /// Indicates a text frame.
    Text,
    /// Indicates a binary frame.
    Binary,
    /// Indicates a close frame.
    Close,
    /// Indicates a ping frame.
    Ping,
    /// Indicates a pong frame.
    Pong,
}

impl TryFrom<u8> for OpCode {
    type Error = Error;

    /// Tries to convert a u8 value into an OpCode.
    ///
    /// # Errors
    ///
    /// Returns an error if the value is not a valid OpCode.
    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(OpCode::Continuation),
            1 => Ok(OpCode::Text),
            2 => Ok(OpCode::Binary),
            8 => Ok(OpCode::Close),
            9 => Ok(OpCode::Ping),
            10 => Ok(OpCode::Pong),
            _ => Err(Error::Protocol(format!("Invalid OpCode: {}", value))),
        }
    }
}

/// Represents a WebSocket frame.
#[derive(Debug)]
pub struct Frame {
    /// Indicates if this is the final fragment in a message.
    pub fin: bool,
    /// First reserved bit.
    pub rsv1: bool,
    /// Second reserved bit.
    pub rsv2: bool,
    /// Third reserved bit.
    pub rsv3: bool,
    /// The opcode for this frame.
    pub opcode: OpCode,
    /// The masking key, if any.
    pub mask: Option<[u8; 4]>,
    /// The payload data.
    pub payload: Vec<u8>,
}

impl Frame {
    /// Creates a new Frame with the given opcode and payload.
    pub fn new(opcode: OpCode, payload: Vec<u8>) -> Self {
        Frame {
            fin: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode,
            mask: None,
            payload,
        }
    }

    /// Creates a close frame with an optional status code.
    pub fn close(status_code: Option<u16>) -> Self {
        let payload = status_code.map(|code| code.to_be_bytes().to_vec()).unwrap_or_default();
        Frame::new(OpCode::Close, payload)
    }

    /// Checks if this frame is a close frame.
    pub fn is_close(&self) -> bool {
        self.opcode == OpCode::Close
    }

    /// Checks if this frame is masked.
    pub fn is_masked(&self) -> bool {
        self.mask.is_some()
    }

    /// Reads a frame from the given AsyncRead stream.
    ///
    /// # Errors
    ///
    /// Returns an error if reading from the stream fails or if the frame is invalid.
    pub async fn read_from<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; 2];
        reader.read_exact(&mut buf).await?;
        let first_byte = buf[0];
        let second_byte = buf[1];

        let fin = (first_byte & 0x80) != 0;
        let rsv1 = (first_byte & 0x40) != 0;
        let rsv2 = (first_byte & 0x20) != 0;
        let rsv3 = (first_byte & 0x10) != 0;
        let opcode = OpCode::try_from(first_byte & 0x0F)?;

        let masked = (second_byte & 0x80) != 0;
        let mut payload_len = (second_byte & 0x7F) as u64;

        if payload_len == 126 {
            let mut buf = [0u8; 2];
            reader.read_exact(&mut buf).await?;
            payload_len = u16::from_be_bytes(buf) as u64;
        } else if payload_len == 127 {
            let mut buf = [0u8; 8];
            reader.read_exact(&mut buf).await?;
            payload_len = u64::from_be_bytes(buf);
        }

        let mask = if masked {
            let mut mask_bytes = [0u8; 4];
            reader.read_exact(&mut mask_bytes).await?;
            Some(mask_bytes)
        } else {
            None
        };

        let mut payload = vec![0u8; payload_len as usize];
        reader.read_exact(&mut payload).await?;

        if let Some(mask) = mask {
            for (i, byte) in payload.iter_mut().enumerate() {
                *byte ^= mask[i % 4];
            }
        }

        Ok(Frame {
            fin,
            rsv1,
            rsv2,
            rsv3,
            opcode,
            mask,
            payload,
        })
    }

    /// Converts the frame to a byte vector.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        let mut first_byte = 0;
        if self.fin {
            first_byte |= 0x80;
        }
        if self.rsv1 {
            first_byte |= 0x40;
        }
        if self.rsv2 {
            first_byte |= 0x20;
        }
        if self.rsv3 {
            first_byte |= 0x10;
        }
        first_byte |= self.opcode as u8;
        bytes.push(first_byte);

        let mut second_byte = 0;
        if self.mask.is_some() {
            second_byte |= 0x80;
        }

        let payload_len = self.payload.len();
        if payload_len < 126 {
            second_byte |= payload_len as u8;
            bytes.push(second_byte);
        } else if payload_len < 65536 {
            second_byte |= 126;
            bytes.push(second_byte);
            bytes.extend_from_slice(&(payload_len as u16).to_be_bytes());
        } else {
            second_byte |= 127;
            bytes.push(second_byte);
            bytes.extend_from_slice(&(payload_len as u64).to_be_bytes());
        }

        if let Some(mask) = self.mask {
            bytes.extend_from_slice(&mask);
        }

        bytes.extend_from_slice(&self.payload);
        bytes
    }
}

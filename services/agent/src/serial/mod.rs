pub mod codec;
pub mod frame;
pub mod payload;

use std::path::Path;

use async_trait::async_trait;
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_serial::{SerialPortBuilderExt, SerialStream};

use crate::error::AgentError;
use frame::Frame;

#[async_trait]
pub trait StationTransport: Send {
    async fn send(&mut self, frame: Frame) -> Result<(), AgentError>;
    async fn receive(&mut self) -> Result<Frame, AgentError>;
}

pub struct SerialStationTransport {
    stream: SerialStream,
    read_buffer: BytesMut,
}

impl SerialStationTransport {
    pub fn open(path: &Path, baud: u32) -> Result<Self, AgentError> {
        let stream = tokio_serial::new(path.to_string_lossy(), baud)
            .open_native_async()
            .map_err(|error| {
                AgentError::Serial(format!("cannot open Station serial port: {error}"))
            })?;
        Ok(Self {
            stream,
            read_buffer: BytesMut::with_capacity(512),
        })
    }
}

fn decode_buffered_frame(buffer: &mut BytesMut) -> Result<Option<Frame>, AgentError> {
    loop {
        match codec::decode_next(buffer) {
            Ok(frame) => return Ok(frame),
            Err(AgentError::Serial(message)) => {
                tracing::warn!(
                    error = %message,
                    "discarded corrupt Station serial prefix; continuing from resynchronized buffer"
                );
            }
            Err(error) => return Err(error),
        }
    }
}

#[async_trait]
impl StationTransport for SerialStationTransport {
    async fn send(&mut self, frame: Frame) -> Result<(), AgentError> {
        let encoded = codec::encode_frame(&frame)?;
        self.stream
            .write_all(&encoded)
            .await
            .map_err(|error| AgentError::Serial(format!("serial write failed: {error}")))?;
        self.stream
            .flush()
            .await
            .map_err(|error| AgentError::Serial(format!("serial flush failed: {error}")))?;
        Ok(())
    }

    async fn receive(&mut self) -> Result<Frame, AgentError> {
        loop {
            if let Some(frame) = decode_buffered_frame(&mut self.read_buffer)? {
                return Ok(frame);
            }

            let mut chunk = [0u8; 512];
            let count = self
                .stream
                .read(&mut chunk)
                .await
                .map_err(|error| AgentError::Serial(format!("serial read failed: {error}")))?;
            if count == 0 {
                return Err(AgentError::Serial("Station serial port reached EOF".into()));
            }
            self.read_buffer.extend_from_slice(&chunk[..count]);
        }
    }
}

#[cfg(test)]
mod tests {
    use bytes::{Bytes, BytesMut};

    use super::{
        codec::encode_frame,
        decode_buffered_frame,
        frame::{Frame, MessageType},
    };

    #[test]
    fn buffered_decoder_skips_corrupt_frame_and_returns_following_valid_frame() {
        // PURPOSE: A recoverable framing error must not discard a valid frame already buffered behind it.
        // ARRANGE: Concatenate one CRC-corrupted COMMAND frame and one valid byte-identical COMMAND frame.
        // ACTION: Ask the transport-level buffered decoder for the next usable frame.
        // ASSERT: It skips the corrupted frame using codec resynchronization and returns the following valid frame.
        // FAILURE MEANS: one corrupt serial frame still forces reconnect and can strand WAIT_ACK evidence.
        let expected = Frame {
            message_type: MessageType::Command,
            payload: Bytes::from(vec![0u8; MessageType::Command.payload_len()]),
        };
        let valid = encode_frame(&expected).unwrap();
        let mut corrupt = valid.to_vec();
        corrupt[12] ^= 0x01;

        let mut buffer = BytesMut::from(corrupt.as_slice());
        buffer.extend_from_slice(&valid);

        assert_eq!(decode_buffered_frame(&mut buffer).unwrap(), Some(expected));
        assert!(buffer.is_empty());
    }
}

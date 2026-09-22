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
            .map_err(|error| AgentError::Serial(format!("cannot open Station serial port: {error}")))?;
        Ok(Self { stream, read_buffer: BytesMut::with_capacity(512) })
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
            match codec::decode_next(&mut self.read_buffer) {
                Ok(Some(frame)) => return Ok(frame),
                Ok(None) => {}
                Err(error) => {
                    // The codec has already discarded the corrupt prefix. Surface the error to the worker;
                    // a subsequent receive call can continue from the resynchronized buffer.
                    return Err(error);
                }
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

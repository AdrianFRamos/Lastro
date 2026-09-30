use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::{
    error::AgentError,
    serial::frame::{Frame, MessageType},
};

const MAGIC: &[u8; 4] = b"LSTR";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 12;
const CRC_LEN: usize = 4;
pub const MAX_FRAME_PAYLOAD: usize = 1024;

/// Incremental codec for:
/// magic[4] | version u8 | type u8 | reserved u16_le | payload_len u32_le |
/// payload | crc32c u32_le.
pub fn encode_frame(frame: &Frame) -> Result<Bytes, AgentError> {
    let payload_len = frame.payload.len();
    if payload_len > MAX_FRAME_PAYLOAD {
        return Err(AgentError::Serial(format!(
            "serial payload exceeds {MAX_FRAME_PAYLOAD} bytes"
        )));
    }
    if payload_len != frame.message_type.payload_len() {
        return Err(AgentError::Serial(format!(
            "serial {:?} payload must be {} bytes, got {payload_len}",
            frame.message_type,
            frame.message_type.payload_len()
        )));
    }

    let mut out = BytesMut::with_capacity(HEADER_LEN + payload_len + CRC_LEN);
    out.extend_from_slice(MAGIC);
    out.put_u8(VERSION);
    out.put_u8(frame.message_type as u8);
    out.put_u16_le(0);
    out.put_u32_le(payload_len as u32);
    out.extend_from_slice(&frame.payload);
    let crc = crc32c::crc32c(&out[4..]);
    out.put_u32_le(crc);
    Ok(out.freeze())
}

pub fn decode_next(buffer: &mut BytesMut) -> Result<Option<Frame>, AgentError> {
    resynchronize_to_magic(buffer);
    if buffer.len() < HEADER_LEN {
        return Ok(None);
    }

    if buffer[4] != VERSION {
        let version = buffer[4];
        buffer.advance(1);
        return Err(AgentError::Serial(format!(
            "unsupported serial protocol version {version}"
        )));
    }

    let message_type = match MessageType::try_from(buffer[5]) {
        Ok(value) => value,
        Err(error) => {
            buffer.advance(1);
            return Err(error);
        }
    };

    let reserved = u16::from_le_bytes([buffer[6], buffer[7]]);
    if reserved != 0 {
        buffer.advance(1);
        return Err(AgentError::Serial(
            "serial reserved bytes must be zero".into(),
        ));
    }

    let payload_len = u32::from_le_bytes(buffer[8..12].try_into().expect("fixed header")) as usize;
    if payload_len > MAX_FRAME_PAYLOAD {
        buffer.advance(1);
        return Err(AgentError::Serial(format!(
            "serial payload length {payload_len} exceeds {MAX_FRAME_PAYLOAD} bytes"
        )));
    }
    if payload_len != message_type.payload_len() {
        buffer.advance(1);
        return Err(AgentError::Serial(format!(
            "serial {:?} payload length must be {} bytes, got {payload_len}",
            message_type,
            message_type.payload_len()
        )));
    }

    let frame_len = HEADER_LEN + payload_len + CRC_LEN;
    if buffer.len() < frame_len {
        return Ok(None);
    }

    let expected_crc = u32::from_le_bytes(
        buffer[HEADER_LEN + payload_len..frame_len]
            .try_into()
            .expect("fixed CRC"),
    );
    let actual_crc = crc32c::crc32c(&buffer[4..HEADER_LEN + payload_len]);
    if expected_crc != actual_crc {
        buffer.advance(1);
        return Err(AgentError::Serial("serial CRC32C mismatch".into()));
    }

    let frame_bytes = buffer.split_to(frame_len).freeze();
    Ok(Some(Frame {
        message_type,
        payload: frame_bytes.slice(HEADER_LEN..HEADER_LEN + payload_len),
    }))
}

fn resynchronize_to_magic(buffer: &mut BytesMut) {
    if buffer.starts_with(MAGIC) {
        return;
    }

    if let Some(index) = buffer
        .windows(MAGIC.len())
        .position(|window| window == MAGIC)
    {
        buffer.advance(index);
        return;
    }

    let keep = (1..MAGIC.len())
        .rev()
        .find(|&len| buffer.ends_with(&MAGIC[..len]))
        .unwrap_or(0);
    if buffer.len() > keep {
        buffer.advance(buffer.len() - keep);
    }
}

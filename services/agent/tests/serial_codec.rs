//! Agent serial framing contracts.

use std::{fs, path::PathBuf};

use bytes::{Bytes, BytesMut};
use lastro_agent::serial::{
    codec::{MAX_FRAME_PAYLOAD, decode_next, encode_frame},
    frame::{Frame, MessageType},
};

fn fixture(name: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::read(root.join("test-vectors").join(name)).expect("read fixture")
}

fn command_frame() -> Frame {
    Frame {
        message_type: MessageType::Command,
        payload: Bytes::from(fixture("serial-command.bin")),
    }
}

#[test]
fn decoder_accepts_frame_split_at_every_byte() {
    // PURPOSE: Framing must work with every UART fragmentation boundary.
    // ASSERT: Every split emits exactly one byte-identical COMMAND and no extra frame.
    // FAILURE MEANS: Serial behavior would depend on operating-system read chunk sizes.
    let encoded = encode_frame(&command_frame()).unwrap();
    for split in 0..=encoded.len() {
        let mut buffer = BytesMut::new();
        buffer.extend_from_slice(&encoded[..split]);
        let first = decode_next(&mut buffer).unwrap();
        if split < encoded.len() {
            assert!(first.is_none());
        }
        buffer.extend_from_slice(&encoded[split..]);
        let decoded = first
            .or_else(|| decode_next(&mut buffer).unwrap())
            .expect("complete frame");
        assert_eq!(decoded, command_frame());
        assert!(decode_next(&mut buffer).unwrap().is_none());
    }
}

#[test]
fn decoder_rejects_bad_crc() {
    // PURPOSE: A corrupted CRC must never pass bytes into the domain layer.
    // ASSERT: Corruption returns CRC error and a following valid frame remains recoverable.
    // FAILURE MEANS: Serial corruption could become accepted Station evidence.
    let valid = encode_frame(&command_frame()).unwrap();
    let mut bad = valid.to_vec();
    let payload_index = 12 + 10;
    bad[payload_index] ^= 1;
    let mut buffer = BytesMut::from(&bad[..]);
    buffer.extend_from_slice(&valid);
    assert!(
        decode_next(&mut buffer)
            .unwrap_err()
            .to_string()
            .contains("CRC32C")
    );
    let decoded = decode_next(&mut buffer)
        .unwrap()
        .expect("recover following frame");
    assert_eq!(decoded, command_frame());
}

#[test]
fn decoder_rejects_oversized_payload() {
    // PURPOSE: The length field must not induce allocation beyond the fixed serial bound.
    // ASSERT: A header declaring more than MAX_FRAME_PAYLOAD fails before the payload exists.
    // FAILURE MEANS: Malformed serial input could create an avoidable memory denial of service.
    let mut bytes = BytesMut::new();
    bytes.extend_from_slice(b"LSTR");
    bytes.extend_from_slice(&[1, MessageType::Command as u8, 0, 0]);
    bytes.extend_from_slice(&((MAX_FRAME_PAYLOAD as u32) + 1).to_le_bytes());
    let error = decode_next(&mut bytes).unwrap_err();
    assert!(error.to_string().contains("exceeds"));
}

#[test]
fn decoder_resynchronizes_after_noise() {
    // PURPOSE: Noise or one corrupted frame must not require restarting the Agent.
    // ASSERT: Decoder resynchronizes on documented magic and returns the next valid frame once.
    // FAILURE MEANS: One bad serial read could permanently desynchronize the bridge.
    let valid = encode_frame(&command_frame()).unwrap();
    let mut bad = valid.to_vec();
    *bad.last_mut().unwrap() ^= 0x40;
    let mut buffer = BytesMut::from(&b"noise-LS"[..]);
    buffer.extend_from_slice(&bad);
    buffer.extend_from_slice(&valid);
    assert!(decode_next(&mut buffer).is_err());
    assert_eq!(decode_next(&mut buffer).unwrap().unwrap(), command_frame());
    assert!(decode_next(&mut buffer).unwrap().is_none());
}

#[test]
fn encoder_matches_firmware_command_vector() {
    // PURPOSE: Rust and C must agree byte-for-byte on the complete COMMAND frame contract.
    // ASSERT: Header fields, payload and CRC32C are deterministic and independently recomputable.
    // FAILURE MEANS: Agent and Station can parse different bytes from the same serial frame.
    let encoded = encode_frame(&command_frame()).unwrap();
    assert_eq!(&encoded[0..4], b"LSTR");
    assert_eq!(encoded[4], 1);
    assert_eq!(encoded[5], 1);
    assert_eq!(&encoded[6..8], &[0, 0]);
    assert_eq!(u32::from_le_bytes(encoded[8..12].try_into().unwrap()), 224);
    assert_eq!(&encoded[12..236], fixture("serial-command.bin"));
    let expected = crc32c::crc32c(&encoded[4..236]);
    assert_eq!(
        u32::from_le_bytes(encoded[236..240].try_into().unwrap()),
        expected
    );
}

#[test]
fn event_ready_decoder_preserves_event_signature_and_rfid() {
    // PURPOSE: Agent framing is transport only and must not normalize signed Station evidence.
    // ASSERT: EVENT_READY payload bytes are preserved exactly after frame encode/decode.
    // FAILURE MEANS: Agent could invalidate the Station signature by transforming evidence.
    let payload = Bytes::from(fixture("serial-event-ready.bin"));
    let frame = Frame {
        message_type: MessageType::EventReady,
        payload: payload.clone(),
    };
    let encoded = encode_frame(&frame).unwrap();
    let mut buffer = BytesMut::from(encoded.as_ref());
    let decoded = decode_next(&mut buffer).unwrap().unwrap();
    assert_eq!(decoded.message_type, MessageType::EventReady);
    assert_eq!(decoded.payload, payload);
}

#[test]
fn arbitrary_serial_bytes_never_panic() {
    // PURPOSE: Treat UART input as fully attacker/corruption controlled at the framing boundary.
    for len in 0usize..=2048 {
        let mut state = (len as u64).wrapping_add(0xd1b5_4a32_d192_ed03);
        let bytes: Vec<u8> = (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            })
            .collect();
        let mut buffer = BytesMut::from(bytes.as_slice());
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| decode_next(&mut buffer)));
        assert!(result.is_ok(), "serial decoder panicked for length {len}");
    }

    // ASSERT: deterministic arbitrary frames can return None/error/frame, but never panic.
    // FAILURE MEANS: malformed serial input can crash the Agent instead of failing closed/resynchronizing.
}

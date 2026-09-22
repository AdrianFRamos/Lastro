//! Mandatory StationEvent decoder rejection contracts.

mod common;

use lastro_protocol::{ProtocolError, StationEvent};

#[test]
fn rejects_275_bytes() {
    // PURPOSE: Enforce exact event length.
    // ASSERT: A truncated 275-byte buffer returns InvalidLength.
    // FAILURE MEANS: Truncated signed messages can be interpreted as valid events.
    assert_eq!(StationEvent::decode(&common::event_bytes("origin")[..275]), Err(ProtocolError::InvalidLength));
}

#[test]
fn rejects_277_bytes() {
    // PURPOSE: Enforce exact event length.
    // ASSERT: A 277-byte buffer with trailing data returns InvalidLength.
    // FAILURE MEANS: Multiple byte strings could represent one logical event.
    let mut bytes = common::event_bytes("origin").to_vec();
    bytes.push(0);
    assert_eq!(StationEvent::decode(&bytes), Err(ProtocolError::InvalidLength));
}

#[test]
fn rejects_wrong_magic() {
    // PURPOSE: Separate Lastro from unrelated binary messages.
    // ASSERT: Changing one magic byte returns InvalidMagic.
    // FAILURE MEANS: Different protocols could collide at the decoder boundary.
    let mut bytes = common::event_bytes("origin");
    bytes[0] ^= 1;
    assert_eq!(StationEvent::decode(&bytes), Err(ProtocolError::InvalidMagic));
}

#[test]
fn rejects_unknown_version() {
    // PURPOSE: Prevent future semantics from being interpreted as version 1.
    // ASSERT: Any version other than 1 returns UnsupportedVersion.
    // FAILURE MEANS: Incompatible formats may be accepted silently.
    let mut bytes = common::event_bytes("origin");
    bytes[4] = 2;
    assert_eq!(StationEvent::decode(&bytes), Err(ProtocolError::UnsupportedVersion));
}

#[test]
fn rejects_unknown_action() {
    // PURPOSE: Keep the domain operation set closed to ORIGIN, TRANSFER, and REIDENTIFY.
    // ASSERT: An action outside 1..=3 returns InvalidAction.
    // FAILURE MEANS: Undefined state transitions could reach downstream code.
    let mut bytes = common::event_bytes("origin");
    bytes[5] = 4;
    assert_eq!(StationEvent::decode(&bytes), Err(ProtocolError::InvalidAction));
}

#[test]
fn rejects_nonzero_reserved() {
    // PURPOSE: Preserve canonical event encoding.
    // ASSERT: Any nonzero reserved byte returns ReservedNotZero.
    // FAILURE MEANS: Equivalent events could have multiple signed encodings.
    let mut bytes = common::event_bytes("origin");
    bytes[6] = 1;
    assert_eq!(StationEvent::decode(&bytes), Err(ProtocolError::ReservedNotZero));
}

#[test]
fn arbitrary_bytes_never_panic_and_non_exact_lengths_never_decode() {
    // PURPOSE: Exercise StationEvent::decode across deterministic attacker-controlled byte strings.
    for len in 0usize..=512 {
        let mut state = (len as u64).wrapping_add(0x9e37_79b9_7f4a_7c15);
        let bytes: Vec<u8> = (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            })
            .collect();
        let result = std::panic::catch_unwind(|| StationEvent::decode(&bytes));
        assert!(result.is_ok(), "decoder panicked for length {len}");
        if len != 276 {
            assert_eq!(result.unwrap(), Err(ProtocolError::InvalidLength));
        }
    }

    // ASSERT: arbitrary inputs never panic, and truncation/trailing bytes always fail exact-length parsing.
    // FAILURE MEANS: external bytes can crash the process or create an ambiguous StationEvent encoding.
}

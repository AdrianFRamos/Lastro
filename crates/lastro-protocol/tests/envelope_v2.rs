use lastro_protocol::error::ProtocolError;
use lastro_protocol::v2::{
    AssetType, DomainEventEnvelope, EventType, V2_ENVELOPE_LEN,
    constants::{DOMAIN_EVENT_ENVELOPE_LEN, SCHEMA_VERSION},
    domain_hash,
};

fn id(value: u8) -> [u8; 32] {
    [value; 32]
}

fn envelope() -> DomainEventEnvelope {
    DomainEventEnvelope::new(
        EventType::ObservationRecorded,
        id(1),
        id(2),
        id(3),
        7,
        id(4),
        id(5),
        id(6),
        1_000,
        1_060,
    )
    .expect("valid envelope")
}

#[test]
fn envelope_round_trips_with_fixed_length() {
    // PURPOSE: the v2 envelope is a fixed 220-byte wire contract shared by chain, API and browser.
    // ASSERT/FAILURE: a length or field drift means a signed envelope decodes differently across layers.
    assert_eq!(V2_ENVELOPE_LEN, DOMAIN_EVENT_ENVELOPE_LEN);
    let original = envelope();
    let encoded = original.encode().expect("encode");
    assert_eq!(encoded.len(), V2_ENVELOPE_LEN);
    let decoded = DomainEventEnvelope::decode(&encoded).expect("decode");
    assert_eq!(decoded, original);
    assert_eq!(decoded.schema_version, SCHEMA_VERSION);
}

#[test]
fn envelope_rejects_invalid_length_and_unknown_schema() {
    // PURPOSE: decoders must fail closed on truncated bytes and unknown schema versions.
    // ASSERT/FAILURE: accepting either lets a future or malformed layout be interpreted as schema 1.
    assert_eq!(
        DomainEventEnvelope::decode(&[0u8; V2_ENVELOPE_LEN - 1]),
        Err(ProtocolError::InvalidV2EnvelopeLength)
    );

    let mut bytes = envelope().encode().expect("encode");
    bytes[0..2].copy_from_slice(&99u16.to_le_bytes());
    assert_eq!(
        DomainEventEnvelope::decode(&bytes),
        Err(ProtocolError::UnsupportedV2SchemaVersion)
    );
}

#[test]
fn envelope_rejects_zero_ids_and_invalid_window() {
    // PURPOSE: all-zero identifiers and windows over the protocol cap are invalid evidence.
    // ASSERT/FAILURE: an anonymous subject or unbounded validity window could be anchored on-chain.
    let mut bytes = envelope().encode().expect("encode");
    bytes[4..36].fill(0);
    assert_eq!(
        DomainEventEnvelope::decode(&bytes),
        Err(ProtocolError::InvalidV2Identifier)
    );

    let mut bytes = envelope().encode().expect("encode");
    bytes[212..220].copy_from_slice(&999_999i64.to_le_bytes());
    assert_eq!(
        DomainEventEnvelope::decode(&bytes),
        Err(ProtocolError::InvalidV2TimeWindow)
    );
}

#[test]
fn enum_values_are_stable_and_unknown_values_are_rejected() {
    // PURPOSE: enum discriminants are wire values and must never be renumbered.
    // ASSERT/FAILURE: a renumbered or open enum silently changes the meaning of stored events.
    assert_eq!(AssetType::try_from(1).expect("animal"), AssetType::Animal);
    assert_eq!(
        EventType::try_from(2).expect("observation"),
        EventType::ObservationRecorded
    );
    assert!(AssetType::try_from(0).is_err());
    assert!(EventType::try_from(u16::MAX).is_err());
}

#[test]
fn domain_hashes_are_separated() {
    // PURPOSE: every v2 hash commits to its domain so preimages cannot cross contexts.
    // ASSERT/FAILURE: a payload hash could be replayed as an event or lineage hash.
    let bytes = b"same bytes";
    assert_ne!(
        domain_hash(b"DOMAIN_A", bytes),
        domain_hash(b"DOMAIN_B", bytes)
    );
    assert_ne!(envelope().event_hash().expect("hash"), [0u8; 32]);
}

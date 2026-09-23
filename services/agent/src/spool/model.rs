use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutboxState {
    Local,
    Server,
    Finalized,
    Quarantined,
}

/// Exact evidence persisted before any POST. These bytes are immutable after insertion.
#[derive(Clone, Debug)]
pub struct OutboxRow {
    pub capture_id: Uuid,
    pub event_hash: [u8; 32],
    pub event_bytes: [u8; 276],
    pub observed_rfid: [u8; 8],
    pub station_pubkey: [u8; 33],
    pub station_signature: [u8; 64],
    pub state: OutboxState,
    pub attempts: u32,
}

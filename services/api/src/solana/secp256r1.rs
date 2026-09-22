//! Builder for the Solana Secp256r1 precompile instruction data used by Lastro.
//!
//! Official Solana wire contract for one signature:
//! - byte 0: number of signatures = 1;
//! - byte 1: padding = 0;
//! - one `Secp256r1SignatureOffsets` = seven little-endian u16 fields (14 bytes);
//! - Lastro stores signature64 then compressed pubkey33 in this same precompile instruction;
//! - the 276-byte message is NOT duplicated here: descriptor references instruction[1].
//!
//! Therefore Lastro precompile data  is exactly 113 bytes:
//! 2 + 14 + 64 + 33. Signature starts at offset 16 and pubkey at offset 80.
//! The program still inspects sysvar::instructions and proves these offsets/indexes match
//! the expected envelope; successful runtime verification alone is not sufficient binding.

use crate::error::ApiError;

pub const SECP256R1_PROGRAM_ID: &str = "Secp256r1SigVerify1111111111111111111111111";
pub const LASTRO_SECP_SIGNATURE_COUNT: u8 = 1;
pub const LASTRO_SECP_OFFSETS_START: usize = 2;
pub const LASTRO_SECP_OFFSETS_LEN: usize = 14;
pub const LASTRO_SECP_SIGNATURE_OFFSET: u16 = 16;
pub const LASTRO_SECP_PUBKEY_OFFSET: u16 = 80;
pub const LASTRO_SECP_INSTRUCTION_DATA_LEN: usize = 113;
pub const LASTRO_SECP_PRECOMPILE_INSTRUCTION_INDEX: u16 = 0;
pub const LASTRO_INSTRUCTION_INDEX: u16 = 1;
pub const LASTRO_SIGNED_MESSAGE_LEN: u16 = 276;

pub struct Secp256r1Descriptor<'a> {
    pub station_pubkey33: &'a [u8; 33],
    pub station_signature64: &'a [u8; 64],
    /// Byte offset of the raw `[u8;276]` argument inside the fully serialized Lastro ix data.
    pub message_data_offset: u16,
}

pub fn build_secp256r1_instruction_data(
    descriptor: Secp256r1Descriptor<'_>,
) -> Result<Vec<u8>, ApiError> {
    if !matches!(descriptor.station_pubkey33[0], 0x02 | 0x03) {
        return Err(ApiError::Validation(
            "Station public key must use 33-byte compressed SEC1 encoding".into(),
        ));
    }

    let mut out = Vec::with_capacity(LASTRO_SECP_INSTRUCTION_DATA_LEN);
    out.push(LASTRO_SECP_SIGNATURE_COUNT);
    out.push(0); // required padding

    // Secp256r1SignatureOffsets: seven u16 little-endian fields, in official order.
    for value in [
        LASTRO_SECP_SIGNATURE_OFFSET,
        LASTRO_SECP_PRECOMPILE_INSTRUCTION_INDEX,
        LASTRO_SECP_PUBKEY_OFFSET,
        LASTRO_SECP_PRECOMPILE_INSTRUCTION_INDEX,
        descriptor.message_data_offset,
        LASTRO_SIGNED_MESSAGE_LEN,
        LASTRO_INSTRUCTION_INDEX,
    ] {
        out.extend_from_slice(&value.to_le_bytes());
    }

    debug_assert_eq!(out.len(), LASTRO_SECP_SIGNATURE_OFFSET as usize);
    out.extend_from_slice(descriptor.station_signature64);
    debug_assert_eq!(out.len(), LASTRO_SECP_PUBKEY_OFFSET as usize);
    out.extend_from_slice(descriptor.station_pubkey33);

    if out.len() != LASTRO_SECP_INSTRUCTION_DATA_LEN {
        return Err(ApiError::Internal);
    }
    Ok(out)
}

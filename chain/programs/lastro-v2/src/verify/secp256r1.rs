//! Bind the Solana Secp256r1 precompile to the exact v2 domain envelope.

use anchor_lang::prelude::*;
use solana_instructions_sysvar::{load_current_index_checked, load_instruction_at_checked};

use crate::error::LastroV2Error;

pub const V2_EVENT_LEN: usize = 220;
const SIGNATURE_OFFSET: u16 = 16;
const SIGNATURE_END: usize = 80;
const PUBLIC_KEY_OFFSET: u16 = 80;
const PUBLIC_KEY_END: usize = 113;
// Anchor serializes the discriminator followed by subject_id, event_id and station_id
// before the 220-byte envelope argument.
const EVENT_OFFSET_IN_ANCHOR_IX: u16 = 8 + 32 + 32 + 32;

fn read_u16_le(bytes: &[u8], offset: usize) -> Result<u16> {
    let range = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| error!(LastroV2Error::InvalidStationProof))?;
    Ok(u16::from_le_bytes(
        range.try_into().expect("two-byte range"),
    ))
}

fn unique_event_offset(instruction_data: &[u8], event: &[u8; V2_EVENT_LEN]) -> Result<u16> {
    let mut found: Option<usize> = None;
    for offset in 0..=instruction_data
        .len()
        .checked_sub(V2_EVENT_LEN)
        .ok_or_else(|| error!(LastroV2Error::InvalidStationProof))?
    {
        if &instruction_data[offset..offset + V2_EVENT_LEN] == event {
            require!(found.is_none(), LastroV2Error::InvalidStationProof);
            found = Some(offset);
        }
    }
    let offset = found.ok_or_else(|| error!(LastroV2Error::InvalidStationProof))?;
    u16::try_from(offset).map_err(|_| error!(LastroV2Error::InvalidStationProof))
}

/// `trailing_args_len` is the byte length of instruction arguments serialized after the
/// 220-byte envelope (0 for observations, 32/64 for identifier events).
pub fn verify_station_precompile_binding(
    instructions_sysvar: &AccountInfo<'_>,
    station_pubkey33: &[u8; 33],
    event: &[u8; V2_EVENT_LEN],
    trailing_args_len: usize,
) -> Result<()> {
    require!(
        load_current_index_checked(instructions_sysvar)? == 1,
        LastroV2Error::InvalidStationProof
    );

    let secp = load_instruction_at_checked(0, instructions_sysvar)?;
    require!(
        secp.program_id == solana_sdk_ids::secp256r1_program::ID,
        LastroV2Error::InvalidStationProof
    );
    require!(secp.accounts.is_empty(), LastroV2Error::InvalidStationProof);
    require!(
        secp.data.len() == PUBLIC_KEY_END,
        LastroV2Error::InvalidStationProof
    );
    require!(
        secp.data[0] == 1 && secp.data[1] == 0,
        LastroV2Error::InvalidStationProof
    );
    require!(
        read_u16_le(&secp.data, 2)? == SIGNATURE_OFFSET,
        LastroV2Error::InvalidStationProof
    );
    require!(
        read_u16_le(&secp.data, 4)? == 0,
        LastroV2Error::InvalidStationProof
    );
    require!(
        read_u16_le(&secp.data, 6)? == PUBLIC_KEY_OFFSET,
        LastroV2Error::InvalidStationProof
    );
    require!(
        read_u16_le(&secp.data, 8)? == 0,
        LastroV2Error::InvalidStationProof
    );
    require!(
        read_u16_le(&secp.data, 10)? == EVENT_OFFSET_IN_ANCHOR_IX,
        LastroV2Error::InvalidStationProof
    );
    require!(
        read_u16_le(&secp.data, 12)? == V2_EVENT_LEN as u16,
        LastroV2Error::InvalidStationProof
    );
    require!(
        read_u16_le(&secp.data, 14)? == 1,
        LastroV2Error::InvalidStationProof
    );
    require!(
        &secp.data[PUBLIC_KEY_OFFSET as usize..PUBLIC_KEY_END] == station_pubkey33,
        LastroV2Error::InvalidStationProof
    );
    require!(
        secp.data[SIGNATURE_OFFSET as usize..SIGNATURE_END].len() == 64,
        LastroV2Error::InvalidStationProof
    );

    let current = load_instruction_at_checked(1, instructions_sysvar)?;
    require!(
        current.program_id == crate::ID,
        LastroV2Error::InvalidStationProof
    );
    let event_offset = unique_event_offset(&current.data, event)?;
    require!(
        event_offset == EVENT_OFFSET_IN_ANCHOR_IX,
        LastroV2Error::InvalidStationProof
    );
    require!(
        current.data.len() == EVENT_OFFSET_IN_ANCHOR_IX as usize + V2_EVENT_LEN + trailing_args_len,
        LastroV2Error::InvalidStationProof
    );
    require_only_trailing_compute_budget(instructions_sysvar)
}

/// Up to two ComputeBudget instructions (unit limit + unit price) may follow the Lastro
/// instruction so wallets can pay priority fees. The runtime reads them from any position,
/// so the Secp256r1 (index 0) and Lastro (index 1) layout stays frozen.
pub const MAX_TRAILING_COMPUTE_BUDGET_INSTRUCTIONS: usize = 2;

fn require_only_trailing_compute_budget(instructions_sysvar: &AccountInfo<'_>) -> Result<()> {
    for index in 2..2 + MAX_TRAILING_COMPUTE_BUDGET_INSTRUCTIONS {
        let Ok(instruction) = load_instruction_at_checked(index, instructions_sysvar) else {
            return Ok(());
        };
        require!(
            instruction.program_id == solana_sdk_ids::compute_budget::ID
                && instruction.accounts.is_empty(),
            LastroV2Error::InvalidStationProof
        );
    }
    require!(
        load_instruction_at_checked(
            2 + MAX_TRAILING_COMPUTE_BUDGET_INSTRUCTIONS,
            instructions_sysvar
        )
        .is_err(),
        LastroV2Error::InvalidStationProof
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_offset_is_exact_and_unique() {
        let event = [0x5a; V2_EVENT_LEN];
        let mut data = vec![0x11; EVENT_OFFSET_IN_ANCHOR_IX as usize];
        data.extend_from_slice(&event);
        assert_eq!(
            unique_event_offset(&data, &event).expect("event offset"),
            EVENT_OFFSET_IN_ANCHOR_IX
        );
    }

    #[test]
    fn duplicate_or_missing_event_is_rejected() {
        let event = [0x5a; V2_EVENT_LEN];
        assert!(unique_event_offset(&[0u8; V2_EVENT_LEN], &event).is_err());
        let mut duplicate = event.to_vec();
        duplicate.extend_from_slice(&event);
        assert!(unique_event_offset(&duplicate, &event).is_err());
    }
}

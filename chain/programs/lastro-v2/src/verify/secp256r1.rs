//! Bind the Solana Secp256r1 precompile to the exact v2 domain envelope.

use anchor_lang::prelude::*;
use solana_instructions_sysvar::{load_current_index_checked, load_instruction_at_checked};

use crate::error::LastroV2Error;

pub const V2_EVENT_LEN: usize = 220;
const SIGNATURE_OFFSET: u16 = 16;
const SIGNATURE_END: usize = 80;
const PUBLIC_KEY_OFFSET: u16 = 80;
const PUBLIC_KEY_END: usize = 113;
const EVENT_OFFSET_IN_ANCHOR_IX: u16 = 8;

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

pub fn verify_station_precompile_binding(
    instructions_sysvar: &AccountInfo<'_>,
    station_pubkey33: &[u8; 33],
    event: &[u8; V2_EVENT_LEN],
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
        current.data.len() == EVENT_OFFSET_IN_ANCHOR_IX as usize + V2_EVENT_LEN,
        LastroV2Error::InvalidStationProof
    );
    require!(
        load_instruction_at_checked(2, instructions_sysvar).is_err(),
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

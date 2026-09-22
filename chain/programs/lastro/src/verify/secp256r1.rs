//! Bind Solana Secp256r1 verification to the exact StationEvent processed by Lastro.

use anchor_lang::{prelude::*, solana_program::sysvar::instructions::{load_current_index_checked, load_instruction_at_checked}};

use crate::{constants::STATION_EVENT_LEN, error::LastroError};

const DESCRIPTOR_END: usize = 16;
const SIGNATURE_OFFSET: u16 = 16;
const SIGNATURE_END: usize = 80;
const PUBLIC_KEY_OFFSET: u16 = 80;
const PUBLIC_KEY_END: usize = 113;

fn read_u16_le(bytes: &[u8], offset: usize) -> Result<u16> {
    let range = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| error!(LastroError::InvalidStationProof))?;
    Ok(u16::from_le_bytes(range.try_into().expect("two-byte range")))
}

fn unique_event_offset(instruction_data: &[u8], event: &[u8; STATION_EVENT_LEN]) -> Result<u16> {
    if instruction_data.len() < STATION_EVENT_LEN {
        return err!(LastroError::InvalidStationProof);
    }
    let mut found: Option<usize> = None;
    for offset in 0..=instruction_data.len() - STATION_EVENT_LEN {
        if &instruction_data[offset..offset + STATION_EVENT_LEN] == event {
            require!(found.is_none(), LastroError::InvalidStationProof);
            found = Some(offset);
        }
    }
    let offset = found.ok_or_else(|| error!(LastroError::InvalidStationProof))?;
    u16::try_from(offset).map_err(|_| error!(LastroError::InvalidStationProof))
}

pub fn verify_station_precompile_binding(
    instructions_sysvar: &AccountInfo<'_>,
    station_pubkey33: &[u8; 33],
    event: &[u8; STATION_EVENT_LEN],
) -> Result<()> {
    require!(load_current_index_checked(instructions_sysvar)? == 1, LastroError::InvalidStationProof);

    let secp = load_instruction_at_checked(0, instructions_sysvar)?;
    require!(secp.program_id == solana_secp256r1_program::ID, LastroError::InvalidStationProof);
    require!(secp.accounts.is_empty(), LastroError::InvalidStationProof);
    require!(secp.data.len() == PUBLIC_KEY_END, LastroError::InvalidStationProof);
    require!(secp.data[0] == 1 && secp.data[1] == 0, LastroError::InvalidStationProof);

    require!(read_u16_le(&secp.data, 2)? == SIGNATURE_OFFSET, LastroError::InvalidStationProof);
    require!(read_u16_le(&secp.data, 4)? == 0, LastroError::InvalidStationProof);
    require!(read_u16_le(&secp.data, 6)? == PUBLIC_KEY_OFFSET, LastroError::InvalidStationProof);
    require!(read_u16_le(&secp.data, 8)? == 0, LastroError::InvalidStationProof);
    require!(read_u16_le(&secp.data, 12)? == STATION_EVENT_LEN as u16, LastroError::InvalidStationProof);
    require!(read_u16_le(&secp.data, 14)? == 1, LastroError::InvalidStationProof);
    require!(&secp.data[PUBLIC_KEY_OFFSET as usize..PUBLIC_KEY_END] == station_pubkey33, LastroError::InvalidStationProof);
    require!(secp.data[SIGNATURE_OFFSET as usize..SIGNATURE_END].len() == 64, LastroError::InvalidStationProof);

    let current = load_instruction_at_checked(1, instructions_sysvar)?;
    require!(current.program_id == crate::ID, LastroError::InvalidStationProof);
    let event_offset = unique_event_offset(&current.data, event)?;
    require!(read_u16_le(&secp.data, 10)? == event_offset, LastroError::InvalidStationProof);

    // The hackathon transaction envelope is deliberately frozen to exactly two instructions.
    require!(load_instruction_at_checked(2, instructions_sysvar).is_err(), LastroError::InvalidStationProof);
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_event_offset_is_unique_and_derived() {
        let event = [0x5a; STATION_EVENT_LEN];
        let mut instruction = vec![0x11; 8];
        instruction.extend_from_slice(&event);
        assert_eq!(unique_event_offset(&instruction, &event).unwrap(), 8);
    }

    #[test]
    fn duplicate_or_missing_event_slice_is_rejected() {
        let event = [0x5a; STATION_EVENT_LEN];
        assert!(unique_event_offset(&[0u8; STATION_EVENT_LEN], &event).is_err());

        let mut duplicated = event.to_vec();
        duplicated.extend_from_slice(&event);
        assert!(unique_event_offset(&duplicated, &event).is_err());
    }
}

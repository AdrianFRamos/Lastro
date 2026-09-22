#![forbid(unsafe_code)]
//! Lastro Anchor program entrypoint.
//!
//! Program identity is deliberately not fabricated in source control. Run
//! `make bootstrap-program-id` once per deployment bootstrap. The script generates the
//! gitignored program keypair, derives the real public key and replaces the marker below
//! with `declare_id!("...")` before `anchor keys sync`.
//!
//! Public instruction surface is intentionally limited to the three domain transitions
//! plus one immutable deployment initializer.

use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;
pub mod verify;

// LASTRO_PROGRAM_ID_BOOTSTRAP_MARKER

#[program]
pub mod lastro {
    use super::*;

    pub fn initialize(
        ctx: Context<instructions::Initialize>,
        deployment_id: [u8; 32],
        station_pubkey33: [u8; 33],
    ) -> Result<()> {
        instructions::initialize::handler(ctx, deployment_id, station_pubkey33)
    }

    pub fn origin(ctx: Context<instructions::Origin>, event: [u8; 276]) -> Result<()> {
        instructions::origin::handler(ctx, event)
    }

    pub fn transfer(ctx: Context<instructions::Transfer>, event: [u8; 276]) -> Result<()> {
        instructions::transfer::handler(ctx, event)
    }

    pub fn reidentify(ctx: Context<instructions::Reidentify>, event: [u8; 276]) -> Result<()> {
        instructions::reidentify::handler(ctx, event)
    }
}

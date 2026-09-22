//! Solana integration for the API.
//!
//! Responsibilities: read canonical RPC state, build public transaction instructions
//! without user private keys, and describe the Secp256r1 + Lastro envelope. The wallet
//! remains the transaction signer.

pub mod rpc;
pub mod secp256r1;
pub mod transaction_builder;

/** Official Solana Secp256r1 signature-verification precompile program address. */
export const SECP256R1_PROGRAM_ID = 'Secp256r1SigVerify1111111111111111111111111' as const

/** Lastro freezes the hackathon envelope to Secp256r1 at index 0 and Lastro at index 1. */
export const LASTRO_PROTOCOL_INSTRUCTION_COUNT = 2 as const

/** Legacy and v0 transactions must serialize to at most 1232 bytes. */
export const SOLANA_LEGACY_V0_MAX_TRANSACTION_BYTES = 1232 as const

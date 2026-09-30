/** Official Solana Secp256r1 signature-verification precompile program address. */
export const SECP256R1_PROGRAM_ID = 'Secp256r1SigVerify1111111111111111111111111' as const

/** Solana ComputeBudget program; the only program allowed after the Lastro instruction. */
export const COMPUTE_BUDGET_PROGRAM_ID = 'ComputeBudget111111111111111111111111111111' as const

/** At most one unit-limit and one unit-price instruction may follow the protocol pair. */
export const MAX_TRAILING_COMPUTE_BUDGET_INSTRUCTIONS = 2 as const

/** Runtime default when no unit-limit instruction is present. */
export const DEFAULT_COMPUTE_UNIT_LIMIT = 200_000n

/** Highest priority fee (lamports) a custodian is asked to sign: 0.0002 SOL, as in the API. */
export const MAX_PRIORITY_FEE_LAMPORTS = 200_000n

/** Legacy and v0 transactions must serialize to at most 1232 bytes. */
export const SOLANA_LEGACY_V0_MAX_TRANSACTION_BYTES = 1232 as const

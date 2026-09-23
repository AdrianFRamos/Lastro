/**
 * Build-time public configuration for the browser application.
 *
 * These values are public by design. Never place tokens/private keys in VITE_* variables.
 * Deployment identity is explicit: the app must not silently fall back to a different API,
 * RPC cluster or program because that would make wallet previews/verifier results ambiguous.
 */
export type SolanaChainIdentifier = `solana:${string}`

export interface WebConfig {
  apiBaseUrl: string
  solanaRpcUrl: string
  solanaChain: SolanaChainIdentifier
  lastroProgramId: string
  lastroDeploymentId: string
  lastroAuthority: string | null
}

function required(name: keyof ImportMetaEnv): string {
  const value = import.meta.env[name]
  if (typeof value !== 'string' || value.trim() === '') {
    throw new Error(`Missing required public build configuration: ${name}`)
  }
  return value.trim()
}

function httpUrl(name: keyof ImportMetaEnv): string {
  const raw = required(name)
  let parsed: URL
  try {
    parsed = new URL(raw)
  } catch {
    throw new Error(`Invalid URL in public build configuration: ${name}`)
  }
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
    throw new Error(`Unsupported URL scheme in public build configuration: ${name}`)
  }
  return raw
}

function solanaChain(name: 'VITE_SOLANA_CHAIN'): SolanaChainIdentifier {
  const raw = required(name)
  if (!raw.startsWith('solana:') || raw.length <= 'solana:'.length) {
    throw new Error(`${name} must be a Wallet Standard Solana chain identifier (solana:...)`)
  }
  return raw as SolanaChainIdentifier
}

function hex32(name: 'VITE_LASTRO_DEPLOYMENT_ID_HEX'): string {
  const raw = required(name)
  if (!/^[0-9a-f]{64}$/.test(raw)) {
    throw new Error(`${name} must be lowercase 32-byte hex`)
  }
  return raw
}

export const webConfig: WebConfig = Object.freeze({
  apiBaseUrl: httpUrl('VITE_API_BASE_URL').replace(/\/$/, ''),
  solanaRpcUrl: httpUrl('VITE_SOLANA_RPC_URL'),
  solanaChain: solanaChain('VITE_SOLANA_CHAIN'),
  lastroProgramId: required('VITE_LASTRO_PROGRAM_ID'),
  lastroDeploymentId: hex32('VITE_LASTRO_DEPLOYMENT_ID_HEX'),
  // This public trust anchor must be provisioned independently of the API and RPC.
  // Verification cannot return VALID when it is absent.
  lastroAuthority: import.meta.env.VITE_LASTRO_AUTHORITY || null,
})

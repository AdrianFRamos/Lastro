/**
 * Wallet login for the participant workspace.
 *
 * 1. The connected wallet signs a login message bound to this site, deployment and a fresh
 *    nonce; the browser verifies that Ed25519 signature against the wallet address, proving the
 *    visitor controls the key (signing a message costs nothing and sends no transaction).
 * 2. The workspace role then comes from the wallet's PartyRecord on Solana, never from a form.
 *    An unregistered wallet browses read-only; a suspended, revoked or expired party is refused.
 *
 * There is no server session: on-chain actions still require the wallet to sign each transaction.
 */
import { ed25519 } from '@noble/curves/ed25519.js'
import { address, getAddressEncoder } from '@solana/kit'
import { webConfig } from '../config'
import { participants, type RoleId } from '../demo/roles'
import { solanaClient } from '../solana/client'
import {
  PARTY_STATUS,
  findPartiesByWallet,
  workspaceRoleOf,
  type Party,
  type PartyAccountFetcher,
} from './partyRegistry'

export interface LoginMessageFields {
  domain: string
  wallet: string
  nonce: string
  issuedAt: string
}

/** Exact text the wallet signs; every line ends with a newline. */
export function buildLoginMessage(fields: LoginMessageFields): string {
  return [
    'Lastro login v1',
    `domain=${fields.domain}`,
    `wallet=${fields.wallet}`,
    `deploymentId=${webConfig.lastroDeploymentId}`,
    `programId=${webConfig.lastroProgramId}`,
    `nonce=${fields.nonce}`,
    `issuedAt=${fields.issuedAt}`,
    '',
  ].join('\n')
}

/** True only for a valid Ed25519 signature of `message` by the key behind `wallet`. */
export function verifyLoginSignature(
  message: Uint8Array,
  signature: Uint8Array,
  wallet: string,
): boolean {
  if (signature.length !== 64) return false
  try {
    const publicKey = new Uint8Array(getAddressEncoder().encode(address(wallet)))
    return ed25519.verify(signature, message, publicKey)
  } catch {
    return false
  }
}

export interface WalletSessionResult {
  role: RoleId
  wallet: string
  /** Null when the wallet is not a registered party of this deployment. */
  partyId: string | null
}

export type LoginOutcome =
  | { kind: 'signed-in'; session: WalletSessionResult }
  | { kind: 'blocked'; status: 'suspended' | 'revoked' | 'expired' }

const BLOCKED_STATUS: Record<number, 'suspended' | 'revoked' | 'expired'> = {
  [PARTY_STATUS.SUSPENDED]: 'suspended',
  [PARTY_STATUS.REVOKED]: 'revoked',
  [PARTY_STATUS.EXPIRED]: 'expired',
}

/**
 * Pick the workspace for a wallet from its verified parties. An active chain participant wins
 * (in chain order); an active non-participant role or no party at all is read-only; parties
 * that exist but are all inactive block the login.
 */
export function chooseSession(wallet: string, parties: Party[]): LoginOutcome {
  const active = parties.filter((party) => party.status === PARTY_STATUS.ACTIVE)
  for (const participant of participants) {
    const match = active.find((party) => workspaceRoleOf(party.role) === participant.id)
    if (match) {
      return {
        kind: 'signed-in',
        session: { role: participant.id, wallet, partyId: match.partyIdHex },
      }
    }
  }
  if (active.length > 0) {
    return {
      kind: 'signed-in',
      session: { role: 'viewer', wallet, partyId: active[0]!.partyIdHex },
    }
  }
  const inactive = parties.find((party) => BLOCKED_STATUS[party.status])
  if (inactive) return { kind: 'blocked', status: BLOCKED_STATUS[inactive.status]! }
  return { kind: 'signed-in', session: { role: 'viewer', wallet, partyId: null } }
}

export interface WalletLoginDeps {
  connectedWallet: () => string | null
  signMessage: (message: Uint8Array) => Promise<Uint8Array>
  fetchParties?: PartyAccountFetcher
  now?: () => Date
  nonce?: () => string
  domain?: string
}

function randomNonce(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16))
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')
}

export const browserWalletDeps: WalletLoginDeps = {
  connectedWallet: () => solanaClient.wallet.getState().connected?.account.address ?? null,
  signMessage: (message) => solanaClient.wallet.signMessage(message),
}

/** Re-read a stored wallet session's role from the chain (e.g. after a reload). */
export async function recheckWalletRole(
  wallet: string,
  fetchParties?: PartyAccountFetcher,
): Promise<LoginOutcome> {
  return chooseSession(wallet, await findPartiesByWallet(wallet, fetchParties))
}

/** Prove control of the connected wallet, then resolve its workspace from the chain. */
export async function loginWithConnectedWallet(
  deps: WalletLoginDeps = browserWalletDeps,
): Promise<LoginOutcome> {
  const wallet = deps.connectedWallet()
  if (!wallet) throw new Error('Connect a Solana wallet before signing in')

  const message = new TextEncoder().encode(
    buildLoginMessage({
      domain: deps.domain ?? window.location.host,
      wallet,
      nonce: (deps.nonce ?? randomNonce)(),
      issuedAt: (deps.now ?? (() => new Date()))().toISOString(),
    }),
  )
  const signature = await deps.signMessage(message)
  if (!verifyLoginSignature(message, signature, wallet)) {
    throw new Error('The wallet signature does not prove control of this address')
  }

  const parties = await findPartiesByWallet(wallet, deps.fetchParties)
  return chooseSession(wallet, parties)
}

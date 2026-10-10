/**
 * Reads the on-chain party registry of the configured Lastro deployment straight from Solana.
 *
 * A party is a `PartyRecord` account at the PDA [b"party", deploymentId, partyId]. It binds one
 * wallet to one role. Accounts are found by wallet with a `getProgramAccounts` filter, then every
 * candidate is re-checked locally (discriminator, wallet bytes and PDA of this deployment), so a
 * misbehaving RPC cannot hand back a party of another deployment or another wallet.
 */
import {
  address,
  getAddressDecoder,
  getAddressEncoder,
  getProgramDerivedAddress,
} from '@solana/kit'
import { webConfig } from '../config'
import type { RoleId } from '../demo/roles'
import { solanaClient } from '../solana/client'

/** Anchor account size: 8-byte discriminator + party_id + wallet + role (u16) + status + bump. */
export const PARTY_RECORD_SIZE = 8 + 32 + 32 + 2 + 1 + 1
const WALLET_OFFSET = 8 + 32
const PARTY_SEED = new TextEncoder().encode('party')

/** On-chain party statuses (programs/lastro-v2/src/instructions/parties.rs). */
export const PARTY_STATUS = { ACTIVE: 1, SUSPENDED: 2, REVOKED: 3, EXPIRED: 4 } as const

/** On-chain party roles (programs/lastro-v2/src/constants.rs). */
export const PARTY_ROLE = {
  PRODUCER: 1,
  CUSTODIAN: 2,
  SELLER: 3,
  BUYER: 4,
  TRANSPORTER: 5,
  SLAUGHTERHOUSE: 6,
  PROCESSING_FACILITY: 7,
  DISTRIBUTOR: 8,
  RETAILER: 9,
  AUDITOR: 10,
  OFFICIAL_SOURCE: 11,
  EXPORTER: 12,
} as const

/**
 * The workspace each on-chain role opens, following the chain: producer → transporter →
 * slaughterhouse → exporter (transport out of the country) → retailer (commerce). Roles without
 * a workspace of their own (custodian, seller, buyer, distributor, auditor, official source)
 * browse the chain read-only.
 */
const WORKSPACE_ROLE_BY_PARTY_ROLE: Record<number, RoleId> = {
  [PARTY_ROLE.PRODUCER]: 'producer',
  [PARTY_ROLE.TRANSPORTER]: 'carrier',
  [PARTY_ROLE.SLAUGHTERHOUSE]: 'slaughterhouse',
  [PARTY_ROLE.PROCESSING_FACILITY]: 'slaughterhouse',
  [PARTY_ROLE.EXPORTER]: 'exporter',
  [PARTY_ROLE.RETAILER]: 'merchant',
}

export function workspaceRoleOf(partyRole: number): RoleId {
  return WORKSPACE_ROLE_BY_PARTY_ROLE[partyRole] ?? 'viewer'
}

export interface PartyAccount {
  /** PDA address of the PartyRecord. */
  address: string
  /** Raw account data at finalized commitment. */
  data: Uint8Array
}

export interface Party {
  address: string
  partyIdHex: string
  wallet: string
  role: number
  status: number
}

export type PartyAccountFetcher = (wallet: string) => Promise<PartyAccount[]>

let discriminatorCache: Promise<Uint8Array> | null = null

function partyDiscriminator(): Promise<Uint8Array> {
  discriminatorCache ??= crypto.subtle
    .digest('SHA-256', new TextEncoder().encode('account:PartyRecord'))
    .then((digest) => new Uint8Array(digest).subarray(0, 8))
  return discriminatorCache
}

function hexToBytes(hex: string): Uint8Array {
  return Uint8Array.from(hex.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16))
}

function toHex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')
}

function equalBytes(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((byte, index) => byte === right[index])
}

/**
 * Decode one PartyRecord and accept it only if it belongs to `wallet` and sits at the PDA of
 * the configured deployment. Returns null for anything else.
 */
export async function decodeParty(account: PartyAccount, wallet: string): Promise<Party | null> {
  const { data } = account
  if (data.length !== PARTY_RECORD_SIZE) return null
  if (!equalBytes(data.subarray(0, 8), await partyDiscriminator())) return null

  const walletBytes = getAddressEncoder().encode(address(wallet))
  if (!equalBytes(data.subarray(WALLET_OFFSET, WALLET_OFFSET + 32), new Uint8Array(walletBytes)))
    return null

  const partyId = data.subarray(8, 40)
  const bump = data[75]!
  const [expected, expectedBump] = await getProgramDerivedAddress({
    programAddress: address(webConfig.lastroProgramId),
    seeds: [PARTY_SEED, hexToBytes(webConfig.lastroDeploymentId), partyId],
  })
  if (String(expected) !== account.address || expectedBump !== bump) return null

  const view = new DataView(data.buffer, data.byteOffset, data.byteLength)
  return {
    address: account.address,
    partyIdHex: toHex(partyId),
    wallet: getAddressDecoder().decode(data.subarray(WALLET_OFFSET, WALLET_OFFSET + 32)),
    role: view.getUint16(72, true),
    status: data[74]!,
  }
}

/** PartyRecord accounts of the Lastro program whose wallet field equals `wallet`. */
export const fetchPartyAccountsByWallet: PartyAccountFetcher = async (wallet) => {
  const accounts = await solanaClient.rpc
    .getProgramAccounts(address(webConfig.lastroProgramId), {
      commitment: 'finalized',
      encoding: 'base64',
      filters: [
        { dataSize: BigInt(PARTY_RECORD_SIZE) },
        {
          memcmp: {
            offset: BigInt(WALLET_OFFSET),
            bytes: wallet as never,
            encoding: 'base58',
          },
        },
      ],
    })
    .send()
  return accounts.map((entry) => ({
    address: String(entry.pubkey),
    data: Uint8Array.from(atob(entry.account.data[0]), (char) => char.charCodeAt(0)),
  }))
}

/** Every party of this deployment registered for `wallet`, verified locally. */
export async function findPartiesByWallet(
  wallet: string,
  fetchAccounts: PartyAccountFetcher = fetchPartyAccountsByWallet,
): Promise<Party[]> {
  const accounts = await fetchAccounts(wallet)
  const parties = await Promise.all(accounts.map((account) => decodeParty(account, wallet)))
  return parties.filter((party): party is Party => party !== null)
}

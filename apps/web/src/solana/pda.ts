/** Lastro v2 program-derived addresses, derived independently of the API. */
import { address, getProgramDerivedAddress, type Address } from '@solana/kit'
import { sha256 } from '../protocol/hash'

const seed = (value: string) => new TextEncoder().encode(value)

async function pda(programId: string, seeds: Uint8Array[]): Promise<[Address, number]> {
  const [derived, bump] = await getProgramDerivedAddress({
    programAddress: address(programId),
    seeds,
  })
  return [derived, bump]
}

export const configPda = (programId: string, deployment: Uint8Array) =>
  pda(programId, [seed('config-v2'), deployment])
export const stationRegistryPda = (programId: string, deployment: Uint8Array) =>
  pda(programId, [seed('station-registry'), deployment])
export const stationPda = (programId: string, deployment: Uint8Array, stationId: Uint8Array) =>
  pda(programId, [seed('station-v2'), deployment, stationId])
export const assetPda = (programId: string, deployment: Uint8Array, assetId: Uint8Array) =>
  pda(programId, [seed('asset'), deployment, assetId])
export const eventPda = (programId: string, deployment: Uint8Array, eventId: Uint8Array) =>
  pda(programId, [seed('event'), deployment, eventId])
export const rfidBindingPda = (programId: string, deployment: Uint8Array, rfidHash: Uint8Array) =>
  pda(programId, [seed('rfid'), deployment, rfidHash])
export const intentPda = (
  programId: string,
  deployment: Uint8Array,
  subjectId: Uint8Array,
  intentId: Uint8Array,
) => pda(programId, [seed('intent'), deployment, subjectId, intentId])

/** Deterministic custody intent id: SHA-256("LASTRO_V2_CUSTODY_INTENT\0" || transfer UUID). */
export function custodyIntentId(transferId: string): Promise<Uint8Array> {
  return sha256(seed('LASTRO_V2_CUSTODY_INTENT\0'), uuidBytes(transferId))
}

export function uuidBytes(value: string): Uint8Array {
  const hex = value.replaceAll('-', '')
  if (!/^[0-9a-f]{32}$/.test(hex)) throw new Error('transfer id must be a lowercase UUID')
  return Uint8Array.from(hex.match(/../g)!, (pair) => Number.parseInt(pair, 16))
}

/** Anchor instruction/account discriminator: first 8 bytes of SHA-256("<namespace>:<name>"). */
export async function discriminator(namespace: 'global' | 'account', name: string) {
  return (await sha256(seed(`${namespace}:${name}`))).subarray(0, 8)
}

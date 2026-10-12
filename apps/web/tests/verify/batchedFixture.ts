/**
 * A real presence proof batched in a handling session, extending the frozen fixture's last
 * head (state 3, tag B), signed with the test-only Station scalar 1 of the v2 vectors.
 */
import { p256 } from '@noble/curves/nist.js'
import vectors from '../../../../test-vectors/v2-capture.json'
import validFixture from '../../../../test-vectors/v2-evidence-package.valid.json'
import type { BatchedObservation, FacilityBoundaryProof } from '../../src/protocol/evidence'
import { fromHex, sha256, toHex } from '../../src/protocol/hash'
import {
  encodeV2DomainEvent,
  V2EventType,
  v2EventHashOfBytes,
  v2IdentifierPayloadHash,
} from '../../src/protocol/v2/domainEvent'
import { batchLeafHash, sessionEventId } from '../../src/protocol/v2/session'

export const SESSION_ID = '5e'.repeat(32)
export const FARM_ID = 'fa'.repeat(32)
/** Canonical boundary bytes of a one-ring square farm (CAR code, 0x00, rings). */
export async function farmBoundary(): Promise<FacilityBoundaryProof> {
  const carCode = 'MT-5103403-0A1B2C3D4E5F'
  const ring: [number, number][] = [
    [-561_000_000, -155_000_000],
    [-560_900_000, -155_000_000],
    [-560_900_000, -154_900_000],
    [-561_000_000, -154_900_000],
    [-561_000_000, -155_000_000],
  ]
  const code = new TextEncoder().encode(carCode)
  const bytes = new Uint8Array(code.length + 1 + 4 + 4 + ring.length * 8)
  const view = new DataView(bytes.buffer)
  bytes.set(code)
  let at = code.length + 1
  view.setUint32(at, 1, true)
  view.setUint32(at + 4, ring.length, true)
  at += 8
  for (const [lon, lat] of ring) {
    view.setInt32(at, lon, true)
    view.setInt32(at + 4, lat, true)
    at += 8
  }
  return {
    facilityId: FARM_ID,
    carCode,
    boundaryBase64: btoa(String.fromCharCode(...bytes)),
    geoCommitment: toHex(await sha256(new TextEncoder().encode('LASTRO_V2_GEO\0'), bytes)),
  }
}
const SIBLING = new Uint8Array(32).fill(0x44)
const last = validFixture.events.at(-1)!

export interface BatchedFixture {
  observation: BatchedObservation
  root: Uint8Array
  observedAt: bigint
}

export async function batchedObservation(
  overrides: { sessionId?: string; previousHash?: string; tagHex?: string } = {},
): Promise<BatchedFixture> {
  const tag = fromHex(overrides.tagHex ?? vectors.tags.b.canonical_hex, 8)
  const tagHash = await sha256(new TextEncoder().encode('LASTRO_RFID\0'), tag)
  const observedAt = BigInt(last.observedAt + 60)
  const envelope = encodeV2DomainEvent({
    schemaVersion: 1,
    eventType: V2EventType.ObservationRecorded,
    deploymentId: validFixture.deploymentId,
    subjectId: validFixture.asset.assetId,
    eventId: toHex(
      await sessionEventId(
        fromHex(overrides.sessionId ?? SESSION_ID, 32),
        fromHex(validFixture.asset.assetId, 32),
      ),
    ),
    stateVersion: BigInt(last.stateVersion),
    expectedPreviousHash: overrides.previousHash ?? last.eventHash,
    payloadHash: await v2IdentifierPayloadHash(tagHash, tagHash),
    sourceId: vectors.station.station_id_hex,
    observedAt,
    expiresAt: observedAt + 3_600n,
  })
  const signature = p256.sign(envelope, fromHex(vectors.station.private_scalar_hex, 32), {
    lowS: true,
  })
  const leaf = await batchLeafHash(envelope, signature)
  const root = await sha256(new TextEncoder().encode('LASTRO_V2_BATCH_NODE\0'), leaf, SIBLING)
  return {
    observation: {
      eventHash: await v2EventHashOfBytes(envelope),
      envelopeBytesBase64: btoa(String.fromCharCode(...envelope)),
      observedRfidHex: toHex(tag),
      stationPubkeyHex: vectors.station.pubkey33_hex,
      stationSignatureHex: toHex(signature),
      sessionId: SESSION_ID,
      facilityId: FARM_ID,
      batchIndex: 0,
      leafIndex: 0,
      leafCount: 2,
      merkleRoot: toHex(root),
      merkleProof: [toHex(SIBLING)],
      txSignature: validFixture.events[0]!.txSignature,
    },
    root,
    observedAt,
  }
}

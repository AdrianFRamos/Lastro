/**
 * Handling-session primitives (SPEC v2.1, S3/S4a), byte-for-byte with
 * `crates/lastro-protocol/src/v2/session.rs`: the session-bound capture/event ids and the
 * batch Merkle tree whose root the program anchors.
 */
import { equalBytes, sha256 } from '../hash'

const encoder = new TextEncoder()
const SESSION_ID_DOMAIN = encoder.encode('LASTRO_V2_SESSION_ID\0')
const SESSION_CAPTURE_DOMAIN = encoder.encode('LASTRO_V2_SESSION_CAPTURE\0')
const CAPTURE_EVENT_DOMAIN = encoder.encode('LASTRO_V2_CAPTURE_EVENT\0')
const BATCH_LEAF_DOMAIN = encoder.encode('LASTRO_V2_BATCH_LEAF\0')
const BATCH_NODE_DOMAIN = encoder.encode('LASTRO_V2_BATCH_NODE\0')

export const MAX_BATCH_LEAVES = 1024
/** A batch must be anchored within seven days of its last observation. */
export const MAX_BATCH_EVENT_AGE_SECONDS = 7 * 86_400

/**
 * The only id `open_session` accepts for these terms: it binds deployment, custodian, Station,
 * window and capacity, so no other wallet can occupy it.
 */
export function sessionId(
  deploymentId: Uint8Array,
  custodian: Uint8Array,
  stationId: Uint8Array,
  validFrom: number,
  expiresAt: number,
  maxEvents: number,
): Promise<Uint8Array> {
  const terms = new Uint8Array(8 + 8 + 4)
  const view = new DataView(terms.buffer)
  view.setBigInt64(0, BigInt(validFrom), true)
  view.setBigInt64(8, BigInt(expiresAt), true)
  view.setUint32(16, maxEvents, true)
  return sha256(SESSION_ID_DOMAIN, deploymentId, custodian, stationId, terms)
}

/** Event id of the one presence proof `assetId` may have in `sessionId`. */
export async function sessionEventId(
  sessionId: Uint8Array,
  assetId: Uint8Array,
): Promise<Uint8Array> {
  const captureId = (await sha256(SESSION_CAPTURE_DOMAIN, sessionId, assetId)).subarray(0, 16)
  return sha256(CAPTURE_EVENT_DOMAIN, captureId)
}

export function batchLeafHash(envelope: Uint8Array, stationSignature: Uint8Array) {
  return sha256(BATCH_LEAF_DOMAIN, envelope, stationSignature)
}

/** Whether `leaf` sits at `index` of a `leafCount`-leaf batch with `root`. */
export async function verifyBatchProof(
  leaf: Uint8Array,
  index: number,
  leafCount: number,
  proof: readonly Uint8Array[],
  root: Uint8Array,
): Promise<boolean> {
  if (leafCount < 1 || leafCount > MAX_BATCH_LEAVES || index < 0 || index >= leafCount) return false
  const depth = leafCount === 1 ? 0 : Math.ceil(Math.log2(leafCount))
  if (proof.length !== depth) return false
  let node = leaf
  let at = index
  for (const sibling of proof) {
    node =
      at % 2 === 0
        ? await sha256(BATCH_NODE_DOMAIN, node, sibling)
        : await sha256(BATCH_NODE_DOMAIN, sibling, node)
    at = Math.floor(at / 2)
  }
  return equalBytes(node, root)
}

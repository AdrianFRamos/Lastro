/** Strict `lastro.evidence-package.v2` transport parser used before cryptographic verification. */
export const EVIDENCE_PACKAGE_SCHEMA = 'lastro.evidence-package.v2' as const
export const MAX_PACKAGE_EVENTS = 1024

export interface EvidenceAsset {
  assetId: string
  assetType: number
  status: number
  custodian: string
  stateVersion: number
  eventSequence: number
  lastEventHash: string
  currentRfidHash: string | null
  availableWeightGrams: number
}

export interface EvidenceEvent {
  eventId: string
  eventHash: string
  deploymentId: string
  subjectId: string
  sourceId: string
  eventType: number
  stateVersion: number
  observedAt: number
  expiresAt: number
  expectedPreviousHash: string
  payloadHash: string
  envelopeBytesBase64: string
  stationPubkeyHex: string
  stationSignatureHex: string
  /** Canonical RFID the Station read; null for events without a physical read. */
  observedRfidHex: string | null
  status: 'FINALIZED'
  txSignature: string
}

export interface CustodyProof {
  transferId: string
  newCustodian: string
  txSignature: string
}

/** A presence proof anchored in a handling-session batch (v2.1). */
export interface BatchedObservation {
  eventHash: string
  envelopeBytesBase64: string
  observedRfidHex: string
  stationPubkeyHex: string
  stationSignatureHex: string
  sessionId: string
  /** Facility the batch's Station was installed at (S5). */
  facilityId: string
  batchIndex: number
  leafIndex: number
  leafCount: number
  merkleRoot: string
  merkleProof: string[]
  txSignature: string
}

export interface EvidencePackage {
  schema: typeof EVIDENCE_PACKAGE_SCHEMA
  deploymentId: string
  lastroProgramId: string
  asset: EvidenceAsset
  events: EvidenceEvent[]
  custodyTransfers: CustodyProof[]
  /** Absent in packages exported before v2.1 (kept absent, so proof bytes round-trip). */
  batchedObservations?: BatchedObservation[]
  /** Boundaries of the facilities batched observations were made at (v2.1, S5). */
  facilities?: FacilityBoundaryProof[]
}

/** A facility's registered boundary: canonical bytes its on-chain geo_commitment hashes. */
export interface FacilityBoundaryProof {
  facilityId: string
  /** Null when the facility registered no boundary (its on-chain commitment is zero). */
  carCode: string | null
  boundaryBase64: string | null
  geoCommitment: string
}

const PACKAGE_KEYS = [
  'asset',
  'custodyTransfers',
  'deploymentId',
  'events',
  'lastroProgramId',
  'schema',
]
const PACKAGE_KEYS_V2_1 = [
  'asset',
  'batchedObservations',
  'custodyTransfers',
  'deploymentId',
  'events',
  'facilities',
  'lastroProgramId',
  'schema',
]
const FACILITY_KEYS = ['boundaryBase64', 'carCode', 'facilityId', 'geoCommitment']
/** Largest canonical boundary: 100-byte CAR, 64 rings, 10 000 vertices. */
const MAX_BOUNDARY_BYTES = 100 + 1 + 4 + 64 * 4 + 10_000 * 8
const BATCHED_KEYS = [
  'batchIndex',
  'envelopeBytesBase64',
  'eventHash',
  'facilityId',
  'leafCount',
  'leafIndex',
  'merkleProof',
  'merkleRoot',
  'observedRfidHex',
  'sessionId',
  'stationPubkeyHex',
  'stationSignatureHex',
  'txSignature',
]
/** A 1 024-leaf batch has a 10-level proof. */
const MAX_BATCH_PROOF_DEPTH = 10
const ASSET_KEYS = [
  'assetId',
  'assetType',
  'availableWeightGrams',
  'currentRfidHash',
  'custodian',
  'eventSequence',
  'lastEventHash',
  'stateVersion',
  'status',
]
const EVENT_KEYS = [
  'deploymentId',
  'envelopeBytesBase64',
  'eventHash',
  'eventId',
  'eventType',
  'expectedPreviousHash',
  'expiresAt',
  'observedAt',
  'observedRfidHex',
  'payloadHash',
  'sourceId',
  'stateVersion',
  'stationPubkeyHex',
  'stationSignatureHex',
  'status',
  'subjectId',
  'txSignature',
]
const CUSTODY_KEYS = ['newCustodian', 'transferId', 'txSignature']

function record(value: unknown, keys: readonly string[], name: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value))
    throw new Error(`invalid ${name} shape`)
  const actual = Object.keys(value).sort()
  if (actual.length !== keys.length || actual.some((key, index) => key !== keys[index]))
    throw new Error(`invalid ${name} shape`)
  return value as Record<string, unknown>
}

function hex(value: unknown, bytes: number, name: string): string {
  if (typeof value !== 'string' || !new RegExp(`^[0-9a-f]{${bytes * 2}}$`).test(value))
    throw new Error(`${name} must be ${bytes} lowercase hex bytes`)
  return value
}

function uint(value: unknown, name: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0)
    throw new Error(`${name} must be a non-negative safe integer`)
  return value
}

function base58(value: unknown, min: number, max: number, name: string): string {
  if (
    typeof value !== 'string' ||
    value.length < min ||
    value.length > max ||
    !/^[1-9A-HJ-NP-Za-km-z]+$/.test(value)
  )
    throw new Error(`${name} must be base58 text`)
  return value
}

function parseAsset(value: unknown): EvidenceAsset {
  const asset = record(value, ASSET_KEYS, 'EvidencePackage asset')
  return {
    assetId: hex(asset.assetId, 32, 'assetId'),
    assetType: uint(asset.assetType, 'assetType'),
    status: uint(asset.status, 'status'),
    custodian: hex(asset.custodian, 32, 'custodian'),
    stateVersion: uint(asset.stateVersion, 'stateVersion'),
    eventSequence: uint(asset.eventSequence, 'eventSequence'),
    lastEventHash: hex(asset.lastEventHash, 32, 'lastEventHash'),
    currentRfidHash:
      asset.currentRfidHash === null ? null : hex(asset.currentRfidHash, 32, 'currentRfidHash'),
    availableWeightGrams: uint(asset.availableWeightGrams, 'availableWeightGrams'),
  }
}

function parseEvent(value: unknown): EvidenceEvent {
  const event = record(value, EVENT_KEYS, 'EvidenceEvent')
  if (
    typeof event.envelopeBytesBase64 !== 'string' ||
    !/^[A-Za-z0-9+/]{294}==$/.test(event.envelopeBytesBase64)
  )
    throw new Error('EvidenceEvent envelope must be base64 of exactly 220 bytes')
  if (event.status !== 'FINALIZED')
    throw new Error('EvidencePackage may only contain FINALIZED events')
  return {
    eventId: hex(event.eventId, 32, 'eventId'),
    eventHash: hex(event.eventHash, 32, 'eventHash'),
    deploymentId: hex(event.deploymentId, 32, 'deploymentId'),
    subjectId: hex(event.subjectId, 32, 'subjectId'),
    sourceId: hex(event.sourceId, 32, 'sourceId'),
    eventType: uint(event.eventType, 'eventType'),
    stateVersion: uint(event.stateVersion, 'stateVersion'),
    observedAt: uint(event.observedAt, 'observedAt'),
    expiresAt: uint(event.expiresAt, 'expiresAt'),
    expectedPreviousHash: hex(event.expectedPreviousHash, 32, 'expectedPreviousHash'),
    payloadHash: hex(event.payloadHash, 32, 'payloadHash'),
    envelopeBytesBase64: event.envelopeBytesBase64,
    stationPubkeyHex: hex(event.stationPubkeyHex, 33, 'Station public key'),
    stationSignatureHex: hex(event.stationSignatureHex, 64, 'Station signature'),
    observedRfidHex:
      event.observedRfidHex === null ? null : hex(event.observedRfidHex, 8, 'observed RFID'),
    status: 'FINALIZED',
    txSignature: base58(event.txSignature, 64, 88, 'transaction signature'),
  }
}

function parseCustodyProof(value: unknown): CustodyProof {
  const proof = record(value, CUSTODY_KEYS, 'CustodyProof')
  if (
    typeof proof.transferId !== 'string' ||
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(proof.transferId)
  )
    throw new Error('transferId must be a lowercase UUID')
  return {
    transferId: proof.transferId,
    newCustodian: hex(proof.newCustodian, 32, 'newCustodian'),
    txSignature: base58(proof.txSignature, 64, 88, 'transaction signature'),
  }
}

function parseBatchedObservation(value: unknown): BatchedObservation {
  const entry = record(value, BATCHED_KEYS, 'BatchedObservation')
  if (
    typeof entry.envelopeBytesBase64 !== 'string' ||
    !/^[A-Za-z0-9+/]{294}==$/.test(entry.envelopeBytesBase64)
  )
    throw new Error('BatchedObservation envelope must be base64 of exactly 220 bytes')
  if (!Array.isArray(entry.merkleProof) || entry.merkleProof.length > MAX_BATCH_PROOF_DEPTH)
    throw new Error(`merkleProof must hold at most ${MAX_BATCH_PROOF_DEPTH} hashes`)
  return {
    eventHash: hex(entry.eventHash, 32, 'eventHash'),
    envelopeBytesBase64: entry.envelopeBytesBase64,
    observedRfidHex: hex(entry.observedRfidHex, 8, 'observed RFID'),
    stationPubkeyHex: hex(entry.stationPubkeyHex, 33, 'Station public key'),
    stationSignatureHex: hex(entry.stationSignatureHex, 64, 'Station signature'),
    sessionId: hex(entry.sessionId, 32, 'sessionId'),
    facilityId: hex(entry.facilityId, 32, 'facilityId'),
    batchIndex: uint(entry.batchIndex, 'batchIndex'),
    leafIndex: uint(entry.leafIndex, 'leafIndex'),
    leafCount: uint(entry.leafCount, 'leafCount'),
    merkleRoot: hex(entry.merkleRoot, 32, 'merkleRoot'),
    merkleProof: entry.merkleProof.map((node) => hex(node, 32, 'merkleProof node')),
    txSignature: base58(entry.txSignature, 64, 88, 'transaction signature'),
  }
}

function parseFacility(value: unknown): FacilityBoundaryProof {
  const entry = record(value, FACILITY_KEYS, 'FacilityBoundaryProof')
  const both = entry.carCode !== null && entry.boundaryBase64 !== null
  const neither = entry.carCode === null && entry.boundaryBase64 === null
  if (!both && !neither) throw new Error('facility boundary must carry both CAR code and bytes')
  if (
    both &&
    (typeof entry.carCode !== 'string' ||
      !/^[\x21-\x7e]{1,100}$/.test(entry.carCode) ||
      typeof entry.boundaryBase64 !== 'string' ||
      entry.boundaryBase64.length > Math.ceil(MAX_BOUNDARY_BYTES / 3) * 4)
  )
    throw new Error('facility boundary is malformed or too large')
  return {
    facilityId: hex(entry.facilityId, 32, 'facilityId'),
    carCode: both ? (entry.carCode as string) : null,
    boundaryBase64: both ? (entry.boundaryBase64 as string) : null,
    geoCommitment: hex(entry.geoCommitment, 32, 'geoCommitment'),
  }
}

function list<T>(value: unknown, parse: (entry: unknown) => T, name: string): T[] {
  if (!Array.isArray(value) || value.length > MAX_PACKAGE_EVENTS)
    throw new Error(`EvidencePackage ${name} must be an array of at most ${MAX_PACKAGE_EVENTS}`)
  return value.map(parse)
}

export function parseEvidencePackage(value: unknown): EvidencePackage {
  const batched = typeof value === 'object' && value !== null && 'batchedObservations' in value
  const pkg = record(value, batched ? PACKAGE_KEYS_V2_1 : PACKAGE_KEYS, 'EvidencePackage')
  if (pkg.schema !== EVIDENCE_PACKAGE_SCHEMA) throw new Error('unsupported EvidencePackage schema')
  return {
    schema: EVIDENCE_PACKAGE_SCHEMA,
    deploymentId: hex(pkg.deploymentId, 32, 'deploymentId'),
    lastroProgramId: base58(pkg.lastroProgramId, 32, 44, 'lastroProgramId'),
    asset: parseAsset(pkg.asset),
    events: list(pkg.events, parseEvent, 'events'),
    custodyTransfers: list(pkg.custodyTransfers, parseCustodyProof, 'custodyTransfers'),
    ...(batched
      ? {
          batchedObservations: list(
            pkg.batchedObservations,
            parseBatchedObservation,
            'batchedObservations',
          ),
          facilities: list(pkg.facilities, parseFacility, 'facilities'),
        }
      : {}),
  }
}

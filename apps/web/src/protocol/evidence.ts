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

export interface EvidencePackage {
  schema: typeof EVIDENCE_PACKAGE_SCHEMA
  deploymentId: string
  lastroProgramId: string
  asset: EvidenceAsset
  events: EvidenceEvent[]
  custodyTransfers: CustodyProof[]
}

const PACKAGE_KEYS = [
  'asset',
  'custodyTransfers',
  'deploymentId',
  'events',
  'lastroProgramId',
  'schema',
]
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

function list<T>(value: unknown, parse: (entry: unknown) => T, name: string): T[] {
  if (!Array.isArray(value) || value.length > MAX_PACKAGE_EVENTS)
    throw new Error(`EvidencePackage ${name} must be an array of at most ${MAX_PACKAGE_EVENTS}`)
  return value.map(parse)
}

export function parseEvidencePackage(value: unknown): EvidencePackage {
  const pkg = record(value, PACKAGE_KEYS, 'EvidencePackage')
  if (pkg.schema !== EVIDENCE_PACKAGE_SCHEMA) throw new Error('unsupported EvidencePackage schema')
  return {
    schema: EVIDENCE_PACKAGE_SCHEMA,
    deploymentId: hex(pkg.deploymentId, 32, 'deploymentId'),
    lastroProgramId: base58(pkg.lastroProgramId, 32, 44, 'lastroProgramId'),
    asset: parseAsset(pkg.asset),
    events: list(pkg.events, parseEvent, 'events'),
    custodyTransfers: list(pkg.custodyTransfers, parseCustodyProof, 'custodyTransfers'),
  }
}

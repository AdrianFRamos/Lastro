export const V2_SCHEMA_VERSION = 1
export const V2_DOMAIN_EVENT_ENVELOPE_LEN = 220
export const V2_MAX_EVENT_AGE_SECONDS = 86_400

export const V2_HASH_DOMAIN_EVENT = 'LASTRO_V2_EVENT\0'
export const V2_HASH_DOMAIN_PAYLOAD = 'LASTRO_V2_PAYLOAD\0'

export enum V2EventType {
  AssetRegistered = 1,
  ObservationRecorded = 2,
  LocationObserved = 3,
  CustodyTransferred = 4,
  SlaughterConfirmed = 5,
  CarcassCreated = 6,
  TransformationStarted = 7,
  TransformationFinalized = 8,
  ProductCreated = 9,
  PackageCreated = 10,
  ShipmentCreated = 11,
  ShipmentAccepted = 12,
  QualityHoldPlaced = 13,
  QualityHoldReleased = 14,
  RecallOpened = 15,
  RecallClosed = 16,
  AssetMigrated = 17,
}

export type Hex32 = string

export interface V2DomainEventEnvelope {
  schemaVersion: number
  eventType: V2EventType
  deploymentId: Hex32
  subjectId: Hex32
  eventId: Hex32
  stateVersion: bigint
  expectedPreviousHash: Hex32
  payloadHash: Hex32
  sourceId: Hex32
  observedAt: bigint
  expiresAt: bigint
}

function assertHex32(value: string, field: string): void {
  if (!/^[0-9a-f]{64}$/i.test(value) || /^0{64}$/i.test(value)) {
    throw new Error(`${field} must be a non-zero 32-byte lowercase hex value`)
  }
}

function hexToBytes(value: string, field: string): Uint8Array {
  assertHex32(value, field)
  return Uint8Array.from(value.match(/../g)!, (pair) => Number.parseInt(pair, 16))
}

function bytesToHex(bytes: Uint8Array): string {
  return [...bytes].map((byte) => byte.toString(16).padStart(2, '0')).join('')
}

function assertEnvelope(envelope: V2DomainEventEnvelope): void {
  if (envelope.schemaVersion !== V2_SCHEMA_VERSION) throw new Error('unsupported v2 schema version')
  if (!(
    envelope.eventType >= V2EventType.AssetRegistered &&
    envelope.eventType <= V2EventType.AssetMigrated
  )) {
    throw new Error('unknown v2 event type')
  }
  assertHex32(envelope.deploymentId, 'deploymentId')
  assertHex32(envelope.subjectId, 'subjectId')
  assertHex32(envelope.eventId, 'eventId')
  assertHex32(envelope.expectedPreviousHash, 'expectedPreviousHash')
  assertHex32(envelope.payloadHash, 'payloadHash')
  assertHex32(envelope.sourceId, 'sourceId')
  if (envelope.observedAt < 0n || envelope.expiresAt < envelope.observedAt) {
    throw new Error('invalid v2 event validity window')
  }
  if (envelope.expiresAt - envelope.observedAt > BigInt(V2_MAX_EVENT_AGE_SECONDS)) {
    throw new Error('v2 event validity window exceeds configured maximum')
  }
}

export function encodeV2DomainEvent(envelope: V2DomainEventEnvelope): Uint8Array {
  assertEnvelope(envelope)
  const bytes = new Uint8Array(V2_DOMAIN_EVENT_ENVELOPE_LEN)
  const view = new DataView(bytes.buffer)
  view.setUint16(0, envelope.schemaVersion, true)
  view.setUint16(2, envelope.eventType, true)
  bytes.set(hexToBytes(envelope.deploymentId, 'deploymentId'), 4)
  bytes.set(hexToBytes(envelope.subjectId, 'subjectId'), 36)
  bytes.set(hexToBytes(envelope.eventId, 'eventId'), 68)
  view.setBigUint64(100, envelope.stateVersion, true)
  bytes.set(hexToBytes(envelope.expectedPreviousHash, 'expectedPreviousHash'), 108)
  bytes.set(hexToBytes(envelope.payloadHash, 'payloadHash'), 140)
  bytes.set(hexToBytes(envelope.sourceId, 'sourceId'), 172)
  view.setBigInt64(204, envelope.observedAt, true)
  view.setBigInt64(212, envelope.expiresAt, true)
  return bytes
}

export function decodeV2DomainEvent(bytes: Uint8Array): V2DomainEventEnvelope {
  if (bytes.byteLength !== V2_DOMAIN_EVENT_ENVELOPE_LEN)
    throw new Error('invalid v2 envelope length')
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  const envelope: V2DomainEventEnvelope = {
    schemaVersion: view.getUint16(0, true),
    eventType: view.getUint16(2, true) as V2EventType,
    deploymentId: bytesToHex(bytes.slice(4, 36)),
    subjectId: bytesToHex(bytes.slice(36, 68)),
    eventId: bytesToHex(bytes.slice(68, 100)),
    stateVersion: view.getBigUint64(100, true),
    expectedPreviousHash: bytesToHex(bytes.slice(108, 140)),
    payloadHash: bytesToHex(bytes.slice(140, 172)),
    sourceId: bytesToHex(bytes.slice(172, 204)),
    observedAt: view.getBigInt64(204, true),
    expiresAt: view.getBigInt64(212, true),
  }
  assertEnvelope(envelope)
  return envelope
}

async function sha256(bytes: Uint8Array): Promise<Uint8Array> {
  const input = new ArrayBuffer(bytes.byteLength)
  new Uint8Array(input).set(bytes)
  const digest = await globalThis.crypto.subtle.digest('SHA-256', input)
  return new Uint8Array(digest)
}

async function domainHash(domain: string, bytes: Uint8Array): Promise<Hex32> {
  const domainBytes = new TextEncoder().encode(domain)
  const input = new Uint8Array(domainBytes.byteLength + bytes.byteLength)
  input.set(domainBytes)
  input.set(bytes, domainBytes.byteLength)
  return bytesToHex(await sha256(input))
}

export async function v2EventHash(envelope: V2DomainEventEnvelope): Promise<Hex32> {
  return domainHash(V2_HASH_DOMAIN_EVENT, encodeV2DomainEvent(envelope))
}

export async function v2PayloadHash(canonicalPayload: Uint8Array): Promise<Hex32> {
  return domainHash(V2_HASH_DOMAIN_PAYLOAD, canonicalPayload)
}

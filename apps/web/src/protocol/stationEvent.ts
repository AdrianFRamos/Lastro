/**
 * Read-only StationEvent decoder used by the independent verifier.
 * Signed bytes are parsed at fixed offsets and are never reserialized before hashing or signature checks.
 */
import { MAGIC, OFFSET, STATION_EVENT_LENGTH, VERSION } from './constants'

export type Action = 1 | 2 | 3

export interface StationEvent {
  action: Action
  deploymentId: Uint8Array
  animalId: Uint8Array
  stationId: Uint8Array
  eventSequence: bigint
  identityRevision: number
  previousEventHash: Uint8Array
  oldRfidHash: Uint8Array
  newRfidHash: Uint8Array
  fromCustodian: Uint8Array
  toCustodian: Uint8Array
}

function copyRange(bytes: Uint8Array, start: number, end: number): Uint8Array {
  return bytes.slice(start, end)
}

function allZero(bytes: Uint8Array): boolean {
  return bytes.every((byte) => byte === 0)
}

function equalBytes(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false
  for (let index = 0; index < a.length; index += 1) {
    if (a[index] !== b[index]) return false
  }
  return true
}

function validateSemantics(event: StationEvent): void {
  if (event.action === 1) {
    if (
      event.eventSequence !== 1n ||
      event.identityRevision !== 1 ||
      !allZero(event.previousEventHash) ||
      !allZero(event.oldRfidHash) ||
      allZero(event.newRfidHash) ||
      !allZero(event.fromCustodian) ||
      allZero(event.toCustodian)
    ) {
      throw new Error('invalid ORIGIN semantics')
    }
    return
  }

  if (
    event.eventSequence < 2n ||
    event.identityRevision === 0 ||
    allZero(event.previousEventHash)
  ) {
    throw new Error('invalid transition sequence or predecessor')
  }

  if (event.action === 2) {
    if (
      allZero(event.oldRfidHash) ||
      !equalBytes(event.oldRfidHash, event.newRfidHash) ||
      allZero(event.fromCustodian) ||
      allZero(event.toCustodian) ||
      equalBytes(event.fromCustodian, event.toCustodian)
    ) {
      throw new Error('invalid TRANSFER semantics')
    }
    return
  }

  if (
    event.identityRevision < 2 ||
    allZero(event.oldRfidHash) ||
    allZero(event.newRfidHash) ||
    equalBytes(event.oldRfidHash, event.newRfidHash) ||
    allZero(event.fromCustodian) ||
    !equalBytes(event.fromCustodian, event.toCustodian)
  ) {
    throw new Error('invalid REIDENTIFY semantics')
  }
}

export function decodeStationEvent(bytes: Uint8Array): StationEvent {
  if (bytes.length !== STATION_EVENT_LENGTH) {
    throw new Error('StationEvent length must be exactly 276 bytes')
  }
  if (!equalBytes(bytes.subarray(OFFSET.magic, OFFSET.version), MAGIC)) {
    throw new Error('invalid StationEvent magic')
  }
  if (bytes[OFFSET.version] !== VERSION) {
    throw new Error('unsupported StationEvent version')
  }
  const action = bytes[OFFSET.action]
  if (action !== 1 && action !== 2 && action !== 3) {
    throw new Error('invalid StationEvent action')
  }
  if (bytes[OFFSET.reserved] !== 0 || bytes[OFFSET.reserved + 1] !== 0) {
    throw new Error('StationEvent reserved bytes must be zero')
  }

  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  const event: StationEvent = {
    action,
    deploymentId: copyRange(bytes, OFFSET.deploymentId, OFFSET.animalId),
    animalId: copyRange(bytes, OFFSET.animalId, OFFSET.stationId),
    stationId: copyRange(bytes, OFFSET.stationId, OFFSET.eventSequence),
    eventSequence: view.getBigUint64(OFFSET.eventSequence, true),
    identityRevision: view.getUint32(OFFSET.identityRevision, true),
    previousEventHash: copyRange(bytes, OFFSET.previousEventHash, OFFSET.oldRfidHash),
    oldRfidHash: copyRange(bytes, OFFSET.oldRfidHash, OFFSET.newRfidHash),
    newRfidHash: copyRange(bytes, OFFSET.newRfidHash, OFFSET.fromCustodian),
    fromCustodian: copyRange(bytes, OFFSET.fromCustodian, OFFSET.toCustodian),
    toCustodian: copyRange(bytes, OFFSET.toCustodian, OFFSET.end),
  }
  validateSemantics(event)
  return event
}

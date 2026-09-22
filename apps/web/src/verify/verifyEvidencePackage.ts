import type { EvidencePackage } from '../protocol/evidence'
import { eventHash, rfidHash, stationId } from '../protocol/hash'
import { verifyStationSignature } from '../protocol/p256'
import { decodeStationEvent, type StationEvent } from '../protocol/stationEvent'
import type { VerificationLayerResult, VerificationResult } from './types'

interface DecodedEvidence {
  bytes: Uint8Array
  event: StationEvent
  hash: Uint8Array
  observedRfid: Uint8Array
  stationPubkey: Uint8Array
  signature: Uint8Array
}

export async function verifyEvidencePackage(pkg: EvidencePackage): Promise<VerificationResult> {
  const rfid = result('RFID_EVIDENCE', 'VALID', 'Every observed RFID explains the signed new RFID hash')
  const station = result('STATION_SIGNATURE', 'VALID', 'Every Station signature and StationID binding is valid')
  const identity = result('IDENTITY_CONTINUITY', 'VALID', 'AnimalID, sequence, predecessor, revision, and RFID continuity are linear')
  const custody = result('CUSTODY', 'VALID', 'Custody transitions form one authorized local history')
  const onChain = result('ON_CHAIN_STATE', 'NOT_CHECKED', 'Canonical Solana state has not been checked yet')

  const decoded: Array<DecodedEvidence | null> = Array(pkg.events.length).fill(null)
  for (let index = 0; index < pkg.events.length; index += 1) {
    const evidence = pkg.events[index]!
    let current: DecodedEvidence
    try {
      const bytes = decodeBase64(evidence.eventBytesBase64, 276)
      const event = decodeStationEvent(bytes)
      current = {
        bytes,
        event,
        hash: await eventHash(bytes),
        observedRfid: decodeHex(evidence.observedRfidHex, 8),
        stationPubkey: decodeHex(evidence.stationPubkeyHex, 33),
        signature: decodeHex(evidence.stationSignatureHex, 64),
      }
      decoded[index] = current
    } catch (error) {
      const detail = eventDetail(index, error)
      identity.status = 'INVALID'
      identity.detail = detail
      station.status = 'INVALID'
      station.detail = detail
      rfid.status = 'INVALID'
      rfid.detail = detail
      custody.status = 'INVALID'
      custody.detail = detail
      continue
    }

    if (!equalHex(current.event.deploymentId, pkg.deploymentId) || !equalHex(current.event.animalId, pkg.animalId)) {
      identity.status = 'INVALID'
      identity.detail = `Event ${index + 1} does not belong to the package deployment and AnimalID`
    }

    try {
      const observedHash = await rfidHash(current.observedRfid)
      if (!equalBytes(observedHash, current.event.newRfidHash)) {
        rfid.status = 'INVALID'
        rfid.detail = `Event ${index + 1} observed RFID does not match newRfidHash`
      }
    } catch (error) {
      rfid.status = 'INVALID'
      rfid.detail = eventDetail(index, error)
    }

    try {
      const derivedStationId = await stationId(current.stationPubkey)
      if (!equalBytes(derivedStationId, current.event.stationId)) {
        station.status = 'INVALID'
        station.detail = `Event ${index + 1} StationID does not match its public key`
      } else if (!verifyStationSignature(current.bytes, current.stationPubkey, current.signature)) {
        station.status = 'INVALID'
        station.detail = `Event ${index + 1} P-256 signature is invalid`
      }
    } catch (error) {
      station.status = 'INVALID'
      station.detail = eventDetail(index, error)
    }

    if (index === 0) {
      if (current.event.action !== 1) {
        identity.status = 'INVALID'
        identity.detail = 'The first event must be ORIGIN'
      }
      continue
    }

    const previous = decoded[index - 1]
    if (!previous) {
      identity.status = 'INVALID'
      identity.detail = `Event ${index + 1} has no valid predecessor`
      custody.status = 'INVALID'
      custody.detail = `Event ${index + 1} custody predecessor is unavailable`
      continue
    }

    if (current.event.eventSequence !== previous.event.eventSequence + 1n) {
      identity.status = 'INVALID'
      identity.detail = `Event ${index + 1} does not advance eventSequence exactly once`
    }
    if (!equalBytes(current.event.previousEventHash, previous.hash)) {
      identity.status = 'INVALID'
      identity.detail = `Event ${index + 1} previousEventHash does not match its predecessor`
    }
    if (!equalBytes(current.event.oldRfidHash, previous.event.newRfidHash)) {
      identity.status = 'INVALID'
      identity.detail = `Event ${index + 1} does not continue from the previous current RFID`
    }
    const expectedRevision = current.event.action === 3
      ? previous.event.identityRevision + 1
      : previous.event.identityRevision
    if (current.event.identityRevision !== expectedRevision) {
      identity.status = 'INVALID'
      identity.detail = `Event ${index + 1} identityRevision is invalid for its action`
    }

    if (!equalBytes(current.event.fromCustodian, previous.event.toCustodian)) {
      custody.status = 'INVALID'
      custody.detail = `Event ${index + 1} is not authorized by the previous current custodian`
    }
    if (current.event.action === 2 && equalBytes(current.event.fromCustodian, current.event.toCustodian)) {
      custody.status = 'INVALID'
      custody.detail = `Event ${index + 1} TRANSFER does not change custodian`
    }
    if (current.event.action === 3 && !equalBytes(current.event.fromCustodian, current.event.toCustodian)) {
      custody.status = 'INVALID'
      custody.detail = `Event ${index + 1} REIDENTIFY changes custodian`
    }
  }

  const layers = [rfid, station, identity, custody, onChain]
  return { animalId: pkg.animalId, layers, valid: layers.every((layer) => layer.status === 'VALID') }
}

function result(layer: VerificationLayerResult['layer'], status: VerificationLayerResult['status'], detail: string): VerificationLayerResult {
  return { layer, status, detail }
}

function decodeHex(value: string, bytes: number): Uint8Array {
  if (!new RegExp(`^[0-9a-f]{${bytes * 2}}$`).test(value)) throw new Error(`expected ${bytes} lowercase hex bytes`)
  const out = new Uint8Array(bytes)
  for (let index = 0; index < bytes; index += 1) out[index] = Number.parseInt(value.slice(index * 2, index * 2 + 2), 16)
  return out
}

function decodeBase64(value: string, bytes: number): Uint8Array {
  const binary = atob(value)
  if (btoa(binary) !== value || binary.length !== bytes) throw new Error(`expected canonical base64 for ${bytes} bytes`)
  return Uint8Array.from(binary, (character) => character.charCodeAt(0))
}

function equalBytes(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((byte, index) => byte === right[index])
}

function equalHex(bytes: Uint8Array, expected: string): boolean {
  return bytes.length * 2 === expected.length
    && bytes.every((byte, index) => byte.toString(16).padStart(2, '0') === expected.slice(index * 2, index * 2 + 2))
}

function eventDetail(index: number, error: unknown): string {
  return `Event ${index + 1}: ${error instanceof Error ? error.message : 'verification failed'}`
}

/**
 * Local (offline) verification of a v2 EvidencePackage: every claim is recomputed from the
 * Station-signed 220-byte envelopes; the API's decoded fields are never trusted.
 */
import type { EvidenceEvent, EvidencePackage } from '../protocol/evidence'
import { fromBase64, fromHex, rfidHash, stationId, toHex } from '../protocol/hash'
import { verifyStationSignature } from '../protocol/p256'
import {
  decodeV2DomainEvent,
  V2EventType,
  v2EventHashOfBytes,
  v2IdentifierPayloadHash,
  type V2DomainEventEnvelope,
} from '../protocol/v2/domainEvent'
import type { VerificationLayerResult, VerificationResult } from './types'

const ZERO_HEX = '0'.repeat(64)

export async function verifyEvidencePackage(pkg: EvidencePackage): Promise<VerificationResult> {
  const rfid = result(
    'RFID_EVIDENCE',
    'Every observed RFID explains the signed payload and the current tag',
  )
  const station = result(
    'STATION_SIGNATURE',
    'Every Station signature and StationId binding is valid',
  )
  const identity = result(
    'IDENTITY_CONTINUITY',
    'Events form one hash-linked history of this asset ending at its current state',
  )
  const custody = result('CUSTODY', 'Accepted custody transfers end at the current custodian')
  const onChain: VerificationLayerResult = {
    layer: 'ON_CHAIN_STATE',
    status: 'NOT_CHECKED',
    detail: 'Canonical Solana state has not been checked yet',
  }
  const fail = (layer: VerificationLayerResult, detail: string) => {
    if (layer.status === 'VALID') {
      layer.status = 'INVALID'
      layer.detail = detail
    }
  }

  let previousHash = ZERO_HEX
  let previousVersion = 0
  let currentRfid = ZERO_HEX
  for (const [index, evidence] of pkg.events.entries()) {
    const label = `Event ${index + 1}`
    let bytes: Uint8Array
    let envelope: V2DomainEventEnvelope
    try {
      bytes = fromBase64(evidence.envelopeBytesBase64, 220)
      envelope = decodeV2DomainEvent(bytes)
    } catch (error) {
      const detail = `${label}: ${error instanceof Error ? error.message : 'invalid envelope'}`
      for (const layer of [rfid, station, identity, custody]) fail(layer, detail)
      continue
    }
    const eventHash = await v2EventHashOfBytes(bytes)

    if (!claimsMatchEnvelope(evidence, envelope, eventHash))
      fail(identity, `${label} fields differ from its signed envelope`)
    if (envelope.deploymentId !== pkg.deploymentId || envelope.subjectId !== pkg.asset.assetId)
      fail(identity, `${label} does not belong to the package deployment and asset`)
    if (envelope.expectedPreviousHash !== previousHash)
      fail(identity, `${label} does not extend the previous event hash`)
    if (Number(envelope.stateVersion) <= previousVersion)
      fail(identity, `${label} does not advance the asset state version`)
    previousHash = eventHash
    previousVersion = Number(envelope.stateVersion)

    const publicKey = fromHex(evidence.stationPubkeyHex, 33)
    try {
      if (toHex(await stationId(publicKey)) !== envelope.sourceId)
        fail(station, `${label} sourceId does not match its Station public key`)
      else if (!verifyStationSignature(bytes, publicKey, fromHex(evidence.stationSignatureHex, 64)))
        fail(station, `${label} P-256 signature is invalid`)
    } catch (error) {
      fail(station, `${label}: ${error instanceof Error ? error.message : 'invalid Station key'}`)
    }

    currentRfid = await replayRfid(envelope, evidence.observedRfidHex, currentRfid, (detail) =>
      fail(rfid, `${label} ${detail}`),
    )
  }

  if (pkg.asset.lastEventHash !== previousHash)
    fail(identity, 'Asset last event hash is not the final event of the package')
  if ((pkg.asset.currentRfidHash ?? ZERO_HEX) !== currentRfid)
    fail(rfid, 'Asset current RFID is not the tag the signed history ends with')

  const transfers = pkg.custodyTransfers
  transfers.forEach((transfer, index) => {
    if (index > 0 && transfer.newCustodian === transfers[index - 1]!.newCustodian)
      fail(custody, `Custody transfer ${index + 1} does not change the custodian`)
  })
  if (transfers.length > 0 && transfers.at(-1)!.newCustodian !== pkg.asset.custodian)
    fail(custody, 'The last accepted custody transfer is not the current custodian')

  const layers = [rfid, station, identity, custody, onChain]
  return {
    assetId: pkg.asset.assetId,
    layers,
    valid: layers.every((layer) => layer.status === 'VALID'),
  }
}

/** Apply one event to the current RFID hash and check the Station's payload commitment. */
async function replayRfid(
  envelope: V2DomainEventEnvelope,
  observedRfidHex: string | null,
  current: string,
  fail: (detail: string) => void,
): Promise<string> {
  const identityEvent =
    envelope.eventType === V2EventType.IdentifierBound ||
    envelope.eventType === V2EventType.IdentifierReplaced
  if (observedRfidHex === null) {
    if (identityEvent) fail('changes the RFID without the observed tag')
    return current
  }
  if (!identityEvent && envelope.eventType !== V2EventType.ObservationRecorded) {
    fail('carries an observed RFID but is not a capture event')
    return current
  }
  const observed = toHex(await rfidHash(fromHex(observedRfidHex, 8)))
  let old: string
  if (envelope.eventType === V2EventType.IdentifierBound) {
    if (current !== ZERO_HEX) fail('binds an RFID to an asset that already has one')
    old = ZERO_HEX
  } else if (envelope.eventType === V2EventType.IdentifierReplaced) {
    if (current === ZERO_HEX || observed === current)
      fail('is not a replacement of the active RFID')
    old = current
  } else {
    if (observed !== current) fail('observed a tag that is not the active RFID')
    old = observed
  }
  const payload = await v2IdentifierPayloadHash(fromHex(old, 32), fromHex(observed, 32))
  if (payload !== envelope.payloadHash) fail('observed RFID does not match the signed payload')
  return observed
}

function claimsMatchEnvelope(
  evidence: EvidenceEvent,
  envelope: V2DomainEventEnvelope,
  eventHash: string,
): boolean {
  return (
    evidence.eventHash === eventHash &&
    evidence.eventId === envelope.eventId &&
    evidence.deploymentId === envelope.deploymentId &&
    evidence.subjectId === envelope.subjectId &&
    evidence.sourceId === envelope.sourceId &&
    evidence.eventType === envelope.eventType &&
    BigInt(evidence.stateVersion) === envelope.stateVersion &&
    BigInt(evidence.observedAt) === envelope.observedAt &&
    BigInt(evidence.expiresAt) === envelope.expiresAt &&
    evidence.expectedPreviousHash === envelope.expectedPreviousHash &&
    evidence.payloadHash === envelope.payloadHash
  )
}

function result(layer: VerificationLayerResult['layer'], detail: string): VerificationLayerResult {
  return { layer, status: 'VALID', detail }
}

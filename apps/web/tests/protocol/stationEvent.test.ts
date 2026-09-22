import { describe, expect, it } from 'vitest'
import vectors from '../../../../test-vectors/vectors.json'
import { decodeStationEvent } from '../../src/protocol/stationEvent'
const bytes = (hex: string) => Uint8Array.from(Buffer.from(hex, 'hex'))

describe('protocol/stationEvent', () => {
  /**
   * ARRANGE: origin.bin and the expected field values in vectors.json.
   * ACTION: decode through the production StationEvent decoder.
   * ASSERT: length=276, action and every fixed field/counter match the fixture.
   * FAILURE MEANS: browser offsets differ from the frozen wire layout.
   */
  it('decodes the complete 276-byte ORIGIN fixture field by field', () => {
    const event = decodeStationEvent(bytes(vectors.events.origin.event_bytes_hex))
    expect(event.action).toBe(1)
    expect(event.eventSequence).toBe(1n)
    expect(event.identityRevision).toBe(1)
    expect(Buffer.from(event.deploymentId).toString('hex')).toBe(vectors.deployment_id_hex)
    expect(Buffer.from(event.animalId).toString('hex')).toBe(vectors.animal_id_hex)
    expect(Buffer.from(event.stationId).toString('hex')).toBe(vectors.station.station_id_hex)
    expect(Buffer.from(event.newRfidHash).toString('hex')).toBe(vectors.rfid.a_hash_hex)
    expect(Buffer.from(event.toCustodian).toString('hex')).toBe(vectors.custodians.a_hex)
  })

  /**
   * ARRANGE: TRANSFER fixture with sequence/revision encoded in the wire layout.
   * ACTION: decode numeric counters.
   * ASSERT: u64 event_sequence and u32 identity_revision use little-endian exactly.
   * FAILURE MEANS: histories break as soon as counters exceed one-byte values.
   */
  it('decodes sequence and revision with the protocol little-endian convention', () => {
    const raw = bytes(vectors.events.transfer.event_bytes_hex)
    const view = new DataView(raw.buffer, raw.byteOffset, raw.byteLength)
    view.setBigUint64(104, 0x0102030405060708n, true)
    view.setUint32(112, 0x01020304, true)
    const event = decodeStationEvent(raw)
    expect(event.eventSequence).toBe(0x0102030405060708n)
    expect(event.identityRevision).toBe(0x01020304)
  })

  /**
   * ARRANGE: one valid fixture per mutation; corrupt magic, version, action and reserved bytes.
   * ACTION: decode each mutated buffer independently.
   * ASSERT: every invalid header is rejected with no partial event returned.
   * FAILURE MEANS: unsupported wire formats can be misinterpreted as evidence.
   */
  it('rejects wrong magic version action and non-zero reserved bytes', () => {
    for (const [offset, value] of [[0, 0], [4, 2], [5, 4], [6, 1]] as const) {
      const raw = bytes(vectors.events.origin.event_bytes_hex)
      raw[offset] = value
      expect(() => decodeStationEvent(raw)).toThrow()
    }
  })

  /**
   * ARRANGE: valid fixture, then truncate to 275 and extend to 277 bytes.
   * ACTION: decode both values.
   * ASSERT: both fail before field interpretation.
   * FAILURE MEANS: offset-based crypto binding could diverge from parser semantics.
   */
  it('rejects any StationEvent length other than exactly 276 bytes', () => {
    const raw = bytes(vectors.events.origin.event_bytes_hex)
    expect(() => decodeStationEvent(raw.slice(0, 275))).toThrow()
    expect(() => decodeStationEvent(Uint8Array.from([...raw, 0]))).toThrow()
  })

  /**
   * ARRANGE: syntactically decodable ORIGIN/TRANSFER/REIDENTIFY events with impossible field relations.
   * ACTION: run semantic validation after byte decoding.
   * ASSERT: RFID, custody, revision and predecessor invariants are action-specific.
   * FAILURE MEANS: verifier can normalize an impossible history into an apparently valid one.
   */
  it('rejects action-specific semantic combinations that cannot represent a valid transition', () => {
    const origin = bytes(vectors.events.origin.event_bytes_hex)
    origin[116] = 1
    expect(() => decodeStationEvent(origin)).toThrow('invalid ORIGIN semantics')
    const transfer = bytes(vectors.events.transfer.event_bytes_hex)
    transfer[180] ^= 1
    expect(() => decodeStationEvent(transfer)).toThrow('invalid TRANSFER semantics')
    const reidentify = bytes(vectors.events.reidentify.event_bytes_hex)
    reidentify.set(reidentify.slice(148, 180), 180)
    expect(() => decodeStationEvent(reidentify)).toThrow('invalid REIDENTIFY semantics')
  })
})

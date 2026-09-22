import { describe, expect, it } from 'vitest'
import vectors from '../../../../test-vectors/vectors.json'
import { eventHash, rfidHash, stationId } from '../../src/protocol/hash'
const bytes = (hex: string) => Uint8Array.from(Buffer.from(hex, 'hex'))
const hex = (value: Uint8Array) => Buffer.from(value).toString('hex')

describe('protocol/hash', () => {
  /**
   * ARRANGE: canonical RFID A/B bytes and expected hashes from vectors.json.
   * ACTION: hash the frozen domain bytes followed by exactly 8 canonical bytes.
   * ASSERT: byte-for-byte equality with both expected 32-byte hashes.
   * FAILURE MEANS: browser RFID identity differs from Rust, firmware, or on-chain inputs.
   */
  it('rfid hash matches frozen cross-language vectors for both physical tags', async () => {
    expect(hex(await rfidHash(bytes(vectors.rfid.a_canonical_hex)))).toBe(vectors.rfid.a_hash_hex)
    expect(hex(await rfidHash(bytes(vectors.rfid.b_canonical_hex)))).toBe(vectors.rfid.b_hash_hex)
  })

  /**
   * ARRANGE: fixture compressed Station pubkey and expected StationID.
   * ACTION: hash LASTRO_STATION domain bytes followed by pubkey33.
   * ASSERT: exact 32-byte fixture StationID.
   * FAILURE MEANS: Station registration and evidence verification derive different identities.
   */
  it('station id matches the frozen compressed-key vector', async () => {
    expect(hex(await stationId(bytes(vectors.station.pubkey_compressed_hex)))).toBe(vectors.station.station_id_hex)
  })

  /**
   * ARRANGE: ORIGIN, TRANSFER, and REIDENTIFY binary fixtures.
   * ACTION: SHA-256 each exact 276-byte buffer.
   * ASSERT: expected event hash for all three events.
   * FAILURE MEANS: predecessor chaining cannot interoperate across layers.
   */
  it('event hash matches every frozen 276-byte StationEvent vector', async () => {
    for (const name of ['origin', 'transfer', 'reidentify']) {
      expect(hex(await eventHash(bytes(vectors.events[name].event_bytes_hex)))).toBe(vectors.events[name].event_hash_hex)
    }
  })

  /**
   * ARRANGE: the same displayed RFID value as canonical bytes and as reader-style text.
   * ACTION: hash both through the public protocol API.
   * ASSERT: only canonical 8-byte input is accepted.
   * FAILURE MEANS: reader-specific representation can contaminate canonical identity.
   */
  it('does not hash reader text or framing as if it were canonical RFID bytes', async () => {
    await expect(rfidHash(new TextEncoder().encode('8000130000000001'))).rejects.toThrow('exactly 8 bytes')
  })
})

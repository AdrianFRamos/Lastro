import { describe, expect, it } from 'vitest'
import vectors from '../../../../test-vectors/v2-capture.json'
import { fromHex, rfidHash, stationId, toHex } from '../../src/protocol/hash'
import {
  decodeV2DomainEvent,
  v2EventHashOfBytes,
  v2IdentifierPayloadHash,
} from '../../src/protocol/v2/domainEvent'

const ZERO = new Uint8Array(32)

describe('protocol/hash', () => {
  /**
   * ARRANGE: canonical RFID A/B bytes and expected hashes from the v2 vectors.
   * ACTION: hash the frozen domain bytes followed by exactly 8 canonical bytes.
   * ASSERT: byte-for-byte equality with both expected 32-byte hashes.
   * FAILURE MEANS: browser RFID identity differs from Rust, firmware, or on-chain inputs.
   */
  it('rfid hash matches frozen cross-language vectors for both physical tags', async () => {
    for (const tag of [vectors.tags.a, vectors.tags.b]) {
      expect(toHex(await rfidHash(fromHex(tag.canonical_hex, 8)))).toBe(tag.hash_hex)
    }
  })

  /**
   * ARRANGE: fixture compressed Station pubkey and expected StationId.
   * ACTION: hash LASTRO_STATION domain bytes followed by pubkey33.
   * ASSERT: exact 32-byte fixture StationId.
   * FAILURE MEANS: Station registration and evidence verification derive different identities.
   */
  it('station id matches the frozen compressed-key vector', async () => {
    expect(toHex(await stationId(fromHex(vectors.station.pubkey33_hex, 33)))).toBe(
      vectors.station.station_id_hex,
    )
  })

  /**
   * ARRANGE: the signed bind/replace/observe envelopes.
   * ACTION: hash each raw envelope and recompute its identifier payload from the tags.
   * ASSERT: event hashes and payload hashes equal the Rust-generated vectors.
   * FAILURE MEANS: the browser verifier would reject genuine evidence or accept a different tag.
   */
  it('event and identifier payload hashes match every v2 capture vector', async () => {
    const a = fromHex(vectors.tags.a.hash_hex, 32)
    const b = fromHex(vectors.tags.b.hash_hex, 32)
    const payloads = { bind: [ZERO, a], replace: [a, b], observe: [b, b] } as const
    for (const [name, [oldHash, newHash]] of Object.entries(payloads)) {
      const capture = vectors.captures[name as keyof typeof payloads]
      const envelope = fromHex(capture.envelope_hex, 220)
      expect(await v2EventHashOfBytes(envelope)).toBe(capture.event_hash_hex)
      expect(await v2IdentifierPayloadHash(oldHash, newHash)).toBe(capture.payload_hash_hex)
      expect(decodeV2DomainEvent(envelope).payloadHash).toBe(capture.payload_hash_hex)
    }
  })

  /**
   * ARRANGE: reader text instead of 8 canonical bytes.
   * ACTION: hash it as an RFID.
   * ASSERT: rejected before hashing.
   * FAILURE MEANS: framing or display text could be mistaken for a physical identifier.
   */
  it('does not hash reader text or framing as if it were canonical RFID bytes', async () => {
    await expect(rfidHash(new TextEncoder().encode('8000130000000001'))).rejects.toThrow('8 bytes')
  })
})

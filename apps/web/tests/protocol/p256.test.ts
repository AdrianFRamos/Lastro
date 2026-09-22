import { describe, expect, it } from 'vitest'
import vectors from '../../../../test-vectors/vectors.json'
import { verifyStationSignature } from '../../src/protocol/p256'

const ORDER = BigInt('0xffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551')

function hex(value: string): Uint8Array {
  return Uint8Array.from(value.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16))
}

function bigintTo32(value: bigint): Uint8Array {
  const out = new Uint8Array(32)
  for (let index = 31; index >= 0; index -= 1) {
    out[index] = Number(value & 0xffn)
    value >>= 8n
  }
  return out
}

function bytesToBigint(value: Uint8Array): bigint {
  return value.reduce((total, byte) => (total << 8n) | BigInt(byte), 0n)
}

describe('protocol/p256', () => {
  /**
   * ARRANGE: load all frozen StationEvent/signature vectors and the frozen compressed Station key.
   * ACTION: verify every signature over its exact raw 276-byte StationEvent.
   * ASSERT: ORIGIN, TRANSFER, and REIDENTIFY signatures are accepted.
   * FAILURE MEANS: browser verification is not interoperable with the shared protocol vectors.
   */
  it('verifies every frozen low-S P-256 fixture over its exact raw StationEvent', () => {
    const publicKey = hex(vectors.station.pubkey_compressed_hex)
    for (const name of ['origin', 'transfer', 'reidentify'] as const) {
      const fixture = vectors.events[name]
      const event = hex(fixture.event_bytes_hex)
      const signature = hex(fixture.station_signature_hex)
      expect(publicKey).toHaveLength(33)
      expect(event).toHaveLength(276)
      expect(signature).toHaveLength(64)
      expect(verifyStationSignature(event, publicKey, signature)).toBe(true)
    }
  })

  /**
   * ARRANGE: mutate one byte of an otherwise valid signed StationEvent.
   * ACTION: verify the original signature against the mutated raw bytes.
   * ASSERT: verification returns false.
   * FAILURE MEANS: tampered signed evidence could remain cryptographically valid.
   */
  it('rejects a signature when one StationEvent byte changes', () => {
    const event = hex(vectors.events.origin.event_bytes_hex)
    event[100] ^= 1
    expect(verifyStationSignature(event, hex(vectors.station.pubkey_compressed_hex), hex(vectors.events.origin.station_signature_hex))).toBe(false)
  })

  /**
   * ARRANGE: keep a valid event/signature but alter the compressed Station public key.
   * ACTION: verify using the substituted key.
   * ASSERT: verification returns false.
   * FAILURE MEANS: evidence could be attributed to a different Station identity.
   */
  it('rejects an otherwise valid signature under a different Station public key', () => {
    const wrongKey = hex(vectors.station.pubkey_compressed_hex)
    wrongKey[32] ^= 1
    expect(verifyStationSignature(hex(vectors.events.origin.event_bytes_hex), wrongKey, hex(vectors.events.origin.station_signature_hex))).toBe(false)
  })

  /**
   * ARRANGE: create wrong-length key/signature values and a non-compressed SEC1 key prefix.
   * ACTION: run Station signature verification for each malformed input.
   * ASSERT: every malformed encoding returns false.
   * FAILURE MEANS: ambiguous cryptographic encodings could cross the verifier boundary.
   */
  it('rejects malformed compressed-key and compact-signature encodings', () => {
    const event = hex(vectors.events.origin.event_bytes_hex)
    const key = hex(vectors.station.pubkey_compressed_hex)
    const signature = hex(vectors.events.origin.station_signature_hex)
    expect(verifyStationSignature(event, key.slice(0, 32), signature)).toBe(false)
    expect(verifyStationSignature(event, Uint8Array.from([4, ...key.slice(1)]), signature)).toBe(false)
    expect(verifyStationSignature(event, key, signature.slice(0, 63))).toBe(false)
  })

  /**
   * ARRANGE: convert a valid low-S signature into its mathematically equivalent high-S form.
   * ACTION: verify the malleated signature over the original event.
   * ASSERT: verification returns false.
   * FAILURE MEANS: browser and Solana acceptance rules could disagree on signature canonicality.
   */
  it('rejects the high-S equivalent of a valid signature', () => {
    const signature = hex(vectors.events.origin.station_signature_hex)
    const highS = ORDER - bytesToBigint(signature.slice(32))
    const malleated = Uint8Array.from([...signature.slice(0, 32), ...bigintTo32(highS)])
    expect(verifyStationSignature(hex(vectors.events.origin.event_bytes_hex), hex(vectors.station.pubkey_compressed_hex), malleated)).toBe(false)
  })

  /**
   * ARRANGE: provide both the raw 276-byte StationEvent and its 32-byte SHA-256 event hash.
   * ACTION: verify the same valid signature against each candidate message.
   * ASSERT: raw bytes verify and the pre-hashed message is rejected.
   * FAILURE MEANS: browser verification could accidentally double hash relative to Solana.
   */
  it('requires raw 276-byte event input and cannot verify by passing event_hash as message', () => {
    const signature = hex(vectors.events.origin.station_signature_hex)
    const key = hex(vectors.station.pubkey_compressed_hex)
    expect(verifyStationSignature(hex(vectors.events.origin.event_bytes_hex), key, signature)).toBe(true)
    expect(verifyStationSignature(hex(vectors.events.origin.event_hash_hex), key, signature)).toBe(false)
  })

  /**
   * ARRANGE: construct a compressed-looking but invalid/off-curve P-256 public key.
   * ACTION: verify a valid event/signature with that hostile key.
   * ASSERT: verification returns false without throwing.
   * FAILURE MEANS: malformed evidence could crash the independent verifier.
   */
  it('maps malformed off-curve public keys to false instead of throwing', () => {
    const malformed = new Uint8Array(33)
    malformed[0] = 2
    expect(() => verifyStationSignature(hex(vectors.events.origin.event_bytes_hex), malformed, hex(vectors.events.origin.station_signature_hex))).not.toThrow()
    expect(verifyStationSignature(hex(vectors.events.origin.event_bytes_hex), malformed, hex(vectors.events.origin.station_signature_hex))).toBe(false)
  })
})

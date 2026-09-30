import { describe, expect, it } from 'vitest'

import {
  decodeV2DomainEvent,
  encodeV2DomainEvent,
  V2_DOMAIN_EVENT_ENVELOPE_LEN,
  V2EventType,
  type V2DomainEventEnvelope,
} from '../../../src/protocol/v2/domainEvent'

const fixture: V2DomainEventEnvelope = {
  schemaVersion: 1,
  eventType: V2EventType.ObservationRecorded,
  deploymentId: '01'.repeat(32),
  subjectId: '02'.repeat(32),
  eventId: '03'.repeat(32),
  stateVersion: 7n,
  expectedPreviousHash: '04'.repeat(32),
  payloadHash: '05'.repeat(32),
  sourceId: '06'.repeat(32),
  observedAt: 1000n,
  expiresAt: 1060n,
}

describe('Lastro protocol v2 envelope', () => {
  /**
   * ARRANGE: a fixed observation envelope fixture.
   * ACTION: encode it with the browser codec.
   * ASSERT: length is 220 and schema/type, version and time fields sit at their Rust offsets in little-endian.
   * FAILURE MEANS: the browser verifier would hash different bytes than the chain and Rust services.
   */
  it('uses the fixed 220-byte little-endian layout', () => {
    const encoded = encodeV2DomainEvent(fixture)
    expect(encoded.byteLength).toBe(V2_DOMAIN_EVENT_ENVELOPE_LEN)
    expect(Array.from(encoded.slice(0, 4))).toEqual([1, 0, 2, 0])
    expect(Array.from(encoded.slice(100, 108))).toEqual([7, 0, 0, 0, 0, 0, 0, 0])
    expect(Array.from(encoded.slice(204, 212))).toEqual([232, 3, 0, 0, 0, 0, 0, 0])
    expect(Array.from(encoded.slice(212, 220))).toEqual([36, 4, 0, 0, 0, 0, 0, 0])
  })

  /**
   * ARRANGE: the same fixture.
   * ACTION: encode then decode it.
   * ASSERT: the decoded envelope equals the original field by field.
   * FAILURE MEANS: verification could silently alter a field before comparing hashes.
   */
  it('round-trips without changing any canonical field', () => {
    expect(decodeV2DomainEvent(encodeV2DomainEvent(fixture))).toEqual(fixture)
  })

  /**
   * ARRANGE: a 219-byte buffer and an encoding with a zeroed deploymentId.
   * ACTION: decode both.
   * ASSERT: each decode throws a length or deploymentId error.
   * FAILURE MEANS: truncated or anonymous envelopes would be accepted as evidence.
   */
  it('rejects malformed length and zero identifiers', () => {
    expect(() => decodeV2DomainEvent(new Uint8Array(219))).toThrow('length')
    const encoded = encodeV2DomainEvent(fixture)
    encoded.fill(0, 4, 36)
    expect(() => decodeV2DomainEvent(encoded)).toThrow('deploymentId')
  })
})

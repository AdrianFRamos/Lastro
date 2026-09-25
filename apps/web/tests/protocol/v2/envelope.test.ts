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
  it('uses the fixed 220-byte little-endian layout', () => {
    const encoded = encodeV2DomainEvent(fixture)
    expect(encoded.byteLength).toBe(V2_DOMAIN_EVENT_ENVELOPE_LEN)
    expect(Array.from(encoded.slice(0, 4))).toEqual([1, 0, 2, 0])
    expect(Array.from(encoded.slice(100, 108))).toEqual([7, 0, 0, 0, 0, 0, 0, 0])
    expect(Array.from(encoded.slice(204, 212))).toEqual([232, 3, 0, 0, 0, 0, 0, 0])
    expect(Array.from(encoded.slice(212, 220))).toEqual([36, 4, 0, 0, 0, 0, 0, 0])
  })

  it('round-trips without changing any canonical field', () => {
    expect(decodeV2DomainEvent(encodeV2DomainEvent(fixture))).toEqual(fixture)
  })

  it('rejects malformed length and zero identifiers', () => {
    expect(() => decodeV2DomainEvent(new Uint8Array(219))).toThrow('length')
    const encoded = encodeV2DomainEvent(fixture)
    encoded.fill(0, 4, 36)
    expect(() => decodeV2DomainEvent(encoded)).toThrow('deploymentId')
  })
})

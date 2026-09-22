import { describe, expect, it } from 'vitest'
import validFixture from '../../../../test-vectors/evidence-package.valid.json'
import { parseEvidencePackage } from '../../src/protocol/evidence'

describe('protocol/evidence', () => {
  /**
   * ARRANGE: load the committed valid EvidencePackage fixture.
   * ACTION: parse it through the strict browser transport parser.
   * ASSERT: every field and event remains byte-for-byte representationally unchanged.
   * FAILURE MEANS: transport parsing can mutate or reinterpret cryptographic evidence.
   */
  it('accepts the strict frozen EvidencePackage fixture without rewriting proof bytes', () => {
    const parsed = parseEvidencePackage(structuredClone(validFixture))
    expect(parsed).toEqual(validFixture)
    expect(parsed.events).toHaveLength(validFixture.events.length)
  })

  /**
   * ARRANGE: change only the package version of an otherwise valid frozen fixture.
   * ACTION: parse the package.
   * ASSERT: unsupported versions are rejected explicitly.
   * FAILURE MEANS: incompatible evidence formats could inherit current verification semantics.
   */
  it('rejects unsupported evidence package versions', () => {
    const value = structuredClone(validFixture) as Record<string, unknown>
    value.version = 2
    expect(() => parseEvidencePackage(value)).toThrow('unsupported EvidencePackage version')
  })

  /**
   * ARRANGE: independently corrupt hex/base64 lengths, remove required fields, and add a verdict field.
   * ACTION: parse each malformed package.
   * ASSERT: every malformed shape is rejected before cryptographic verification.
   * FAILURE MEANS: ambiguous or backend-authored evidence could enter the independent verifier.
   */
  it('rejects malformed encodings missing fields wrong lengths and undeclared verdict fields', () => {
    const malformedHex = structuredClone(validFixture) as any
    malformedHex.events[0].observedRfidHex = '00'
    expect(() => parseEvidencePackage(malformedHex)).toThrow()

    const malformedBase64 = structuredClone(validFixture) as any
    malformedBase64.events[0].eventBytesBase64 = '***'
    expect(() => parseEvidencePackage(malformedBase64)).toThrow()

    const missing = structuredClone(validFixture) as any
    delete missing.events[0].stationSignatureHex
    expect(() => parseEvidencePackage(missing)).toThrow()

    const verdict = structuredClone(validFixture) as any
    verdict.valid = true
    expect(() => parseEvidencePackage(verdict)).toThrow()
  })

  /**
   * ARRANGE: load the committed valid EvidencePackage fixture in canonical event order.
   * ACTION: parse and serialize the transport object without cryptographic transformation.
   * ASSERT: event order and all signed/evidence fields exactly match the original fixture.
   * FAILURE MEANS: browser transport handling can alter the proof being verified.
   */
  it('round trips evidence transport without changing event order or signed bytes', () => {
    const parsed = parseEvidencePackage(structuredClone(validFixture))
    expect(JSON.parse(JSON.stringify(parsed))).toEqual(validFixture)
  })
  /**
   * ARRANGE: create malformed and oversized transaction signature text.
   * ACTION: parse each transport object before cryptographic verification.
   * ASSERT: transaction-reference bounds reject both hostile variants.
   * FAILURE MEANS: a local/API package can carry ambiguous transaction identifiers.
   */
  it('rejects malformed transaction signatures', () => {
    const malformedSignature = structuredClone(validFixture) as any
    malformedSignature.events[0].txSignature = 'not-base58'
    expect(() => parseEvidencePackage(malformedSignature)).toThrow('transaction signature')

    const oversizedSignature = structuredClone(validFixture) as any
    oversizedSignature.events[0].txSignature = '1'.repeat(89)
    expect(() => parseEvidencePackage(oversizedSignature)).toThrow('transaction signature')
  })

})

import { describe, expect, it } from 'vitest'
import validFixture from '../../../../test-vectors/v2-evidence-package.valid.json'
import { parseEvidencePackage } from '../../src/protocol/evidence'

const fixture = () => structuredClone(validFixture) as any

describe('protocol/evidence', () => {
  /**
   * ARRANGE: load the committed v2 EvidencePackage fixture (bind -> replace -> observe).
   * ACTION: parse it through the strict browser transport parser.
   * ASSERT: every field and event remains representationally unchanged.
   * FAILURE MEANS: transport parsing can mutate or reinterpret cryptographic evidence.
   */
  it('accepts the strict frozen EvidencePackage fixture without rewriting proof bytes', () => {
    expect(parseEvidencePackage(fixture())).toEqual(validFixture)
  })

  /**
   * ARRANGE: expand the package beyond the bounded history.
   * ACTION: parse it before signature verification or RPC work.
   * ASSERT: parsing rejects the oversized history.
   * FAILURE MEANS: a hostile package can force unbounded decoding or RPC work.
   */
  it('rejects histories above the bounded verification budget', () => {
    const tooMany = fixture()
    tooMany.events = Array.from({ length: 1025 }, () => structuredClone(validFixture.events[0]))
    expect(() => parseEvidencePackage(tooMany)).toThrow('at most 1024')
  })

  /**
   * ARRANGE: change only the schema identifier.
   * ACTION: parse the package.
   * ASSERT: other formats (including v1 packages) are rejected explicitly.
   * FAILURE MEANS: incompatible evidence could inherit v2 verification semantics.
   */
  it('rejects unsupported evidence package schemas', () => {
    const value = fixture()
    value.schema = 'lastro.evidence-package.v1'
    expect(() => parseEvidencePackage(value)).toThrow('unsupported EvidencePackage schema')
  })

  /**
   * ARRANGE: corrupt encodings, remove fields, add a verdict field, include a non-final event.
   * ACTION: parse each malformed package.
   * ASSERT: every malformed shape is rejected before cryptographic verification.
   * FAILURE MEANS: ambiguous or backend-authored evidence could enter the independent verifier.
   */
  it('rejects malformed encodings missing fields undeclared fields and non-final events', () => {
    const mutations: Array<(value: any) => void> = [
      (value) => (value.events[0].observedRfidHex = '00'),
      (value) =>
        (value.events[0].envelopeBytesBase64 = value.events[0].envelopeBytesBase64.slice(4)),
      (value) => delete value.asset.currentRfidHash,
      (value) => (value.valid = true),
      (value) => (value.events[0].status = 'SUBMITTED'),
      (value) => (value.events[0].txSignature = 'not-base58'),
      (value) =>
        (value.custodyTransfers = [{ transferId: 'x', newCustodian: '00', txSignature: '1' }]),
    ]
    for (const mutate of mutations) {
      const value = fixture()
      mutate(value)
      expect(() => parseEvidencePackage(value)).toThrow()
    }
  })
})

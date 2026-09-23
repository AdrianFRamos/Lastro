import { describe, expect, it } from 'vitest'
import validFixture from '../../../../test-vectors/evidence-package.valid.json'
import tamperedFixture from '../../../../test-vectors/evidence-package.tampered.json'
import { parseEvidencePackage } from '../../src/protocol/evidence'
import { verifyEvidencePackage } from '../../src/verify/verifyEvidencePackage'

function validPackage() {
  return parseEvidencePackage(structuredClone(validFixture))
}

async function localStatuses(value: unknown) {
  return (await verifyEvidencePackage(parseEvidencePackage(value))).layers.filter(
    (layer) => layer.layer !== 'ON_CHAIN_STATE',
  )
}

describe('verify/evidence-package local verification', () => {
  /**
   * ARRANGE: parse the committed known-good EvidencePackage.
   * ACTION: run all local independent verification layers without relying on RPC.
   * ASSERT: every local layer is VALID, ON_CHAIN_STATE is NOT_CHECKED, and overall is not yet VALID.
   * FAILURE MEANS: local verification either rejects valid proof or improperly treats unchecked chain state as valid.
   */
  it('valid frozen package passes every local verification layer before RPC', async () => {
    const result = await verifyEvidencePackage(validPackage())
    expect(
      result.layers
        .filter((layer) => layer.layer !== 'ON_CHAIN_STATE')
        .every((layer) => layer.status === 'VALID'),
    ).toBe(true)
    expect(result.layers.find((layer) => layer.layer === 'ON_CHAIN_STATE')?.status).toBe(
      'NOT_CHECKED',
    )
    expect(result.valid).toBe(false)
  })

  /**
   * ARRANGE: load the committed fixture with one signed StationEvent byte changed.
   * ACTION: run local independent verification.
   * ASSERT: at least one local verification layer is INVALID.
   * FAILURE MEANS: signed evidence tampering could survive independent verification.
   */
  it('one changed signed event byte makes the package invalid', async () => {
    const layers = await localStatuses(structuredClone(tamperedFixture))
    expect(layers.some((layer) => layer.status === 'INVALID')).toBe(true)
  })

  /**
   * ARRANGE: keep signed bytes unchanged but replace the observed physical RFID value.
   * ACTION: run local verification.
   * ASSERT: RFID_EVIDENCE is INVALID.
   * FAILURE MEANS: evidence could claim a physical RFID that the StationEvent hash does not bind.
   */
  it('changed observed RFID value breaks the event-to-physical-evidence binding', async () => {
    const value = structuredClone(validFixture)
    value.events[0]!.observedRfidHex = '8000130000000002'
    const layers = await localStatuses(value)
    expect(layers.find((layer) => layer.layer === 'RFID_EVIDENCE')?.status).toBe('INVALID')
  })

  /**
   * ARRANGE: remove the middle event from an otherwise valid three-event package.
   * ACTION: verify the shortened local history.
   * ASSERT: IDENTITY_CONTINUITY is INVALID.
   * FAILURE MEANS: omitted canonical history could appear continuous.
   */
  it('omitting a middle event is detected by sequence and predecessor validation', async () => {
    const value = structuredClone(validFixture)
    value.events.splice(1, 1)
    const layers = await localStatuses(value)
    expect(layers.find((layer) => layer.layer === 'IDENTITY_CONTINUITY')?.status).toBe('INVALID')
  })

  /**
   * ARRANGE: reorder individually valid signed events in the package.
   * ACTION: verify the reordered local history.
   * ASSERT: IDENTITY_CONTINUITY is INVALID.
   * FAILURE MEANS: package order could rewrite protocol chronology.
   */
  it('reordering individually valid signed events invalidates the history', async () => {
    const value = structuredClone(validFixture)
    ;[value.events[1], value.events[2]] = [value.events[2]!, value.events[1]!]
    const layers = await localStatuses(value)
    expect(layers.find((layer) => layer.layer === 'IDENTITY_CONTINUITY')?.status).toBe('INVALID')
  })

  /**
   * ARRANGE: duplicate a valid transition so two entries descend from the same predecessor.
   * ACTION: verify the forked package.
   * ASSERT: IDENTITY_CONTINUITY is INVALID.
   * FAILURE MEANS: individually valid signatures could be misrepresented as one linear history.
   */
  it('detects a fork even when each individual Station signature is valid', async () => {
    const value = structuredClone(validFixture)
    value.events.splice(2, 0, structuredClone(value.events[1]!))
    const layers = await localStatuses(value)
    expect(layers.find((layer) => layer.layer === 'IDENTITY_CONTINUITY')?.status).toBe('INVALID')
  })

  /**
   * ARRANGE: append an older signed transfer after a valid REIDENTIFY event.
   * ACTION: verify the resulting local history.
   * ASSERT: IDENTITY_CONTINUITY is INVALID because the retired RFID/predecessor is stale.
   * FAILURE MEANS: a previous RFID could become current again without a valid REIDENTIFY transition.
   */
  it('rejects later use of the retired RFID after REIDENTIFY', async () => {
    const value = structuredClone(validFixture)
    value.events.push(structuredClone(value.events[1]!))
    const layers = await localStatuses(value)
    expect(layers.find((layer) => layer.layer === 'IDENTITY_CONTINUITY')?.status).toBe('INVALID')
  })
})

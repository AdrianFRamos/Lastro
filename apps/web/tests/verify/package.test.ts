import { describe, expect, it } from 'vitest'
import validFixture from '../../../../test-vectors/v2-evidence-package.valid.json'
import { parseEvidencePackage, type EvidencePackage } from '../../src/protocol/evidence'
import { verifyEvidencePackage } from '../../src/verify/verifyEvidencePackage'
import { batchedObservation, farmBoundary } from './batchedFixture'

const fixture = (): EvidencePackage => parseEvidencePackage(structuredClone(validFixture))

async function layer(pkg: EvidencePackage, name: string) {
  const result = await verifyEvidencePackage(pkg)
  return result.layers.find((entry) => entry.layer === name)!
}

function flipEnvelopeByte(pkg: EvidencePackage, index: number, offset: number): void {
  const bytes = Uint8Array.from(atob(pkg.events[index]!.envelopeBytesBase64), (c) =>
    c.charCodeAt(0),
  )
  bytes[offset] = bytes[offset]! ^ 1
  pkg.events[index]!.envelopeBytesBase64 = btoa(String.fromCharCode(...bytes))
}

describe('verify/evidence-package local verification', () => {
  /**
   * ARRANGE: the signed bind -> replace -> observe history.
   * ACTION: verify it locally.
   * ASSERT: every local layer is VALID and the chain layer stays NOT_CHECKED.
   * FAILURE MEANS: genuine evidence would be rejected, or local checks would claim chain truth.
   */
  it('valid frozen package passes every local verification layer before RPC', async () => {
    const result = await verifyEvidencePackage(fixture())
    expect(result.assetId).toBe(validFixture.asset.assetId)
    expect(result.layers.map((entry) => [entry.layer, entry.status])).toEqual([
      ['RFID_EVIDENCE', 'VALID'],
      ['STATION_SIGNATURE', 'VALID'],
      ['IDENTITY_CONTINUITY', 'VALID'],
      ['CUSTODY', 'VALID'],
      ['FACILITY_ORIGIN', 'VALID'],
      ['ON_CHAIN_STATE', 'NOT_CHECKED'],
    ])
    expect(result.valid).toBe(false)
  })

  /**
   * ARRANGE: flip one byte of a signed envelope (inside the event id).
   * ACTION: verify.
   * ASSERT: the Station signature and the claimed fields no longer match.
   * FAILURE MEANS: tampered evidence could still verify.
   */
  it('one changed signed envelope byte makes the package invalid', async () => {
    const pkg = fixture()
    flipEnvelopeByte(pkg, 1, 80)
    expect((await layer(pkg, 'STATION_SIGNATURE')).status).toBe('INVALID')
    expect((await layer(pkg, 'IDENTITY_CONTINUITY')).status).toBe('INVALID')
  })

  /**
   * ARRANGE: replace the observed tag of the bind event with another tag.
   * ACTION: verify.
   * ASSERT: RFID evidence is INVALID because the signed payload commits to the real tag.
   * FAILURE MEANS: a package could claim a different physical tag than the Station read.
   */
  it('changed observed RFID breaks the event-to-physical-evidence binding', async () => {
    const pkg = fixture()
    pkg.events[0]!.observedRfidHex = '8000130000000009'
    expect((await layer(pkg, 'RFID_EVIDENCE')).status).toBe('INVALID')
  })

  /**
   * ARRANGE: drop the middle (replace) event.
   * ACTION: verify.
   * ASSERT: the hash chain is broken and the presence proof no longer reads the active tag.
   * FAILURE MEANS: history could be silently truncated.
   */
  it('omitting a middle event is detected', async () => {
    const pkg = fixture()
    pkg.events.splice(1, 1)
    expect((await layer(pkg, 'IDENTITY_CONTINUITY')).status).toBe('INVALID')
    expect((await layer(pkg, 'RFID_EVIDENCE')).status).toBe('INVALID')
  })

  /**
   * ARRANGE: swap two individually valid events.
   * ACTION: verify.
   * ASSERT: continuity is INVALID.
   * FAILURE MEANS: reordered history could pass as canonical.
   */
  it('reordering individually valid signed events invalidates the history', async () => {
    const pkg = fixture()
    ;[pkg.events[0], pkg.events[1]] = [pkg.events[1]!, pkg.events[0]!]
    expect((await layer(pkg, 'IDENTITY_CONTINUITY')).status).toBe('INVALID')
  })

  /**
   * ARRANGE: claim a current RFID different from where the signed history ends.
   * ACTION: verify.
   * ASSERT: RFID evidence is INVALID.
   * FAILURE MEANS: a retired tag could be presented as the animal's current identity.
   */
  it('rejects a current RFID that the history does not end with', async () => {
    const pkg = fixture()
    pkg.asset.currentRfidHash = '8a604528f19062cc9ada87a5e225a5c967e2cf148c85d86daf02d04212ebf6e1'
    expect((await layer(pkg, 'RFID_EVIDENCE')).status).toBe('INVALID')
  })

  /**
   * ARRANGE: the fixture plus one session presence proof extending its last head; then the
   *          same proof moved to another session, extending an older head, observing the
   *          retired tag, or with a broken Merkle path.
   * ACTION: verify locally.
   * ASSERT: the genuine proof keeps every local layer VALID; each forgery fails its layer.
   * FAILURE MEANS: batched presence proofs could be replayed, misattributed or invented.
   */
  it('verifies batched presence proofs leaf by leaf', async () => {
    const pkg = fixture()
    pkg.batchedObservations = [(await batchedObservation()).observation]
    pkg.facilities = [await farmBoundary()]
    const result = await verifyEvidencePackage(pkg)
    expect(result.layers.filter((entry) => entry.status === 'INVALID')).toEqual([])

    const otherSession = fixture()
    const moved = (await batchedObservation({ sessionId: '6f'.repeat(32) })).observation
    otherSession.facilities = [await farmBoundary()]
    otherSession.batchedObservations = [{ ...moved, sessionId: '5e'.repeat(32) }]
    expect((await layer(otherSession, 'IDENTITY_CONTINUITY')).status).toBe('INVALID')

    const staleHead = fixture()
    staleHead.facilities = [await farmBoundary()]
    staleHead.batchedObservations = [
      (await batchedObservation({ previousHash: validFixture.events[1]!.eventHash })).observation,
    ]
    expect((await layer(staleHead, 'IDENTITY_CONTINUITY')).status).toBe('INVALID')

    const retiredTag = fixture()
    retiredTag.facilities = [await farmBoundary()]
    retiredTag.batchedObservations = [
      (await batchedObservation({ tagHex: '8000130000000001' })).observation,
    ]
    expect((await layer(retiredTag, 'RFID_EVIDENCE')).status).toBe('INVALID')

    const brokenPath = fixture()
    const genuine = (await batchedObservation()).observation
    brokenPath.facilities = [await farmBoundary()]
    brokenPath.batchedObservations = [{ ...genuine, merkleProof: ['55'.repeat(32)] }]
    expect((await layer(brokenPath, 'IDENTITY_CONTINUITY')).status).toBe('INVALID')

    const forgedSignature = fixture()
    forgedSignature.facilities = [await farmBoundary()]
    forgedSignature.batchedObservations = [
      { ...genuine, stationSignatureHex: genuine.stationSignatureHex.replace(/^../, '00') },
    ]
    expect((await layer(forgedSignature, 'STATION_SIGNATURE')).status).toBe('INVALID')
  })

  /**
   * ARRANGE: a genuine batched proof whose farm boundary is listed; then the boundary bytes
   *          altered, the farm omitted, and a farm without boundary claiming a commitment.
   * ACTION: verify locally.
   * ASSERT: FACILITY_ORIGIN is VALID only for the genuine boundary.
   * FAILURE MEANS: a package could locate presence proofs at a polygon nobody registered.
   */
  it('checks every batched observation against its facility boundary', async () => {
    const proof = (await batchedObservation()).observation
    const farm = await farmBoundary()
    const pkg = fixture()
    pkg.batchedObservations = [proof]
    pkg.facilities = [farm]
    expect((await layer(pkg, 'FACILITY_ORIGIN')).status).toBe('VALID')

    const moved = fixture()
    moved.batchedObservations = [proof]
    const bytes = Uint8Array.from(atob(farm.boundaryBase64!), (c) => c.charCodeAt(0))
    bytes[bytes.length - 1] = bytes[bytes.length - 1]! ^ 1
    moved.facilities = [{ ...farm, boundaryBase64: btoa(String.fromCharCode(...bytes)) }]
    expect((await layer(moved, 'FACILITY_ORIGIN')).status).toBe('INVALID')

    const omitted = fixture()
    omitted.batchedObservations = [proof]
    omitted.facilities = []
    expect((await layer(omitted, 'FACILITY_ORIGIN')).status).toBe('INVALID')

    const claimed = fixture()
    claimed.batchedObservations = [proof]
    claimed.facilities = [{ ...farm, carCode: null, boundaryBase64: null }]
    expect((await layer(claimed, 'FACILITY_ORIGIN')).status).toBe('INVALID')
  })

  /**
   * ARRANGE: a custody transfer list whose last recipient is not the current custodian.
   * ACTION: verify.
   * ASSERT: CUSTODY is INVALID.
   * FAILURE MEANS: custody history could contradict the canonical custodian.
   */
  it('requires custody transfers to end at the current custodian', async () => {
    const pkg = fixture()
    pkg.custodyTransfers = [
      {
        transferId: '00000000-0000-4000-8000-000000000001',
        newCustodian: 'c2'.repeat(32),
        txSignature: validFixture.events[0]!.txSignature,
      },
    ]
    expect((await layer(pkg, 'CUSTODY')).status).toBe('INVALID')
    pkg.custodyTransfers[0]!.newCustodian = pkg.asset.custodian
    expect((await layer(pkg, 'CUSTODY')).status).toBe('VALID')
  })
})

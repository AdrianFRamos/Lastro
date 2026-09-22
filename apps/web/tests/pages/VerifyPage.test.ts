import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import validFixture from '../../../../test-vectors/evidence-package.valid.json'
import VerifyPage from '../../src/pages/VerifyPage.vue'
import type { VerificationLayerResult } from '../../src/verify/types'

const mocks = vi.hoisted(() => ({
  getEvidencePackage: vi.fn(),
  verifyCanonicalChainState: vi.fn(),
}))

vi.mock('vue-router', () => ({
  useRoute: () => ({ params: {} }),
}))

vi.mock('../../src/api/client', () => ({
  api: { getEvidencePackage: mocks.getEvidencePackage },
}))

vi.mock('../../src/verify/verifyChain', () => ({
  verifyCanonicalChainState: mocks.verifyCanonicalChainState,
}))

const canonicalValid: VerificationLayerResult = {
  layer: 'ON_CHAIN_STATE',
  status: 'VALID',
  detail: 'Canonical Solana state matches the complete evidence history',
}

const finalizedSignatures = [
  '2AXDGYSE4f2sz7tvMMzyHvUfcoJmxudvdhBcmiUSo6ijwfYmfZYsKRxboQMPh3R4kUhXRVdtSXFXMheka4Rc4P2',
  '3L3RY5sT8K4kyEnqhizwaqxLEbcYvpGrGPNEYRwtbCSUtL6YL86jdrvCbohnP5q8VxQ3qzGmt3W3iQJW97rD7m3',
  '4VZdodJgBy6dxMgm45zusmRzrPvKtiumu5YrK9RLPJADpzeJzgebxHsoQD4B58FCFS6aGUufKZka56xFiBGpB94',
  '5f5r5AjuFd8WwUagQSztAgufUCE6rdYhXmjU5rtnBPsxmfC5fFCUGiqQCcQZmAfFzuo6gyYYm616Roc1HEhREX5',
]

function finalizedFixture() {
  const fixture = structuredClone(validFixture)
  fixture.events.forEach((entry, index) => {
    entry.txSignature = finalizedSignatures[index]!
  })
  return fixture
}

function mountPage(): VueWrapper {
  return mount(VerifyPage)
}

async function selectFile(wrapper: VueWrapper, name: string, contents: string): Promise<void> {
  const input = wrapper.get('input[type="file"]')
  const file = new File([contents], name, { type: 'application/json' })
  Object.defineProperty(input.element, 'files', { configurable: true, value: [file] })
  await input.trigger('change')
  await flushPromises()
  await new Promise((resolve) => setTimeout(resolve, 50))
  await flushPromises()
}

beforeEach(() => {
  mocks.getEvidencePackage.mockReset()
  mocks.verifyCanonicalChainState.mockReset()
  mocks.verifyCanonicalChainState.mockResolvedValue(canonicalValid)
})

describe('pages/VerifyPage independent verification UX', () => {
  /**
   * ARRANGE: block evidence-package API usage and provide the committed valid EvidencePackage as a local File.
   * ACTION: select the file in VerifyPage.
   * ASSERT: local crypto/history verification runs from file bytes and only final Solana comparison uses its verifier dependency.
   * FAILURE MEANS: verifier still depends on backend availability for proof bytes.
   */
  it('accepts a local evidence package and runs verification without the Lastro evidence API', async () => {
    const wrapper = mountPage()
    const fixture = finalizedFixture()
    await selectFile(wrapper, 'evidence.json', JSON.stringify(fixture))

    expect(mocks.getEvidencePackage).not.toHaveBeenCalled()
    expect(mocks.verifyCanonicalChainState).toHaveBeenCalledTimes(1)
    expect(mocks.verifyCanonicalChainState).toHaveBeenCalledWith(expect.objectContaining({
      animalId: validFixture.animalId,
      events: expect.arrayContaining([expect.objectContaining({ txSignature: finalizedSignatures[0] })]),
    }))
    expect(wrapper.text()).toContain('Evidence and canonical Solana state agree.')
    expect(wrapper.text()).toContain('Overall: VALID')
    wrapper.unmount()
  })

  /**
   * ARRANGE: fresh VerifyPage with no AnimalID lookup/package selected.
   * ACTION: render.
   * ASSERT: all layers start NOT_CHECKED; none use VALID status before evidence is supplied.
   * FAILURE MEANS: UI presents validity before performing evidence checks.
   */
  it('initial verification layers are NOT_CHECKED rather than optimistic VALID', () => {
    const wrapper = mountPage()
    const layerItems = wrapper.findAll('[aria-label="Verification result"] li')
    expect(layerItems).toHaveLength(5)
    for (const item of layerItems) expect(item.text()).toContain('NOT_CHECKED')
    expect(layerItems.some((item) => /— VALID\b/.test(item.text()))).toBe(false)
    expect(wrapper.text()).toContain('Overall: NOT_CHECKED')
    wrapper.unmount()
  })

  /**
   * ARRANGE: malformed JSON, unsupported schema version, and a structurally valid package whose StationEvent bytes are invalid.
   * ACTION: load each as a local package.
   * ASSERT: transport errors stay non-VALID and invalid event bytes produce INVALID without invoking canonical RPC comparison.
   * FAILURE MEANS: later checks can mask invalid evidence transport or signed-event bytes.
   */
  it('malformed or unsupported packages can never reach a canonical VALID result', async () => {
    const malformed = mountPage()
    await selectFile(malformed, 'malformed.json', '{not-json')
    expect(malformed.text()).not.toContain('Overall: VALID')
    expect(mocks.verifyCanonicalChainState).not.toHaveBeenCalled()
    malformed.unmount()

    const unsupported = mountPage()
    await selectFile(unsupported, 'unsupported.json', JSON.stringify({ ...validFixture, version: 2 }))
    expect(unsupported.text()).toContain('unsupported EvidencePackage version')
    expect(unsupported.text()).not.toContain('Overall: VALID')
    expect(mocks.verifyCanonicalChainState).not.toHaveBeenCalled()
    unsupported.unmount()

    const invalidEvent = structuredClone(validFixture)
    invalidEvent.events[0]!.eventBytesBase64 = btoa('\0'.repeat(276))
    const invalid = mountPage()
    await selectFile(invalid, 'invalid-event.json', JSON.stringify(invalidEvent))
    expect(invalid.text()).toContain('Overall: INVALID')
    expect(mocks.verifyCanonicalChainState).not.toHaveBeenCalled()
    invalid.unmount()
  })

  /**
   * ARRANGE: locally valid package and a simulated RPC outage represented as NOT_CHECKED.
   * ACTION: verify the local file.
   * ASSERT: local layers stay VALID, on-chain layer is NOT_CHECKED, and overall remains NOT_CHECKED.
   * FAILURE MEANS: UX conflates unavailable canonical state with proof acceptance.
   */
  it('shows RPC unavailability separately from local evidence validity', async () => {
    mocks.verifyCanonicalChainState.mockResolvedValueOnce({
      layer: 'ON_CHAIN_STATE',
      status: 'NOT_CHECKED',
      detail: 'Solana RPC unavailable',
    })
    const wrapper = mountPage()
    await selectFile(wrapper, 'evidence.json', JSON.stringify(finalizedFixture()))

    const layerItems = wrapper.findAll('[aria-label="Verification result"] li')
    expect(layerItems.slice(0, 4).every((item) => item.text().includes('VALID'))).toBe(true)
    expect(layerItems[4]!.text()).toContain('NOT_CHECKED')
    expect(layerItems[4]!.text()).toContain('Solana RPC unavailable')
    expect(wrapper.text()).toContain('Overall: NOT_CHECKED')
    expect(wrapper.text()).not.toContain('Evidence and canonical Solana state agree.')
    wrapper.unmount()
  })
})

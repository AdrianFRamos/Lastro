import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import VerificationPanel from '../../src/components/VerificationPanel.vue'
import type { VerificationLayerResult } from '../../src/verify/types'

const layers: VerificationLayerResult[] = [
  { layer: 'RFID_EVIDENCE', status: 'VALID', detail: 'rfid ok' },
  { layer: 'STATION_SIGNATURE', status: 'VALID', detail: 'signature ok' },
  { layer: 'IDENTITY_CONTINUITY', status: 'VALID', detail: 'identity ok' },
  { layer: 'CUSTODY', status: 'VALID', detail: 'custody ok' },
  { layer: 'ON_CHAIN_STATE', status: 'NOT_CHECKED', detail: 'rpc unavailable' },
]

describe('VerificationPanel', () => {
  /**
   * ARRANGE: provide one result for each required independent verification layer.
   * ACTION: render the panel.
   * ASSERT: all five layer names, statuses, and details remain separately visible.
   * FAILURE MEANS: the UI could collapse independent evidence checks into one opaque verdict.
   */
  it('renders all five verification layers independently', () => {
    const wrapper = mount(VerificationPanel, { props: { layers } })
    expect(wrapper.findAll('li')).toHaveLength(5)
    for (const layer of layers) expect(wrapper.text()).toContain(layer.detail)
    wrapper.unmount()
  })

  /**
   * ARRANGE: make one otherwise valid layer INVALID with a concrete tampering reason.
   * ACTION: render the panel.
   * ASSERT: INVALID and its exact reason remain visible.
   * FAILURE MEANS: verifier UI could hide why evidence failed.
   */
  it('keeps the invalid layer and concrete failure reason visible', () => {
    const invalid = layers.map((layer) => layer.layer === 'STATION_SIGNATURE'
      ? { ...layer, status: 'INVALID' as const, detail: 'P-256 signature is invalid' }
      : layer)
    const wrapper = mount(VerificationPanel, { props: { layers: invalid } })
    expect(wrapper.text()).toContain('INVALID')
    expect(wrapper.text()).toContain('P-256 signature is invalid')
    wrapper.unmount()
  })
})

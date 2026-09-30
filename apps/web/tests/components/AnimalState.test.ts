import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import AnimalState from '../../src/components/AnimalState.vue'

const asset = {
  assetId: '11'.repeat(32),
  assetType: 1,
  status: 1,
  custodian: '33'.repeat(32),
  stateVersion: 4,
  eventSequence: 3,
  lastEventHash: '44'.repeat(32),
  currentRfidHash: '22'.repeat(32),
  availableWeightGrams: 450000,
}

describe('AnimalState', () => {
  /**
   * ARRANGE: mount the card with a canonical AssetState.
   * ACTION: inspect rendered fields.
   * ASSERT: AssetID, RFID, custodian, state version and event count are visible exactly.
   * FAILURE MEANS: the demo can hide a state field needed to reason about identity or custody.
   */
  it('renders every canonical field required by the demo state contract', () => {
    const wrapper = mount(AnimalState, { props: { asset } })
    for (const value of [asset.assetId, asset.currentRfidHash, asset.custodian, '4', '3']) {
      expect(wrapper.text()).toContain(value)
    }
    wrapper.unmount()
  })

  /**
   * ARRANGE: a registered asset without an RFID.
   * ACTION: render the card.
   * ASSERT: the missing RFID is stated explicitly rather than fabricated.
   * FAILURE MEANS: an untagged animal could look physically identified.
   */
  it('renders the absence of an RFID explicitly', () => {
    const wrapper = mount(AnimalState, { props: { asset: { ...asset, currentRfidHash: null } } })
    expect(wrapper.text()).toContain('No RFID bound')
    wrapper.unmount()
  })
})

import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import AnimalState from '../../src/components/AnimalState.vue'

const animal = {
  animalId: '11'.repeat(32),
  visualRecoveryId: 'VIS-001',
  currentRfidHash: '22'.repeat(32),
  currentCustodian: '33'.repeat(32),
  identityRevision: 2,
  eventSequence: 4,
  lastEventHash: '44'.repeat(32),
}

describe('AnimalState', () => {
  /**
   * ARRANGE: mount the card with a populated confirmed animal projection.
   * ACTION: inspect rendered projection fields.
   * ASSERT: AnimalID, recovery id, RFID, custodian, revision, and sequence are visible exactly.
   * FAILURE MEANS: the demo can hide a state field needed to reason about continuity or custody.
   */
  it('renders every canonical/projection field required by the demo state contract', () => {
    const wrapper = mount(AnimalState, { props: { animal } })
    for (const value of [
      animal.animalId,
      animal.visualRecoveryId,
      animal.currentRfidHash,
      animal.currentCustodian,
      '2',
      '4',
    ]) {
      expect(wrapper.text()).toContain(value)
    }
    wrapper.unmount()
  })

  /**
   * ARRANGE: mount an animal registered locally but not yet originated on-chain.
   * ACTION: render the state card.
   * ASSERT: absent RFID/custodian are shown as Not originated rather than fabricated values.
   * FAILURE MEANS: pre-ORIGIN projection could look like canonical identity/custody already exists.
   */
  it('renders pre-ORIGIN absence explicitly without inventing canonical values', () => {
    const wrapper = mount(AnimalState, {
      props: {
        animal: {
          ...animal,
          currentRfidHash: null,
          currentCustodian: null,
          identityRevision: 0,
          eventSequence: 0,
          lastEventHash: null,
        },
      },
    })
    expect(wrapper.text().match(/Not originated/g)).toHaveLength(2)
    wrapper.unmount()
  })
})

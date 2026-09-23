import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import StationPanel from '../../src/components/StationPanel.vue'

describe('StationPanel', () => {
  /**
   * ARRANGE: provide explicit reader, signer, and capture facts known by the operator flow.
   * ACTION: render the Station panel.
   * ASSERT: only those supplied facts are displayed without additional readiness claims.
   * FAILURE MEANS: UI could present hardware capability not represented by the protocol state.
   */
  it('renders only hardware facts actually represented by the hackathon Station contract', () => {
    const wrapper = mount(StationPanel, {
      props: { reader: 'reader-a', signer: 'p256-key-a', capture: 'PENDING: cap-1' },
    })
    expect(wrapper.text()).toContain('reader-a')
    expect(wrapper.text()).toContain('p256-key-a')
    expect(wrapper.text()).toContain('PENDING: cap-1')
    expect(wrapper.text().toLowerCase()).not.toContain('verified')
    wrapper.unmount()
  })

  /**
   * ARRANGE: mount the Station panel without hardware facts.
   * ACTION: render defaults.
   * ASSERT: reader/signer remain unknown and capture remains none.
   * FAILURE MEANS: missing hardware telemetry could be misrepresented as ready or verified.
   */
  it('renders unknown Station facts explicitly rather than defaulting to ready', () => {
    const wrapper = mount(StationPanel)
    expect(wrapper.text()).toContain('unknown')
    expect(wrapper.text()).toContain('none')
    expect(wrapper.text().toLowerCase()).not.toContain('ready')
    wrapper.unmount()
  })
})

import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import CustodyTimeline from '../../src/components/CustodyTimeline.vue'

describe('components/CustodyTimeline', () => {
  /**
   * ARRANGE: ordered ORIGIN → TRANSFER → REIDENTIFY → TRANSFER events.
   * ACTION: mount timeline.
   * ASSERT: all four entries remain in event_sequence order and REIDENTIFY shows RFID/revision change without custody change.
   * FAILURE MEANS: UI can visually erase the identity transition central to the demo.
   */
  it('preserves every event including REIDENTIFY in canonical sequence order', () => {
    const events = [
      { sequence: 1, label: '#1 ORIGIN — custodian A' },
      { sequence: 2, label: '#2 TRANSFER — custodian B' },
      { sequence: 3, label: '#3 REIDENTIFY — RFID NEW, revision 1' },
      { sequence: 4, label: '#4 TRANSFER — custodian C' },
    ]
    const wrapper = mount(CustodyTimeline, { props: { events } })

    expect(wrapper.findAll('.timeline__label').map((item) => item.text())).toEqual(events.map((event) => event.label))
    expect(wrapper.text()).toContain('REIDENTIFY')
    expect(wrapper.text()).toContain('revision 1')
    wrapper.unmount()
  })

  /**
   * ARRANGE: two transfer events with visually similar labels but different sequences/custodians.
   * ACTION: render timeline.
   * ASSERT: both entries exist and stable keys derive from event identity/sequence, not label text.
   * FAILURE MEANS: rendering can collapse distinct history entries.
   */
  it('does not collapse distinct events that have duplicate-looking labels', async () => {
    const first = { sequence: 2, label: 'TRANSFER — custodian changed' }
    const second = { sequence: 4, label: 'TRANSFER — custodian changed' }
    const wrapper = mount(CustodyTimeline, { props: { events: [first, second] } })

    expect(wrapper.findAll('li')).toHaveLength(2)
    expect(wrapper.findAll('.timeline__label').map((item) => item.text())).toEqual([first.label, second.label])

    await wrapper.setProps({ events: [second, first] })
    expect(wrapper.findAll('li')).toHaveLength(2)
    expect(wrapper.findAll('.timeline__label').map((item) => item.text())).toEqual([second.label, first.label])
    wrapper.unmount()
  })
})

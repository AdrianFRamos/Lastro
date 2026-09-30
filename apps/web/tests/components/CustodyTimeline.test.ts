import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import CustodyTimeline from '../../src/components/CustodyTimeline.vue'

describe('components/CustodyTimeline', () => {
  /**
   * ARRANGE: ordered BIND → PRESENCE → REPLACE → CUSTODY entries.
   * ACTION: mount timeline.
   * ASSERT: all four entries remain in order and REPLACE explains the retired tag.
   * FAILURE MEANS: UI can visually erase the identity transition central to the demo.
   */
  it('preserves every event including RFID replacement in canonical order', () => {
    const events = [
      { sequence: 1, label: '#1 RFID BOUND — tag 8000130000000001' },
      { sequence: 2, label: '#2 PRESENCE PROVEN — tag 8000130000000001' },
      { sequence: 3, label: '#3 RFID REPLACED — new tag 8000130000000002' },
      { sequence: 4, label: 'CUSTODY → recipient' },
    ]
    const wrapper = mount(CustodyTimeline, { props: { events } })

    expect(wrapper.findAll('.timeline__label').map((item) => item.text())).toEqual(
      events.map((event) => event.label),
    )
    expect(wrapper.text()).toContain('REPLACE')
    expect(wrapper.text()).toContain('old RFID retired')
    wrapper.unmount()
  })

  /**
   * ARRANGE: two transfer events with visually similar labels but different sequences/custodians.
   * ACTION: render timeline.
   * ASSERT: both entries exist and stable keys derive from event identity/sequence, not label text.
   * FAILURE MEANS: rendering can collapse distinct history entries.
   */
  it('does not collapse distinct events that have duplicate-looking labels', async () => {
    const first = { sequence: 2, label: 'CUSTODY → custodian changed' }
    const second = { sequence: 4, label: 'CUSTODY → custodian changed' }
    const wrapper = mount(CustodyTimeline, { props: { events: [first, second] } })

    expect(wrapper.findAll('li')).toHaveLength(2)
    expect(wrapper.findAll('.timeline__label').map((item) => item.text())).toEqual([
      first.label,
      second.label,
    ])

    await wrapper.setProps({ events: [second, first] })
    expect(wrapper.findAll('li')).toHaveLength(2)
    expect(wrapper.findAll('.timeline__label').map((item) => item.text())).toEqual([
      second.label,
      first.label,
    ])
    wrapper.unmount()
  })
})

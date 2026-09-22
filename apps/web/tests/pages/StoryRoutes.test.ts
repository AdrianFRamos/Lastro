import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import { router } from '../../src/router'
import LandingPage from '../../src/pages/LandingPage.vue'
import ProblemPage from '../../src/pages/ProblemPage.vue'
import FuturePage from '../../src/pages/FuturePage.vue'

const routerLinkStub = {
  template: '<a><slot /></a>',
}

describe('product storytelling routes', () => {
  /**
   * ARRANGE: resolve registered application router definitions.
   * ACTION: inspect route path catalog.
   * ASSERT: core navigation routes are defined and match public IA.
   * FAILURE MEANS: navigation tree has diverged from documented routes.
   */
  it('exposes the intended public information architecture', () => {
    const paths = router.getRoutes().map((route) => route.path)
    expect(paths).toEqual(expect.arrayContaining([
      '/',
      '/problem',
      '/future',
      '/demo',
      '/verify/:animalId?',
    ]))
  })

  /**
   * ARRANGE: mount LandingPage with stubbed RouterLink.
   * ACTION: inspect rendered heading and primary CTA buttons.
   * ASSERT: value proposition and primary demo actions are rendered.
   * FAILURE MEANS: landing page copy or primary navigation actions missing.
   */
  it('renders the landing paths with Demo as the primary action', () => {
    const wrapper = mount(LandingPage, {
      global: { stubs: { RouterLink: routerLinkStub } },
    })

    expect(wrapper.get('h1').text()).toBe('Verifiable identity and custody for physical livestock.')
    expect(wrapper.text()).toContain('Open Demo')
    expect(wrapper.text()).toContain('The Problem')
    expect(wrapper.text()).toContain('The Future')
    wrapper.unmount()
  })

  /**
   * ARRANGE: mount ProblemPage with stubbed RouterLink.
   * ACTION: inspect rendered narrative text and regulatory citations.
   * ASSERT: physical trust gap narrative and external citations exist.
   * FAILURE MEANS: problem narrative lost regulatory evidence or structural context.
   */
  it('renders the sourced physical trust gap story', () => {
    const wrapper = mount(ProblemPage, {
      global: { stubs: { RouterLink: routerLinkStub } },
    })

    expect(wrapper.get('h1').text()).toContain('The asset moves.')
    expect(wrapper.text()).toContain('NO SHARED VERIFIABLE HISTORY')
    expect(wrapper.text()).toContain('MAPA / PNIB')
    expect(wrapper.text()).toContain('European Commission')
    expect(wrapper.text()).toContain('WOAH Terrestrial Code')
    wrapper.unmount()
  })

  /**
   * ARRANGE: mount FuturePage with stubbed RouterLink.
   * ACTION: inspect section hierarchy and thematic flow.
   * ASSERT: proven functionality is rendered before financial expansion path.
   * FAILURE MEANS: roadmap narrative conflates current proof with future expansion.
   */
  it('separates proven functionality from the future expansion path', () => {
    const wrapper = mount(FuturePage, {
      global: { stubs: { RouterLink: routerLinkStub } },
    })

    expect(wrapper.get('h1').text()).toContain('Start with what the physical asset is.')
    expect(wrapper.text()).toContain('PROVEN TODAY')
    expect(wrapper.text()).toContain('EXPANSION PATH')
    expect(wrapper.text()).toContain('Lastro starts with identity and custody.')

    const text = wrapper.text()
    expect(text.indexOf('PHYSICAL IDENTITY')).toBeLessThan(text.indexOf('FINANCIAL INFRASTRUCTURE'))
    wrapper.unmount()
  })

})

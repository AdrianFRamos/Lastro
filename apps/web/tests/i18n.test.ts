import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it } from 'vitest'
import LanguageSwitch from '../src/components/LanguageSwitch.vue'
import { locale, setLocale } from '../src/i18n'
import LandingPage from '../src/pages/LandingPage.vue'

const routerLinkStub = {
  template: '<a><slot /></a>',
}

describe('language switch', () => {
  beforeEach(() => setLocale('en'))

  /**
   * ARRANGE: mount LandingPage in English with stubbed RouterLink.
   * ACTION: click the Brazilian flag, then the UK flag.
   * ASSERT: the page copy, the pressed flag and the document language follow each click.
   * FAILURE MEANS: the flags no longer switch the site language.
   */
  it('switches the page between English and Portuguese', async () => {
    const wrapper = mount(LandingPage, {
      global: { stubs: { RouterLink: routerLinkStub } },
    })
    expect(wrapper.get('h1').text()).toBe('Verifiable identity and custody for physical livestock.')

    await wrapper.get('button[aria-label="Português (Brasil)"]').trigger('click')
    expect(wrapper.get('h1').text()).toBe('Identidade e custódia verificáveis para o rebanho.')
    expect(wrapper.text()).toContain('Abrir Demo')
    expect(wrapper.get('button[aria-label="Português (Brasil)"]').attributes('aria-pressed')).toBe(
      'true',
    )
    expect(document.documentElement.lang).toBe('pt-BR')

    await wrapper.get('button[aria-label="English"]').trigger('click')
    expect(wrapper.text()).toContain('Open Demo')
    expect(document.documentElement.lang).toBe('en')
    wrapper.unmount()
  })

  /**
   * ARRANGE: mount the switch alone.
   * ACTION: choose Portuguese.
   * ASSERT: the choice is stored for the next visit.
   * FAILURE MEANS: visitors would lose their language on every reload.
   */
  it('remembers the chosen language in this browser', async () => {
    const wrapper = mount(LanguageSwitch)
    await wrapper.get('button[aria-label="Português (Brasil)"]').trigger('click')
    expect(locale.value).toBe('pt')
    expect(window.localStorage.getItem('lastro.locale')).toBe('pt')
    wrapper.unmount()
  })
})

import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it } from 'vitest'
import LoginPage from '../../src/pages/LoginPage.vue'
import { setLocale } from '../../src/i18n'

const routerLinkStub = {
  template: '<a><slot /></a>',
}

describe('demo login page', () => {
  // These pages were designed in Portuguese; their copy is asserted in that language.
  beforeEach(() => setLocale('pt'))

  /**
   * ARRANGE: mount LoginPage with stubbed RouterLink.
   * ACTION: fill the credentials and submit the form.
   * ASSERT: the form renders both fields and answers with the simulated-access notice.
   * FAILURE MEANS: the ENTRAR destination lost its form or hides that login is not wired yet.
   */
  it('renders the login form and explains that access is simulated', async () => {
    const wrapper = mount(LoginPage, {
      global: { stubs: { RouterLink: routerLinkStub } },
    })

    expect(wrapper.get('h1').text()).toBe('Login')
    expect(wrapper.find('[role="status"]').exists()).toBe(false)

    await wrapper.get('#login-email').setValue('demo@lastro.app')
    await wrapper.get('#login-password').setValue('segredo123')
    await wrapper.get('form').trigger('submit')

    expect(wrapper.get('[role="status"]').text()).toContain('Demonstração')
    wrapper.unmount()
  })
})

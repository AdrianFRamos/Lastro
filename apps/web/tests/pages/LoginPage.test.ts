import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it } from 'vitest'
import { createMemoryHistory, createRouter } from 'vue-router'
import LoginPage from '../../src/pages/LoginPage.vue'
import { reloadWorkspace, session } from '../../src/demo/workspace'
import { setLocale } from '../../src/i18n'

async function mountLogin() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/chain-history', component: { template: '<div />' } },
      { path: '/login', name: 'login', component: LoginPage },
      { path: '/painel', name: 'workspace', component: { template: '<div />' } },
    ],
  })
  await router.push('/login')
  await router.isReady()
  const wrapper = mount(LoginPage, { global: { plugins: [router] } })
  return { wrapper, router }
}

describe('demo login page', () => {
  // These pages were designed in Portuguese; their copy is asserted in that language.
  beforeEach(() => {
    setLocale('pt')
    window.localStorage.clear()
    reloadWorkspace()
  })

  /**
   * ARRANGE: mount LoginPage with a router.
   * ACTION: submit with credentials but no access profile.
   * ASSERT: the form asks for the profile, explains that access is simulated and stays put.
   * FAILURE MEANS: the ENTRAR destination opens a workspace without knowing the profile.
   */
  it('asks for an access profile and explains that access is simulated', async () => {
    const { wrapper, router } = await mountLogin()

    expect(wrapper.get('h1').text()).toBe('Login')
    expect(wrapper.text()).toContain('Demonstração')
    expect(wrapper.findAll('#login-role option').map((option) => option.text())).toEqual([
      'Selecione seu perfil…',
      'Produtor',
      'Transportador',
      'Frigorífico',
      'Exportador',
      'Comerciante',
      'Usuário comum',
    ])

    await wrapper.get('#login-email').setValue('demo@lastro.app')
    await wrapper.get('#login-password').setValue('segredo123')
    await wrapper.get('form').trigger('submit')

    expect(wrapper.get('[role="alert"]').text()).toContain('perfil de acesso')
    expect(session.value).toBeNull()
    expect(router.currentRoute.value.name).toBe('login')
    wrapper.unmount()
  })

  /**
   * ARRANGE: mount LoginPage with a router.
   * ACTION: fill the credentials, pick the slaughterhouse profile and submit.
   * ASSERT: the profile is signed in and the visitor lands on the workspace.
   * FAILURE MEANS: signing in no longer redirects to the page of the chosen profile.
   */
  it('signs in with the chosen profile and opens its workspace', async () => {
    const { wrapper, router } = await mountLogin()

    await wrapper.get('#login-email').setValue('frigorifico@lastro.app')
    await wrapper.get('#login-password').setValue('segredo123')
    await wrapper.get('#login-role').setValue('slaughterhouse')
    await wrapper.get('form').trigger('submit')
    await flushPromises()

    expect(session.value).toEqual({ role: 'slaughterhouse', email: 'frigorifico@lastro.app' })
    expect(router.currentRoute.value.name).toBe('workspace')
    wrapper.unmount()
  })
})

import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createMemoryHistory, createRouter } from 'vue-router'
import type { LoginOutcome } from '../../src/auth/walletLogin'
import LoginPage from '../../src/pages/LoginPage.vue'
import { reloadWorkspace, session } from '../../src/demo/workspace'
import { setLocale } from '../../src/i18n'

const wallet = vi.hoisted(() => ({
  names: ['Phantom'] as string[],
  connected: [] as string[],
}))
const login = vi.hoisted(() => ({
  outcome: null as LoginOutcome | Error | null,
}))

vi.mock('../../src/solana/wallet', () => ({
  discoveredWalletNames: () => wallet.names,
  onWalletStateChange: () => () => {},
  connectWalletByName: async (name: string) => {
    wallet.connected.push(name)
  },
}))

vi.mock('../../src/auth/walletLogin', () => ({
  loginWithConnectedWallet: async () => {
    if (login.outcome instanceof Error) throw login.outcome
    return login.outcome
  },
}))

const WALLET = 'Vote111111111111111111111111111111111111111'

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

describe('wallet login page', () => {
  // These pages were designed in Portuguese; their copy is asserted in that language.
  beforeEach(() => {
    setLocale('pt')
    window.localStorage.clear()
    reloadWorkspace()
    wallet.names = ['Phantom']
    wallet.connected = []
    login.outcome = null
  })

  /**
   * ARRANGE: a browser with the Phantom wallet; the chain registers it as a slaughterhouse.
   * ACTION: sign in with Phantom.
   * ASSERT: there is no email, password or profile picker; the wallet is connected and the
   *         session takes the on-chain role, then the workspace opens.
   * FAILURE MEANS: the profile could still be chosen by the visitor instead of the chain.
   */
  it('signs a registered wallet into the role recorded on Solana', async () => {
    login.outcome = {
      kind: 'signed-in',
      session: { role: 'slaughterhouse', wallet: WALLET, partyId: '07'.repeat(32) },
    }
    const { wrapper, router } = await mountLogin()

    expect(wrapper.find('input').exists()).toBe(false)
    expect(wrapper.find('select').exists()).toBe(false)
    await wrapper.get('[data-wallet="Phantom"]').trigger('click')
    await flushPromises()

    expect(wallet.connected).toEqual(['Phantom'])
    expect(session.value).toEqual({
      mode: 'wallet',
      role: 'slaughterhouse',
      wallet: WALLET,
      partyId: '07'.repeat(32),
    })
    expect(router.currentRoute.value.name).toBe('workspace')
    wrapper.unmount()
  })

  /**
   * ARRANGE: the chain reports the wallet's registration as revoked, then the visitor cancels
   *          the signature in the wallet.
   * ACTION: try to sign in each time.
   * ASSERT: each attempt shows why it failed, no session starts and the page stays on login.
   * FAILURE MEANS: a revoked company or an unsigned login could open the workspace.
   */
  it('refuses revoked registrations and cancelled signatures', async () => {
    const { wrapper, router } = await mountLogin()

    login.outcome = { kind: 'blocked', status: 'revoked' }
    await wrapper.get('[data-wallet="Phantom"]').trigger('click')
    await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain('revogada')

    login.outcome = new Error('User rejected the request')
    await wrapper.get('[data-wallet="Phantom"]').trigger('click')
    await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain('cancelada')

    expect(session.value).toBeNull()
    expect(router.currentRoute.value.name).toBe('login')
    wrapper.unmount()
  })

  /**
   * ARRANGE: a browser without any Solana wallet.
   * ACTION: open the login and continue as a visitor.
   * ASSERT: the page explains how to get a wallet, and the visitor opens a read-only session.
   * FAILURE MEANS: judges without a wallet could not see the workspace at all.
   */
  it('offers a read-only visitor session when no wallet is installed', async () => {
    wallet.names = []
    const { wrapper, router } = await mountLogin()

    expect(wrapper.find('[data-action="wallet-login"]').exists()).toBe(false)
    expect(wrapper.text()).toContain('Nenhuma carteira Solana')
    await wrapper.get('[data-action="visitor-login"]').trigger('click')
    await flushPromises()

    expect(session.value).toEqual({ mode: 'visitor', role: 'viewer' })
    expect(router.currentRoute.value.name).toBe('workspace')
    wrapper.unmount()
  })
})

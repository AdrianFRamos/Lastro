import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createMemoryHistory, createRouter } from 'vue-router'
import WorkspacePage from '../../src/pages/WorkspacePage.vue'
import type { LoginOutcome } from '../../src/auth/walletLogin'
import {
  listRecords,
  reloadWorkspace,
  session,
  signInAsVisitor,
  signInWithWallet,
} from '../../src/demo/workspace'
import { setLocale } from '../../src/i18n'

const chain = vi.hoisted(() => ({ outcome: null as LoginOutcome | null }))

vi.mock('../../src/auth/walletLogin', () => ({
  recheckWalletRole: async () => {
    if (!chain.outcome) throw new Error('RPC unavailable')
    return chain.outcome
  },
}))

const WALLET = 'Vote111111111111111111111111111111111111111'
const PARTY = '07'.repeat(32)

function signInAs(role: 'producer' | 'merchant') {
  signInWithWallet({ role, wallet: WALLET, partyId: PARTY })
  chain.outcome = { kind: 'signed-in', session: { role, wallet: WALLET, partyId: PARTY } }
}

async function mountWorkspace() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/', component: { template: '<div />' } },
      { path: '/chain-history', component: { template: '<div />' } },
      { path: '/login', name: 'login', component: { template: '<div />' } },
      { path: '/painel', name: 'workspace', component: WorkspacePage },
    ],
  })
  await router.push('/painel')
  await router.isReady()
  const wrapper = mount(WorkspacePage, { global: { plugins: [router] } })
  return { wrapper, router }
}

describe('demo workspace page', () => {
  // These pages were designed in Portuguese; their copy is asserted in that language.
  beforeEach(() => {
    setLocale('pt')
    window.localStorage.clear()
    reloadWorkspace()
    chain.outcome = null
  })

  /**
   * ARRANGE: sign in as the producer and open the workspace.
   * ACTION: create a property through the form, edit it and delete it.
   * ASSERT: the producer sees its six sections and every step changes the table.
   * FAILURE MEANS: the producer cannot manage its records from the screen.
   */
  it('lets the producer create, edit and delete its records', async () => {
    signInAs('producer')
    const { wrapper } = await mountWorkspace()

    expect(wrapper.get('h1').text()).toBe('Produtor')
    expect(wrapper.findAll('[data-resource]').map((tab) => tab.text())).toEqual([
      'Propriedades',
      'Lotes de animais',
      'Animais avulsos',
      'Antenas',
      'Vacinas',
      'Peso',
    ])
    const before = wrapper.findAll('tbody tr').length

    await wrapper.get('[data-action="create"]').trigger('click')
    await wrapper.get('form').trigger('submit')
    expect(wrapper.get('[role="alert"]').text()).toContain('Nome')

    await wrapper.get('#producer-properties-name').setValue('Fazenda Nova')
    await wrapper.get('#producer-properties-city').setValue('Dourados/MS')
    await wrapper.get('#producer-properties-area').setValue('250')
    await wrapper.get('form').trigger('submit')
    expect(wrapper.findAll('tbody tr')).toHaveLength(before + 1)
    expect(wrapper.findAll('tbody tr').at(-1)?.text()).toContain('250 ha')

    await wrapper.findAll('[data-action="edit"]').at(-1)?.trigger('click')
    await wrapper.get('#producer-properties-city').setValue('Ponta Porã/MS')
    await wrapper.get('form').trigger('submit')
    expect(wrapper.findAll('tbody tr').at(-1)?.text()).toContain('Ponta Porã/MS')

    await wrapper.findAll('[data-action="delete"]').at(-1)?.trigger('click')
    await wrapper.get('[data-action="confirm-delete"]').trigger('click')
    expect(wrapper.findAll('tbody tr')).toHaveLength(before)
    expect(listRecords('producer.properties')).toHaveLength(before)
    wrapper.unmount()
  })

  /**
   * ARRANGE: sign in as the common user and open the workspace.
   * ACTION: pick the merchant and open its sales.
   * ASSERT: every participant can be browsed and no create, edit or delete action exists.
   * FAILURE MEANS: the common user can change records or cannot see the chain.
   */
  it('shows the whole chain to the common user without any change action', async () => {
    signInAsVisitor()
    const { wrapper } = await mountWorkspace()

    expect(wrapper.findAll('[data-participant]').map((tab) => tab.text())).toEqual([
      'Produtor',
      'Transportador',
      'Frigorífico',
      'Exportador',
      'Comerciante',
    ])

    await wrapper.get('[data-participant="merchant"]').trigger('click')
    await wrapper.get('[data-resource="merchant.sales"]').trigger('click')

    expect(wrapper.get('tbody tr').text()).toContain('R$ 125,86')
    expect(wrapper.text()).toContain('Somente leitura')
    expect(wrapper.find('[data-action="create"]').exists()).toBe(false)
    expect(wrapper.find('[data-action="edit"]').exists()).toBe(false)
    expect(wrapper.find('[data-action="delete"]').exists()).toBe(false)
    wrapper.unmount()
  })

  /**
   * ARRANGE: a stored producer wallet session whose registration was revoked on Solana since.
   * ACTION: open the workspace (it re-checks the role on the chain).
   * ASSERT: the session is dropped and the visitor is sent back to the login.
   * FAILURE MEANS: a stale browser session keeps a revoked company inside its workspace.
   */
  it('drops a stored wallet session whose party was revoked on Solana', async () => {
    signInAs('producer')
    chain.outcome = { kind: 'blocked', status: 'revoked' }
    const { wrapper, router } = await mountWorkspace()
    await flushPromises()

    expect(session.value).toBeNull()
    expect(router.currentRoute.value.name).toBe('login')
    wrapper.unmount()
  })

  /**
   * ARRANGE: a stored merchant wallet session whose on-chain role is now carrier.
   * ACTION: open the workspace.
   * ASSERT: the workspace follows the chain role and says the role was verified on Solana.
   * FAILURE MEANS: an edited browser session could keep a role the chain does not grant.
   */
  it('follows the on-chain role over the stored one', async () => {
    signInAs('merchant')
    chain.outcome = {
      kind: 'signed-in',
      session: { role: 'carrier', wallet: WALLET, partyId: PARTY },
    }
    const { wrapper } = await mountWorkspace()
    await flushPromises()

    expect(session.value).toMatchObject({ role: 'carrier' })
    expect(wrapper.get('h1').text()).toBe('Transportador')
    expect(wrapper.get('[data-role-check]').text()).toContain('verificado na Solana')
    expect(wrapper.get('[data-identity]').text()).toBe('Vote…1111')
    wrapper.unmount()
  })

  /**
   * ARRANGE: sign in as the merchant and open the workspace.
   * ACTION: sign out.
   * ASSERT: the visitor is sent back to the login.
   * FAILURE MEANS: the workspace stays open after leaving the profile.
   */
  it('returns to the login after signing out', async () => {
    signInAs('merchant')
    const { wrapper, router } = await mountWorkspace()

    await wrapper.get('[data-action="sign-out"]').trigger('click')
    await flushPromises()

    expect(router.currentRoute.value.name).toBe('login')
    wrapper.unmount()
  })
})

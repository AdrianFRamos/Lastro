import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it } from 'vitest'
import ChainHistoryPage from '../../src/pages/ChainHistoryPage.vue'
import { setLocale } from '../../src/i18n'

const routerLinkStub = {
  props: ['to'],
  template: '<a :href="to"><slot /></a>',
}

describe('chain history demo page', () => {
  // These pages were designed in Portuguese; their copy is asserted in that language.
  beforeEach(() => setLocale('pt'))

  /**
   * ARRANGE: mount ChainHistoryPage with stubbed RouterLink.
   * ACTION: inspect the rendered timeline stages.
   * ASSERT: the seven stages render newest first (producer → carrier → slaughterhouse → carrier →
   *         exporter → carrier → market), each custody starting where the previous one ended,
   *         with one current stage.
   * FAILURE MEANS: the Open Demo chain history no longer shows the simulated product journey.
   */
  it('renders the product journey newest stage first with the current stage marked', () => {
    const wrapper = mount(ChainHistoryPage, {
      global: { stubs: { RouterLink: routerLinkStub } },
    })

    const stages = wrapper.findAll('.chain-stage')
    expect(stages.map((stage) => stage.get('.chain-stage__name').text())).toEqual([
      'Mercado',
      'Transportadora no destino',
      'Exportador',
      'Transportadora de carne',
      'Frigorífico',
      'Transportadora de gado',
      'Produtor da Silva',
    ])
    expect(stages.map((stage) => stage.get('.chain-stage__period').text())).toEqual([
      expect.stringMatching(/13\/11\/2026 → hoje\s*Em andamento/),
      expect.stringMatching(/12\/11\/2026 → 13\/11\/2026\s*1 dia/),
      expect.stringMatching(/21\/10\/2026 → 12\/11\/2026\s*22 dias/),
      expect.stringMatching(/20\/10\/2026 → 21\/10\/2026\s*1 dia/),
      expect.stringMatching(/12\/10\/2026 → 20\/10\/2026\s*8 dias/),
      expect.stringMatching(/11\/10\/2026 → 12\/10\/2026\s*1 dia/),
      expect.stringMatching(/15\/09\/2024 → 11\/10\/2026\s*756 dias/),
    ])
    expect(stages.map((stage) => stage.get('.chain-stage__step').text())).toEqual([
      'Etapa 07',
      'Etapa 06',
      'Etapa 05',
      'Etapa 04',
      'Etapa 03',
      'Etapa 02',
      'Etapa 01',
    ])
    expect(wrapper.findAll('.chain-stage--current')).toHaveLength(1)
    expect(stages[0]?.classes()).toContain('chain-stage--current')
    expect(wrapper.text()).toContain('Entrar')
    wrapper.unmount()
  })

  /**
   * ARRANGE: mount ChainHistoryPage with a RouterLink stub that keeps the destination.
   * ACTION: read the link of every stage card.
   * ASSERT: each stage opens its own detail page.
   * FAILURE MEANS: clicking a stage in the Open Demo no longer reaches the stage page.
   */
  it('links every stage to its detail page', () => {
    const wrapper = mount(ChainHistoryPage, {
      global: { stubs: { RouterLink: routerLinkStub } },
    })

    expect(wrapper.findAll('.chain-stage__link').map((link) => link.attributes('href'))).toEqual([
      '/chain-history/mercado',
      '/chain-history/transportadora/entrega',
      '/chain-history/exportador',
      '/chain-history/transportadora/carne',
      '/chain-history/frigorifico',
      '/chain-history/transportadora/gado',
      '/chain-history/produtor',
    ])
    wrapper.unmount()
  })
})

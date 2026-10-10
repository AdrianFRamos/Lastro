import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it } from 'vitest'
import type { Component } from 'vue'
import { demoHash, routes, slaughterhouse } from '../../src/demo/chainHistory'
import CarrierPage from '../../src/pages/chain/CarrierPage.vue'
import ExporterPage from '../../src/pages/chain/ExporterPage.vue'
import MarketPage from '../../src/pages/chain/MarketPage.vue'
import ProducerAnimalPage from '../../src/pages/chain/ProducerAnimalPage.vue'
import ProducerPage from '../../src/pages/chain/ProducerPage.vue'
import ProducerPropertyPage from '../../src/pages/chain/ProducerPropertyPage.vue'
import SlaughterhousePage from '../../src/pages/chain/SlaughterhousePage.vue'
import { setLocale } from '../../src/i18n'

const routerLinkStub = {
  props: ['to'],
  template: '<a :href="to"><slot /></a>',
}

function mountPage(page: Component, props: Record<string, unknown> = {}) {
  return mount(page, { props, global: { stubs: { RouterLink: routerLinkStub } } })
}

function labels(wrapper: ReturnType<typeof mountPage>): string[] {
  return wrapper
    .findAll('.chain-field__label, .chain-list__title')
    .map((label) => label.text().toLowerCase())
}

describe('chain history stage pages', () => {
  // These pages were designed in Portuguese; their copy is asserted in that language.
  beforeEach(() => setLocale('pt'))

  /**
   * ARRANGE: mount each stage page with a RouterLink stub.
   * ACTION: collect the record field and list labels.
   * ASSERT: every page shows the fields from its design.
   * FAILURE MEANS: a stage page lost information the demo is meant to present.
   */
  it.each([
    [
      'Frigorífico',
      SlaughterhousePage,
      [
        'hash de transferência',
        'hash do lote dos animais',
        'hash do animal',
        'peso',
        'perda',
        'data do abatimento',
        'data do corte',
        'peças derivadas do animal',
      ],
    ],
    [
      'Exportador',
      ExporterPage,
      [
        'hash de transferência',
        'contêiner',
        'porto de embarque',
        'porto de destino',
        'cadeia de frio',
        'documentos de exportação',
        'peças exportadas',
      ],
    ],
    [
      'Mercado',
      MarketPage,
      [
        'hash de transferência',
        'hash da peça do animal',
        'hash do animal',
        'peso da peça',
        'perdas',
      ],
    ],
    [
      'Produtor',
      ProducerPage,
      [
        'hash de transferência',
        'propriedade',
        'hash do lote dos animais',
        'animais do lote',
        'hardwares utilizados',
      ],
    ],
    [
      'Produtor · animal',
      ProducerAnimalPage,
      ['hash do animal', 'propriedade', 'histórico de vacinas', 'histórico de pesos'],
    ],
    [
      'Produtor · propriedade',
      ProducerPropertyPage,
      ['latitude', 'longitude', 'tamanho', 'animais cadastrados'],
    ],
  ])('%s shows the fields from its design', (_name, page, expected) => {
    const wrapper = mountPage(page)
    expect(labels(wrapper)).toEqual(expect.arrayContaining(expected))
    wrapper.unmount()
  })

  /**
   * ARRANGE: mount the carrier page once per transport leg.
   * ACTION: read its kicker, record fields, sender and recipient links.
   * ASSERT: each leg sits at its own stage number and hands custody between the expected
   *         neighbours: producer → slaughterhouse, slaughterhouse → exporter, exporter → market.
   * FAILURE MEANS: a transport leg would show the wrong custodians or the wrong stage number.
   */
  it.each([
    ['livestock', 'Etapa 02', routes.producer, routes.slaughterhouse, 'animais transportados'],
    ['meat', 'Etapa 04', routes.slaughterhouse, routes.exporter, 'peças transportadas'],
    ['delivery', 'Etapa 06', routes.exporter, routes.market, 'peças transportadas'],
  ])('carrier leg %s links its sender and recipient', (leg, kicker, sender, recipient, cargo) => {
    const wrapper = mountPage(CarrierPage, { leg })
    expect(wrapper.get('.stage-kicker').text()).toContain(kicker.toUpperCase())
    expect(labels(wrapper)).toEqual(
      expect.arrayContaining([
        'hash de transferência',
        'remetente',
        'destinatário',
        'peso transportado',
        'paradas no caminho',
        cargo,
      ]),
    )
    const links = wrapper.findAll('a').map((link) => link.attributes('href'))
    expect(links).toEqual(expect.arrayContaining([sender, recipient]))
    wrapper.unmount()
  })

  /**
   * ARRANGE: mount the producer and slaughterhouse pages.
   * ACTION: read the destinations of their links.
   * ASSERT: animal, property and lot references lead to the related pages.
   * FAILURE MEANS: the stage pages stopped connecting the piece back to its origin.
   */
  it('connects the records back to the animal and its property', () => {
    const producer = mountPage(ProducerPage)
    const producerLinks = producer.findAll('a').map((link) => link.attributes('href'))
    expect(producerLinks).toEqual(
      expect.arrayContaining([
        '/chain-history/produtor/propriedade',
        '/chain-history/produtor/animal',
      ]),
    )
    producer.unmount()

    const slaughter = mountPage(SlaughterhousePage)
    expect(slaughter.text()).toContain(slaughterhouse.animalHash)
    expect(slaughter.findAll('a').map((link) => link.attributes('href'))).toEqual(
      expect.arrayContaining(['/chain-history/produtor', '/chain-history/produtor/animal']),
    )
    slaughter.unmount()
  })

  /**
   * ARRANGE: none.
   * ACTION: derive demo hashes from fixed seeds.
   * ASSERT: values are stable 64-character lowercase hex and differ between seeds.
   * FAILURE MEANS: the demo would show different identifiers on every reload.
   */
  it('derives stable demo identifiers', () => {
    expect(demoHash('animal01')).toMatch(/^[0-9a-f]{64}$/)
    expect(demoHash('animal01')).toBe(demoHash('animal01'))
    expect(demoHash('animal01')).not.toBe(demoHash('animal02'))
  })
})

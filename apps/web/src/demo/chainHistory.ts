/**
 * Simulated chain history behind Open Demo: one piece of meat (a picanha) followed from the
 * producer to the market. Every page of the demo reads from here so names, dates and hashes
 * stay consistent across the timeline and the stage pages. Nothing here comes from the chain.
 * Texts that differ by language are `Localized`; names and identifiers are plain strings.
 */
import { locale, type Localized, type Text } from '../i18n'

/** Deterministic, realistic-looking 32-byte hex for demo records. Not a cryptographic hash. */
export function demoHash(seed: string): string {
  let state = 2166136261
  for (const char of seed) state = Math.imul(state ^ char.charCodeAt(0), 16777619) >>> 0
  let hex = ''
  while (hex.length < 64) {
    state ^= state << 13
    state ^= state >>> 17
    state ^= state << 5
    state >>>= 0
    hex += state.toString(16).padStart(8, '0')
  }
  return hex.slice(0, 64)
}

export interface Period {
  startedAt: string
  /** Missing while the stage still holds custody. */
  endedAt?: string
}

const DAY_MS = 86_400_000
const MONTHS = {
  pt: ['JAN', 'FEV', 'MAR', 'ABR', 'MAI', 'JUN', 'JUL', 'AGO', 'SET', 'OUT', 'NOV', 'DEZ'],
  en: ['JAN', 'FEB', 'MAR', 'APR', 'MAY', 'JUN', 'JUL', 'AUG', 'SEP', 'OCT', 'NOV', 'DEC'],
}

function dateParts(iso: string) {
  const [year = '', month = '', day = ''] = iso.split('-')
  return { year, month, day }
}

/** Day/month/year in both languages (Brazilian and British order). */
export function formatDate(iso: string): string {
  const { year, month, day } = dateParts(iso)
  return `${day}/${month}/${year}`
}

export function dayAndMonth(iso: string): string {
  const { month, day } = dateParts(iso)
  return `${day} ${MONTHS[locale.value][Number(month) - 1] ?? ''}`
}

export function yearOf(iso: string): string {
  return dateParts(iso).year
}

export function formatDuration(period: Period): string {
  const pt = locale.value === 'pt'
  if (!period.endedAt) return pt ? 'Em andamento' : 'In progress'
  const days = Math.round((Date.parse(period.endedAt) - Date.parse(period.startedAt)) / DAY_MS)
  if (days === 1) return pt ? '1 dia' : '1 day'
  return pt ? `${days} dias` : `${days} days`
}

export const routes = {
  history: '/chain-history',
  producer: '/chain-history/produtor',
  producerAnimal: '/chain-history/produtor/animal',
  producerProperty: '/chain-history/produtor/propriedade',
  carrierLivestock: '/chain-history/transportadora/gado',
  slaughterhouse: '/chain-history/frigorifico',
  carrierMeat: '/chain-history/transportadora/carne',
  exporter: '/chain-history/exportador',
  carrierDelivery: '/chain-history/transportadora/entrega',
  market: '/chain-history/mercado',
} as const

export interface LinkedItem {
  label: Text
  hash?: string
  to?: string
  note?: Text
}

const THIS_PIECE: Localized = { pt: 'Esta peça', en: 'This piece' }

const animalHash = demoHash('animal01')
const lotHash = demoHash('lote-2026-1012')
const pieceHash = demoHash('peca-picanha-01')

const cuts = {
  picanha: { label: 'Picanha', hash: pieceHash, note: THIS_PIECE },
  alcatra: { label: { pt: 'Alcatra', en: 'Rump' }, hash: demoHash('peca-alcatra-01') },
  contrafile: {
    label: { pt: 'Contrafilé', en: 'Striploin' },
    hash: demoHash('peca-contrafile-01'),
  },
  maminha: { label: { pt: 'Maminha', en: 'Tri-tip' }, hash: demoHash('peca-maminha-01') },
  fraldinha: { label: { pt: 'Fraldinha', en: 'Flank' }, hash: demoHash('peca-fraldinha-01') },
} satisfies Record<string, LinkedItem>

export const product = {
  name: { pt: 'Picanha', en: 'Picanha (rump cap)' } satisfies Localized,
  lot: 'LT-2026-1012',
}

export const producer = {
  name: 'Produtor da Silva',
  period: { startedAt: '2024-09-15', endedAt: '2026-10-11' } satisfies Period,
  transferHash: demoHash('transfer-produtor-transportadora'),
  lotHash,
  property: {
    name: { pt: 'Propriedade 1', en: 'Property 1' } satisfies Localized,
    farm: 'Fazenda Boa Vista',
    latitude: '-20.4697',
    longitude: '-54.6201',
    size: '420 ha',
  },
  animals: [
    {
      label: 'Animal 01',
      hash: animalHash,
      to: routes.producerAnimal,
      note: { pt: 'Origem desta peça', en: 'Origin of this piece' },
    },
    { label: 'Animal 02', hash: demoHash('animal02') },
    { label: 'Animal 03', hash: demoHash('animal03') },
    { label: 'Animal 04', hash: demoHash('animal04') },
  ] satisfies LinkedItem[],
  hardware: [
    {
      label: 'Station ESP32-C5',
      note: { pt: 'Assina cada leitura com chave P-256', en: 'Signs every read with a P-256 key' },
    },
    {
      label: { pt: 'Leitor RFID FDX-B', en: 'FDX-B RFID reader' },
      note: { pt: 'Lê o brinco eletrônico do animal', en: "Reads the animal's electronic ear tag" },
    },
    {
      label: { pt: 'Balança de tronco', en: 'Cattle crush scale' },
      note: { pt: 'Pesagens periódicas do lote', en: 'Periodic weighing of the lot' },
    },
  ] satisfies LinkedItem[],
}

export const animal = {
  label: 'Animal 01',
  hash: animalHash,
  rfidTag: 'BR 076 0000 1234',
  vaccines: [
    {
      date: '2024-11-10',
      name: { pt: 'Febre aftosa', en: 'Foot-and-mouth disease' },
      note: { pt: '1ª dose', en: '1st dose' },
    },
    { date: '2025-05-12', name: { pt: 'Brucelose (B19)', en: 'Brucellosis (B19)' } },
    {
      date: '2025-05-12',
      name: { pt: 'Febre aftosa', en: 'Foot-and-mouth disease' },
      note: { pt: 'reforço', en: 'booster' },
    },
    { date: '2025-11-08', name: { pt: 'Raiva', en: 'Rabies' } },
    { date: '2026-05-14', name: { pt: 'Clostridioses', en: 'Clostridial diseases' } },
  ] satisfies { date: string; name: Localized; note?: Localized }[],
  weights: [
    { date: '2024-09-15', kg: 182 },
    { date: '2025-03-15', kg: 268 },
    { date: '2025-09-15', kg: 356 },
    { date: '2026-03-15', kg: 452 },
    { date: '2026-10-10', kg: 538 },
  ],
}

export const slaughterhouse = {
  name: { pt: 'Frigorífico', en: 'Slaughterhouse' } satisfies Localized,
  period: { startedAt: '2026-10-12', endedAt: '2026-10-20' } satisfies Period,
  transferHash: demoHash('transfer-frigorifico-transportadora-carne'),
  lotHash,
  animalHash,
  weight: { pt: '538 kg (vivo)', en: '538 kg (live)' } satisfies Localized,
  loss: {
    pt: '2,8% (resfriamento da carcaça)',
    en: '2.8% (carcass chilling)',
  } satisfies Localized,
  slaughteredAt: '2026-10-13',
  cutAt: '2026-10-18',
  pieces: [
    cuts.picanha,
    cuts.alcatra,
    cuts.contrafile,
    cuts.maminha,
    cuts.fraldinha,
  ] satisfies LinkedItem[],
}

const MARKET_NAME: Localized = { pt: 'Mercado', en: 'Market' }
const EXPORTER_NAME: Localized = { pt: 'Exportador', en: 'Exporter' }

export interface CarrierLeg {
  /** 1-based position of this stage in the chain, from the producer. */
  step: number
  name: Localized
  description: Localized
  period: Period
  transferHash: string
  sender: { label: Text; to: string }
  recipient: { label: Text; to: string }
  transportedWeight: Localized
  stops: LinkedItem[]
  cargoTitle: Localized
  cargo: LinkedItem[]
  to: string
}

/** The three transport legs between custodians, keyed by the route that opens them. */
export const carriers = {
  livestock: {
    step: 2,
    name: { pt: 'Transportadora de gado', en: 'Livestock carrier' },
    description: {
      pt: 'Transporte dos animais vivos da fazenda até o frigorífico.',
      en: 'Live animal transport from the farm to the slaughterhouse.',
    },
    period: { startedAt: '2026-10-11', endedAt: '2026-10-12' },
    transferHash: demoHash('transfer-transportadora-gado-frigorifico'),
    sender: { label: producer.name, to: routes.producer },
    recipient: { label: slaughterhouse.name, to: routes.slaughterhouse },
    transportedWeight: {
      pt: '2.152 kg (4 animais vivos)',
      en: '2,152 kg (4 live animals)',
    },
    stops: [
      {
        label: { pt: 'Saída da fazenda', en: 'Left the farm' },
        note: '11/10/2026 · 16:30',
      },
      {
        label: { pt: 'Posto de fiscalização sanitária', en: 'Animal health checkpoint' },
        note: {
          pt: '11/10/2026 · 21:05 · conferência da GTA',
          en: '11/10/2026 · 21:05 · animal transit permit check',
        },
      },
      {
        label: { pt: 'Chegada ao frigorífico', en: 'Arrived at the slaughterhouse' },
        note: '12/10/2026 · 05:20',
      },
    ],
    cargoTitle: { pt: 'Animais transportados', en: 'Transported animals' },
    cargo: producer.animals,
    to: routes.carrierLivestock,
  },
  meat: {
    step: 4,
    name: { pt: 'Transportadora de carne', en: 'Meat carrier' },
    description: {
      pt: 'Transporte refrigerado do frigorífico até o exportador.',
      en: 'Refrigerated transport from the slaughterhouse to the exporter.',
    },
    period: { startedAt: '2026-10-20', endedAt: '2026-10-21' },
    transferHash: demoHash('transfer-transportadora-carne-exportador'),
    sender: { label: slaughterhouse.name, to: routes.slaughterhouse },
    recipient: { label: EXPORTER_NAME, to: routes.exporter },
    transportedWeight: {
      pt: '1.240 kg (carga refrigerada a 0–4 °C)',
      en: '1,240 kg (refrigerated load at 0–4 °C)',
    },
    stops: [
      {
        label: { pt: 'Saída do frigorífico', en: 'Left the slaughterhouse' },
        note: '20/10/2026 · 22:10',
      },
      {
        label: { pt: 'Centro de distribuição', en: 'Distribution centre' },
        note: {
          pt: '21/10/2026 · 03:40 · conferência de temperatura',
          en: '21/10/2026 · 03:40 · temperature check',
        },
      },
      {
        label: {
          pt: 'Chegada ao terminal do exportador',
          en: "Arrived at the exporter's terminal",
        },
        note: '21/10/2026 · 09:15',
      },
    ],
    cargoTitle: { pt: 'Peças transportadas', en: 'Transported pieces' },
    cargo: [cuts.picanha, cuts.alcatra, cuts.contrafile],
    to: routes.carrierMeat,
  },
  delivery: {
    step: 6,
    name: { pt: 'Transportadora no destino', en: 'Destination carrier' },
    description: {
      pt: 'Entrega refrigerada do porto de destino até o mercado.',
      en: 'Refrigerated delivery from the destination port to the market.',
    },
    period: { startedAt: '2026-11-12', endedAt: '2026-11-13' },
    transferHash: demoHash('transfer-transportadora-destino-mercado'),
    sender: { label: EXPORTER_NAME, to: routes.exporter },
    recipient: { label: MARKET_NAME, to: routes.market },
    transportedWeight: {
      pt: '320 kg (carga refrigerada a 0–4 °C)',
      en: '320 kg (refrigerated load at 0–4 °C)',
    },
    stops: [
      {
        label: { pt: 'Liberação no porto de Roterdã', en: 'Released at the Port of Rotterdam' },
        note: '12/11/2026 · 14:00',
      },
      {
        label: { pt: 'Centro de distribuição', en: 'Distribution centre' },
        note: {
          pt: '12/11/2026 · 23:30 · conferência de temperatura',
          en: '12/11/2026 · 23:30 · temperature check',
        },
      },
      {
        label: { pt: 'Chegada ao mercado', en: 'Arrived at the market' },
        note: '13/11/2026 · 06:45',
      },
    ],
    cargoTitle: { pt: 'Peças transportadas', en: 'Transported pieces' },
    cargo: [cuts.picanha, cuts.contrafile],
    to: routes.carrierDelivery,
  },
} satisfies Record<string, CarrierLeg>

export type CarrierLegId = keyof typeof carriers

export const exporter = {
  name: EXPORTER_NAME,
  period: { startedAt: '2026-10-21', endedAt: '2026-11-12' } satisfies Period,
  transferHash: demoHash('transfer-exportador-transportadora-destino'),
  container: 'MSKU 482915-3',
  originPort: { pt: 'Porto de Santos (BR)', en: 'Port of Santos (BR)' } satisfies Localized,
  destinationPort: {
    pt: 'Porto de Roterdã (NL)',
    en: 'Port of Rotterdam (NL)',
  } satisfies Localized,
  temperature: {
    pt: 'Refrigerado, −1 a 2 °C durante toda a viagem',
    en: 'Chilled, −1 to 2 °C for the whole voyage',
  } satisfies Localized,
  documents: [
    {
      label: {
        pt: 'Certificado Sanitário Internacional',
        en: 'International Health Certificate',
      },
      hash: demoHash('csi-2026-1012'),
    },
    {
      label: { pt: 'Declaração de exportação', en: 'Export declaration' },
      hash: demoHash('due-2026-1012'),
    },
    {
      label: { pt: 'Conhecimento de embarque', en: 'Bill of lading' },
      hash: demoHash('bl-2026-1012'),
    },
  ] satisfies LinkedItem[],
  pieces: [cuts.picanha, cuts.contrafile] satisfies LinkedItem[],
}

export const market = {
  name: MARKET_NAME,
  period: { startedAt: '2026-11-13' } satisfies Period,
  transferHash: demoHash('transfer-mercado-recebimento'),
  pieceHash,
  animalHash,
  pieceWeight: { pt: '1,28 kg', en: '1.28 kg' } satisfies Localized,
  loss: {
    pt: '0,04 kg (aparas na exposição)',
    en: '0.04 kg (trimming on display)',
  } satisfies Localized,
}

export type StageIcon = 'store' | 'truck' | 'ship' | 'factory' | 'farm'

export interface ChainStage {
  id: string
  name: Text
  description: Localized
  icon: StageIcon
  period: Period
  to: string
}

function carrierStage(id: CarrierLegId): ChainStage {
  const leg = carriers[id]
  return {
    id: `carrier-${id}`,
    name: leg.name,
    description: leg.description,
    icon: 'truck',
    period: leg.period,
    to: leg.to,
  }
}

/**
 * Timeline order: newest custody first. The journey runs producer → carrier → slaughterhouse →
 * carrier → exporter → carrier → market.
 */
export const stages: ChainStage[] = [
  {
    id: 'market',
    name: market.name,
    description: {
      pt: 'Recebimento, conferência da etiqueta e exposição para venda no destino.',
      en: 'Receiving, label check and display for sale at the destination.',
    },
    icon: 'store',
    period: market.period,
    to: routes.market,
  },
  carrierStage('delivery'),
  {
    id: 'exporter',
    name: exporter.name,
    description: {
      pt: 'Certificação sanitária, conteinerização refrigerada e embarque para o exterior.',
      en: 'Health certification, refrigerated containerisation and shipping abroad.',
    },
    icon: 'ship',
    period: exporter.period,
    to: routes.exporter,
  },
  carrierStage('meat'),
  {
    id: 'slaughterhouse',
    name: slaughterhouse.name,
    description: {
      pt: 'Abate, desossa, maturação e embalagem do corte.',
      en: 'Slaughter, deboning, ageing and packing of the cut.',
    },
    icon: 'factory',
    period: slaughterhouse.period,
    to: routes.slaughterhouse,
  },
  carrierStage('livestock'),
  {
    id: 'producer',
    name: producer.name,
    description: {
      pt: 'Criação do animal e identificação pelo brinco RFID lido pela Station.',
      en: 'Raising the animal and identifying it by the RFID ear tag read by the Station.',
    },
    icon: 'farm',
    period: producer.period,
    to: routes.producer,
  },
]

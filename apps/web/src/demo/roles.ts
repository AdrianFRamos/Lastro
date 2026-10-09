/**
 * Access profiles of the demo workspace behind ENTRAR. Each chain participant manages its own
 * records (create, read, update, delete); a common user only reads. Every resource is described
 * by its fields so one generic screen can list, create, edit and delete any of them. The seed
 * rows follow the same piece of meat as the chain history pages. Nothing here reaches the API or
 * the chain: the workspace is simulated and kept in this browser only.
 */
import type { Localized } from '../i18n'

export type RoleId = 'producer' | 'carrier' | 'slaughterhouse' | 'exporter' | 'merchant' | 'viewer'
export type ParticipantId = Exclude<RoleId, 'viewer'>

export type FieldType = 'text' | 'number' | 'date' | 'select' | 'ref'

export interface FieldOption {
  value: string
  label: Localized
}

export interface FieldDef {
  key: string
  label: Localized
  type: FieldType
  required?: boolean
  /** Choices of a `select` field. */
  options?: FieldOption[]
  /** Resource whose records a `ref` field points to (same participant). */
  ref?: string
  /** Shown next to numbers, e.g. kg or ha; money units are shown before the value. */
  unit?: string
  placeholder?: Localized
}

export interface ResourceDef {
  id: string
  role: ParticipantId
  label: Localized
  /** One record, used in "New …" and form titles. */
  singular: Localized
  description: Localized
  fields: FieldDef[]
}

export interface RoleDef {
  id: RoleId
  label: Localized
  summary: Localized
  access: 'crud' | 'read'
}

/** A stored row: field values are kept as typed in the form (strings). */
export type DemoRecord = { id: string } & Record<string, string>

const both = (pt: string, en: string): Localized => ({ pt, en })

const option = (value: string, pt: string, en: string): FieldOption => ({
  value,
  label: { pt, en },
})

export const roles: RoleDef[] = [
  {
    id: 'producer',
    label: both('Produtor', 'Producer'),
    summary: both(
      'Cadastra propriedades, lotes e animais, antenas de leitura, vacinas e pesagens.',
      'Registers properties, lots and animals, reader antennas, vaccines and weighings.',
    ),
    access: 'crud',
  },
  {
    id: 'carrier',
    label: both('Transportador', 'Carrier'),
    summary: both(
      'Registra trajetos, paradas e as características do transporte.',
      'Records routes, stops and the characteristics of the transport.',
    ),
    access: 'crud',
  },
  {
    id: 'slaughterhouse',
    label: both('Frigorífico', 'Slaughterhouse'),
    summary: both(
      'Registra as peças produzidas e as perdas do processo.',
      'Records the pieces produced and the losses of the process.',
    ),
    access: 'crud',
  },
  {
    id: 'exporter',
    label: both('Exportador', 'Exporter'),
    summary: both(
      'Registra trajetos de exportação, paradas e as características do transporte.',
      'Records export routes, stops and the characteristics of the transport.',
    ),
    access: 'crud',
  },
  {
    id: 'merchant',
    label: both('Comerciante', 'Merchant'),
    summary: both(
      'Registra as peças recebidas, as vendas e as perdas.',
      'Records the pieces received, the sales and the losses.',
    ),
    access: 'crud',
  },
  {
    id: 'viewer',
    label: both('Usuário comum', 'Common user'),
    summary: both(
      'Consulta os registros de toda a cadeia, sem alterar nada.',
      'Looks up the records of the whole chain without changing anything.',
    ),
    access: 'read',
  },
]

export const participants = roles.filter(
  (role): role is RoleDef & { id: ParticipantId } => role.id !== 'viewer',
)

export function findRole(id: unknown): RoleDef | undefined {
  return roles.find((role) => role.id === id)
}

const ANIMAL_OR_LOT = both('Animal ou lote', 'Animal or lot')
const WEIGHT_KG = both('Peso', 'Weight')

function routeFields(origin: Localized, destination: Localized): FieldDef[] {
  return [
    { key: 'code', label: both('Código', 'Code'), type: 'text', required: true },
    { key: 'origin', label: origin, type: 'text', required: true },
    { key: 'destination', label: destination, type: 'text', required: true },
    { key: 'departure', label: both('Saída', 'Departure'), type: 'date', required: true },
    { key: 'arrival', label: both('Chegada prevista', 'Expected arrival'), type: 'date' },
  ]
}

function stopFields(routeResource: string, reasons: FieldOption[]): FieldDef[] {
  return [
    {
      key: 'route',
      label: both('Trajeto', 'Route'),
      type: 'ref',
      ref: routeResource,
      required: true,
    },
    { key: 'place', label: both('Local', 'Place'), type: 'text', required: true },
    { key: 'arrival', label: both('Chegada', 'Arrival'), type: 'date' },
    { key: 'departure', label: both('Saída', 'Departure'), type: 'date' },
    { key: 'reason', label: both('Motivo', 'Reason'), type: 'select', options: reasons },
  ]
}

export const resources: ResourceDef[] = [
  // Producer
  {
    id: 'producer.properties',
    role: 'producer',
    label: both('Propriedades', 'Properties'),
    singular: both('propriedade', 'property'),
    description: both(
      'Onde os animais são criados: localização e área.',
      'Where the animals are raised: location and area.',
    ),
    fields: [
      { key: 'name', label: both('Nome', 'Name'), type: 'text', required: true },
      { key: 'city', label: both('Município/UF', 'City/State'), type: 'text', required: true },
      { key: 'latitude', label: both('Latitude', 'Latitude'), type: 'number' },
      { key: 'longitude', label: both('Longitude', 'Longitude'), type: 'number' },
      { key: 'area', label: both('Área', 'Area'), type: 'number', unit: 'ha' },
    ],
  },
  {
    id: 'producer.lots',
    role: 'producer',
    label: both('Lotes de animais', 'Animal lots'),
    singular: both('lote', 'lot'),
    description: both(
      'Grupos de animais criados e movimentados juntos.',
      'Groups of animals raised and moved together.',
    ),
    fields: [
      { key: 'code', label: both('Código do lote', 'Lot code'), type: 'text', required: true },
      {
        key: 'property',
        label: both('Propriedade', 'Property'),
        type: 'ref',
        ref: 'producer.properties',
        required: true,
      },
      { key: 'quantity', label: both('Quantidade', 'Quantity'), type: 'number', required: true },
      { key: 'breed', label: both('Raça', 'Breed'), type: 'text' },
      { key: 'entryDate', label: both('Entrada', 'Entry date'), type: 'date' },
    ],
  },
  {
    id: 'producer.animals',
    role: 'producer',
    label: both('Animais avulsos', 'Single animals'),
    singular: both('animal', 'animal'),
    description: both(
      'Animais identificados individualmente, fora de um lote.',
      'Animals identified one by one, outside a lot.',
    ),
    fields: [
      { key: 'tag', label: both('Brinco RFID', 'RFID ear tag'), type: 'text', required: true },
      {
        key: 'property',
        label: both('Propriedade', 'Property'),
        type: 'ref',
        ref: 'producer.properties',
        required: true,
      },
      { key: 'breed', label: both('Raça', 'Breed'), type: 'text' },
      {
        key: 'sex',
        label: both('Sexo', 'Sex'),
        type: 'select',
        options: [option('M', 'Macho', 'Male'), option('F', 'Fêmea', 'Female')],
      },
      { key: 'birthDate', label: both('Nascimento', 'Birth date'), type: 'date' },
    ],
  },
  {
    id: 'producer.antennas',
    role: 'producer',
    label: both('Antenas', 'Antennas'),
    singular: both('antena', 'antenna'),
    description: both(
      'Leitores RFID que identificam os animais na propriedade.',
      'RFID readers that identify the animals on the property.',
    ),
    fields: [
      { key: 'code', label: both('Identificação', 'Identifier'), type: 'text', required: true },
      {
        key: 'property',
        label: both('Propriedade', 'Property'),
        type: 'ref',
        ref: 'producer.properties',
        required: true,
      },
      { key: 'location', label: both('Local de instalação', 'Installed at'), type: 'text' },
      {
        key: 'status',
        label: both('Situação', 'Status'),
        type: 'select',
        required: true,
        options: [
          option('active', 'Ativa', 'Active'),
          option('maintenance', 'Em manutenção', 'Under maintenance'),
          option('inactive', 'Inativa', 'Inactive'),
        ],
      },
      { key: 'installedAt', label: both('Instalação', 'Installation date'), type: 'date' },
    ],
  },
  {
    id: 'producer.vaccines',
    role: 'producer',
    label: both('Vacinas', 'Vaccines'),
    singular: both('vacina', 'vaccine'),
    description: both(
      'Vacinas aplicadas em animais ou lotes.',
      'Vaccines given to animals or lots.',
    ),
    fields: [
      { key: 'target', label: ANIMAL_OR_LOT, type: 'text', required: true },
      { key: 'vaccine', label: both('Vacina', 'Vaccine'), type: 'text', required: true },
      { key: 'dose', label: both('Dose', 'Dose'), type: 'text' },
      { key: 'date', label: both('Data', 'Date'), type: 'date', required: true },
      { key: 'vet', label: both('Responsável', 'Responsible'), type: 'text' },
    ],
  },
  {
    id: 'producer.weights',
    role: 'producer',
    label: both('Peso', 'Weight'),
    singular: both('pesagem', 'weighing'),
    description: both(
      'Pesagens de animais ou lotes ao longo da criação.',
      'Weighings of animals or lots while they are raised.',
    ),
    fields: [
      { key: 'target', label: ANIMAL_OR_LOT, type: 'text', required: true },
      { key: 'weight', label: WEIGHT_KG, type: 'number', unit: 'kg', required: true },
      { key: 'date', label: both('Data', 'Date'), type: 'date', required: true },
      {
        key: 'antenna',
        label: both('Lido pela antena', 'Read by antenna'),
        type: 'ref',
        ref: 'producer.antennas',
      },
    ],
  },
  // Carrier
  {
    id: 'carrier.routes',
    role: 'carrier',
    label: both('Trajetos', 'Routes'),
    singular: both('trajeto', 'route'),
    description: both(
      'Viagens entre a origem e o destino da carga.',
      'Trips between the origin and the destination of the load.',
    ),
    fields: routeFields(both('Origem', 'Origin'), both('Destino', 'Destination')),
  },
  {
    id: 'carrier.stops',
    role: 'carrier',
    label: both('Paradas', 'Stops'),
    singular: both('parada', 'stop'),
    description: both('Paradas feitas durante um trajeto.', 'Stops made along a route.'),
    fields: stopFields('carrier.routes', [
      option('fuel', 'Abastecimento', 'Refuelling'),
      option('rest', 'Descanso', 'Rest'),
      option('inspection', 'Fiscalização', 'Inspection'),
      option('delivery', 'Entrega', 'Delivery'),
      option('other', 'Outro', 'Other'),
    ]),
  },
  {
    id: 'carrier.transport',
    role: 'carrier',
    label: both('Características do transporte', 'Transport characteristics'),
    singular: both('veículo', 'vehicle'),
    description: both(
      'Veículos usados e as condições da carga.',
      'Vehicles used and the conditions of the load.',
    ),
    fields: [
      { key: 'plate', label: both('Placa', 'Plate'), type: 'text', required: true },
      {
        key: 'type',
        label: both('Tipo', 'Type'),
        type: 'select',
        required: true,
        options: [
          option('refrigerated', 'Caminhão refrigerado', 'Refrigerated truck'),
          option('isothermal', 'Baú isotérmico', 'Insulated box'),
          option('livestock', 'Boiadeiro', 'Livestock truck'),
        ],
      },
      { key: 'temperature', label: both('Temperatura', 'Temperature'), type: 'number', unit: '°C' },
      { key: 'capacity', label: both('Capacidade', 'Capacity'), type: 'number', unit: 'kg' },
      { key: 'driver', label: both('Motorista', 'Driver'), type: 'text' },
    ],
  },
  // Slaughterhouse
  {
    id: 'slaughterhouse.pieces',
    role: 'slaughterhouse',
    label: both('Peças', 'Pieces'),
    singular: both('peça', 'piece'),
    description: both(
      'Cortes produzidos a partir dos animais recebidos.',
      'Cuts produced from the animals received.',
    ),
    fields: [
      { key: 'code', label: both('Código', 'Code'), type: 'text', required: true },
      { key: 'cut', label: both('Corte', 'Cut'), type: 'text', required: true },
      { key: 'lot', label: both('Lote de origem', 'Source lot'), type: 'text' },
      { key: 'weight', label: WEIGHT_KG, type: 'number', unit: 'kg', required: true },
      { key: 'packedAt', label: both('Embalagem', 'Packed on'), type: 'date' },
    ],
  },
  {
    id: 'slaughterhouse.losses',
    role: 'slaughterhouse',
    label: both('Perda', 'Loss'),
    singular: both('perda', 'loss'),
    description: both(
      'Quanto se perdeu no processo e por quê.',
      'How much was lost in the process and why.',
    ),
    fields: [
      {
        key: 'piece',
        label: both('Peça', 'Piece'),
        type: 'ref',
        ref: 'slaughterhouse.pieces',
      },
      {
        key: 'weight',
        label: both('Peso perdido', 'Weight lost'),
        type: 'number',
        unit: 'kg',
        required: true,
      },
      {
        key: 'reason',
        label: both('Motivo', 'Reason'),
        type: 'select',
        required: true,
        options: [
          option('trimming', 'Desossa e aparas', 'Deboning and trimming'),
          option('chilling', 'Quebra de resfriamento', 'Chilling shrink'),
          option('condemned', 'Condenação sanitária', 'Sanitary condemnation'),
          option('other', 'Outro', 'Other'),
        ],
      },
      { key: 'date', label: both('Data', 'Date'), type: 'date', required: true },
    ],
  },
  // Exporter
  {
    id: 'exporter.routes',
    role: 'exporter',
    label: both('Trajetos', 'Routes'),
    singular: both('trajeto', 'route'),
    description: both(
      'Envios do Brasil até o país de destino.',
      'Shipments from Brazil to the destination country.',
    ),
    fields: routeFields(both('Origem', 'Origin'), both('Destino', 'Destination')),
  },
  {
    id: 'exporter.stops',
    role: 'exporter',
    label: both('Paradas', 'Stops'),
    singular: both('parada', 'stop'),
    description: both(
      'Portos, inspeções e transbordos no caminho.',
      'Ports, inspections and transshipments along the way.',
    ),
    fields: stopFields('exporter.routes', [
      option('transshipment', 'Transbordo', 'Transshipment'),
      option('sanitary', 'Inspeção sanitária', 'Sanitary inspection'),
      option('customs', 'Alfândega', 'Customs'),
      option('fuel', 'Abastecimento', 'Refuelling'),
      option('other', 'Outro', 'Other'),
    ]),
  },
  {
    id: 'exporter.transport',
    role: 'exporter',
    label: both('Características do transporte', 'Transport characteristics'),
    singular: both('contêiner', 'container'),
    description: both(
      'Contêineres, modal e condições da carga exportada.',
      'Containers, mode and conditions of the exported load.',
    ),
    fields: [
      { key: 'container', label: both('Contêiner', 'Container'), type: 'text', required: true },
      {
        key: 'mode',
        label: both('Modal', 'Mode'),
        type: 'select',
        required: true,
        options: [
          option('sea', 'Marítimo', 'Sea'),
          option('air', 'Aéreo', 'Air'),
          option('road', 'Rodoviário', 'Road'),
        ],
      },
      { key: 'carrierName', label: both('Navio ou voo', 'Vessel or flight'), type: 'text' },
      { key: 'temperature', label: both('Temperatura', 'Temperature'), type: 'number', unit: '°C' },
      { key: 'capacity', label: both('Capacidade', 'Capacity'), type: 'number', unit: 'kg' },
    ],
  },
  // Merchant
  {
    id: 'merchant.pieces',
    role: 'merchant',
    label: both('Peças', 'Pieces'),
    singular: both('peça', 'piece'),
    description: both('Peças recebidas para venda.', 'Pieces received for sale.'),
    fields: [
      { key: 'code', label: both('Código', 'Code'), type: 'text', required: true },
      { key: 'cut', label: both('Corte', 'Cut'), type: 'text', required: true },
      { key: 'supplier', label: both('Fornecedor', 'Supplier'), type: 'text' },
      { key: 'weight', label: WEIGHT_KG, type: 'number', unit: 'kg', required: true },
      { key: 'receivedAt', label: both('Recebimento', 'Received on'), type: 'date' },
      { key: 'price', label: both('Preço', 'Price'), type: 'number', unit: 'R$/kg' },
    ],
  },
  {
    id: 'merchant.sales',
    role: 'merchant',
    label: both('Venda', 'Sales'),
    singular: both('venda', 'sale'),
    description: both(
      'Vendas feitas a partir das peças recebidas.',
      'Sales made from the pieces received.',
    ),
    fields: [
      {
        key: 'piece',
        label: both('Peça', 'Piece'),
        type: 'ref',
        ref: 'merchant.pieces',
        required: true,
      },
      { key: 'date', label: both('Data', 'Date'), type: 'date', required: true },
      {
        key: 'weight',
        label: both('Peso vendido', 'Weight sold'),
        type: 'number',
        unit: 'kg',
        required: true,
      },
      { key: 'total', label: both('Valor', 'Amount'), type: 'number', unit: 'R$', required: true },
      { key: 'customer', label: both('Cliente', 'Customer'), type: 'text' },
    ],
  },
  {
    id: 'merchant.losses',
    role: 'merchant',
    label: both('Perda', 'Loss'),
    singular: both('perda', 'loss'),
    description: both(
      'Peças descartadas ou avariadas no ponto de venda.',
      'Pieces discarded or damaged at the point of sale.',
    ),
    fields: [
      {
        key: 'piece',
        label: both('Peça', 'Piece'),
        type: 'ref',
        ref: 'merchant.pieces',
      },
      {
        key: 'weight',
        label: both('Peso perdido', 'Weight lost'),
        type: 'number',
        unit: 'kg',
        required: true,
      },
      {
        key: 'reason',
        label: both('Motivo', 'Reason'),
        type: 'select',
        required: true,
        options: [
          option('expired', 'Validade vencida', 'Past its date'),
          option('damaged', 'Avaria', 'Damaged'),
          option('cold-chain', 'Quebra da cadeia de frio', 'Cold chain break'),
          option('other', 'Outro', 'Other'),
        ],
      },
      { key: 'date', label: both('Data', 'Date'), type: 'date', required: true },
    ],
  },
]

export function findResource(id: string): ResourceDef | undefined {
  return resources.find((resource) => resource.id === id)
}

export function resourcesOf(role: ParticipantId): ResourceDef[] {
  return resources.filter((resource) => resource.role === role)
}

/** Example rows, consistent with the chain history of the picanha from lot LT-2026-1012. */
export const seedRecords: Record<string, DemoRecord[]> = {
  'producer.properties': [
    {
      id: 'prop-1',
      name: 'Propriedade 1 · Fazenda Boa Vista',
      city: 'Campo Grande/MS',
      latitude: '-20.4697',
      longitude: '-54.6201',
      area: '420',
    },
    {
      id: 'prop-2',
      name: 'Propriedade 2 · Sítio Esperança',
      city: 'Terenos/MS',
      latitude: '-20.4421',
      longitude: '-54.8603',
      area: '180',
    },
  ],
  'producer.lots': [
    {
      id: 'lot-1',
      code: 'LT-2026-1012',
      property: 'prop-1',
      quantity: '4',
      breed: 'Nelore',
      entryDate: '2024-09-15',
    },
  ],
  'producer.animals': [
    {
      id: 'animal-1',
      tag: 'BR-076-000001234',
      property: 'prop-1',
      breed: 'Nelore',
      sex: 'M',
      birthDate: '2024-03-02',
    },
  ],
  'producer.antennas': [
    {
      id: 'antenna-1',
      code: 'ANT-01',
      property: 'prop-1',
      location: 'Curral de manejo',
      status: 'active',
      installedAt: '2024-08-20',
    },
    {
      id: 'antenna-2',
      code: 'ANT-02',
      property: 'prop-1',
      location: 'Balança',
      status: 'active',
      installedAt: '2024-08-20',
    },
  ],
  'producer.vaccines': [
    {
      id: 'vaccine-1',
      target: 'LT-2026-1012',
      vaccine: 'Febre aftosa',
      dose: '2ª dose',
      date: '2025-05-10',
      vet: 'Dra. Ana Souza',
    },
  ],
  'producer.weights': [
    {
      id: 'weight-1',
      target: 'BR-076-000001234',
      weight: '512',
      date: '2026-09-30',
      antenna: 'antenna-2',
    },
  ],
  'carrier.routes': [
    {
      id: 'croute-1',
      code: 'TR-2026-0458',
      origin: 'Frigorífico Pantanal · Campo Grande/MS',
      destination: 'Mercado Central · São Paulo/SP',
      departure: '2026-10-20',
      arrival: '2026-10-21',
    },
  ],
  'carrier.stops': [
    {
      id: 'cstop-1',
      route: 'croute-1',
      place: 'Posto Rodoanel · Presidente Prudente/SP',
      arrival: '2026-10-20',
      departure: '2026-10-20',
      reason: 'fuel',
    },
  ],
  'carrier.transport': [
    {
      id: 'vehicle-1',
      plate: 'QAB-4F21',
      type: 'refrigerated',
      temperature: '2',
      capacity: '12000',
      driver: 'Carlos Lima',
    },
  ],
  'slaughterhouse.pieces': [
    {
      id: 'piece-1',
      code: 'PC-0001',
      cut: 'Picanha',
      lot: 'LT-2026-1012',
      weight: '1.4',
      packedAt: '2026-10-19',
    },
    {
      id: 'piece-2',
      code: 'PC-0002',
      cut: 'Alcatra',
      lot: 'LT-2026-1012',
      weight: '3.2',
      packedAt: '2026-10-19',
    },
  ],
  'slaughterhouse.losses': [
    {
      id: 'sloss-1',
      piece: 'piece-2',
      weight: '0.3',
      reason: 'trimming',
      date: '2026-10-19',
    },
  ],
  'exporter.routes': [
    {
      id: 'eroute-1',
      code: 'EX-2026-0091',
      origin: 'Porto de Santos/SP',
      destination: 'Porto de Roterdã · Países Baixos',
      departure: '2026-11-03',
      arrival: '2026-11-21',
    },
  ],
  'exporter.stops': [
    {
      id: 'estop-1',
      route: 'eroute-1',
      place: 'Porto de Santos/SP',
      arrival: '2026-11-02',
      departure: '2026-11-03',
      reason: 'customs',
    },
  ],
  'exporter.transport': [
    {
      id: 'container-1',
      container: 'MSKU 123456-7',
      mode: 'sea',
      carrierName: 'MV Atlântico Sul',
      temperature: '-18',
      capacity: '26000',
    },
  ],
  'merchant.pieces': [
    {
      id: 'mpiece-1',
      code: 'PC-0001',
      cut: 'Picanha',
      supplier: 'Frigorífico Pantanal',
      weight: '1.4',
      receivedAt: '2026-10-21',
      price: '89.90',
    },
  ],
  'merchant.sales': [
    {
      id: 'sale-1',
      piece: 'mpiece-1',
      date: '2026-10-22',
      weight: '1.4',
      total: '125.86',
      customer: 'Consumidor final',
    },
  ],
  'merchant.losses': [],
}

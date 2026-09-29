<template>
  <main class="page page--workbench operations-page">
    <header class="app-header">
      <RouterLink class="app-header__brand" to="/" aria-label="Lastro home">
        <img src="/logo.svg" alt="" width="36" height="36" />
        <span><strong>LASTRO</strong><span class="app-header__context">Operational workspace</span></span>
      </RouterLink>
      <RouterLink class="button" to="/lineage">Public lineage</RouterLink>
    </header>

    <div class="page-intro">
      <div>
        <p class="eyebrow">V2 / AUTHENTICATED OPERATIONS</p>
        <h1>From lot registration to recall.</h1>
        <p>
          Bootstrap console for producer, slaughterhouse, carrier and quality teams. The token is
          entered at runtime and is never part of the public Vite bundle.
        </p>
      </div>
    </div>

    <AppCard class="operations-auth">
      <div class="control">
        <label for="operator-token">Operator API token</label>
        <input id="operator-token" v-model.trim="token" class="input mono" type="password" autocomplete="off" placeholder="Bearer token — kept only in this tab" />
        <p class="control__help">This is a temporary bootstrap boundary. Replace it with wallet/role sessions before broad production use.</p>
      </div>
    </AppCard>

    <nav class="operations-tabs" aria-label="Operational areas">
      <button v-for="tab in tabs" :key="tab.id" class="operations-tab" :class="{ 'operations-tab--active': activeTab === tab.id }" type="button" @click="activeTab = tab.id">{{ tab.label }}</button>
    </nav>

    <p v-if="message" class="status-message" :data-tone="error ? 'danger' : 'success'" aria-live="polite">{{ message }}</p>

    <section v-if="activeTab === 'registry'" class="operations-grid">
      <AppCard>
        <header class="app-card__header"><div><p class="eyebrow">PRODUCER / PARTY</p><h2>Register party</h2></div></header>
        <form class="operation-form" @submit.prevent="registerParty">
          <label>Party ID<input v-model.trim="party.partyId" class="input mono" required /></label>
          <label>Legal name<input v-model.trim="party.legalName" class="input" required /></label>
          <label>Wallet (32-byte hex)<input v-model.trim="party.wallet" class="input mono" required /></label>
          <label>Role<select v-model.number="party.role" class="select"><option :value="1">Producer</option><option :value="2">Transporter</option><option :value="3">Slaughterhouse</option><option :value="4">Processor</option><option :value="5">Distributor</option><option :value="6">Retail</option></select></label>
          <label>Tax ID hash (optional)<input v-model.trim="party.taxIdHash" class="input mono" placeholder="64 lowercase hex" /></label>
          <AppButton type="submit" :disabled="busy">Register party</AppButton>
        </form>
      </AppCard>
      <AppCard>
        <header class="app-card__header"><div><p class="eyebrow">FACILITY</p><h2>Register facility</h2></div></header>
        <form class="operation-form" @submit.prevent="registerFacility">
          <label>Facility ID<input v-model.trim="facility.facilityId" class="input mono" required /></label>
          <label>Owner party ID<input v-model.trim="facility.ownerPartyId" class="input mono" required /></label>
          <label>Display name<input v-model.trim="facility.displayName" class="input" required /></label>
          <label>Facility type<select v-model.number="facility.facilityType" class="select"><option :value="1">Farm</option><option :value="2">Transport hub</option><option :value="3">Slaughterhouse</option><option :value="4">Processing facility</option><option :value="5">Distribution center</option><option :value="6">Retail</option><option :value="7">Inspection site</option></select></label>
          <label>Credential hash<input v-model.trim="facility.credentialHash" class="input mono" required /></label>
          <div class="control-row"><label>Valid from<input v-model.number="facility.validFrom" class="input" type="number" min="0" /></label><label>Valid until<input v-model.number="facility.validUntil" class="input" type="number" min="1" /></label></div>
          <AppButton type="submit" :disabled="busy">Register facility</AppButton>
        </form>
      </AppCard>
      <AppCard class="operations-wide">
        <header class="app-card__header"><div><p class="eyebrow">LOT / MATRICULA</p><h2>Register cattle lot</h2><p class="app-card__description">Asset IDs must already exist in the canonical v2 projection and their role must match their asset type.</p></div></header>
        <form class="operation-form operation-form--wide" @submit.prevent="registerLot">
          <div class="control-row"><label>Lot ID<input v-model.trim="lot.lotId" class="input mono" required /></label><label>External reference<input v-model.trim="lot.externalReference" class="input" /></label></div>
          <div class="control-row"><label>Facility ID<input v-model.trim="lot.facilityId" class="input mono" required /></label><label>Owner party ID<input v-model.trim="lot.ownerPartyId" class="input mono" required /></label></div>
          <div class="control-row"><label>Head count<input v-model.number="lot.headCount" class="input" type="number" min="1" /></label><label>Live weight (grams)<input v-model.number="lot.liveWeightGrams" class="input" type="number" min="1" /></label></div>
          <label>Assets JSON<textarea v-model="lot.assetsJson" class="input mono" rows="6" spellcheck="false"></textarea></label>
          <AppButton type="submit" :disabled="busy">Register lot</AppButton>
        </form>
      </AppCard>
    </section>

    <section v-else-if="activeTab === 'processing'" class="operations-grid">
      <AppCard class="operations-wide">
        <header class="app-card__header"><div><p class="eyebrow">SLAUGHTER / BUTCHERY / PROCESSING</p><h2>Register mass-balanced operation</h2><p class="app-card__description">Inputs, outputs, byproducts and losses must balance exactly. Canonical finalization is blocked until the linked transformation is FINALIZED.</p></div></header>
        <form class="operation-form operation-form--wide" @submit.prevent="registerProcessing">
          <div class="control-row"><label>Operation ID<input v-model.trim="processing.operationId" class="input mono" required /></label><label>Kind<select v-model="processing.operationKind" class="select"><option>SLAUGHTER</option><option>BUTCHERY</option><option>PROCESSING</option></select></label></div>
          <div class="control-row"><label>Facility ID<input v-model.trim="processing.facilityId" class="input mono" required /></label><label>Operator party ID<input v-model.trim="processing.operatorPartyId" class="input mono" required /></label></div>
          <div class="control-row"><label>Lot ID (optional)<input v-model.trim="processing.lotId" class="input mono" /></label><label>Finalized transformation ID<input v-model.trim="processing.transformationId" class="input mono" placeholder="required before /ready" /></label></div>
          <label>Items JSON<textarea v-model="processing.itemsJson" class="input mono" rows="8" spellcheck="false"></textarea></label>
          <label>Notes<textarea v-model="processing.notes" class="input" rows="3"></textarea></label>
          <div class="control-row"><AppButton type="submit" :disabled="busy">Register operation</AppButton><AppButton variant="secondary" :disabled="busy || !processing.operationId" @click="markReady">Ready for chain</AppButton><AppButton variant="secondary" :disabled="busy || !processing.operationId" @click="finalizeOperation">Finalize after Solana</AppButton></div>
        </form>
      </AppCard>
    </section>

    <section v-else-if="activeTab === 'shipment'" class="operations-grid">
      <AppCard class="operations-wide">
        <header class="app-card__header"><div><p class="eyebrow">CARRIER / CUSTODY</p><h2>Create shipment</h2><p class="app-card__description">Shipment lifecycle is DRAFT → DISPATCHED → IN_TRANSIT → DELIVERED, with rejection paths.</p></div></header>
        <form class="operation-form operation-form--wide" @submit.prevent="registerShipment">
          <div class="control-row"><label>Shipment ID<input v-model.trim="shipment.shipmentId" class="input mono" required /></label><label>Planned departure (RFC3339)<input v-model.trim="shipment.plannedDeparture" class="input mono" placeholder="2026-09-28T12:00:00Z" /></label></div>
          <div class="control-row"><label>Origin facility<input v-model.trim="shipment.originFacilityId" class="input mono" required /></label><label>Destination facility<input v-model.trim="shipment.destinationFacilityId" class="input mono" required /></label></div>
          <div class="control-row"><label>Carrier party<input v-model.trim="shipment.carrierPartyId" class="input mono" required /></label><label>Created by party<input v-model.trim="shipment.createdByPartyId" class="input mono" required /></label></div>
          <label>Items JSON<textarea v-model="shipment.itemsJson" class="input mono" rows="6" spellcheck="false"></textarea></label>
          <label>Notes<textarea v-model="shipment.notes" class="input" rows="3"></textarea></label>
          <div class="control-row"><AppButton type="submit" :disabled="busy">Create shipment</AppButton><select v-model="shipment.nextStatus" class="select"><option>DISPATCHED</option><option>IN_TRANSIT</option><option>DELIVERED</option><option>REJECTED</option></select><AppButton variant="secondary" :disabled="busy || !shipment.shipmentId" @click="changeShipmentStatus">Change status</AppButton></div>
        </form>
      </AppCard>
    </section>

    <section v-else class="operations-grid">
      <AppCard class="operations-wide">
        <header class="app-card__header"><div><p class="eyebrow">QUALITY / RECALL</p><h2>Open bounded recall</h2><p class="app-card__description">The server snapshots the selected lot, animal, product or transformation and traverses connected lineage in both directions.</p></div></header>
        <form class="operation-form operation-form--wide" @submit.prevent="openRecall">
          <div class="control-row"><label>Recall ID<input v-model.trim="recall.recallId" class="input mono" required /></label><label>Opened by party<input v-model.trim="recall.openedByPartyId" class="input mono" required /></label></div>
          <div class="control-row"><label>Scope type<select v-model="recall.scopeType" class="select"><option>LOT</option><option>ANIMAL</option><option>TRANSFORMATION</option><option>PRODUCT</option><option>ASSET</option></select></label><label>Scope ID<input v-model.trim="recall.scopeId" class="input mono" required /></label></div>
          <label>Reason<textarea v-model="recall.reason" class="input" rows="4" required></textarea></label>
          <AppButton type="submit" :disabled="busy">Open recall and snapshot</AppButton>
        </form>
      </AppCard>
    </section>

    <AppCard v-if="result" class="operations-wide operations-result">
      <header class="app-card__header"><div><p class="eyebrow">LAST RESPONSE</p><h2>Durable API result</h2></div></header>
      <pre class="mono">{{ result }}</pre>
    </AppCard>
  </main>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { RouterLink } from 'vue-router'
import { api, ApiClientError } from '../api/client'
import AppButton from '../components/AppButton.vue'
import AppCard from '../components/AppCard.vue'

const tabs = [
  { id: 'registry', label: 'Producer / registry' },
  { id: 'processing', label: 'Slaughterhouse' },
  { id: 'shipment', label: 'Carrier / shipment' },
  { id: 'recall', label: 'Quality / recall' },
] as const
type TabId = (typeof tabs)[number]['id']
const activeTab = ref<TabId>('registry')
const token = ref('')
const busy = ref(false)
const message = ref('')
const error = ref(false)
const result = ref('')

const party = ref({ partyId: newId(), legalName: '', taxIdHash: '', wallet: '', role: 1 })
const facility = ref({ facilityId: newId(), ownerPartyId: '', displayName: '', facilityType: 1, credentialHash: '', validFrom: 0, validUntil: 4102444800 })
const lot = ref({ lotId: newId(), facilityId: '', ownerPartyId: '', externalReference: '', headCount: 1, liveWeightGrams: 1, assetsJson: '[\n  { "assetId": "<64-hex>", "quantity": 1, "weightGrams": 1, "role": "ANIMAL" }\n]' })
const processing = ref({ operationId: newId(), facilityId: '', lotId: '', transformationId: '', operatorPartyId: '', operationKind: 'SLAUGHTER', notes: '', itemsJson: '[\n  { "assetId": "<animal-64-hex>", "direction": "INPUT", "quantity": 1, "weightGrams": 1000 },\n  { "assetId": "<carcass-64-hex>", "direction": "OUTPUT", "quantity": 1, "weightGrams": 900 },\n  { "assetId": null, "direction": "LOSS", "quantity": 1, "weightGrams": 100 }\n]' })
const shipment = ref({ shipmentId: newId(), originFacilityId: '', destinationFacilityId: '', carrierPartyId: '', createdByPartyId: '', plannedDeparture: '', notes: '', nextStatus: 'DISPATCHED', itemsJson: '[\n  { "assetId": "<64-hex>", "quantity": 1, "weightGrams": 900 }\n]' })
const recall = ref({ recallId: newId(), openedByPartyId: '', scopeType: 'LOT', scopeId: '', reason: '' })

async function registerParty(): Promise<void> {
  await run(async () => api.createParty(token.value, { ...party.value, taxIdHash: party.value.taxIdHash || null }))
}
async function registerFacility(): Promise<void> {
  await run(async () => api.createFacility(token.value, facility.value))
}
async function registerLot(): Promise<void> {
  await run(async () => api.createLot(token.value, { ...lot.value, externalReference: lot.value.externalReference || null, assets: parseJson(lot.value.assetsJson, 'lot assets') }))
}
async function registerProcessing(): Promise<void> {
  await run(async () => api.createProcessing(token.value, { ...processing.value, lotId: processing.value.lotId || null, transformationId: processing.value.transformationId || null, items: parseJson(processing.value.itemsJson, 'processing items') }))
}
async function markReady(): Promise<void> { await run(async () => api.setProcessingReady(token.value, processing.value.operationId)) }
async function finalizeOperation(): Promise<void> { await run(async () => api.finalizeProcessing(token.value, processing.value.operationId)) }
async function registerShipment(): Promise<void> {
  await run(async () => api.createShipment(token.value, { ...shipment.value, items: parseJson(shipment.value.itemsJson, 'shipment items') }))
}
async function changeShipmentStatus(): Promise<void> { await run(async () => api.setShipmentStatus(token.value, shipment.value.shipmentId, shipment.value.nextStatus)) }
async function openRecall(): Promise<void> { await run(async () => api.openRecall(token.value, recall.value)) }

async function run(work: () => Promise<unknown>): Promise<void> {
  if (busy.value) return
  busy.value = true
  error.value = false
  message.value = ''
  try {
    const value = await work()
    result.value = JSON.stringify(value, null, 2)
    message.value = 'Operation accepted by the durable API projection.'
  } catch (caught) {
    error.value = true
    message.value = apiErrorMessage(caught)
  } finally {
    busy.value = false
  }
}

function parseJson(value: string, name: string): unknown {
  try { return JSON.parse(value) } catch { throw new Error(`${name} must be valid JSON`) }
}
function apiErrorMessage(value: unknown): string {
  if (value instanceof ApiClientError && value.responseBody) {
    try { const body = JSON.parse(value.responseBody) as { message?: unknown }; if (typeof body.message === 'string') return body.message } catch { /* fallback */ }
  }
  return value instanceof Error ? value.message : 'Operation failed'
}
function newId(): string { const bytes = new Uint8Array(32); crypto.getRandomValues(bytes); return Array.from(bytes, (value) => value.toString(16).padStart(2, '0')).join('') }
</script>

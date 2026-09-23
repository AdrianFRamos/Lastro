<template>
  <main class="page page--verifier">
    <header class="app-header">
      <a class="app-header__brand" href="/" aria-label="Lastro home">
        <img src="/logo.svg" alt="" width="36" height="36" />
        <span>
          <strong>LASTRO</strong>
          <span class="app-header__context">Independent verifier</span>
        </span>
      </a>
      <div class="app-header__tools">
        <span class="network-pill">{{ webConfig.solanaChain }}</span>
        <a class="button" href="/demo">Open Demo</a>
      </div>
    </header>

    <div class="page-intro">
      <div>
        <p class="eyebrow">SOFTWARE DEMO / INDEPENDENT VERIFICATION</p>
        <h1>Verify evidence without trusting the interface.</h1>
        <p>
          Load a portable EvidencePackage or request one by AnimalID. Local evidence checks run
          before canonical Solana comparison.
        </p>
      </div>
    </div>

    <div class="verifier-shell">
      <AppCard>
        <header class="app-card__header">
          <div>
            <h2>Evidence source</h2>
            <p class="app-card__description">
              Use either an AnimalID lookup or a local exported package.
            </p>
          </div>
        </header>

        <div class="verifier-source-grid">
          <div class="verifier-inputs">
            <div class="control">
              <label for="animal-id">AnimalID</label>
              <input
                id="animal-id"
                v-model.trim="animalId"
                class="input mono"
                aria-label="AnimalID"
                autocomplete="off"
                placeholder="64-character AnimalID"
                :disabled="busy"
              />
            </div>
            <AppButton :disabled="busy || !animalId" :busy="busy" @click="loadFromApi"
              >Verify AnimalID</AppButton
            >
          </div>

          <div class="file-control">
            <label for="evidence-file">EvidencePackage file</label>
            <input
              id="evidence-file"
              class="file-input"
              type="file"
              accept="application/json"
              aria-label="Evidence package file"
              :disabled="busy"
              @change="loadFile"
            />
          </div>
        </div>
      </AppCard>

      <section class="verdict" :data-status="overall" aria-labelledby="overall-result">
        <div>
          <span class="verdict__label">OVERALL RESULT</span>
          <div id="overall-result" class="verdict__value">Overall: {{ overall }}</div>
        </div>
        <span class="status-badge" :data-tone="overallTone">{{ overall }}</span>
      </section>

      <p v-if="message" class="status-message" :data-tone="messageTone" aria-live="polite">
        {{ message }}
      </p>

      <VerificationPanel :layers="layers" />

      <AppCard v-if="verifiedPackage">
        <details class="evidence-details">
          <summary>Advanced evidence details</summary>
          <dl class="data-list evidence-details__summary">
            <dt>AnimalID</dt>
            <dd class="mono break-all">{{ verifiedPackage.animalId }}</dd>
            <dt>Deployment</dt>
            <dd class="mono break-all">{{ verifiedPackage.deploymentId }}</dd>
            <dt>Event count</dt>
            <dd>{{ verifiedPackage.events.length }}</dd>
          </dl>
          <div class="evidence-events">
            <article
              v-for="(event, index) in verifiedPackage.events"
              :key="index + '-' + event.stationSignatureHex"
              class="evidence-event"
            >
              <p>
                <strong>Sequence {{ index + 1 }}</strong>
              </p>
              <p>
                Observed RFID: <code class="break-all">{{ event.observedRfidHex }}</code>
              </p>
              <p>
                Station key: <code class="break-all">{{ event.stationPubkeyHex }}</code>
              </p>
              <p>
                Transaction:
                <code class="break-all">{{ event.txSignature ?? 'Not finalized' }}</code>
              </p>
            </article>
          </div>
        </details>
      </AppCard>
    </div>
  </main>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute } from 'vue-router'
import { api } from '../api/client'
import AppButton from '../components/AppButton.vue'
import AppCard from '../components/AppCard.vue'
import VerificationPanel from '../components/VerificationPanel.vue'
import { webConfig } from '../config'
import { parseEvidencePackage, type EvidencePackage } from '../protocol/evidence'
import { verifyCanonicalChainState } from '../verify/verifyChain'
import { verifyEvidencePackage } from '../verify/verifyEvidencePackage'
import type { VerificationLayerResult, VerificationStatus } from '../verify/types'

const route = useRoute()
const animalId = ref(typeof route.params.animalId === 'string' ? route.params.animalId : '')
const busy = ref(false)
const message = ref('No package loaded')
const layers = ref<VerificationLayerResult[]>(uncheckedLayers())
const verifiedPackage = ref<EvidencePackage | null>(null)
const overall = computed<VerificationStatus>(() => {
  if (layers.value.some((layer) => layer.status === 'INVALID')) return 'INVALID'
  return layers.value.every((layer) => layer.status === 'VALID') ? 'VALID' : 'NOT_CHECKED'
})
const overallTone = computed<'success' | 'danger' | undefined>(() => {
  if (overall.value === 'VALID') return 'success'
  if (overall.value === 'INVALID') return 'danger'
  return undefined
})
const messageTone = computed<'success' | 'danger' | undefined>(() => {
  if (overall.value === 'VALID') return 'success'
  if (overall.value === 'INVALID') return 'danger'
  return undefined
})

async function loadFromApi(): Promise<void> {
  await run(async () => api.getEvidencePackage(animalId.value))
}

async function loadFile(event: Event): Promise<void> {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) return
  await run(async () => parseEvidencePackage(JSON.parse(await file.text()) as unknown))
}

async function run(load: () => Promise<EvidencePackage>): Promise<void> {
  busy.value = true
  message.value = 'Verifying evidence...'
  layers.value = uncheckedLayers()
  verifiedPackage.value = null
  try {
    const pkg = await load()
    verifiedPackage.value = pkg
    const local = await verifyEvidencePackage(pkg)
    layers.value = local.layers
    const localValid = local.layers
      .filter((layer) => layer.layer !== 'ON_CHAIN_STATE')
      .every((layer) => layer.status === 'VALID')
    if (!localValid) {
      message.value = 'Evidence is invalid before canonical Solana comparison.'
      return
    }
    const onChain = await verifyCanonicalChainState(pkg)
    layers.value = local.layers.map((layer) => (layer.layer === 'ON_CHAIN_STATE' ? onChain : layer))
    message.value =
      onChain.status === 'VALID'
        ? 'Evidence and canonical Solana state agree.'
        : 'Local evidence verification completed; canonical Solana verification did not validate.'
  } catch (error) {
    layers.value = uncheckedLayers()
    message.value = error instanceof Error ? error.message : 'Verification failed'
  } finally {
    busy.value = false
  }
}

function uncheckedLayers(): VerificationLayerResult[] {
  return [
    { layer: 'RFID_EVIDENCE', status: 'NOT_CHECKED', detail: 'No package loaded' },
    { layer: 'STATION_SIGNATURE', status: 'NOT_CHECKED', detail: 'No package loaded' },
    { layer: 'IDENTITY_CONTINUITY', status: 'NOT_CHECKED', detail: 'No package loaded' },
    { layer: 'CUSTODY', status: 'NOT_CHECKED', detail: 'No package loaded' },
    { layer: 'ON_CHAIN_STATE', status: 'NOT_CHECKED', detail: 'No package loaded' },
  ]
}
</script>

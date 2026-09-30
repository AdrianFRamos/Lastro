<template>
  <main class="page page--verifier">
    <header class="app-header">
      <a class="app-header__brand" href="/" aria-label="Lastro home">
        <img src="/logo.svg" alt="" width="36" height="36" />
        <span>
          <strong>LASTRO</strong>
          <span class="app-header__context">Public lineage explorer</span>
        </span>
      </a>
      <div class="app-header__tools">
        <span class="network-pill">{{ webConfig.solanaChain }}</span>
        <a class="button" href="/verify">Independent verifier</a>
      </div>
    </header>

    <div class="page-intro">
      <div>
        <p class="eyebrow">PUBLIC TRACEABILITY / LINEAGE</p>
        <h1>Follow an asset through transformation.</h1>
        <p>
          Enter an asset identifier to inspect the public lineage projection. The graph is a
          read-only view; evidence and canonical Solana verification remain separate checks.
        </p>
      </div>
    </div>

    <div class="stack">
      <AppCard>
        <header class="app-card__header">
          <div>
            <h2>Asset lookup</h2>
            <p class="app-card__description">Use the lowercase 64-character asset identifier.</p>
          </div>
        </header>
        <form class="lineage-search" @submit.prevent="loadLineage">
          <div class="control">
            <label for="lineage-asset-id">Asset ID</label>
            <input
              id="lineage-asset-id"
              v-model.trim="assetId"
              class="input mono"
              autocomplete="off"
              placeholder="64-character lowercase hex"
              :disabled="busy"
            />
          </div>
          <AppButton :disabled="busy || !assetId" :busy="busy" type="submit">
            Load lineage
          </AppButton>
        </form>
      </AppCard>

      <p v-if="message" class="status-message" :data-tone="messageTone" aria-live="polite">
        {{ message }}
      </p>

      <AppCard v-if="edges.length" class="lineage-card">
        <div class="lineage-summary">
          <div>
            <span>Edges</span><strong>{{ edges.length }}</strong>
          </div>
          <div>
            <span>Transformations</span><strong>{{ transformationCount }}</strong>
          </div>
          <div>
            <span>Weight referenced</span><strong>{{ totalWeight }}</strong>
          </div>
        </div>
        <LineageGraph :edges="edges" />
      </AppCard>

      <AppCard v-if="edges.length">
        <header class="app-card__header">
          <div>
            <h2>Lineage records</h2>
            <p class="app-card__description">Every edge is displayed with its role and quantity.</p>
          </div>
        </header>
        <div class="lineage-table-wrap">
          <table class="lineage-table">
            <thead>
              <tr>
                <th>Transformation</th>
                <th>Parent</th>
                <th>Child</th>
                <th>Role</th>
                <th>Weight</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="edge in edges"
                :key="`${edge.transformationId}-${edge.position}-${edge.childAssetId}`"
              >
                <td class="mono">{{ shortId(edge.transformationId) }}</td>
                <td class="mono">{{ shortId(edge.parentAssetId) }}</td>
                <td class="mono">{{ shortId(edge.childAssetId) }}</td>
                <td>{{ roleLabel(edge.role) }}</td>
                <td>{{ formatWeight(edge.weightGrams) }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </AppCard>
    </div>
  </main>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute } from 'vue-router'
import AppButton from '../components/AppButton.vue'
import AppCard from '../components/AppCard.vue'
import { api } from '../api/client'
import type { Hex32, LineageEdge } from '../api/types'
import { webConfig } from '../config'
import LineageGraph from '../components/LineageGraph.vue'

const route = useRoute()
const assetId = ref(typeof route.params.assetId === 'string' ? route.params.assetId : '')
const edges = ref<LineageEdge[]>([])
const busy = ref(false)
const message = ref('Enter an asset ID to begin.')
const messageTone = ref<'success' | 'danger' | undefined>()
const transformationCount = computed(
  () => new Set(edges.value.map((edge) => edge.transformationId)).size,
)
const totalWeight = computed(() =>
  formatWeight(edges.value.reduce((sum, edge) => sum + edge.weightGrams, 0)),
)

if (assetId.value) void loadLineage()

async function loadLineage(): Promise<void> {
  if (!/^[0-9a-f]{64}$/.test(assetId.value)) {
    message.value = 'Asset ID must be exactly 64 lowercase hexadecimal characters.'
    messageTone.value = 'danger'
    edges.value = []
    return
  }
  busy.value = true
  message.value = 'Loading verified lineage projection…'
  messageTone.value = undefined
  try {
    edges.value = await api.getLineage(assetId.value as Hex32)
    message.value = edges.value.length
      ? 'Lineage projection loaded. Verify the underlying evidence before making a business decision.'
      : 'No public lineage edges were found for this asset.'
    messageTone.value = edges.value.length ? 'success' : undefined
  } catch (error) {
    edges.value = []
    message.value = error instanceof Error ? error.message : 'Lineage lookup failed.'
    messageTone.value = 'danger'
  } finally {
    busy.value = false
  }
}

function shortId(value: string): string {
  return `${value.slice(0, 8)}…${value.slice(-8)}`
}

function roleLabel(role: number): string {
  if (role === 1) return 'Input'
  if (role === 2) return 'Output'
  if (role === 3) return 'Byproduct'
  return 'Loss'
}

function formatWeight(grams: number): string {
  return `${(grams / 1000).toLocaleString(undefined, { maximumFractionDigits: 3 })} kg`
}
</script>

<script setup lang="ts">
import AppCard from './AppCard.vue'
import type { AssetProjection } from '../api/types'

defineProps<{ asset: AssetProjection | null }>()
</script>

<template>
  <AppCard aria-label="Animal state">
    <header class="app-card__header">
      <div>
        <p class="eyebrow">ANIMAL</p>
        <h2>Identity state</h2>
        <p class="app-card__description">Canonical Solana AssetState at finalized commitment.</p>
      </div>
      <span class="status-badge" :data-tone="asset ? 'proof' : undefined">
        {{ asset ? 'RESOLVED' : 'UNRESOLVED' }}
      </span>
    </header>

    <dl v-if="asset" class="data-list">
      <dt>AssetID</dt>
      <dd class="mono break-all">{{ asset.assetId }}</dd>
      <dt>Current RFID</dt>
      <dd class="mono break-all">{{ asset.currentRfidHash ?? 'No RFID bound' }}</dd>
      <dt>Custodian</dt>
      <dd class="mono break-all">{{ asset.custodian }}</dd>
      <dt>State version</dt>
      <dd>{{ asset.stateVersion }}</dd>
      <dt>Events</dt>
      <dd>{{ asset.eventSequence }}</dd>
    </dl>

    <div v-else class="empty-state">
      <strong>No animal selected.</strong>
      <p>Register a new animal on Solana or look one up by AssetID or RFID.</p>
    </div>
  </AppCard>
</template>

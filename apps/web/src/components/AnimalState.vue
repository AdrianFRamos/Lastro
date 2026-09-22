<script setup lang="ts">
import AppCard from './AppCard.vue'
import type { AnimalProjection } from '../api/types'

defineProps<{ animal: AnimalProjection | null }>()
</script>

<template>
  <AppCard aria-label="Animal state">
    <header class="app-card__header">
      <div>
        <p class="eyebrow">ANIMAL</p>
        <h2>Identity state</h2>
        <p class="app-card__description">Current durable projection after finalized canonical transitions.</p>
      </div>
      <span class="status-badge" :data-tone="animal ? 'proof' : undefined">
        {{ animal ? 'RESOLVED' : 'UNRESOLVED' }}
      </span>
    </header>

    <dl v-if="animal" class="data-list">
      <dt>AnimalID</dt><dd class="mono break-all">{{ animal.animalId }}</dd>
      <dt>Visual recovery</dt><dd>{{ animal.visualRecoveryId }}</dd>
      <dt>Current RFID</dt><dd class="mono break-all">{{ animal.currentRfidHash ?? 'Not originated' }}</dd>
      <dt>Custodian</dt><dd class="mono break-all">{{ animal.currentCustodian ?? 'Not originated' }}</dd>
      <dt>Revision</dt><dd>{{ animal.identityRevision }}</dd>
      <dt>Sequence</dt><dd>{{ animal.eventSequence }}</dd>
    </dl>

    <div v-else class="empty-state">
      <strong>No animal selected.</strong>
      <p>Resolve an existing identity or create a new recovery record before starting a canonical action.</p>
    </div>
  </AppCard>
</template>

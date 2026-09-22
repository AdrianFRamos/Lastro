<script setup lang="ts">
import AppCard from './AppCard.vue'
import type { VerificationLayerName, VerificationLayerResult, VerificationStatus } from '../verify/types'

defineProps<{ layers: VerificationLayerResult[] }>()

function layerLabel(layer: VerificationLayerName): string {
  return layer.replaceAll('_', ' ')
}

function statusTone(status: VerificationStatus): 'success' | 'danger' | undefined {
  if (status === 'VALID') return 'success'
  if (status === 'INVALID') return 'danger'
  return undefined
}
</script>

<template>
  <AppCard aria-label="Verification result">
    <header class="app-card__header">
      <div>
        <p class="eyebrow">VERIFICATION LAYERS</p>
        <h2>Independent checks</h2>
        <p class="app-card__description">Each layer is evaluated separately; NOT_CHECKED never counts as valid.</p>
      </div>
    </header>

    <ul class="verification-list">
      <li v-for="layer in layers" :key="layer.layer" class="verification-layer">
        <strong class="verification-layer__name">
          {{ layerLabel(layer.layer) }}
          <span class="verification-layer__summary"> — {{ layer.status }}</span>
        </strong>
        <span class="status-badge" :data-tone="statusTone(layer.status)">{{ layer.status }}</span>
        <span class="verification-layer__detail">{{ layer.detail }}</span>
      </li>
    </ul>
  </AppCard>
</template>

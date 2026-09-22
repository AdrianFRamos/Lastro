<script setup lang="ts">
import { computed } from 'vue'
import AppCard from './AppCard.vue'

const props = withDefaults(defineProps<{ reader?: string; signer?: string; capture?: string }>(), {
  reader: 'unknown',
  signer: 'unknown',
  capture: 'none',
})

const captureState = computed(() => {
  const value = props.capture.toUpperCase()
  if (value.includes('FINALIZED')) return { label: 'FINALIZED', tone: 'success' }
  if (value.includes('SUBMITTED')) return { label: 'SUBMITTED', tone: 'chain' }
  if (value.includes('EVIDENCE_ACCEPTED')) return { label: 'SIGNED', tone: 'proof' }
  if (value.includes('EXPIRED') || value.includes('CANCELLED') || value.includes('REJECTED')) {
    return { label: 'ERROR', tone: 'danger' }
  }
  if (value.includes('PENDING') || value.includes('DISPATCHED')) return { label: 'WAITING', tone: 'warning' }
  return { label: 'UNAVAILABLE', tone: undefined }
})

function factState(value: string): { label: string; tone: 'proof' | undefined } {
  return value === 'unknown' ? { label: 'UNAVAILABLE', tone: undefined } : { label: 'READY', tone: 'proof' }
}
</script>

<template>
  <AppCard aria-label="Station status">
    <header class="app-card__header">
      <div>
        <p class="eyebrow">STATION</p>
        <h2>Physical evidence</h2>
        <p class="app-card__description">Reader observation and P-256 signature state exposed by the current capture.</p>
      </div>
      <span class="status-badge" :data-tone="captureState.tone">{{ captureState.label }}</span>
    </header>

    <dl class="station-list">
      <div class="station-row">
        <dt>Reader</dt>
        <dd>{{ reader }}</dd>
        <span class="status-badge" :data-tone="factState(reader).tone">{{ factState(reader).label }}</span>
      </div>
      <div class="station-row">
        <dt>P-256 signer</dt>
        <dd>{{ signer }}</dd>
        <span class="status-badge" :data-tone="factState(signer).tone">{{ factState(signer).label }}</span>
      </div>
      <div class="station-row">
        <dt>Capture</dt>
        <dd class="mono break-all">{{ capture }}</dd>
        <span class="status-badge" :data-tone="captureState.tone">{{ captureState.label }}</span>
      </div>
    </dl>
  </AppCard>
</template>

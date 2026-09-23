<script setup lang="ts">
import AppCard from './AppCard.vue'

type TimelineEvent = { sequence: number; label: string }
type TimelineKind = 'ORIGIN' | 'TRANSFER' | 'REIDENTIFY' | 'EVENT'

defineProps<{ events: TimelineEvent[] }>()

function eventKind(label: string): TimelineKind {
  if (label.includes('REIDENTIFY')) return 'REIDENTIFY'
  if (label.includes('TRANSFER')) return 'TRANSFER'
  if (label.includes('ORIGIN')) return 'ORIGIN'
  return 'EVENT'
}
</script>

<template>
  <AppCard aria-label="Custody timeline">
    <header class="app-card__header">
      <div>
        <p class="eyebrow">CUSTODY / IDENTITY TIMELINE</p>
        <h2>Canonical history</h2>
        <p class="app-card__description">
          Custody transitions and physical identity rebinding remain distinct events.
        </p>
      </div>
    </header>

    <ol v-if="events.length" class="timeline">
      <li
        v-for="event in events"
        :key="event.sequence + '-' + event.label"
        class="timeline__item"
        :data-kind="eventKind(event.label)"
      >
        <span class="timeline__marker">{{ event.sequence }}</span>
        <div class="timeline__content">
          <div class="timeline__topline">
            <span class="timeline__type">{{ eventKind(event.label) }}</span>
            <span
              v-if="eventKind(event.label) === 'REIDENTIFY'"
              class="status-badge"
              data-tone="proof"
              >IDENTITY</span
            >
            <span
              v-else-if="eventKind(event.label) === 'TRANSFER'"
              class="status-badge"
              data-tone="chain"
              >CUSTODY</span
            >
          </div>
          <p class="timeline__label">{{ event.label }}</p>
          <p v-if="eventKind(event.label) === 'REIDENTIFY'" class="timeline__note">
            AnimalID unchanged · RFID changed · revision increased · custodian unchanged
          </p>
        </div>
      </li>
    </ol>

    <div v-else class="empty-state">
      <strong>No canonical events yet.</strong>
      <p>
        ORIGIN will create the first custody and physical identity binding after finalized Solana
        confirmation.
      </p>
    </div>
  </AppCard>
</template>

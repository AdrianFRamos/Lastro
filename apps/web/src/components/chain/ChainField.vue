<script setup lang="ts">
/** One labelled value of a stage record. Hashes render in full, monospaced, and may link. */
import { RouterLink } from 'vue-router'

defineProps<{
  label: string
  value?: string
  /** Render the value as a hash (monospaced, wraps anywhere). */
  hash?: boolean
  to?: string
  /** Take both record columns. */
  wide?: boolean
}>()
</script>

<template>
  <div class="chain-field" :class="{ 'chain-field--wide': wide }">
    <span class="chain-field__label">{{ label }}</span>
    <div class="chain-field__value" :class="{ 'chain-field__value--hash': hash }">
      <slot>
        <RouterLink v-if="to" :to="to">
          {{ value }}
          <span aria-hidden="true">→</span>
        </RouterLink>
        <template v-else>{{ value }}</template>
      </slot>
    </div>
  </div>
</template>

<style scoped>
.chain-field {
  display: grid;
  gap: 6px;
  min-width: 0;
}

.chain-field--wide {
  grid-column: 1 / -1;
}

.chain-field__label {
  color: var(--muted);
  font: 700 10px/1.3 var(--font-mono);
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.chain-field__value {
  min-height: 42px;
  padding: 11px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--canvas);
  color: var(--text);
  font-size: 14px;
  line-height: 1.45;
}

.chain-field__value--hash {
  font: 600 12px/1.5 var(--font-mono);
  word-break: break-all;
}

.chain-field__value a {
  color: var(--proof);
  text-decoration: none;
}

.chain-field__value a:hover,
.chain-field__value a:focus-visible {
  text-decoration: underline;
}
</style>

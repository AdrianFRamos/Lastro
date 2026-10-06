<script setup lang="ts">
/**
 * Titled list inside a stage record (animals, pieces, stops, vaccines...). Items may link.
 * `illustrative` tags data the current software does not record (an expansion example).
 */
import { RouterLink } from 'vue-router'
import type { LinkedItem } from '../../demo/chainHistory'
import { tx, useCopy } from '../../i18n'

defineProps<{
  title: string
  items: LinkedItem[]
  wide?: boolean
  illustrative?: boolean
}>()

const copy = useCopy({
  en: {
    tag: 'Expansion example',
    hint: 'The current software does not record this data yet.',
  },
  pt: {
    tag: 'Exemplo de expansão',
    hint: 'O software atual ainda não registra este dado.',
  },
})
</script>

<template>
  <section class="chain-list" :class="{ 'chain-list--wide': wide }">
    <div class="chain-list__heading">
      <h3 class="chain-list__title">{{ title }}</h3>
      <span v-if="illustrative" class="chain-list__tag" :title="copy.hint">{{ copy.tag }}</span>
    </div>
    <ul>
      <li v-for="item in items" :key="tx(item.label) + (item.hash ?? tx(item.note ?? ''))">
        <div class="chain-list__head">
          <RouterLink v-if="item.to" :to="item.to" class="chain-list__label">
            {{ tx(item.label) }} <span aria-hidden="true">→</span>
          </RouterLink>
          <span v-else class="chain-list__label">{{ tx(item.label) }}</span>
          <span v-if="item.note" class="chain-list__note">{{ tx(item.note) }}</span>
        </div>
        <code v-if="item.hash" class="chain-list__hash">{{ item.hash }}</code>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.chain-list {
  display: grid;
  align-content: start;
  gap: 6px;
  min-width: 0;
}

.chain-list--wide {
  grid-column: 1 / -1;
}

.chain-list__heading {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 6px 10px;
}

.chain-list__tag {
  padding: 3px 8px;
  border: 1px solid color-mix(in srgb, var(--warning) 45%, var(--border));
  border-radius: 999px;
  color: var(--warning);
  font: 700 10px/1.2 var(--font-mono);
}

.chain-list__title {
  margin: 0;
  color: var(--muted);
  font: 700 10px/1.3 var(--font-mono);
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.chain-list ul {
  display: grid;
  margin: 0;
  padding: 4px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--canvas);
  list-style: none;
}

.chain-list li {
  display: grid;
  gap: 4px;
  padding: 11px 0;
  border-bottom: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
}

.chain-list li:last-child {
  border-bottom: 0;
}

.chain-list__head {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  justify-content: space-between;
  gap: 4px 12px;
}

.chain-list__label {
  color: var(--text);
  font-size: 14px;
  font-weight: 650;
  text-decoration: none;
}

a.chain-list__label {
  color: var(--proof);
}

a.chain-list__label:hover,
a.chain-list__label:focus-visible {
  text-decoration: underline;
}

.chain-list__note {
  color: var(--muted);
  font-size: 12px;
}

.chain-list__hash {
  color: color-mix(in srgb, var(--text) 70%, transparent);
  font: 500 11px/1.5 var(--font-mono);
  word-break: break-all;
}
</style>

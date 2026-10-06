<script setup lang="ts">
/**
 * Shell of every Open Demo stage page, in the visual language of /future: a centered kicker,
 * large title and custody dates (as in the design), the stage record panel (default slot) and a
 * way back to the full timeline.
 */
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import ChainHeader from './ChainHeader.vue'
import { formatDate, formatDuration, routes, type Period } from '../../demo/chainHistory'
import { useCopy } from '../../i18n'

const props = defineProps<{
  kicker: string
  title: string
  /** Breadcrumb label for this page. */
  crumb: string
  /** Small line above the title, e.g. the animal being shown. */
  preTitle?: string
  period?: Period
  recordTitle: string
  /** Parent page between the timeline and this one, for nested pages. */
  parent?: { label: string; to: string }
}>()

const copy = useCopy({
  en: {
    history: 'Chain history',
    crumbs: 'You are here',
    simulated: 'Simulated record for the demo',
    record: 'RECORD',
    journey: 'JOURNEY',
    others: 'See the other stages of this piece.',
  },
  pt: {
    history: 'Histórico da cadeia',
    crumbs: 'Você está em',
    simulated: 'Registro simulado para demonstração',
    record: 'REGISTRO',
    journey: 'JORNADA',
    others: 'Veja as outras etapas desta peça.',
  },
})

const periodLabel = computed(() => {
  if (!props.period) return ''
  const end = props.period.endedAt ? formatDate(props.period.endedAt) : ''
  return `${formatDate(props.period.startedAt)} - ${end}`.trimEnd()
})
</script>

<template>
  <main class="stage-page">
    <div class="stage-page__frame">
      <ChainHeader>
        <slot v-if="$slots.actions" name="actions" />
      </ChainHeader>

      <nav class="stage-page__crumbs" :aria-label="copy.crumbs">
        <RouterLink :to="routes.history">{{ copy.history }}</RouterLink>
        <template v-if="parent">
          <span aria-hidden="true">/</span>
          <RouterLink :to="parent.to">{{ parent.label }}</RouterLink>
        </template>
        <span aria-hidden="true">/</span>
        <span aria-current="page">{{ crumb }}</span>
      </nav>

      <section class="stage-hero" aria-labelledby="stage-title">
        <p class="stage-kicker">{{ kicker }}</p>
        <p v-if="preTitle" class="stage-hero__pre">{{ preTitle }}</p>
        <h1 id="stage-title">{{ title }}</h1>
        <p v-if="period" class="stage-hero__period">{{ periodLabel }}</p>
        <div class="stage-hero__meta">
          <span v-if="period" :class="{ 'is-current': !period.endedAt }">
            {{ formatDuration(period) }}
          </span>
          <slot name="aside" />
          <span class="is-muted">{{ copy.simulated }}</span>
        </div>
      </section>

      <section class="stage-record" aria-labelledby="stage-record-title">
        <p class="stage-kicker">{{ copy.record }}</p>
        <h2 id="stage-record-title">{{ recordTitle }}</h2>
        <div class="stage-record__panel">
          <slot />
        </div>
      </section>

      <footer class="stage-footer">
        <div>
          <p class="stage-kicker">{{ copy.journey }}</p>
          <h2>{{ copy.others }}</h2>
        </div>
        <RouterLink class="stage-footer__link" :to="routes.history">
          {{ copy.history }}
          <span aria-hidden="true">→</span>
        </RouterLink>
      </footer>
    </div>
  </main>
</template>

<style scoped>
.stage-page {
  min-height: 100vh;
  background:
    radial-gradient(
      circle at 78% 12%,
      color-mix(in srgb, var(--primary) 8%, transparent),
      transparent 26rem
    ),
    var(--canvas);
  color: var(--text);
}

.stage-page__frame {
  width: min(100% - 64px, 1480px);
  margin: 0 auto;
  padding-top: 8px;
}

.stage-page__crumbs {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  padding: 18px 0 0;
  color: var(--muted);
  font: 600 11px/1.4 var(--font-mono);
}

.stage-page__crumbs a {
  color: var(--muted);
  text-decoration: none;
}

.stage-page__crumbs a:hover,
.stage-page__crumbs a:focus-visible {
  color: var(--proof);
}

.stage-page__crumbs [aria-current='page'] {
  color: var(--text);
}

.stage-kicker {
  margin: 0 0 12px;
  color: var(--proof);
  font: 700 10px/1.3 var(--font-mono);
  letter-spacing: 0.16em;
}

/* Centered title block, as in the design: kicker, optional pre-title, title, dates. */
.stage-hero {
  display: grid;
  justify-items: center;
  padding: 56px 0;
  border-bottom: 1px solid var(--border);
  text-align: center;
}

.stage-hero__pre {
  margin: 0 0 10px;
  color: var(--muted);
  font: 700 15px/1.3 var(--font-mono);
}

.stage-hero h1 {
  max-width: 18ch;
  margin: 0;
  font-size: clamp(42px, 4.6vw, 68px);
  font-weight: 580;
  line-height: 1;
  letter-spacing: -0.052em;
  text-wrap: balance;
}

.stage-hero__period {
  margin: 18px 0 0;
  font: 700 clamp(16px, 1.4vw, 20px) / 1.3 var(--font-mono);
}

.stage-hero__meta {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 8px;
  margin-top: 18px;
}

.stage-hero__meta > span,
.stage-hero__meta > :slotted(span) {
  padding: 7px 12px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: color-mix(in srgb, var(--surface) 80%, transparent);
  color: var(--text);
  font: 600 12px/1 var(--font-mono);
}

.stage-hero__meta > .is-current {
  border-color: color-mix(in srgb, var(--proof) 55%, var(--border));
  color: var(--proof);
}

.stage-hero__meta > .is-muted {
  color: var(--muted);
}

.stage-record {
  padding: 56px 0;
  border-bottom: 1px solid var(--border);
}

.stage-record h2 {
  margin: 0 0 28px;
  font-size: clamp(28px, 2.5vw, 38px);
  font-weight: 560;
  line-height: 1.05;
  letter-spacing: -0.035em;
}

/* Two-column record; fields and lists choose their own width. */
.stage-record__panel {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px 18px;
  padding: 26px;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
}

.stage-footer {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 40px;
  padding: 56px 0 72px;
}

.stage-footer h2 {
  max-width: 24ch;
  margin: 0;
  font-size: clamp(26px, 2.6vw, 38px);
  line-height: 1.08;
  letter-spacing: -0.03em;
}

.stage-footer__link {
  display: inline-flex;
  gap: 36px;
  align-items: center;
  padding: 14px 16px;
  border: 1px solid color-mix(in srgb, var(--primary) 62%, var(--border));
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--primary) 12%, var(--canvas));
  color: var(--text);
  font: 700 12px/1 var(--font-mono);
  text-decoration: none;
  white-space: nowrap;
}

.stage-footer__link:hover,
.stage-footer__link:focus-visible {
  background: color-mix(in srgb, var(--primary) 22%, var(--canvas));
}

@media (max-width: 700px) {
  .stage-page__frame {
    width: min(100% - 32px, 1480px);
  }

  .stage-hero {
    padding: 40px 0;
  }

  .stage-hero h1 {
    max-width: 100%;
    font-size: clamp(36px, 10.5vw, 48px);
  }

  .stage-record {
    padding: 40px 0;
  }

  .stage-record__panel {
    grid-template-columns: 1fr;
    padding: 18px;
  }

  .stage-footer {
    flex-direction: column;
    align-items: flex-start;
    gap: 24px;
  }
}
</style>

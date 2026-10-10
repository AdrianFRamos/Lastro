<script setup lang="ts">
/**
 * Open Demo entry: simulates what a consumer sees when looking up the chain history of a piece
 * of meat. Demo data comes from demo/chainHistory; the newest stage is drawn at the top and the
 * spine fades out below the oldest stage to show the history goes further back.
 */
import { RouterLink } from 'vue-router'
import ChainHeader from '../components/chain/ChainHeader.vue'
import {
  dayAndMonth,
  formatDate,
  formatDuration,
  product,
  stages,
  yearOf,
} from '../demo/chainHistory'
import { tr, tx, useCopy } from '../i18n'

const copy = useCopy({
  en: {
    eyebrow: 'Chain history',
    title: 'The journey of this cut, stage by stage.',
    support: 'Every custody is recorded with a start and an end. Click a stage to see the details.',
    facts: 'Product summary',
    product: 'Product',
    lot: 'Lot',
    stages: 'Stages',
    recorded: 'recorded',
    demo: 'Demo',
    timeline: 'Chain stages, newest first',
    step: 'Stage',
    done: 'Completed',
    current: 'Current',
    today: 'today',
  },
  pt: {
    eyebrow: 'Histórico da cadeia',
    title: 'A jornada deste corte, etapa por etapa.',
    support:
      'Cada custódia fica registrada com início e fim. Clique em uma etapa para ver os detalhes.',
    facts: 'Resumo do produto',
    product: 'Produto',
    lot: 'Lote',
    stages: 'Etapas',
    recorded: 'registradas',
    demo: 'Demonstração',
    timeline: 'Etapas da cadeia, da mais recente para a mais antiga',
    step: 'Etapa',
    done: 'Concluída',
    current: 'Atual',
    today: 'hoje',
  },
})

function stepLabel(index: number): string {
  return `${copy.value.step} ${String(stages.length - index).padStart(2, '0')}`
}
</script>

<template>
  <main class="chain-history">
    <div class="chain-history__frame">
      <ChainHeader />

      <section class="chain-history__intro" aria-labelledby="chain-history-title">
        <p class="chain-history__eyebrow">{{ copy.eyebrow }}</p>
        <h1 id="chain-history-title">{{ copy.title }}</h1>
        <p class="chain-history__support">
          {{ copy.support }}
        </p>
        <ul class="chain-history__facts" :aria-label="copy.facts">
          <li>
            <span>{{ copy.product }}</span
            >{{ tr(product.name) }}
          </li>
          <li>
            <span>{{ copy.lot }}</span
            >{{ product.lot }}
          </li>
          <li>
            <span>{{ copy.stages }}</span
            >{{ stages.length }} {{ copy.recorded }}
          </li>
          <li class="chain-history__demo-tag">{{ copy.demo }}</li>
        </ul>
      </section>

      <ol class="chain-timeline" :aria-label="copy.timeline">
        <li
          v-for="(stage, index) in stages"
          :key="stage.id"
          class="chain-stage"
          :class="[
            index % 2 === 0 ? 'chain-stage--left' : 'chain-stage--right',
            { 'chain-stage--current': !stage.period.endedAt },
          ]"
          :style="{ '--i': index }"
        >
          <article class="chain-stage__card">
            <div class="chain-stage__meta">
              <span class="chain-stage__step">{{ stepLabel(index) }}</span>
              <span class="chain-stage__status">
                {{ stage.period.endedAt ? copy.done : copy.current }}
              </span>
            </div>
            <h2 class="chain-stage__name">
              <RouterLink class="chain-stage__link" :to="stage.to">{{ tx(stage.name) }}</RouterLink>
            </h2>
            <p class="chain-stage__description">{{ tr(stage.description) }}</p>
            <p class="chain-stage__period">
              <time :datetime="stage.period.startedAt">
                {{ formatDate(stage.period.startedAt) }}
              </time>
              <span aria-hidden="true"> → </span>
              <time v-if="stage.period.endedAt" :datetime="stage.period.endedAt">
                {{ formatDate(stage.period.endedAt) }}
              </time>
              <span v-else>{{ copy.today }}</span>
              <span class="chain-stage__duration">{{ formatDuration(stage.period) }}</span>
            </p>
          </article>

          <span class="chain-stage__connector" aria-hidden="true"></span>

          <span class="chain-stage__node" aria-hidden="true">
            <svg viewBox="0 0 24 24" width="24" height="24">
              <template v-if="stage.icon === 'store'">
                <path d="M4 9.5 5.5 4h13L20 9.5" />
                <path d="M4 9.5a2.7 2.7 0 0 0 5.3 0 2.7 2.7 0 0 0 5.4 0 2.7 2.7 0 0 0 5.3 0" />
                <path d="M5.5 12v8h13v-8M10 20v-5h4v5" />
              </template>
              <template v-else-if="stage.icon === 'truck'">
                <path d="M2.5 6h11v10h-11zM13.5 9.5h4l3 3.5V16h-7" />
                <circle cx="7" cy="17.5" r="1.8" />
                <circle cx="17" cy="17.5" r="1.8" />
              </template>
              <template v-else-if="stage.icon === 'ship'">
                <path d="M3 15h18l-2.5 4.5h-13z" />
                <path d="M6 15V9h12v6M9 9V6h6v3M12 6V3.5" />
              </template>
              <template v-else-if="stage.icon === 'farm'">
                <path d="M3 20V11l6-5 6 5v9z" />
                <path d="M7 20v-5h4v5M15 13h6v7h-6M17 10l2-2 2 2" />
              </template>
              <template v-else>
                <path d="M3 20V10l5 3V10l5 3V5h3v15z" />
                <path d="M16 20h5V8h-5M7 17h2M12 17h2" />
              </template>
            </svg>
          </span>

          <p class="chain-stage__date" aria-hidden="true">
            <strong>{{ dayAndMonth(stage.period.startedAt) }}</strong>
            <span>{{ yearOf(stage.period.startedAt) }}</span>
          </p>
        </li>
      </ol>
    </div>
  </main>
</template>

<style scoped>
.chain-history {
  min-height: 100svh;
  background:
    radial-gradient(
      ellipse 60% 40% at 50% 0%,
      color-mix(in srgb, var(--primary) 14%, transparent),
      transparent 70%
    ),
    var(--canvas);
  color: var(--text);
}

.chain-history__frame {
  width: min(100% - 64px, 1600px);
  margin: 0 auto;
  padding: 28px 0 0;
}

/* Intro */
.chain-history__intro {
  display: grid;
  justify-items: center;
  max-width: 720px;
  margin: 0 auto;
  padding: clamp(40px, 8vh, 88px) 0 clamp(48px, 8vh, 80px);
  text-align: center;
}

.chain-history__eyebrow {
  margin: 0 0 18px;
  color: var(--proof);
  font: 700 11px/1.4 var(--font-mono);
  letter-spacing: 0.18em;
  text-transform: uppercase;
}

.chain-history__intro h1 {
  margin: 0;
  font-size: clamp(34px, 3.6vw, 56px);
  font-weight: 580;
  line-height: 1.04;
  letter-spacing: -0.045em;
  text-wrap: balance;
}

.chain-history__support {
  max-width: 520px;
  margin: 18px 0 0;
  color: color-mix(in srgb, var(--text) 72%, transparent);
  font-size: 16px;
  line-height: 1.6;
}

.chain-history__facts {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 8px;
  margin: 28px 0 0;
  padding: 0;
  list-style: none;
}

.chain-history__facts li {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: color-mix(in srgb, var(--surface) 80%, transparent);
  font: 600 12px/1 var(--font-mono);
}

.chain-history__facts li span {
  color: var(--muted);
  font-weight: 500;
}

.chain-history__facts .chain-history__demo-tag {
  border-color: color-mix(in srgb, var(--warning) 45%, var(--border));
  color: var(--warning);
}

/* Timeline: card | node | date, mirrored on alternating rows. */
.chain-timeline {
  --node: 56px;
  --row-gap: clamp(36px, 6vh, 64px);
  position: relative;
  display: grid;
  gap: var(--row-gap);
  max-width: 1100px;
  margin: 0 auto;
  padding: 0 0 120px;
  list-style: none;
}

.chain-timeline::before {
  content: '';
  position: absolute;
  top: 0;
  bottom: 0;
  left: 50%;
  width: 4px;
  border-radius: 2px;
  background: linear-gradient(
    to bottom,
    var(--proof) 0%,
    var(--chain) 45%,
    color-mix(in srgb, var(--primary) 70%, transparent) 80%,
    transparent 100%
  );
  transform: translateX(-50%);
}

.chain-stage {
  position: relative;
  display: grid;
  grid-template-columns: 1fr 48px var(--node) 48px 1fr;
  align-items: center;
  animation: stage-in 520ms cubic-bezier(0.2, 0.7, 0.2, 1) both;
  animation-delay: calc(var(--i) * 120ms + 80ms);
}

.chain-stage__node {
  z-index: 1;
  display: grid;
  grid-column: 3;
  grid-row: 1;
  place-items: center;
  width: var(--node);
  height: var(--node);
  border: 2px solid var(--border-strong);
  border-radius: 50%;
  background: var(--raised);
  color: var(--muted);
  box-shadow: 0 0 0 6px var(--canvas);
}

.chain-stage__node svg {
  fill: none;
  stroke: currentColor;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.chain-stage__connector {
  grid-row: 1;
  height: 2px;
  background: var(--border-strong);
}

.chain-stage__card {
  position: relative;
  grid-row: 1;
  display: grid;
  gap: 10px;
  padding: 20px 22px;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: color-mix(in srgb, var(--surface) 92%, transparent);
  box-shadow: 0 18px 40px -24px rgb(0 0 0 / 0.8);
  transition:
    border-color 150ms ease,
    transform 150ms ease;
}

.chain-stage__card:hover,
.chain-stage__card:focus-within {
  border-color: var(--border-strong);
  transform: translateY(-2px);
}

.chain-stage__date {
  grid-row: 1;
  display: grid;
  gap: 4px;
  margin: 0;
  font-family: var(--font-mono);
}

.chain-stage__date strong {
  font-size: clamp(22px, 2vw, 30px);
  font-weight: 700;
  letter-spacing: -0.02em;
}

.chain-stage__date span {
  color: var(--muted);
  font-size: 12px;
  letter-spacing: 0.12em;
}

.chain-stage--left .chain-stage__card {
  grid-column: 1;
}

.chain-stage--left .chain-stage__connector {
  grid-column: 2;
}

.chain-stage--left .chain-stage__date {
  grid-column: 5;
  justify-items: start;
  padding-left: 8px;
}

.chain-stage--right .chain-stage__card {
  grid-column: 5;
}

.chain-stage--right .chain-stage__connector {
  grid-column: 4;
}

.chain-stage--right .chain-stage__date {
  grid-column: 1;
  justify-items: end;
  padding-right: 8px;
  text-align: right;
}

.chain-stage__meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.chain-stage__step {
  color: var(--muted);
  font: 700 11px/1 var(--font-mono);
  letter-spacing: 0.14em;
  text-transform: uppercase;
}

.chain-stage__status {
  padding: 5px 9px;
  border: 1px solid var(--border);
  border-radius: 999px;
  color: var(--muted);
  font: 700 10px/1 var(--font-mono);
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

.chain-stage__name {
  margin: 0;
  font-size: clamp(20px, 1.7vw, 26px);
  font-weight: 750;
  line-height: 1.15;
  letter-spacing: 0.01em;
  text-transform: uppercase;
}

.chain-stage__name a {
  color: inherit;
  text-decoration: none;
}

/* The whole card opens the stage page. */
.chain-stage__link::after {
  content: '';
  position: absolute;
  inset: 0;
  border-radius: inherit;
}

.chain-stage__card:has(.chain-stage__link:focus-visible) {
  outline: 2px solid var(--proof);
  outline-offset: 2px;
}

.chain-stage__name a:hover,
.chain-stage__name a:focus-visible {
  color: var(--proof);
}

.chain-stage__description {
  margin: 0;
  color: color-mix(in srgb, var(--text) 70%, transparent);
  font-size: 14px;
  line-height: 1.55;
}

.chain-stage__period {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px 6px;
  margin: 4px 0 0;
  padding-top: 12px;
  border-top: 1px solid color-mix(in srgb, var(--border) 80%, transparent);
  color: var(--muted);
  font: 600 12px/1.3 var(--font-mono);
}

.chain-stage__duration {
  margin-left: auto;
  color: var(--text);
}

/* The stage that holds custody now. */
.chain-stage--current .chain-stage__card {
  border-color: color-mix(in srgb, var(--proof) 45%, var(--border));
  background: color-mix(in srgb, var(--proof) 6%, var(--surface));
}

.chain-stage--current .chain-stage__node {
  border-color: var(--proof);
  background: color-mix(in srgb, var(--proof) 16%, var(--raised));
  color: var(--proof);
  animation: node-pulse 2.4s ease-out infinite;
}

.chain-stage--current .chain-stage__connector {
  background: color-mix(in srgb, var(--proof) 70%, var(--border));
}

.chain-stage--current .chain-stage__status {
  border-color: color-mix(in srgb, var(--proof) 55%, var(--border));
  color: var(--proof);
}

.chain-stage--current .chain-stage__duration {
  color: var(--proof);
}

@keyframes stage-in {
  from {
    opacity: 0;
    transform: translateY(16px);
  }
}

@keyframes node-pulse {
  0% {
    box-shadow:
      0 0 0 6px var(--canvas),
      0 0 0 6px color-mix(in srgb, var(--proof) 50%, transparent);
  }
  100% {
    box-shadow:
      0 0 0 6px var(--canvas),
      0 0 0 22px transparent;
  }
}

/* Narrow screens: spine on the left, every card to its right, date inside the flow. */
@media (max-width: 768px) {
  .chain-history__frame {
    width: min(100% - 32px, 640px);
    padding-top: 20px;
  }

  .chain-history__intro {
    justify-items: start;
    text-align: left;
  }

  .chain-history__facts {
    justify-content: flex-start;
  }

  .chain-timeline {
    --node: 44px;
  }

  .chain-timeline::before {
    left: calc(var(--node) / 2);
  }

  .chain-stage {
    grid-template-columns: var(--node) 16px 1fr;
    grid-template-rows: auto auto;
    align-items: start;
    row-gap: 8px;
  }

  .chain-stage__node {
    grid-column: 1;
    grid-row: 1 / span 2;
    align-self: start;
  }

  .chain-stage--left .chain-stage__connector,
  .chain-stage--right .chain-stage__connector {
    grid-column: 2;
    grid-row: 1;
    align-self: center;
  }

  .chain-stage--left .chain-stage__date,
  .chain-stage--right .chain-stage__date {
    grid-column: 3;
    grid-row: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: var(--node);
    padding: 0;
    text-align: left;
  }

  .chain-stage__date strong {
    font-size: 18px;
  }

  .chain-stage--left .chain-stage__card,
  .chain-stage--right .chain-stage__card {
    grid-column: 3;
    grid-row: 2;
  }
}

@media (prefers-reduced-motion: reduce) {
  .chain-stage,
  .chain-stage--current .chain-stage__node {
    animation: none;
  }

  .chain-stage__card {
    transition: none;
  }
}
</style>

<script setup lang="ts">
const layers = [
  {
    phase: 'PROVEN TODAY',
    tone: 'proof',
    title: 'PHYSICAL IDENTITY',
    items: ['Asset', 'RFID evidence', 'Recovery', 'Custody'],
    description:
      'Bind signed physical evidence to a persistent AnimalID and enforce custody transitions.',
  },
  {
    phase: 'PROVEN TODAY',
    tone: 'chain',
    title: 'VERIFIABLE HISTORY',
    items: ['Signed events', 'Continuity', 'Canonical state', 'Independent verification'],
    description:
      'Carry identity and custody forward as a history that can be checked independently.',
  },
  {
    phase: 'EXPANSION PATH',
    tone: 'future',
    title: 'TRACEABILITY & COMPLIANCE',
    items: [
      'Supplier history',
      'Certification evidence',
      'Official-system interoperability',
      'Compliance evidence',
    ],
    description:
      'More participants can attach evidence around the same persistent physical history.',
  },
  {
    phase: 'EXPANSION PATH',
    tone: 'future',
    title: 'RISK INFRASTRUCTURE',
    items: ['Underwriting inputs', 'Insurance', 'Financing', 'Asset risk assessment'],
    description:
      'Verified history can become an input to risk models without pretending Lastro is the risk model.',
  },
  {
    phase: 'EXPANSION PATH',
    tone: 'future',
    title: 'FINANCIAL INFRASTRUCTURE',
    items: ['Collateral', 'Guarantees', 'Securitization', 'RWA infrastructure'],
    description:
      'Financial structures can reference physical assets only after identity, control and history are dependable.',
  },
] as const
</script>

<template>
  <div class="trust-stack-diagram">
    <article
      v-for="(layer, index) in layers"
      :key="layer.title"
      class="trust-stack-diagram__layer"
      :data-tone="layer.tone"
    >
      <div class="trust-stack-diagram__meta">
        <span class="trust-stack-diagram__phase">{{ layer.phase }}</span>
        <span class="trust-stack-diagram__number">0{{ index + 1 }}</span>
      </div>
      <div class="trust-stack-diagram__body">
        <h3>{{ layer.title }}</h3>
        <p>{{ layer.description }}</p>
      </div>
      <ul>
        <li v-for="item in layer.items" :key="item">{{ item }}</li>
      </ul>
    </article>
  </div>
</template>

<style scoped>
.trust-stack-diagram {
  display: grid;
  gap: 8px;
  width: min(100%, 1180px);
  margin-left: auto;
}

.trust-stack-diagram__layer {
  display: grid;
  grid-template-columns: 170px minmax(280px, 0.8fr) minmax(0, 1fr);
  gap: 28px;
  align-items: center;
  min-height: 132px;
  padding: 22px 26px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: color-mix(in srgb, var(--surface) 88%, transparent);
}

.trust-stack-diagram__layer[data-tone='future'] {
  border-color: color-mix(in srgb, var(--border) 76%, transparent);
}

.trust-stack-diagram__layer[data-tone='chain'] {
  border-color: color-mix(in srgb, var(--chain) 46%, var(--border));
  background: color-mix(in srgb, var(--chain) 5%, var(--surface));
}

.trust-stack-diagram__layer[data-tone='proof'] {
  border-color: color-mix(in srgb, var(--proof) 48%, var(--border));
  background: color-mix(in srgb, var(--proof) 5%, var(--surface));
}

.trust-stack-diagram__meta {
  display: grid;
  gap: 10px;
}

.trust-stack-diagram__phase {
  color: var(--muted);
  font: 700 9px/1.3 var(--font-mono);
  letter-spacing: 0.12em;
}

.trust-stack-diagram__layer[data-tone='proof'] .trust-stack-diagram__phase {
  color: var(--proof);
}

.trust-stack-diagram__layer[data-tone='chain'] .trust-stack-diagram__phase {
  color: var(--chain);
}

.trust-stack-diagram__number {
  color: color-mix(in srgb, var(--muted) 55%, transparent);
  font: 700 20px/1 var(--font-mono);
}

.trust-stack-diagram__body h3 {
  margin: 0;
  font: 700 14px/1.3 var(--font-mono);
  letter-spacing: 0.08em;
}

.trust-stack-diagram__body p {
  max-width: 46ch;
  margin: 8px 0 0;
  color: var(--muted);
  font-size: 12px;
  line-height: 1.55;
}

.trust-stack-diagram__layer ul {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px 18px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.trust-stack-diagram__layer li {
  position: relative;
  padding-left: 13px;
  color: color-mix(in srgb, var(--text) 86%, transparent);
  font-size: 12px;
}

.trust-stack-diagram__layer li::before {
  content: '';
  position: absolute;
  top: 0.55em;
  left: 0;
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: var(--border-strong);
}

.trust-stack-diagram__layer[data-tone='proof'] li::before {
  background: var(--proof);
}

.trust-stack-diagram__layer[data-tone='chain'] li::before {
  background: var(--chain);
}

@media (max-width: 1000px) {
  .trust-stack-diagram__layer {
    grid-template-columns: 130px minmax(0, 1fr);
  }

  .trust-stack-diagram__layer ul {
    grid-column: 2;
  }
}

@media (max-width: 700px) {
  .trust-stack-diagram__layer {
    grid-template-columns: 1fr;
    gap: 16px;
    padding: 20px;
  }

  .trust-stack-diagram__meta {
    grid-template-columns: 1fr auto;
  }

  .trust-stack-diagram__layer ul {
    grid-column: auto;
    grid-template-columns: 1fr;
  }
}
</style>

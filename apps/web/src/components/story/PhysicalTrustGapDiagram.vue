<script setup lang="ts">
const participants = [
  'Producer',
  'Trader',
  'Transport',
  'Processor',
  'Exporter',
  'Importer / Market',
] as const

const evidenceStores = [
  'Private systems',
  'Government systems',
  'Certification systems',
  'Spreadsheets & documents',
] as const
</script>

<template>
  <div class="trust-gap">
    <div class="trust-gap__asset">
      <span class="trust-gap__asset-mark" aria-hidden="true">ID</span>
      <div>
        <strong>PHYSICAL ASSET</strong>
        <span>Identity persists in the real world.</span>
      </div>
    </div>

    <ol class="trust-gap__chain" aria-label="Example supply-chain participants">
      <li v-for="(participant, index) in participants" :key="participant">
        <span class="trust-gap__index">0{{ index + 1 }}</span>
        <strong>{{ participant }}</strong>
        <span v-if="index < participants.length - 1" class="trust-gap__arrow" aria-hidden="true">
          ↓
        </span>
      </li>
    </ol>

    <div class="trust-gap__fragmentation">
      <p>EVIDENCE FRAGMENTS ACROSS</p>
      <div class="trust-gap__stores">
        <span v-for="store in evidenceStores" :key="store">{{ store }}</span>
      </div>
      <div class="trust-gap__result">
        <span aria-hidden="true"></span>
        <strong>NO SHARED VERIFIABLE HISTORY</strong>
      </div>
    </div>
  </div>
</template>

<style scoped>
.trust-gap {
  display: grid;
  grid-template-columns: .72fr 1fr 1.08fr;
  gap: 22px;
  align-items: stretch;
}

.trust-gap__asset,
.trust-gap__fragmentation,
.trust-gap__chain {
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: color-mix(in srgb, var(--surface) 82%, transparent);
}

.trust-gap__asset {
  display: grid;
  place-items: center;
  align-content: center;
  min-height: 480px;
  padding: 32px;
  text-align: center;
}

.trust-gap__asset-mark {
  display: grid;
  place-items: center;
  width: 104px;
  height: 104px;
  margin-bottom: 22px;
  border: 1px solid color-mix(in srgb, var(--proof) 65%, var(--border));
  border-radius: 50%;
  color: var(--proof);
  box-shadow: inset 0 0 0 18px color-mix(in srgb, var(--proof) 5%, transparent);
  font: 750 21px/1 var(--font-mono);
  letter-spacing: .1em;
}

.trust-gap__asset strong,
.trust-gap__asset span {
  display: block;
}

.trust-gap__asset strong {
  font: 700 12px/1.2 var(--font-mono);
  letter-spacing: .12em;
}

.trust-gap__asset div span {
  max-width: 24ch;
  margin-top: 8px;
  color: var(--muted);
  font-size: 13px;
}

.trust-gap__chain {
  display: grid;
  margin: 0;
  padding: 22px 28px;
  list-style: none;
}

.trust-gap__chain li {
  position: relative;
  display: grid;
  grid-template-columns: 36px 1fr;
  gap: 12px;
  align-items: center;
  min-height: 68px;
  border-bottom: 1px solid color-mix(in srgb, var(--border) 72%, transparent);
}

.trust-gap__chain li:last-child {
  border-bottom: 0;
}

.trust-gap__index {
  color: var(--chain);
  font: 700 10px/1 var(--font-mono);
}

.trust-gap__chain strong {
  font-size: 14px;
  font-weight: 650;
}

.trust-gap__arrow {
  position: absolute;
  right: 2px;
  bottom: -11px;
  z-index: 1;
  color: var(--muted);
  background: var(--surface);
  font: 700 13px/1 var(--font-mono);
}

.trust-gap__fragmentation {
  display: flex;
  flex-direction: column;
  min-height: 480px;
  padding: 28px;
}

.trust-gap__fragmentation > p {
  margin: 0;
  color: var(--muted);
  font: 700 10px/1.3 var(--font-mono);
  letter-spacing: .12em;
}

.trust-gap__stores {
  display: grid;
  gap: 10px;
  margin-top: 28px;
}

.trust-gap__stores span {
  padding: 14px 15px;
  border: 1px solid var(--border);
  border-left: 2px solid var(--warning);
  border-radius: var(--radius-sm);
  background: #0d1218;
  color: color-mix(in srgb, var(--text) 88%, transparent);
  font-size: 13px;
}

.trust-gap__result {
  margin-top: auto;
  padding-top: 30px;
}

.trust-gap__result > span {
  display: block;
  width: 100%;
  height: 1px;
  margin-bottom: 20px;
  background: linear-gradient(90deg, var(--warning), transparent);
}

.trust-gap__result strong {
  color: #ffd36a;
  font: 700 12px/1.4 var(--font-mono);
  letter-spacing: .08em;
}

@media (max-width: 980px) {
  .trust-gap {
    grid-template-columns: 1fr 1fr;
  }

  .trust-gap__asset {
    min-height: 300px;
  }

  .trust-gap__fragmentation {
    grid-column: 1 / -1;
    min-height: 0;
  }
}

@media (max-width: 700px) {
  .trust-gap {
    grid-template-columns: 1fr;
  }

  .trust-gap__asset {
    min-height: 240px;
  }

  .trust-gap__fragmentation {
    grid-column: auto;
  }
}

@media (max-width: 430px) {
  .trust-gap__fragmentation {
    padding: 20px;
  }
}
</style>

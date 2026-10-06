<script setup lang="ts">
import { useCopy } from '../../i18n'

type Tone = 'proof' | 'chain' | 'partial' | 'future'

interface Layer {
  phase: string
  tone: Tone
  title: string
  /** `proven` marks items the current software already demonstrates inside a partial layer. */
  items: { label: string; proven?: boolean }[]
  description: string
}

const PROVEN = { en: 'PROVEN TODAY', pt: 'PROVADO HOJE' }
const PARTIAL = { en: 'PARTLY PROVEN', pt: 'PARCIALMENTE PROVADO' }
const EXPANSION = { en: 'EXPANSION PATH', pt: 'CAMINHO DE EXPANSÃO' }

const layers = useCopy<{ layers: Layer[]; provenLegend: string }>({
  en: {
    provenLegend: 'Already in the software',
    layers: [
      {
        phase: PROVEN.en,
        tone: 'proof',
        title: 'PHYSICAL IDENTITY',
        items: [
          { label: 'Asset' },
          { label: 'RFID evidence' },
          { label: 'Re-identification' },
          { label: 'Custody' },
        ],
        description:
          'Bind signed physical evidence to a persistent AssetID and enforce custody transitions.',
      },
      {
        phase: PROVEN.en,
        tone: 'chain',
        title: 'VERIFIABLE HISTORY',
        items: [
          { label: 'Signed events' },
          { label: 'Continuity' },
          { label: 'Canonical state' },
          { label: 'Independent verification' },
        ],
        description:
          'Carry identity and custody forward as a history that can be checked independently.',
      },
      {
        phase: PARTIAL.en,
        tone: 'partial',
        title: 'TRACEABILITY & COMPLIANCE',
        items: [
          { label: 'Processing lineage (animal → cuts)', proven: true },
          { label: 'Registered participants & facilities', proven: true },
          { label: 'Certification evidence' },
          { label: 'Official-system interoperability' },
          { label: 'Geolocated origin' },
          { label: 'Compliance evidence' },
        ],
        description:
          'Processing already records which cuts came from which animal. Certifiers, official systems and compliance evidence come next.',
      },
      {
        phase: EXPANSION.en,
        tone: 'future',
        title: 'RISK INFRASTRUCTURE',
        items: [
          { label: 'Underwriting inputs' },
          { label: 'Insurance' },
          { label: 'Financing' },
          { label: 'Asset risk assessment' },
        ],
        description:
          'Verified history can become an input to risk models without pretending Lastro is the risk model.',
      },
      {
        phase: EXPANSION.en,
        tone: 'future',
        title: 'FINANCIAL INFRASTRUCTURE',
        items: [
          { label: 'Collateral' },
          { label: 'Guarantees' },
          { label: 'Securitization' },
          { label: 'RWA infrastructure' },
        ],
        description:
          'Financial structures can reference physical assets only after identity, control and history are dependable.',
      },
    ],
  },
  pt: {
    provenLegend: 'Já existe no software',
    layers: [
      {
        phase: PROVEN.pt,
        tone: 'proof',
        title: 'IDENTIDADE FÍSICA',
        items: [
          { label: 'Ativo' },
          { label: 'Evidência RFID' },
          { label: 'Reidentificação' },
          { label: 'Custódia' },
        ],
        description:
          'Liga a evidência física assinada a um AssetID persistente e controla as transferências de custódia.',
      },
      {
        phase: PROVEN.pt,
        tone: 'chain',
        title: 'HISTÓRICO VERIFICÁVEL',
        items: [
          { label: 'Eventos assinados' },
          { label: 'Continuidade' },
          { label: 'Estado canônico' },
          { label: 'Verificação independente' },
        ],
        description:
          'Leva identidade e custódia adiante como um histórico que qualquer pessoa pode conferir.',
      },
      {
        phase: PARTIAL.pt,
        tone: 'partial',
        title: 'RASTREABILIDADE E CONFORMIDADE',
        items: [
          { label: 'Linhagem do processamento (animal → cortes)', proven: true },
          { label: 'Participantes e instalações cadastrados', proven: true },
          { label: 'Evidência de certificação' },
          { label: 'Integração com sistemas oficiais' },
          { label: 'Origem georreferenciada' },
          { label: 'Evidência de conformidade' },
        ],
        description:
          'O processamento já registra quais cortes vieram de qual animal. Certificadoras, sistemas oficiais e evidência de conformidade vêm a seguir.',
      },
      {
        phase: EXPANSION.pt,
        tone: 'future',
        title: 'INFRAESTRUTURA DE RISCO',
        items: [
          { label: 'Dados para subscrição' },
          { label: 'Seguro' },
          { label: 'Financiamento' },
          { label: 'Avaliação de risco do ativo' },
        ],
        description:
          'O histórico verificado pode alimentar modelos de risco, sem o Lastro fingir ser o modelo de risco.',
      },
      {
        phase: EXPANSION.pt,
        tone: 'future',
        title: 'INFRAESTRUTURA FINANCEIRA',
        items: [
          { label: 'Garantia' },
          { label: 'Fianças' },
          { label: 'Securitização' },
          { label: 'Infraestrutura RWA' },
        ],
        description:
          'Estruturas financeiras só podem se apoiar em ativos físicos depois que identidade, controle e histórico forem confiáveis.',
      },
    ],
  },
})
</script>

<template>
  <div class="trust-stack-diagram">
    <article
      v-for="(layer, index) in layers.layers"
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
        <li
          v-for="item in layer.items"
          :key="item.label"
          :class="{ 'is-proven': item.proven }"
          :title="item.proven ? layers.provenLegend : undefined"
        >
          {{ item.label }}
        </li>
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

.trust-stack-diagram__layer[data-tone='partial'] {
  border-color: color-mix(in srgb, var(--proof) 26%, var(--border));
  background: linear-gradient(
    90deg,
    color-mix(in srgb, var(--proof) 4%, var(--surface)),
    color-mix(in srgb, var(--surface) 88%, transparent)
  );
}

.trust-stack-diagram__layer[data-tone='partial'] .trust-stack-diagram__phase {
  color: color-mix(in srgb, var(--proof) 75%, var(--muted));
}

.trust-stack-diagram__layer li.is-proven {
  color: var(--text);
}

.trust-stack-diagram__layer li.is-proven::before {
  background: var(--proof);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--proof) 22%, transparent);
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

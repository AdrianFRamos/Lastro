<script setup lang="ts">
import PhysicalTrustGapDiagram from '../components/story/PhysicalTrustGapDiagram.vue'
import StoryHeader from '../components/story/StoryHeader.vue'
import StoryPageFooter from '../components/story/StoryPageFooter.vue'
import StorySectionHeading from '../components/story/StorySectionHeading.vue'
import { useCopy } from '../i18n'

const copy = useCopy({
  en: {
    kicker: 'THE PHYSICAL TRUST GAP',
    title: 'The asset moves. The data fragments. The market still needs proof.',
    lede: 'Physical supply chains depend on evidence of identity, origin, custody, history and compliance while the underlying asset moves across organizations, systems and jurisdictions.',
    movementKicker: 'CURRENT WORLD',
    movementTitle: 'One physical asset. Many institutional handoffs.',
    evidenceKicker: 'WHY THIS MATTERS',
    evidenceTitle: 'Traceability is becoming infrastructure, not paperwork.',
    brazilType: 'FACT / BRAZIL',
    brazilTitle:
      'Brazil is moving from lot-based traceability toward individual cattle and buffalo identification.',
    brazilBody:
      "MAPA's PNIB rolls out in four phases from July 2025 to the end of 2032. From 1 January 2033, cattle and buffalo that are not individually identified and registered in the official system can no longer be moved.",
    brazilSource: 'Source · MAPA / PNIB ↗',
    brazilSchedule: 'Schedule · Portaria SDA/MAPA 1.331/2025 ↗',
    euType: 'FACT / EUROPEAN UNION',
    euTitle: 'Cattle are within the scope of the EU Deforestation Regulation.',
    euBody:
      'Operators and traders placing covered commodities on the EU market must be able to show that the products do not originate from recently deforested land or contribute to forest degradation. Current application dates begin 30 December 2026 for large and medium operators.',
    euSource: 'Source · European Commission ↗',
    woahType: 'FACT / INTERNATIONAL STANDARD',
    woahTitle:
      'Animal identification and traceability support health, food safety, certification and trade.',
    woahBody:
      'WOAH treats identification and traceability as tools that can strengthen disease response, movement controls, inspection, certification and fair trade practices.',
    woahSource: 'Source · WOAH Terrestrial Code ↗',
    thesisKicker: 'LASTRO THESIS',
    thesisTitle: 'Move from reconciliation toward independently verifiable continuity.',
    currentLabel: 'CURRENT WORLD',
    current: [
      'Physical reality',
      'Multiple organizations',
      'Fragmented records',
      'Reconciliation',
      'Trust',
    ],
    lastroLabel: 'LASTRO PRIMITIVE',
    lastro: [
      'Physical event',
      'Signed evidence',
      'Persistent identity',
      'Canonical history',
      'Independent verification',
    ],
    note: 'Lastro does not claim to replace official traceability or compliance systems. The thesis is narrower: verifiable identity and custody can become a foundation those architectures can reference.',
    footerTitle: 'See how the primitive can expand without confusing vision with proof.',
    footerLink: 'Explore The Future',
  },
  pt: {
    kicker: 'A LACUNA DE CONFIANÇA FÍSICA',
    title: 'O ativo se move. Os dados se fragmentam. O mercado ainda precisa de prova.',
    lede: 'Cadeias físicas dependem de evidências de identidade, origem, custódia, histórico e conformidade, enquanto o ativo passa por empresas, sistemas e jurisdições diferentes.',
    movementKicker: 'MUNDO ATUAL',
    movementTitle: 'Um ativo físico. Muitas passagens entre instituições.',
    evidenceKicker: 'POR QUE ISSO IMPORTA',
    evidenceTitle: 'Rastreabilidade está virando infraestrutura, não papelada.',
    brazilType: 'FATO / BRASIL',
    brazilTitle:
      'O Brasil está saindo da rastreabilidade por lote para a identificação individual de bovinos e búfalos.',
    brazilBody:
      'O PNIB, do MAPA, é implantado em quatro fases, de julho de 2025 até o fim de 2032. A partir de 1º de janeiro de 2033, bovinos e búfalos sem identificação individual e registro no sistema oficial não poderão mais ser movimentados.',
    brazilSource: 'Fonte · MAPA / PNIB ↗',
    brazilSchedule: 'Cronograma · Portaria SDA/MAPA 1.331/2025 ↗',
    euType: 'FATO / UNIÃO EUROPEIA',
    euTitle: 'O gado está no escopo do Regulamento Europeu Antidesmatamento (EUDR).',
    euBody:
      'Quem coloca esses produtos no mercado europeu precisa demonstrar que eles não vêm de áreas desmatadas recentemente nem contribuem para a degradação florestal. A aplicação começa em 30 de dezembro de 2026 para empresas grandes e médias.',
    euSource: 'Fonte · Comissão Europeia ↗',
    woahType: 'FATO / PADRÃO INTERNACIONAL',
    woahTitle:
      'Identificação e rastreabilidade animal apoiam sanidade, segurança dos alimentos, certificação e comércio.',
    woahBody:
      'A WOAH trata identificação e rastreabilidade como ferramentas que fortalecem a resposta a doenças, o controle de trânsito, a inspeção, a certificação e práticas comerciais justas.',
    woahSource: 'Fonte · Código Terrestre da WOAH ↗',
    thesisKicker: 'TESE DO LASTRO',
    thesisTitle: 'Sair da conciliação para uma continuidade verificável por qualquer um.',
    currentLabel: 'MUNDO ATUAL',
    current: [
      'Realidade física',
      'Várias organizações',
      'Registros fragmentados',
      'Conciliação',
      'Confiança',
    ],
    lastroLabel: 'PRIMITIVA LASTRO',
    lastro: [
      'Evento físico',
      'Evidência assinada',
      'Identidade persistente',
      'Histórico canônico',
      'Verificação independente',
    ],
    note: 'O Lastro não pretende substituir os sistemas oficiais de rastreabilidade ou conformidade. A tese é mais estreita: identidade e custódia verificáveis podem virar uma base que esses sistemas consigam referenciar.',
    footerTitle: 'Veja como a primitiva pode crescer sem confundir visão com prova.',
    footerLink: 'Explorar O Futuro',
  },
})
</script>

<template>
  <main class="story-page">
    <div class="story-page__frame">
      <StoryHeader active="problem" />

      <section class="problem-hero" aria-labelledby="problem-title">
        <div>
          <p class="story-kicker">{{ copy.kicker }}</p>
          <h1 id="problem-title">{{ copy.title }}</h1>
        </div>
        <p class="problem-hero__lede">{{ copy.lede }}</p>
      </section>

      <section class="movement" aria-labelledby="movement-title">
        <StorySectionHeading
          :kicker="copy.movementKicker"
          :title="copy.movementTitle"
          heading-id="movement-title"
        />

        <PhysicalTrustGapDiagram />
      </section>

      <section class="evidence" aria-labelledby="evidence-title">
        <StorySectionHeading
          :kicker="copy.evidenceKicker"
          :title="copy.evidenceTitle"
          heading-id="evidence-title"
        />

        <div class="evidence__grid">
          <article class="fact-block">
            <span class="fact-block__type">{{ copy.brazilType }}</span>
            <h3>{{ copy.brazilTitle }}</h3>
            <p>{{ copy.brazilBody }}</p>
            <a
              href="https://www.gov.br/agricultura/pt-br/assuntos/sanidade-animal-e-vegetal/saude-animal/rastreabilidade-animal/pnib"
              target="_blank"
              rel="noreferrer"
            >
              {{ copy.brazilSource }}
            </a>
            <a
              href="https://www.in.gov.br/en/web/dou/-/portaria-sda/mapa-n-1.331-de-21-de-julho-de-2025-643581903"
              target="_blank"
              rel="noreferrer"
            >
              {{ copy.brazilSchedule }}
            </a>
          </article>

          <article class="fact-block">
            <span class="fact-block__type">{{ copy.euType }}</span>
            <h3>{{ copy.euTitle }}</h3>
            <p>{{ copy.euBody }}</p>
            <a
              href="https://environment.ec.europa.eu/topics/forests/deforestation/regulation-deforestation-free-products_en"
              target="_blank"
              rel="noreferrer"
            >
              {{ copy.euSource }}
            </a>
          </article>

          <article class="fact-block">
            <span class="fact-block__type">{{ copy.woahType }}</span>
            <h3>{{ copy.woahTitle }}</h3>
            <p>{{ copy.woahBody }}</p>
            <a
              href="https://www.woah.org/fileadmin/Home/eng/Health_standards/tahc/current/en_chapitre_ident_traceability.htm"
              target="_blank"
              rel="noreferrer"
            >
              {{ copy.woahSource }}
            </a>
          </article>
        </div>
      </section>

      <section class="comparison" aria-labelledby="comparison-title">
        <StorySectionHeading
          :kicker="copy.thesisKicker"
          :title="copy.thesisTitle"
          heading-id="comparison-title"
        />

        <div class="comparison__grid">
          <article class="flow-panel flow-panel--current">
            <span class="flow-panel__label">{{ copy.currentLabel }}</span>
            <ol>
              <li v-for="step in copy.current" :key="step">{{ step }}</li>
            </ol>
          </article>

          <div class="comparison__bridge" aria-hidden="true">→</div>

          <article class="flow-panel flow-panel--lastro">
            <span class="flow-panel__label">{{ copy.lastroLabel }}</span>
            <ol>
              <li v-for="step in copy.lastro" :key="step">{{ step }}</li>
            </ol>
          </article>
        </div>

        <p class="comparison__note">{{ copy.note }}</p>
      </section>

      <StoryPageFooter :title="copy.footerTitle" to="/future" :link-label="copy.footerLink" />
    </div>
  </main>
</template>

<style scoped>
.story-page {
  min-height: 100vh;
  background:
    linear-gradient(to bottom, color-mix(in srgb, var(--proof) 4%, transparent), transparent 18rem),
    var(--canvas);
  color: var(--text);
}

.story-page__frame {
  width: min(100% - 64px, 1480px);
  margin: 0 auto;
}

.story-kicker {
  margin: 0 0 12px;
  color: var(--proof);
  font: 700 10px/1.3 var(--font-mono);
  letter-spacing: 0.16em;
}

.problem-hero {
  display: grid;
  grid-template-columns: minmax(0, 1.2fr) minmax(280px, 0.7fr);
  gap: 8vw;
  align-items: end;
  padding: 72px 0 64px;
  border-bottom: 1px solid var(--border);
}

.problem-hero h1 {
  max-width: 14ch;
  margin: 0;
  font-size: clamp(44px, 4.7vw, 68px);
  font-weight: 580;
  line-height: 0.99;
  letter-spacing: -0.052em;
}

.problem-hero__lede {
  max-width: 48ch;
  margin: 0 0 8px;
  color: var(--muted);
  font-size: 17px;
  line-height: 1.65;
}

.movement,
.evidence,
.comparison {
  padding: 64px 0;
  border-bottom: 1px solid var(--border);
}

.evidence__grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 16px;
}

.fact-block {
  display: flex;
  flex-direction: column;
  min-height: 360px;
  padding: 24px;
  border-top: 2px solid var(--chain);
  border-right: 1px solid var(--border);
  border-bottom: 1px solid var(--border);
  border-left: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
}

.fact-block h3 {
  margin: 22px 0 0;
  font-size: 21px;
  line-height: 1.25;
  letter-spacing: -0.02em;
}

.fact-block p {
  margin: 16px 0 24px;
  color: var(--muted);
  font-size: 14px;
  line-height: 1.65;
}

.fact-block a {
  display: inline-block;
  margin-top: auto;
  color: var(--chain);
  font: 650 11px/1.4 var(--font-mono);
  text-underline-offset: 4px;
}

.fact-block a + a {
  margin-top: 8px;
}

.comparison__grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 70px minmax(0, 1fr);
  gap: 18px;
  align-items: center;
}

.flow-panel {
  padding: 28px;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
}

.flow-panel--lastro {
  border-color: color-mix(in srgb, var(--proof) 42%, var(--border));
}

.flow-panel ol {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 0;
  margin: 26px 0 0;
  padding: 0;
  list-style: none;
  counter-reset: flow;
}

.flow-panel li {
  position: relative;
  min-height: 90px;
  padding: 0 16px 0 0;
  color: var(--text);
  font-size: 13px;
}

.flow-panel li:not(:last-child)::after {
  content: '→';
  position: absolute;
  right: 7px;
  top: 0;
  color: var(--muted);
}

.comparison__bridge {
  color: var(--primary);
  font: 700 30px/1 var(--font-mono);
  text-align: center;
}

.comparison__note {
  max-width: 78ch;
  margin: 28px 0 0;
  color: var(--muted);
  font-size: 13px;
  line-height: 1.65;
}

@media (max-width: 980px) {
  .problem-hero {
    grid-template-columns: 1fr;
    gap: 24px;
  }

  .evidence__grid {
    grid-template-columns: 1fr;
  }

  .fact-block {
    min-height: 0;
  }

  .comparison__grid {
    grid-template-columns: 1fr;
  }

  .comparison__bridge {
    transform: rotate(90deg);
  }
}

@media (max-width: 700px) {
  .story-page__frame {
    width: min(100% - 32px, 1480px);
  }

  .problem-hero {
    padding: 48px 0 44px;
  }

  .problem-hero h1 {
    max-width: 100%;
    font-size: clamp(38px, 11vw, 50px);
  }

  .movement,
  .evidence,
  .comparison {
    padding: 48px 0;
  }

  .flow-panel ol {
    grid-template-columns: 1fr;
  }

  .flow-panel li {
    min-height: 52px;
    padding: 0 0 24px 22px;
    border-left: 1px solid var(--border-strong);
  }

  .flow-panel li:not(:last-child)::after {
    content: '↓';
    left: -5px;
    right: auto;
    top: 29px;
    background: var(--surface);
  }
}

@media (min-width: 1200px) and (min-aspect-ratio: 16 / 10) {
  .problem-hero {
    padding-top: 64px;
    padding-bottom: 58px;
  }

  .movement,
  .evidence,
  .comparison {
    padding-block: 58px;
  }
}

@media (max-width: 430px) {
  .story-page__frame {
    width: min(100% - 24px, 1480px);
  }

  .flow-panel,
  .fact-block {
    padding: 20px;
  }
}
</style>

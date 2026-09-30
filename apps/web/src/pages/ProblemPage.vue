<script setup lang="ts">
import PhysicalTrustGapDiagram from '../components/story/PhysicalTrustGapDiagram.vue'
import StoryHeader from '../components/story/StoryHeader.vue'
import StoryPageFooter from '../components/story/StoryPageFooter.vue'
import StorySectionHeading from '../components/story/StorySectionHeading.vue'
</script>

<template>
  <main class="story-page">
    <div class="story-page__frame">
      <StoryHeader active="problem" />

      <section class="problem-hero" aria-labelledby="problem-title">
        <div>
          <p class="story-kicker">THE PHYSICAL TRUST GAP</p>
          <h1 id="problem-title">
            The asset moves. The data fragments. The market still needs proof.
          </h1>
        </div>
        <p class="problem-hero__lede">
          Physical supply chains depend on evidence of identity, origin, custody, history and
          compliance while the underlying asset moves across organizations, systems and
          jurisdictions.
        </p>
      </section>

      <section class="movement" aria-labelledby="movement-title">
        <StorySectionHeading
          kicker="CURRENT WORLD"
          title="One physical asset. Many institutional handoffs."
          heading-id="movement-title"
        />

        <PhysicalTrustGapDiagram />
      </section>

      <section class="evidence" aria-labelledby="evidence-title">
        <StorySectionHeading
          kicker="WHY THIS MATTERS"
          title="Traceability is becoming infrastructure, not paperwork."
          heading-id="evidence-title"
        />

        <div class="evidence__grid">
          <article class="fact-block">
            <span class="fact-block__type">FACT / BRAZIL</span>
            <h3>
              Brazil is moving from lot-based traceability toward individual cattle and buffalo
              identification.
            </h3>
            <p>
              MAPA's PNIB plan describes a gradual transition to individual identification, with a
              2025–2032 implementation horizon.
            </p>
            <a
              href="https://www.gov.br/agricultura/pt-br/assuntos/sanidade-animal-e-vegetal/saude-animal/rastreabilidade-animal/pnib"
              target="_blank"
              rel="noreferrer"
            >
              Source · MAPA / PNIB ↗
            </a>
          </article>

          <article class="fact-block">
            <span class="fact-block__type">FACT / EUROPEAN UNION</span>
            <h3>Cattle are within the scope of the EU Deforestation Regulation.</h3>
            <p>
              Operators and traders placing covered commodities on the EU market must be able to
              show that the products do not originate from recently deforested land or contribute to
              forest degradation. Current application dates begin 30 December 2026 for large and
              medium operators.
            </p>
            <a
              href="https://environment.ec.europa.eu/topics/forests/deforestation/regulation-deforestation-free-products_en"
              target="_blank"
              rel="noreferrer"
            >
              Source · European Commission ↗
            </a>
          </article>

          <article class="fact-block">
            <span class="fact-block__type">FACT / INTERNATIONAL STANDARD</span>
            <h3>
              Animal identification and traceability support health, food safety, certification and
              trade.
            </h3>
            <p>
              WOAH treats identification and traceability as tools that can strengthen disease
              response, movement controls, inspection, certification and fair trade practices.
            </p>
            <a
              href="https://www.woah.org/fileadmin/Home/eng/Health_standards/tahc/current/en_chapitre_ident_traceability.htm"
              target="_blank"
              rel="noreferrer"
            >
              Source · WOAH Terrestrial Code ↗
            </a>
          </article>
        </div>
      </section>

      <section class="comparison" aria-labelledby="comparison-title">
        <StorySectionHeading
          kicker="LASTRO THESIS"
          title="Move from reconciliation toward independently verifiable continuity."
          heading-id="comparison-title"
        />

        <div class="comparison__grid">
          <article class="flow-panel flow-panel--current">
            <span class="flow-panel__label">CURRENT WORLD</span>
            <ol>
              <li>Physical reality</li>
              <li>Multiple organizations</li>
              <li>Fragmented records</li>
              <li>Reconciliation</li>
              <li>Trust</li>
            </ol>
          </article>

          <div class="comparison__bridge" aria-hidden="true">→</div>

          <article class="flow-panel flow-panel--lastro">
            <span class="flow-panel__label">LASTRO PRIMITIVE</span>
            <ol>
              <li>Physical event</li>
              <li>Signed evidence</li>
              <li>Persistent identity</li>
              <li>Canonical history</li>
              <li>Independent verification</li>
            </ol>
          </article>
        </div>

        <p class="comparison__note">
          Lastro does not claim to replace official traceability or compliance systems. The thesis
          is narrower: verifiable identity and custody can become a foundation those architectures
          can reference.
        </p>
      </section>

      <StoryPageFooter
        title="See how the primitive can expand without confusing vision with proof."
        to="/future"
        link-label="Explore The Future"
      />
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

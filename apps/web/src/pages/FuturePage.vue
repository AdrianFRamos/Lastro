<script setup lang="ts">
import LastroTrustStack from '../components/story/LastroTrustStack.vue'
import StoryHeader from '../components/story/StoryHeader.vue'
import StoryPageFooter from '../components/story/StoryPageFooter.vue'
import StorySectionHeading from '../components/story/StorySectionHeading.vue'
</script>

<template>
  <main class="future-page">
    <div class="future-page__frame">
      <StoryHeader active="future" />

      <section class="future-hero" aria-labelledby="future-title">
        <div>
          <p class="future-kicker">LASTRO TRUST STACK</p>
          <h1 id="future-title">Start with what the physical asset is. Build upward from proof.</h1>
        </div>
        <div class="future-hero__thesis">
          <span>LASTRO THESIS</span>
          <p>
            You cannot build trusted financial infrastructure around a physical asset before you can
            prove what it is, who controls it, and what happened to it.
          </p>
          <strong>Lastro starts with identity and custody.</strong>
        </div>
      </section>

      <section class="trust-stack" aria-labelledby="stack-title">
        <StorySectionHeading
          kicker="ARCHITECTURE"
          title="Proof at the base. Optional systems above it."
          heading-id="stack-title"
          description="Only the first two layers are demonstrated by the current hackathon core. Everything above them is an expansion path, not implemented functionality."
        />

        <LastroTrustStack />
      </section>

      <section class="expansion" aria-labelledby="expansion-title">
        <StorySectionHeading
          kicker="HOW EXPANSION WORKS"
          title="More evidence around the same identity primitive."
          heading-id="expansion-title"
        />

        <div class="expansion__flow">
          <article class="expansion-stage expansion-stage--today">
            <div class="expansion-stage__head">
              <span>TODAY</span>
              <strong>PROVEN</strong>
            </div>
            <ol>
              <li>Individual physical asset</li>
              <li>Signed physical evidence</li>
              <li>Persistent identity</li>
              <li>Custody</li>
              <li>Verification</li>
            </ol>
          </article>

          <div class="expansion__arrow" aria-hidden="true">→</div>

          <article class="expansion-stage">
            <div class="expansion-stage__head">
              <span>NEXT</span>
              <strong>EXPANSION PATH</strong>
            </div>
            <ol>
              <li>More participants</li>
              <li>Suppliers</li>
              <li>Certifiers</li>
              <li>Processors</li>
              <li>Official systems</li>
              <li>Compliance evidence</li>
            </ol>
          </article>

          <div class="expansion__arrow" aria-hidden="true">→</div>

          <article class="expansion-stage">
            <div class="expansion-stage__head">
              <span>LATER</span>
              <strong>EXPANSION PATH</strong>
            </div>
            <ol>
              <li>Verified asset history</li>
              <li>Risk models</li>
              <li>Insurance</li>
              <li>Credit</li>
              <li>Collateral</li>
            </ol>
          </article>
        </div>
      </section>

      <section class="boundary" aria-labelledby="boundary-title">
        <div>
          <p class="future-kicker">PRODUCT BOUNDARY</p>
          <h2 id="boundary-title">The ambition is broad. The proof remains deliberately narrow.</h2>
        </div>
        <div class="boundary__columns">
          <article>
            <span class="boundary__label boundary__label--proven">SOFTWARE DEMONSTRATED</span>
            <p>
              Signed StationEvent processing, persistent AnimalID, custody authorization,
              re-identification, canonical Solana state and independent verification in the software
              demo. Physical reader validation remains a separate hardware gate.
            </p>
          </article>
          <article>
            <span class="boundary__label">EXPANSION PATH</span>
            <p>
              Traceability, compliance interoperability, risk and financial infrastructure built
              around verifiable physical history.
            </p>
          </article>
        </div>
      </section>

      <StoryPageFooter
        title="See the narrow primitive work end to end."
        to="/demo"
        link-label="Open Demo"
      />
    </div>
  </main>
</template>

<style scoped>
.future-page {
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

.future-page__frame {
  width: min(100% - 64px, 1480px);
  margin: 0 auto;
}

.future-kicker {
  margin: 0 0 12px;
  color: var(--proof);
  font: 700 10px/1.3 var(--font-mono);
  letter-spacing: 0.16em;
}

.future-hero {
  display: grid;
  grid-template-columns: minmax(0, 1.05fr) minmax(320px, 0.72fr);
  gap: 8vw;
  align-items: end;
  padding: 72px 0 64px;
  border-bottom: 1px solid var(--border);
}

.future-hero h1 {
  max-width: 15ch;
  margin: 0;
  font-size: clamp(44px, 4.6vw, 68px);
  font-weight: 580;
  line-height: 1;
  letter-spacing: -0.052em;
}

.future-hero__thesis {
  padding: 24px;
  border-left: 2px solid var(--primary);
  background: linear-gradient(
    90deg,
    color-mix(in srgb, var(--primary) 8%, transparent),
    transparent
  );
}

.future-hero__thesis span,
.expansion-stage__head,
.boundary__label {
  font: 700 10px/1.3 var(--font-mono);
  letter-spacing: 0.12em;
}

.future-hero__thesis span {
  color: #b8aaff;
}

.future-hero__thesis p {
  margin: 14px 0;
  color: color-mix(in srgb, var(--text) 88%, transparent);
  font-size: 18px;
  line-height: 1.55;
}

.future-hero__thesis strong {
  color: var(--proof);
  font-size: 14px;
}

.trust-stack,
.expansion,
.boundary {
  padding: 64px 0;
  border-bottom: 1px solid var(--border);
}

.expansion__flow {
  display: grid;
  grid-template-columns: 1fr 56px 1fr 56px 1fr;
  gap: 10px;
  align-items: center;
}

.expansion-stage {
  min-height: 350px;
  padding: 24px;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
}

.expansion-stage--today {
  border-color: color-mix(in srgb, var(--proof) 48%, var(--border));
}

.expansion-stage__head {
  display: flex;
  justify-content: space-between;
  gap: 20px;
  color: var(--muted);
}

.expansion-stage--today .expansion-stage__head strong {
  color: var(--proof);
}

.expansion-stage ol {
  display: grid;
  gap: 0;
  margin: 28px 0 0;
  padding: 0;
  list-style: none;
  counter-reset: step;
}

.expansion-stage li {
  counter-increment: step;
  position: relative;
  min-height: 50px;
  padding: 0 0 20px 34px;
  border-left: 1px solid var(--border-strong);
  color: color-mix(in srgb, var(--text) 88%, transparent);
  font-size: 13px;
}

.expansion-stage li::before {
  content: counter(step, decimal-leading-zero);
  position: absolute;
  left: -10px;
  top: -2px;
  padding: 3px 0;
  background: var(--surface);
  color: var(--muted);
  font: 700 9px/1 var(--font-mono);
}

.expansion-stage--today li::before {
  color: var(--proof);
}

.expansion-stage li:last-child {
  border-left-color: transparent;
}

.expansion__arrow {
  color: var(--primary);
  font: 700 26px/1 var(--font-mono);
  text-align: center;
}

.boundary {
  display: grid;
  grid-template-columns: minmax(0, 0.8fr) minmax(0, 1.2fr);
  gap: 8vw;
}

.boundary__columns {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}

.boundary__columns article {
  padding: 22px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
}

.boundary__columns p {
  margin: 14px 0 0;
  color: var(--muted);
  font-size: 13px;
  line-height: 1.65;
}

.boundary__label {
  color: var(--muted);
}

.boundary__label--proven {
  color: var(--proof);
}

@media (max-width: 1000px) {
  .future-hero,
  .boundary {
    grid-template-columns: 1fr;
    gap: 28px;
  }

  .expansion__flow {
    grid-template-columns: 1fr;
  }

  .expansion__arrow {
    transform: rotate(90deg);
  }

  .expansion-stage {
    min-height: 0;
  }
}

@media (max-width: 700px) {
  .future-page__frame {
    width: min(100% - 32px, 1480px);
  }

  .future-hero {
    padding: 48px 0 44px;
  }

  .future-hero h1 {
    max-width: 100%;
    font-size: clamp(38px, 11vw, 50px);
  }

  .trust-stack,
  .expansion,
  .boundary {
    padding: 48px 0;
  }

  .boundary__columns {
    grid-template-columns: 1fr;
  }
}

@media (min-width: 1200px) and (min-aspect-ratio: 16 / 10) {
  .future-hero {
    padding-top: 64px;
    padding-bottom: 58px;
  }

  .trust-stack,
  .expansion,
  .boundary {
    padding-block: 58px;
  }
}

@media (max-width: 430px) {
  .future-page__frame {
    width: min(100% - 24px, 1480px);
  }

  .future-hero__thesis {
    padding: 18px;
  }
}
</style>

<script setup lang="ts">
import { RouterLink } from 'vue-router'
import PhysicalAssetReveal from '../components/PhysicalAssetReveal.vue'
</script>

<template>
  <main class="landing">
    <section class="landing-hero" aria-labelledby="landing-title">
      <PhysicalAssetReveal />

      <div class="landing-hero__frame">
        <header class="landing-hero__header">
          <RouterLink class="landing-brand" to="/" aria-label="Lastro home">
            <img src="/logo.svg" alt="" width="36" height="36" />
            <span>LASTRO</span>
          </RouterLink>
        </header>

        <div class="landing-hero__content">
          <p class="landing-hero__eyebrow">LASTRO</p>
          <h1 id="landing-title">Verifiable identity and custody for physical livestock.</h1>
          <p class="landing-hero__support">
            Physical identifiers can change.<br />
            Verifiable identity and custody should not.
          </p>

          <RouterLink class="landing-hero__cta" to="/demo">
            <span>Open Demo</span>
            <span aria-hidden="true">→</span>
          </RouterLink>

          <nav class="landing-paths" aria-label="Secondary exploration">
            <RouterLink class="landing-path" to="/problem">
              <span>The Problem</span>
              <small>Understand the global physical trust gap.</small>
            </RouterLink>
            <RouterLink class="landing-path" to="/future">
              <span>The Future</span>
              <small>See what verifiable physical history can enable.</small>
            </RouterLink>
          </nav>
        </div>
      </div>
    </section>
  </main>
</template>

<style scoped>
.landing {
  min-height: 100vh;
  background: var(--canvas);
  color: var(--text);
}

.landing-hero {
  position: relative;
  min-height: 100svh;
  overflow: hidden;
  isolation: isolate;
  background: var(--canvas);
}

.landing-hero__frame {
  position: relative;
  z-index: 2;
  display: grid;
  grid-template-rows: auto 1fr;
  width: min(100% - 64px, 1600px);
  min-height: 100svh;
  margin: 0 auto;
  padding: 28px 0 24px;
}

.landing-hero__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 28px;
  min-height: 40px;
}

.landing-brand {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  color: var(--text);
  font: 750 14px/1 var(--font-mono);
  letter-spacing: .2em;
  text-decoration: none;
}

.landing-brand img {
  width: 32px;
  height: 32px;
}


.landing-hero__content {
  align-self: center;
  width: min(820px, 58vw);
  padding: 3vh 0 2vh;
}

.landing-hero__eyebrow {
  margin: 0 0 20px;
  color: var(--proof);
  font: 700 10px/1.4 var(--font-mono);
  letter-spacing: .18em;
}

.landing-hero h1 {
  max-width: 14ch;
  margin: 0;
  font-size: clamp(52px, 4.8vw, 78px);
  font-weight: 580;
  line-height: .99;
  letter-spacing: -.055em;
  text-wrap: balance;
}

.landing-hero__support {
  margin: 26px 0 0;
  color: color-mix(in srgb, var(--text) 72%, transparent);
  font-size: clamp(16px, 1.35vw, 19px);
  line-height: 1.62;
}

.landing-hero__cta {
  display: inline-flex;
  align-items: center;
  justify-content: space-between;
  gap: 54px;
  min-width: 202px;
  margin-top: 28px;
  padding: 15px 16px 15px 18px;
  border: 1px solid color-mix(in srgb, var(--primary) 78%, var(--border));
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--primary) 18%, var(--canvas));
  color: var(--text);
  font: 700 13px/1 var(--font-mono);
  text-decoration: none;
  transition: background 150ms ease, border-color 150ms ease, transform 150ms ease;
}

.landing-hero__cta:hover,
.landing-hero__cta:focus-visible {
  border-color: color-mix(in srgb, var(--primary) 86%, white);
  background: color-mix(in srgb, var(--primary) 25%, var(--canvas));
  transform: translateY(-1px);
}

.landing-paths {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 230px));
  gap: 12px 26px;
  margin-top: 26px;
}

.landing-path {
  display: grid;
  gap: 5px;
  padding: 12px 0 0;
  border-top: 1px solid color-mix(in srgb, var(--border) 85%, transparent);
  text-decoration: none;
}

.landing-path span {
  color: var(--text);
  font: 700 11px/1.3 var(--font-mono);
}

.landing-path small {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.45;
}

.landing-path:hover,
.landing-path:focus-visible {
  border-top-color: var(--proof);
}


@media (min-width: 1200px) and (min-aspect-ratio: 16 / 10) {
  .landing-hero__frame {
    padding-top: 24px;
    padding-bottom: 20px;
  }

  .landing-hero__content {
    padding-top: 1vh;
    padding-bottom: 0;
  }

  .landing-hero__support {
    margin-top: 22px;
  }

  .landing-paths {
    margin-top: 22px;
  }
}

@media (max-width: 1024px) {
  .landing-hero__content {
    width: min(620px, 67vw);
  }
}

@media (max-width: 768px) {
  .landing-hero__frame {
    width: min(100% - 32px, 640px);
    padding: 20px 0 16px;
  }

  .landing-hero__header {
    align-items: flex-start;
  }

  .landing-hero__content {
    align-self: start;
    width: 100%;
    padding: 8vh 0 0;
  }

  .landing-hero__eyebrow {
    margin-bottom: 14px;
  }

  .landing-hero h1 {
    max-width: 100%;
    font-size: clamp(40px, 10.8vw, 52px);
    line-height: 1;
  }

  .landing-hero__support {
    max-width: 400px;
    margin-top: 18px;
    font-size: 15px;
    line-height: 1.52;
  }

  .landing-hero__cta {
    margin-top: 20px;
  }

  .landing-paths {
    width: min(100%, 440px);
    margin-top: 24px;
  }
}

@media (max-width: 430px) {
  .landing-hero__frame {
    width: min(100% - 24px, 640px);
  }

  .landing-brand span {
    display: none;
  }

  .landing-hero__content {
    padding-top: 6.5vh;
  }

  .landing-paths {
    grid-template-columns: 1fr;
    gap: 10px;
  }

  .landing-path small {
    max-width: 34ch;
  }
}

@media (prefers-reduced-motion: reduce) {
  .landing-hero__cta {
    transition: none;
  }
}
</style>

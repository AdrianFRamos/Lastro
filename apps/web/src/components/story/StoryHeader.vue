<script setup lang="ts">
import { RouterLink } from 'vue-router'
import LanguageSwitch from '../LanguageSwitch.vue'
import { useCopy } from '../../i18n'

defineProps<{ active?: 'problem' | 'future' | 'demo' }>()

const copy = useCopy({
  en: { problem: 'The Problem', future: 'The Future', demo: 'Open Demo', nav: 'Primary' },
  pt: { problem: 'O Problema', future: 'O Futuro', demo: 'Abrir Demo', nav: 'Principal' },
})
</script>

<template>
  <header class="story-header">
    <RouterLink class="story-header__brand" to="/" aria-label="Lastro home">
      <img src="/logo.svg" alt="" width="34" height="34" />
      <span>LASTRO</span>
    </RouterLink>

    <nav class="story-header__nav" :aria-label="copy.nav">
      <RouterLink to="/problem" :aria-current="active === 'problem' ? 'page' : undefined">
        {{ copy.problem }}
      </RouterLink>
      <RouterLink to="/future" :aria-current="active === 'future' ? 'page' : undefined">
        {{ copy.future }}
      </RouterLink>
      <RouterLink
        class="story-header__demo"
        to="/chain-history"
        :aria-current="active === 'demo' ? 'page' : undefined"
      >
        {{ copy.demo }}
      </RouterLink>
      <LanguageSwitch />
    </nav>
  </header>
</template>

<style scoped>
.story-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 28px;
  min-height: 68px;
  border-bottom: 1px solid color-mix(in srgb, var(--border) 86%, transparent);
}

.story-header__brand {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  color: var(--text);
  font: 750 13px/1 var(--font-mono);
  letter-spacing: 0.18em;
  text-decoration: none;
}

.story-header__brand img {
  width: 30px;
  height: 30px;
}

.story-header__nav {
  display: flex;
  align-items: center;
  gap: 24px;
  color: color-mix(in srgb, var(--muted) 86%, transparent);
  font: 650 12px/1 var(--font-mono);
}

.story-header__nav a {
  padding: 10px 0;
  text-decoration: none;
}

.story-header__nav a:hover,
.story-header__nav a:focus-visible,
.story-header__nav a[aria-current='page'] {
  color: var(--text);
}

.story-header__demo {
  min-height: 36px;
  padding: 11px 14px !important;
  border: 1px solid color-mix(in srgb, var(--primary) 70%, var(--border));
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--primary) 13%, var(--canvas));
  color: var(--text) !important;
}

@media (max-width: 700px) {
  .story-header {
    align-items: flex-start;
    flex-direction: column;
    gap: 12px;
    padding: 16px 0;
  }

  .story-header__nav {
    flex-wrap: wrap;
    width: 100%;
    justify-content: space-between;
    gap: 10px;
    font-size: 11px;
  }
}
</style>

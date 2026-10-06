<script setup lang="ts">
/** Flag buttons that switch the site between Brazilian Portuguese and English. */
import { locale, setLocale, type Locale } from '../i18n'

const options: { value: Locale; label: string; short: string }[] = [
  { value: 'pt', label: 'Português (Brasil)', short: 'PT' },
  { value: 'en', label: 'English', short: 'EN' },
]
</script>

<template>
  <div class="language-switch" role="group" aria-label="Idioma / Language">
    <button
      v-for="option in options"
      :key="option.value"
      type="button"
      class="language-switch__option"
      :aria-pressed="locale === option.value"
      :aria-label="option.label"
      :title="option.label"
      @click="setLocale(option.value)"
    >
      <svg v-if="option.value === 'pt'" viewBox="0 0 28 20" aria-hidden="true">
        <rect width="28" height="20" fill="#009c3b" />
        <path d="M14 2.4 25.2 10 14 17.6 2.8 10z" fill="#ffdf00" />
        <circle cx="14" cy="10" r="4.6" fill="#002776" />
        <path d="M9.6 9.1c3-.9 6.1-.5 8.8 1.1" stroke="#fff" stroke-width="0.9" fill="none" />
      </svg>
      <svg v-else viewBox="0 0 60 40" aria-hidden="true">
        <clipPath id="language-switch-uk">
          <rect width="60" height="40" />
        </clipPath>
        <g clip-path="url(#language-switch-uk)">
          <rect width="60" height="40" fill="#012169" />
          <path d="M0 0l60 40M60 0 0 40" stroke="#fff" stroke-width="8" />
          <path d="M0 0l60 40M60 0 0 40" stroke="#c8102e" stroke-width="3" />
          <path d="M30 0v40M0 20h60" stroke="#fff" stroke-width="12" />
          <path d="M30 0v40M0 20h60" stroke="#c8102e" stroke-width="7" />
        </g>
      </svg>
      <span>{{ option.short }}</span>
    </button>
  </div>
</template>

<style scoped>
.language-switch {
  display: inline-flex;
  gap: 4px;
  padding: 3px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: color-mix(in srgb, var(--surface) 85%, transparent);
}

.language-switch__option {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 9px 5px 6px;
  border: 1px solid transparent;
  border-radius: 999px;
  background: transparent;
  color: var(--muted);
  font: 700 11px/1 var(--font-mono);
  cursor: pointer;
  opacity: 0.7;
  transition:
    opacity 150ms ease,
    background 150ms ease;
}

.language-switch__option svg {
  width: 20px;
  height: 14px;
  border-radius: 2px;
  box-shadow: 0 0 0 1px rgb(0 0 0 / 0.35);
}

.language-switch__option:hover,
.language-switch__option:focus-visible {
  opacity: 1;
}

.language-switch__option:focus-visible {
  outline: 2px solid var(--proof);
  outline-offset: 1px;
}

.language-switch__option[aria-pressed='true'] {
  border-color: var(--border-strong);
  background: var(--raised);
  color: var(--text);
  opacity: 1;
}

@media (prefers-reduced-motion: reduce) {
  .language-switch__option {
    transition: none;
  }
}
</style>

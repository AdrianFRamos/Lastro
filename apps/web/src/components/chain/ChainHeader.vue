<script setup lang="ts">
/**
 * Top bar of the Open Demo chain pages: brand on the left; language flags and actions on the
 * right (sign in by default, or the workspace once a demo profile is signed in).
 */
import { RouterLink } from 'vue-router'
import LanguageSwitch from '../LanguageSwitch.vue'
import { session } from '../../demo/workspace'
import { useCopy } from '../../i18n'

const copy = useCopy({
  en: { signIn: 'Sign in', workspace: 'My workspace' },
  pt: { signIn: 'Entrar', workspace: 'Meu painel' },
})
</script>

<template>
  <header class="chain-header">
    <RouterLink class="chain-header__brand" to="/" aria-label="Lastro home">
      <img src="/logo.svg" alt="" width="32" height="32" />
      <span>LASTRO</span>
    </RouterLink>
    <div class="chain-header__actions">
      <LanguageSwitch />
      <slot>
        <RouterLink v-if="session" class="chain-header__button" to="/painel">
          {{ copy.workspace }}
        </RouterLink>
        <RouterLink v-else class="chain-header__button" to="/login">{{ copy.signIn }}</RouterLink>
      </slot>
    </div>
  </header>
</template>

<style scoped>
.chain-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 28px;
  min-height: 68px;
}

.chain-header__brand {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  color: var(--text);
  font: 750 14px/1 var(--font-mono);
  letter-spacing: 0.2em;
  text-decoration: none;
}

.chain-header__brand img {
  width: 32px;
  height: 32px;
}

.chain-header__actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.chain-header__actions :slotted(.chain-header__button),
.chain-header__button {
  min-width: 132px;
  padding: 12px 18px;
  border: 1px solid color-mix(in srgb, var(--primary) 78%, var(--border));
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--primary) 18%, var(--canvas));
  color: var(--text);
  font: 700 13px/1 var(--font-mono);
  letter-spacing: 0.08em;
  text-align: center;
  text-decoration: none;
  text-transform: uppercase;
  cursor: pointer;
  transition:
    background 150ms ease,
    border-color 150ms ease;
}

.chain-header__actions :slotted(.chain-header__button:hover),
.chain-header__actions :slotted(.chain-header__button:focus-visible),
.chain-header__button:hover,
.chain-header__button:focus-visible {
  border-color: color-mix(in srgb, var(--primary) 86%, white);
  background: color-mix(in srgb, var(--primary) 25%, var(--canvas));
}

@media (max-width: 768px) {
  .chain-header__actions {
    gap: 8px;
  }

  .chain-header__actions :slotted(.chain-header__button),
  .chain-header__button {
    min-width: 0;
    padding: 11px 12px;
  }

  .chain-header__brand span {
    display: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .chain-header__actions :slotted(.chain-header__button),
  .chain-header__button {
    transition: none;
  }
}
</style>

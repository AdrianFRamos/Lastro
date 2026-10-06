<script setup lang="ts">
/**
 * Demo login screen reached from ENTRAR, over the same animated asset backdrop as the home.
 * Authentication is not wired yet, so submitting only validates the fields and explains that
 * access is simulated.
 */
import { ref } from 'vue'
import { RouterLink } from 'vue-router'
import LanguageSwitch from '../components/LanguageSwitch.vue'
import PhysicalAssetReveal from '../components/PhysicalAssetReveal.vue'
import { useCopy } from '../i18n'

const email = ref('')
const password = ref('')
const submitted = ref(false)

const copy = useCopy({
  en: {
    back: 'Back to history',
    title: 'Login',
    email: 'Email',
    emailPlaceholder: 'you@company.com',
    password: 'Password',
    submit: 'Sign in',
    notice: 'Demo: access is not connected to a real account yet.',
  },
  pt: {
    back: 'Voltar ao histórico',
    title: 'Login',
    email: 'E-mail',
    emailPlaceholder: 'voce@empresa.com',
    password: 'Senha',
    submit: 'Entrar',
    notice: 'Demonstração: o acesso ainda não está conectado a uma conta real.',
  },
})

function submit() {
  submitted.value = true
}
</script>

<template>
  <main class="login">
    <PhysicalAssetReveal />

    <RouterLink class="login__back" to="/chain-history">
      <span aria-hidden="true">←</span> {{ copy.back }}
    </RouterLink>

    <div class="login__language">
      <LanguageSwitch />
    </div>

    <section class="login__card" aria-labelledby="login-title">
      <h1 id="login-title">{{ copy.title }}</h1>

      <form class="login__form" @submit.prevent="submit">
        <label for="login-email">{{ copy.email }}</label>
        <input
          id="login-email"
          v-model.trim="email"
          type="email"
          autocomplete="username"
          :placeholder="copy.emailPlaceholder"
          required
        />

        <label for="login-password">{{ copy.password }}</label>
        <input
          id="login-password"
          v-model="password"
          type="password"
          autocomplete="current-password"
          placeholder="••••••••"
          required
        />

        <button type="submit">{{ copy.submit }}</button>
      </form>

      <p v-if="submitted" class="login__notice" role="status">
        {{ copy.notice }}
      </p>
    </section>
  </main>
</template>

<style scoped>
.login {
  position: relative;
  display: grid;
  min-height: 100svh;
  /* Card on the dark left half, like the home copy, so the animal stays visible on the right. */
  align-items: center;
  justify-items: start;
  overflow: hidden;
  padding: 88px max(32px, calc((100% - 1600px) / 2 + 8vw)) 48px;
  background: var(--canvas);
  color: var(--text);
}

.login__back {
  position: absolute;
  top: 28px;
  left: 32px;
  z-index: 2;
  display: inline-flex;
  gap: 8px;
  padding: 10px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--canvas);
  color: var(--text);
  font: 700 12px/1 var(--font-mono);
  text-decoration: none;
}

.login__language {
  position: absolute;
  top: 28px;
  right: 32px;
  z-index: 2;
}

.login__back:hover,
.login__back:focus-visible {
  border-color: var(--border-strong);
}

.login__card {
  position: relative;
  z-index: 1;
  width: min(100%, 380px);
  padding: 28px 28px 32px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-lg);
  background: var(--surface);
  box-shadow:
    0 30px 60px -20px rgb(0 0 0 / 0.85),
    10px 12px 0 rgb(0 0 0 / 0.35);
}

.login__card h1 {
  margin: 0 0 24px;
  font-size: 24px;
  font-style: italic;
  font-weight: 850;
  letter-spacing: 0.02em;
  text-align: center;
  text-transform: uppercase;
}

.login__form {
  display: grid;
  gap: 8px;
}

.login__form label {
  margin-top: 8px;
  color: var(--muted);
  font: 700 11px/1.3 var(--font-mono);
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.login__form input {
  width: 100%;
  padding: 12px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--canvas);
  color: var(--text);
  font: inherit;
  font-size: 15px;
}

.login__form input:focus-visible {
  border-color: var(--proof);
  outline: 2px solid color-mix(in srgb, var(--proof) 35%, transparent);
  outline-offset: 1px;
}

.login__form button {
  margin-top: 20px;
  padding: 14px 18px;
  border: 1px solid color-mix(in srgb, var(--primary) 78%, var(--border));
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--primary) 30%, var(--canvas));
  color: var(--text);
  font: 700 13px/1 var(--font-mono);
  letter-spacing: 0.08em;
  text-transform: uppercase;
  cursor: pointer;
  transition: background 150ms ease;
}

.login__form button:hover,
.login__form button:focus-visible {
  background: color-mix(in srgb, var(--primary) 42%, var(--canvas));
}

.login__notice {
  margin: 16px 0 0;
  padding: 10px 12px;
  border: 1px solid color-mix(in srgb, var(--warning) 45%, var(--border));
  border-radius: var(--radius-sm);
  color: var(--warning);
  font-size: 13px;
  line-height: 1.45;
}

@media (max-width: 768px) {
  .login {
    justify-items: center;
    padding: 88px 16px 48px;
  }

  .login__back {
    top: 20px;
    left: 16px;
  }

  .login__language {
    top: 22px;
    right: 16px;
  }

  .login__card {
    padding: 24px 20px 28px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .login__form button {
    transition: none;
  }
}
</style>

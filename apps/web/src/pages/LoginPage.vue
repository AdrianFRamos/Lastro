<script setup lang="ts">
/**
 * Demo login screen reached from ENTRAR, over the same animated asset backdrop as the home.
 * There are no real accounts yet: the visitor picks an access profile (producer, carrier,
 * slaughterhouse, exporter, merchant or common user) and is taken to that profile's workspace.
 */
import { ref } from 'vue'
import { RouterLink, useRouter } from 'vue-router'
import LanguageSwitch from '../components/LanguageSwitch.vue'
import PhysicalAssetReveal from '../components/PhysicalAssetReveal.vue'
import { findRole, roles, type RoleId } from '../demo/roles'
import { signIn } from '../demo/workspace'
import { tr, useCopy } from '../i18n'

const router = useRouter()

const email = ref('')
const password = ref('')
const role = ref<RoleId | ''>('')
const error = ref('')

const copy = useCopy({
  en: {
    back: 'Back to history',
    title: 'Login',
    email: 'Email',
    emailPlaceholder: 'you@company.com',
    password: 'Password',
    role: 'Access profile',
    choose: 'Select your profile…',
    submit: 'Sign in',
    notice: 'Demo: pick a profile to open its workspace. No real account is checked yet.',
    missing: 'Fill in the email, the password and the access profile.',
  },
  pt: {
    back: 'Voltar ao histórico',
    title: 'Login',
    email: 'E-mail',
    emailPlaceholder: 'voce@empresa.com',
    password: 'Senha',
    role: 'Perfil de acesso',
    choose: 'Selecione seu perfil…',
    submit: 'Entrar',
    notice:
      'Demonstração: escolha um perfil para abrir o painel dele. Nenhuma conta real é verificada ainda.',
    missing: 'Preencha o e-mail, a senha e o perfil de acesso.',
  },
})

async function submit() {
  const chosen = findRole(role.value)
  if (!email.value || !password.value || !chosen) {
    error.value = copy.value.missing
    return
  }
  error.value = ''
  signIn(chosen.id, email.value)
  await router.push({ name: 'workspace' })
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

      <form class="login__form" novalidate @submit.prevent="submit">
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

        <label for="login-role">{{ copy.role }}</label>
        <select id="login-role" v-model="role" required>
          <option value="" disabled>{{ copy.choose }}</option>
          <option v-for="option in roles" :key="option.id" :value="option.id">
            {{ tr(option.label) }}
          </option>
        </select>

        <p v-if="error" class="login__error" role="alert">{{ error }}</p>

        <button type="submit">{{ copy.submit }}</button>
      </form>

      <p class="login__notice">{{ copy.notice }}</p>
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

.login__form input,
.login__form select {
  width: 100%;
  padding: 12px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--canvas);
  color: var(--text);
  font: inherit;
  font-size: 15px;
}

.login__form select {
  color-scheme: dark;
}

.login__form input:focus-visible,
.login__form select:focus-visible {
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

.login__error {
  margin: 8px 0 0;
  color: var(--warning);
  font-size: 13px;
  line-height: 1.45;
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

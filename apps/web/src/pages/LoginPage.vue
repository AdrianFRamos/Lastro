<script setup lang="ts">
/**
 * Login screen reached from ENTRAR, over the same animated asset backdrop as the home.
 * A participant signs in with a Solana wallet: the wallet signs a login message (proving control
 * of the key) and the workspace role is read from the wallet's PartyRecord on Solana. Without a
 * wallet, a visitor can still browse every participant's records read-only.
 */
import { onMounted, onUnmounted, ref } from 'vue'
import { RouterLink, useRouter } from 'vue-router'
import LanguageSwitch from '../components/LanguageSwitch.vue'
import PhysicalAssetReveal from '../components/PhysicalAssetReveal.vue'
import { loginWithConnectedWallet } from '../auth/walletLogin'
import { signInAsVisitor, signInWithWallet } from '../demo/workspace'
import { useCopy } from '../i18n'
import { connectWalletByName, discoveredWalletNames, onWalletStateChange } from '../solana/wallet'

const router = useRouter()

const wallets = ref<string[]>([])
const busyWallet = ref('')
const error = ref('')

const copy = useCopy({
  en: {
    back: 'Back to history',
    title: 'Login',
    intro: 'Sign in with the Solana wallet registered for your company in the chain.',
    connect: 'Sign in with',
    signing: 'Check your wallet…',
    noWallet:
      'No Solana wallet was found in this browser. Install Phantom (or another Solana wallet) and switch it to devnet.',
    install: 'Get Phantom',
    or: 'or',
    visitor: 'Explore as a visitor',
    visitorNote: 'Read-only, no wallet. Records shown are simulated.',
    notice:
      'Your wallet signs a login message only: it costs nothing and sends no transaction. Your profile comes from the participant registry on Solana.',
    rejected: 'The signature was cancelled in the wallet.',
    failed: 'Could not sign in with this wallet.',
    suspended: 'This wallet is suspended in the participant registry.',
    revoked: 'This wallet was revoked in the participant registry.',
    expired: 'This wallet registration has expired.',
  },
  pt: {
    back: 'Voltar ao histórico',
    title: 'Login',
    intro: 'Entre com a carteira Solana cadastrada para a sua empresa na cadeia.',
    connect: 'Entrar com',
    signing: 'Confirme na sua carteira…',
    noWallet:
      'Nenhuma carteira Solana foi encontrada neste navegador. Instale a Phantom (ou outra carteira Solana) e mude para a devnet.',
    install: 'Instalar a Phantom',
    or: 'ou',
    visitor: 'Explorar como visitante',
    visitorNote: 'Somente leitura, sem carteira. Os registros exibidos são simulados.',
    notice:
      'Sua carteira assina apenas uma mensagem de login: não custa nada e não envia transação. Seu perfil vem do cadastro de participantes na Solana.',
    rejected: 'A assinatura foi cancelada na carteira.',
    failed: 'Não foi possível entrar com esta carteira.',
    suspended: 'Esta carteira está suspensa no cadastro de participantes.',
    revoked: 'Esta carteira foi revogada no cadastro de participantes.',
    expired: 'O cadastro desta carteira expirou.',
  },
})

let stopWatchingWallets: (() => void) | null = null

function refreshWallets(): void {
  wallets.value = discoveredWalletNames()
}

onMounted(() => {
  refreshWallets()
  stopWatchingWallets = onWalletStateChange(refreshWallets)
})

onUnmounted(() => stopWatchingWallets?.())

function isUserRejection(cause: unknown): boolean {
  const text = cause instanceof Error ? `${cause.name} ${cause.message}` : String(cause)
  return /reject|denied|cancel/i.test(text)
}

async function signInWith(name: string): Promise<void> {
  if (busyWallet.value) return
  busyWallet.value = name
  error.value = ''
  try {
    await connectWalletByName(name)
    const outcome = await loginWithConnectedWallet()
    if (outcome.kind === 'blocked') {
      error.value = copy.value[outcome.status]
      return
    }
    signInWithWallet(outcome.session)
    await router.push({ name: 'workspace' })
  } catch (cause) {
    error.value = isUserRejection(cause) ? copy.value.rejected : copy.value.failed
  } finally {
    busyWallet.value = ''
  }
}

async function exploreAsVisitor(): Promise<void> {
  signInAsVisitor()
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
      <p class="login__intro">{{ copy.intro }}</p>

      <div class="login__form">
        <template v-if="wallets.length > 0">
          <button
            v-for="name in wallets"
            :key="name"
            type="button"
            data-action="wallet-login"
            :data-wallet="name"
            :disabled="busyWallet !== ''"
            @click="signInWith(name)"
          >
            {{ busyWallet === name ? copy.signing : `${copy.connect} ${name}` }}
          </button>
        </template>
        <p v-else class="login__empty">
          {{ copy.noWallet }}
          <a href="https://phantom.com/download" target="_blank" rel="noopener noreferrer">{{
            copy.install
          }}</a>
        </p>

        <p v-if="error" class="login__error" role="alert">{{ error }}</p>

        <p class="login__or">
          <span>{{ copy.or }}</span>
        </p>

        <button
          type="button"
          class="login__visitor"
          data-action="visitor-login"
          :disabled="busyWallet !== ''"
          @click="exploreAsVisitor"
        >
          {{ copy.visitor }}
        </button>
        <p class="login__visitor-note">{{ copy.visitorNote }}</p>
      </div>

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

.login__intro {
  margin: -12px 0 20px;
  color: var(--muted);
  font-size: 14px;
  line-height: 1.5;
  text-align: center;
}

.login__empty {
  margin: 0;
  color: var(--text);
  font-size: 14px;
  line-height: 1.5;
}

.login__empty a {
  display: inline-block;
  margin-top: 8px;
  color: var(--proof);
}

.login__or {
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 12px 0 0;
  color: var(--muted);
  font: 700 11px/1 var(--font-mono);
  text-transform: uppercase;
}

.login__or::before,
.login__or::after {
  content: '';
  flex: 1;
  height: 1px;
  background: var(--border);
}

.login__form .login__visitor {
  margin-top: 4px;
  background: var(--canvas);
  border-color: var(--border-strong);
}

.login__visitor-note {
  margin: 0;
  color: var(--muted);
  font-size: 12px;
  line-height: 1.4;
  text-align: center;
}

.login__form button:disabled {
  cursor: progress;
  opacity: 0.6;
}

.login__form button {
  margin-top: 8px;
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

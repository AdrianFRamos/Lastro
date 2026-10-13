<script setup lang="ts">
/**
 * Login screen reached from ENTRAR, over the same animated asset backdrop as the home.
 * A participant signs in with a Solana wallet: the wallet signs a login message (proving control
 * of the key) and the workspace role is read from the wallet's PartyRecord on Solana. Without a
 * wallet, a visitor can still browse every participant's records read-only.
 */
import { computed, onMounted, onUnmounted, ref, shallowRef } from 'vue'
import { useRouter } from 'vue-router'
import {
  RECOMMENDED_WALLETS,
  connectForLogin,
  loginWallets,
  onLoginWalletsChange,
  signLoginMessage,
  type StandardWallet,
} from '../auth/loginWallets'
import { loginWithConnectedWallet } from '../auth/walletLogin'
import { signInAsVisitor, signInWithWallet } from '../demo/workspace'
import { useCopy } from '../i18n'

const router = useRouter()

const wallets = shallowRef<StandardWallet[]>([])
/** Recommended wallets not detected in this browser, offered as install links. */
const missing = computed(() =>
  RECOMMENDED_WALLETS.filter((option) => !wallets.value.some((w) => w.name === option.name)),
)
const busyWallet = ref('')
const error = ref('')

const copy = useCopy({
  en: {
    back: 'Back to history',
    title: 'Login',
    intro: 'Sign in with the Solana wallet registered for your company in the chain.',
    connect: 'Sign in with',
    signing: 'Check your wallet…',
    noWallet: 'No Solana wallet was found in this browser.',
    install: 'Install',
    walletHint:
      'In MetaMask, use the Solana account (not the 0x… Ethereum one). Any Solana network works to sign in.',
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
    noWallet: 'Nenhuma carteira Solana foi encontrada neste navegador.',
    install: 'Instalar',
    walletHint:
      'Na MetaMask, use a conta Solana (não a conta Ethereum 0x…). Qualquer rede Solana serve para entrar.',
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
  wallets.value = loginWallets()
}

onMounted(() => {
  refreshWallets()
  stopWatchingWallets = onLoginWalletsChange(refreshWallets)
})

onUnmounted(() => stopWatchingWallets?.())

function isUserRejection(cause: unknown): boolean {
  const text = cause instanceof Error ? `${cause.name} ${cause.message}` : String(cause)
  return /reject|denied|cancel/i.test(text)
}

async function signInWith(wallet: StandardWallet): Promise<void> {
  if (busyWallet.value) return
  busyWallet.value = wallet.name
  error.value = ''
  try {
    const account = await connectForLogin(wallet)
    const outcome = await loginWithConnectedWallet({
      connectedWallet: () => account.address,
      signMessage: (message) => signLoginMessage(wallet, account, message),
    })
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
    <section class="login__card" aria-labelledby="login-title">
      <h1 id="login-title">{{ copy.title }}</h1>
      <p class="login__intro">{{ copy.intro }}</p>

      <div class="login__form">
        <button
          v-for="wallet in wallets"
          :key="wallet.name"
          type="button"
          data-action="wallet-login"
          :data-wallet="wallet.name"
          :disabled="busyWallet !== ''"
          @click="signInWith(wallet)"
        >
          <img v-if="wallet.icon" :src="wallet.icon" alt="" width="18" height="18" />
          {{ busyWallet === wallet.name ? copy.signing : `${copy.connect} ${wallet.name}` }}
        </button>
        <p v-if="wallets.length === 0" class="login__empty">{{ copy.noWallet }}</p>
        <a
          v-for="option in missing"
          :key="option.name"
          class="login__install"
          :data-install="option.name"
          :href="option.install"
          target="_blank"
          rel="noopener noreferrer"
          >{{ copy.install }} {{ option.name }} ↗</a
        >
        <p class="login__hint">{{ copy.walletHint }}</p>

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
  display: grid;
  grid-template-columns: minmax(0, 440px) minmax(0, 1fr);
  align-items: center;
  gap: 56px;
  width: min(100% - 48px, var(--page-width));
  min-height: calc(100svh - var(--header-height) - 120px);
  margin: 0 auto;
  padding: 48px 0 64px;
  color: var(--text);
}

.login::after {
  content: '';
  align-self: stretch;
  min-height: 420px;
  border-radius: var(--radius-lg);
  background: url('/lastro2.png') 72% 50% / cover no-repeat;
}

.login__card {
  width: 100%;
  padding: 28px;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
  box-shadow: var(--shadow-md);
}

.login__card h1 {
  margin: 0 0 6px;
  font-size: 26px;
  font-weight: 700;
  letter-spacing: -0.02em;
}

.login__intro {
  margin: 0 0 22px;
  color: var(--muted);
  font-size: 15px;
  line-height: 1.5;
}

.login__form {
  display: grid;
  gap: 8px;
}

.login__empty {
  margin: 0;
  padding: 10px 12px;
  border-radius: var(--radius-sm);
  background: var(--raised);
  color: var(--text);
  font-size: 14px;
  line-height: 1.5;
}

.login__install {
  display: block;
  padding: 11px 14px;
  border: 1px dashed var(--border-strong);
  border-radius: var(--radius-sm);
  color: var(--primary);
  font-size: 14px;
  font-weight: 600;
  text-align: center;
  text-decoration: none;
}

.login__install:hover,
.login__install:focus-visible {
  border-color: var(--primary);
  background: var(--primary-soft);
}

.login__hint {
  margin: 2px 0 0;
  color: var(--muted);
  font-size: 13px;
  line-height: 1.45;
}

.login__form button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  min-height: 44px;
  margin-top: 4px;
  padding: 10px 16px;
  border: 1px solid var(--primary);
  border-radius: var(--radius-sm);
  background: var(--primary);
  color: var(--on-primary);
  font-size: 15px;
  font-weight: 600;
  cursor: pointer;
}

.login__form button:hover:not(:disabled) {
  border-color: var(--primary-hover);
  background: var(--primary-hover);
}

.login__form .login__visitor {
  border-color: var(--border-strong);
  background: var(--surface);
  color: var(--text);
}

.login__form .login__visitor:hover:not(:disabled) {
  border-color: var(--border-strong);
  background: var(--raised);
}

.login__form button:disabled {
  cursor: progress;
  opacity: 0.6;
}

.login__or {
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 14px 0 2px;
  color: var(--faint);
  font-size: 13px;
}

.login__or::before,
.login__or::after {
  content: '';
  flex: 1;
  height: 1px;
  background: var(--border);
}

.login__visitor-note {
  margin: 0;
  color: var(--muted);
  font-size: 13px;
  line-height: 1.45;
}

.login__error {
  margin: 6px 0 0;
  padding: 10px 12px;
  border-radius: var(--radius-sm);
  background: var(--danger-soft);
  color: var(--danger);
  font-size: 13.5px;
  line-height: 1.45;
}

.login__notice {
  margin: 20px 0 0;
  padding: 10px 12px;
  border-left: 3px solid var(--warning);
  border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
  background: var(--warning-soft);
  color: var(--text);
  font-size: 13px;
  line-height: 1.5;
}

@media (max-width: 860px) {
  .login {
    grid-template-columns: minmax(0, 1fr);
    width: min(100% - 32px, 480px);
    min-height: 0;
    padding-top: 28px;
  }

  .login::after {
    display: none;
  }

  .login__card {
    padding: 22px 18px;
  }
}
</style>

/**
 * Wallet discovery for login only.
 *
 * Login needs a wallet that can connect and sign a message; it does not need the wallet to be
 * on the deployment's cluster (the party registry is read through Lastro's own RPC). So, unlike
 * the transaction client, this listens to the Wallet Standard registry directly and accepts any
 * wallet with a Solana chain, `standard:connect` and `solana:signMessage` — Phantom, MetaMask
 * (Solana account), Solflare, Backpack… The registry protocol is the two window events below.
 */

const CONNECT = 'standard:connect'
const SIGN_MESSAGE = 'solana:signMessage'

interface StandardAccount {
  address: string
  chains: readonly string[]
  features: readonly string[]
}

export interface StandardWallet {
  name: string
  icon?: string
  chains: readonly string[]
  accounts: readonly StandardAccount[]
  features: Record<string, unknown>
}

type ConnectFeature = {
  connect: (input?: { silent?: boolean }) => Promise<{ accounts: readonly StandardAccount[] }>
}
type SignMessageFeature = {
  signMessage: (
    ...inputs: { account: StandardAccount; message: Uint8Array }[]
  ) => Promise<readonly { signature: Uint8Array }[]>
}

/** Wallets offered on the login screen even before they are detected, with an install link. */
export const RECOMMENDED_WALLETS = [
  { name: 'Phantom', install: 'https://phantom.com/download' },
  { name: 'MetaMask', install: 'https://metamask.io/download' },
] as const

export function isLoginCapable(wallet: StandardWallet): boolean {
  return (
    wallet.chains.some((chain) => chain.startsWith('solana:')) &&
    CONNECT in wallet.features &&
    SIGN_MESSAGE in wallet.features
  )
}

const registered: StandardWallet[] = []
const listeners = new Set<() => void>()
let started = false

function register(...wallets: StandardWallet[]): () => void {
  for (const wallet of wallets) if (!registered.includes(wallet)) registered.push(wallet)
  listeners.forEach((listener) => listener())
  return () => {}
}

function start(): void {
  if (started || typeof window === 'undefined') return
  started = true
  // Wallets loaded after the app announce themselves with this event…
  window.addEventListener('wallet-standard:register-wallet', ((event: CustomEvent) => {
    const callback = event.detail as (api: { register: typeof register }) => void
    callback({ register })
  }) as EventListener)
  // …and wallets loaded before it register when the app announces it is ready.
  window.dispatchEvent(
    new CustomEvent('wallet-standard:app-ready', { detail: Object.freeze({ register }) }),
  )
}

/** Detected wallets that can sign a Solana login message, deduplicated by name. */
export function loginWallets(): StandardWallet[] {
  start()
  const seen = new Set<string>()
  return registered.filter((wallet) => {
    if (!isLoginCapable(wallet) || seen.has(wallet.name)) return false
    seen.add(wallet.name)
    return true
  })
}

export function onLoginWalletsChange(listener: () => void): () => void {
  start()
  listeners.add(listener)
  return () => listeners.delete(listener)
}

/** Ask the wallet to connect and return its first Solana account. */
export async function connectForLogin(wallet: StandardWallet): Promise<StandardAccount> {
  const { accounts } = await (wallet.features[CONNECT] as ConnectFeature).connect()
  const account = (accounts.length > 0 ? accounts : wallet.accounts).find((candidate) =>
    candidate.chains.some((chain) => chain.startsWith('solana:')),
  )
  if (!account) throw new Error(`${wallet.name} has no Solana account to sign in with`)
  return account
}

export async function signLoginMessage(
  wallet: StandardWallet,
  account: StandardAccount,
  message: Uint8Array,
): Promise<Uint8Array> {
  const [output] = await (wallet.features[SIGN_MESSAGE] as SignMessageFeature).signMessage({
    account,
    message,
  })
  if (!output) throw new Error(`${wallet.name} returned no signature`)
  return output.signature
}

import { address, getAddressEncoder } from '@solana/kit'
import { solanaClient } from './client'

export interface WalletChoice {
  name: string
  address: string
}

/** Return public wallet choices currently discovered for the configured Solana chain. */
export function availableWalletChoices(): WalletChoice[] {
  return solanaClient.wallet.getState().wallets.flatMap((wallet) => {
    const account = wallet.accounts[0]
    return account ? [{ name: wallet.name, address: account.address }] : []
  })
}

/** Connect one explicitly selected Wallet Standard wallet. */
export async function connectWalletByName(name: string): Promise<void> {
  const { wallets } = solanaClient.wallet.getState()
  const wallet = wallets.find((candidate) => candidate.name === name)
  if (!wallet) throw new Error(`Wallet Standard wallet not found: ${name}`)
  await solanaClient.wallet.connect(wallet)
}

/** Connect the first compatible Wallet Standard wallet discovered by the configured Solana client. */
export async function connectFirstAvailableWallet(): Promise<void> {
  const { wallets } = solanaClient.wallet.getState()
  if (wallets.length === 0) throw new Error('No Wallet Standard wallet discovered')
  await solanaClient.wallet.connect(wallets[0]!)
}

export function currentWalletAddress(): string | null {
  return solanaClient.wallet.getState().connected?.account.address ?? null
}

/** Convert a Solana wallet address to the protocol's canonical 32-byte lowercase custodian hex. */
export function walletAddressToCustodianHex(value: string): string {
  const bytes = getAddressEncoder().encode(address(value))
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')
}

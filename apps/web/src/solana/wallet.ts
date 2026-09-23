import { address, getAddressEncoder } from '@solana/kit'
import type {
  CaptureAuthorizationChallenge,
  CaptureAuthorizationIntent,
  CaptureAuthorizationProof,
} from '../api/types'
import { webConfig } from '../config'
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

/** Sign exactly one server-issued capture intent after independently validating its public deployment anchors. */
export async function signCaptureAuthorization(
  challenge: CaptureAuthorizationChallenge,
  intent: CaptureAuthorizationIntent,
): Promise<CaptureAuthorizationProof> {
  const connected = solanaClient.wallet.getState().connected
  if (!connected) throw new Error('Connect the required wallet before authorizing a capture')
  if (connected.account.address !== challenge.requiredSigner) {
    throw new Error('Connected wallet does not match capture authorization required signer')
  }
  if (challenge.deploymentId !== webConfig.lastroDeploymentId) {
    throw new Error('Capture authorization deployment does not match browser configuration')
  }
  if (challenge.expiresAtUnix <= Math.floor(Date.now() / 1000)) {
    throw new Error('Capture authorization challenge expired before signing')
  }

  const message = decodeCanonicalBase64(challenge.messageBase64)
  const expected = new TextEncoder().encode(
    [
      'Lastro capture authorization v1',
      `challengeId=${challenge.challengeId}`,
      `deploymentId=${webConfig.lastroDeploymentId}`,
      `programId=${webConfig.lastroProgramId}`,
      `action=${intent.action}`,
      `animalId=${intent.animalId}`,
      `nextCustodian=${intent.nextCustodian ?? 'none'}`,
      ...(intent.supersedeCaptureId ? [`supersedeCaptureId=${intent.supersedeCaptureId}`] : []),
      `requiredSigner=${challenge.requiredSigner}`,
      `expiresAtUnix=${challenge.expiresAtUnix}`,
      '',
    ].join('\n'),
  )
  if (!equalBytes(message, expected)) {
    throw new Error(
      'Server capture authorization message does not match the exact trusted capture authorization message',
    )
  }

  const signature = await solanaClient.wallet.signMessage(message)
  if (signature.length !== 64) {
    throw new Error('Wallet returned an invalid capture authorization signature length')
  }
  return {
    challengeId: challenge.challengeId,
    signatureBase64: bytesToBase64(signature),
  }
}

function decodeCanonicalBase64(value: string): Uint8Array {
  let decoded: string
  try {
    decoded = atob(value)
  } catch {
    throw new Error('Capture authorization message must be canonical base64')
  }
  if (decoded.length === 0 || btoa(decoded) !== value) {
    throw new Error('Capture authorization message must be canonical base64')
  }
  return Uint8Array.from(decoded, (character) => character.charCodeAt(0))
}

function bytesToBase64(value: Uint8Array): string {
  return btoa(String.fromCharCode(...value))
}

function equalBytes(left: Uint8Array, right: Uint8Array): boolean {
  if (left.length !== right.length) return false
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return false
  }
  return true
}

/** Convert a Solana wallet address to the protocol's canonical 32-byte lowercase custodian hex. */
export function walletAddressToCustodianHex(value: string): string {
  const bytes = getAddressEncoder().encode(address(value))
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')
}

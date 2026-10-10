import { describe, expect, it } from 'vitest'
import {
  connectForLogin,
  isLoginCapable,
  loginWallets,
  onLoginWalletsChange,
  signLoginMessage,
  type StandardWallet,
} from '../../src/auth/loginWallets'

const SOLANA_ACCOUNT = { address: 'SoLAccount', chains: ['solana:mainnet'], features: [] }
const EVM_ACCOUNT = { address: '0xabc', chains: ['eip155:1'], features: [] }

function fakeWallet(name: string, overrides: Partial<StandardWallet> = {}): StandardWallet {
  return {
    name,
    chains: ['solana:mainnet'],
    accounts: [],
    features: {
      'standard:connect': { connect: async () => ({ accounts: [EVM_ACCOUNT, SOLANA_ACCOUNT] }) },
      'solana:signMessage': {
        signMessage: async ({ message }: { message: Uint8Array }) => [
          { signature: new Uint8Array(64).fill(message.length) },
        ],
      },
    },
    ...overrides,
  }
}

/** Register like a real wallet extension does, via the Wallet Standard window event. */
function announce(wallet: StandardWallet) {
  window.dispatchEvent(
    new CustomEvent('wallet-standard:register-wallet', {
      detail: (api: { register: (w: StandardWallet) => void }) => api.register(wallet),
    }),
  )
}

describe('auth/loginWallets', () => {
  /**
   * ARRANGE: wallets announcing Solana with connect + signMessage (Phantom, MetaMask on mainnet
   *          only), one without signMessage, and one Ethereum-only wallet.
   * ACTION: let them register through the Wallet Standard event and list login wallets.
   * ASSERT: Phantom and MetaMask are offered even though MetaMask is not on devnet; the others
   *         are not, and the change listener fires.
   * FAILURE MEANS: MetaMask (or any wallet not set to devnet) could never be used to sign in.
   */
  it('offers every wallet able to sign a Solana message, whatever its cluster', () => {
    let changes = 0
    const stop = onLoginWalletsChange(() => (changes += 1))

    announce(fakeWallet('Phantom', { chains: ['solana:devnet', 'solana:mainnet'] }))
    announce(fakeWallet('MetaMask'))
    announce(fakeWallet('NoSign', { features: { 'standard:connect': {} } }))
    announce(fakeWallet('EthOnly', { chains: ['eip155:1'] }))
    stop()

    expect(loginWallets().map((w) => w.name)).toEqual(['Phantom', 'MetaMask'])
    expect(changes).toBe(4)
    expect(isLoginCapable(fakeWallet('NoSign', { features: { 'standard:connect': {} } }))).toBe(
      false,
    )
  })

  /**
   * ARRANGE: a MetaMask-like wallet whose connect returns an Ethereum account first.
   * ACTION: connect for login and sign a message.
   * ASSERT: the Solana account is chosen and the wallet's signature is returned; a wallet with
   *         only an Ethereum account is refused.
   * FAILURE MEANS: the login would try to sign with a 0x… Ethereum account.
   */
  it('signs in with the Solana account, never the Ethereum one', async () => {
    const metamask = fakeWallet('MetaMask')
    const account = await connectForLogin(metamask)
    expect(account.address).toBe('SoLAccount')
    expect(await signLoginMessage(metamask, account, new Uint8Array(5))).toEqual(
      new Uint8Array(64).fill(5),
    )

    const ethOnly = fakeWallet('EthOnly', {
      features: {
        'standard:connect': { connect: async () => ({ accounts: [EVM_ACCOUNT] }) },
        'solana:signMessage': {},
      },
    })
    await expect(connectForLogin(ethOnly)).rejects.toThrow('no Solana account')
  })
})

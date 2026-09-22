import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  state: {
    wallets: [] as unknown[],
    connected: null as null | { account: { address: string } },
  },
  connect: vi.fn(),
}))

vi.mock('../../src/solana/client', () => ({
  solanaClient: {
    wallet: {
      getState: () => mocks.state,
      connect: mocks.connect,
    },
  },
}))

import { availableWalletChoices, connectFirstAvailableWallet, connectWalletByName, currentWalletAddress } from '../../src/solana/wallet'

beforeEach(() => {
  mocks.state.wallets = []
  mocks.state.connected = null
  mocks.connect.mockReset()
})

describe('solana/wallet boundary', () => {
  /**
   * ARRANGE: expose two Wallet Standard compatible wallets through the configured Solana client discovery state.
   * ACTION: invoke Lastro connection path.
   * ASSERT: the first discovered Wallet Standard wallet is passed directly to the client connect operation.
   * FAILURE MEANS: implementation diverges from the chosen Wallet Standard/Solana client integration.
   */
  it('connects through Wallet Standard discovery using the configured Solana client integration', async () => {
    const first = { name: 'First Wallet' }
    const second = { name: 'Second Wallet' }
    mocks.state.wallets = [first, second]
    mocks.connect.mockResolvedValueOnce(undefined)

    await connectFirstAvailableWallet()

    expect(mocks.connect).toHaveBeenCalledTimes(1)
    expect(mocks.connect).toHaveBeenCalledWith(first)
  })

  /**
   * ARRANGE: no compatible wallet is available and there is no connected account.
   * ACTION: request connection and inspect current authority.
   * ASSERT: connection rejects explicitly and currentWalletAddress remains null.
   * FAILURE MEANS: UI can imply authority that does not exist.
   */
  it('reports missing wallet explicitly and never fabricates a connected authority', async () => {
    await expect(connectFirstAvailableWallet()).rejects.toThrow('No Wallet Standard wallet discovered')
    expect(mocks.connect).not.toHaveBeenCalled()
    expect(currentWalletAddress()).toBeNull()
  })

  /**
   * ARRANGE: expose wallet/account proxies that throw if private-key-like properties are read.
   * ACTION: discover/connect the wallet and read the current public address.
   * ASSERT: only public wallet/account information is touched; private key, secret key, and seed properties are never read or logged.
   * FAILURE MEANS: application violates the wallet security boundary.
   */
  it('never requests stores or logs wallet private key material', async () => {
    const forbidden = new Set(['privateKey', 'secretKey', 'seed', 'mnemonic'])
    const guard = <T extends object>(value: T): T => new Proxy(value, {
      get(target, property, receiver) {
        if (typeof property === 'string' && forbidden.has(property)) throw new Error(`forbidden secret access: ${property}`)
        return Reflect.get(target, property, receiver)
      },
    })
    const wallet = guard({ name: 'Guarded Wallet' })
    const account = guard({ address: '11111111111111111111111111111111' })
    mocks.state.wallets = [wallet]
    mocks.state.connected = { account }
    mocks.connect.mockResolvedValueOnce(undefined)
    const log = vi.spyOn(console, 'log').mockImplementation(() => undefined)
    const debug = vi.spyOn(console, 'debug').mockImplementation(() => undefined)

    await expect(connectFirstAvailableWallet()).resolves.toBeUndefined()
    expect(currentWalletAddress()).toBe(account.address)
    expect(log).not.toHaveBeenCalled()
    expect(debug).not.toHaveBeenCalled()

    log.mockRestore()
    debug.mockRestore()
  })

  /**
   * ARRANGE: discovery exposes Wallet A and Wallet B with one public account each.
   * ACTION: enumerate choices and connect Wallet B by name.
   * ASSERT: only public name/address data is returned and the exact Wallet B handle is connected.
   * FAILURE MEANS: the operator cannot intentionally switch custody actors without bypassing Wallet Standard.
   */
  it('enumerates public wallet choices and connects one explicitly by name', async () => {
    const walletA = { name: 'Wallet A', accounts: [{ address: 'wallet-a' }] }
    const walletB = { name: 'Wallet B', accounts: [{ address: 'wallet-b' }] }
    mocks.state.wallets = [walletA, walletB]
    mocks.connect.mockResolvedValueOnce(undefined)

    expect(availableWalletChoices()).toEqual([
      { name: 'Wallet A', address: 'wallet-a' },
      { name: 'Wallet B', address: 'wallet-b' },
    ])
    await connectWalletByName('Wallet B')
    expect(mocks.connect).toHaveBeenCalledWith(walletB)
  })

  /**
   * ARRANGE: one discovered wallet exists but a different name is requested.
   * ACTION: connect by the missing name.
   * ASSERT: the boundary fails explicitly without connecting another wallet as a fallback.
   * FAILURE MEANS: an operator selection could silently authorize with the wrong custodian.
   */
  it('never falls back to another wallet when an explicit wallet name is missing', async () => {
    mocks.state.wallets = [{ name: 'Wallet A', accounts: [{ address: 'wallet-a' }] }]
    await expect(connectWalletByName('Wallet B')).rejects.toThrow('Wallet Standard wallet not found: Wallet B')
    expect(mocks.connect).not.toHaveBeenCalled()
  })

})

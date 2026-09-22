import { afterEach, describe, expect, it, vi } from 'vitest'

const validEnvironment = {
  VITE_API_BASE_URL: 'https://api.lastro.example/',
  VITE_SOLANA_RPC_URL: 'https://rpc.lastro.example/path?cluster=demo',
  VITE_SOLANA_CHAIN: 'solana:devnet',
  VITE_LASTRO_PROGRAM_ID: 'Vote111111111111111111111111111111111111111',
} as const

async function loadConfig(overrides: Partial<Record<keyof typeof validEnvironment, string>> = {}) {
  vi.resetModules()
  for (const [name, value] of Object.entries({ ...validEnvironment, ...overrides })) vi.stubEnv(name, value)
  return (await import('../src/config')).webConfig
}

afterEach(() => {
  vi.unstubAllEnvs()
  vi.resetModules()
})

describe('public web deployment configuration', () => {
  /**
   * ARRANGE: start from a complete valid VITE_* environment and remove each required variable independently.
   * ACTION: reset the module cache and import src/config.ts for every case.
   * ASSERT: VITE_API_BASE_URL, VITE_SOLANA_RPC_URL, VITE_SOLANA_CHAIN and VITE_LASTRO_PROGRAM_ID
   *         each fail closed before API/RPC/wallet clients can be constructed.
   * FAILURE MEANS: browser can silently target an unintended deployment or program.
   */
  it('rejects every missing required VITE deployment variable independently', async () => {
    for (const name of Object.keys(validEnvironment) as Array<keyof typeof validEnvironment>) {
      await expect(loadConfig({ [name]: '' })).rejects.toThrow(`Missing required public build configuration: ${name}`)
      vi.unstubAllEnvs()
    }
  })

  /**
   * ARRANGE: malformed API/RPC URLs including non-URL text and file:/ws:/ftp: schemes.
   * ACTION: load config in an isolated module environment.
   * ASSERT: only explicit http:/https: endpoints are accepted for API and RPC.
   * FAILURE MEANS: deployment transport semantics can differ from the reviewed configuration.
   */
  it('rejects malformed or unsupported API and RPC URL schemes', async () => {
    for (const field of ['VITE_API_BASE_URL', 'VITE_SOLANA_RPC_URL'] as const) {
      for (const value of ['not a url', 'file:///tmp/lastro', 'ws://rpc.example', 'ftp://rpc.example']) {
        await expect(loadConfig({ [field]: value })).rejects.toThrow()
        vi.unstubAllEnvs()
      }
    }
    await expect(loadConfig()).resolves.toMatchObject({
      apiBaseUrl: 'https://api.lastro.example',
      solanaRpcUrl: validEnvironment.VITE_SOLANA_RPC_URL,
    })
  })

  /**
   * ARRANGE: VITE_SOLANA_CHAIN values such as "devnet", "ethereum:1", "solana:" and one valid
   *          Wallet Standard identifier such as "solana:devnet".
   * ACTION: load config.
   * ASSERT: only a non-empty `solana:...` chain identifier is accepted and the typed value is preserved exactly.
   * FAILURE MEANS: wallet discovery/signing can target a chain identifier that is not Solana.
   */
  it('requires an explicit Wallet Standard solana chain identifier', async () => {
    for (const value of ['devnet', 'ethereum:1', 'solana:']) {
      await expect(loadConfig({ VITE_SOLANA_CHAIN: value })).rejects.toThrow(
        'VITE_SOLANA_CHAIN must be a Wallet Standard Solana chain identifier (solana:...)',
      )
      vi.unstubAllEnvs()
    }
    await expect(loadConfig({ VITE_SOLANA_CHAIN: 'solana:custom-demo' })).resolves.toMatchObject({ solanaChain: 'solana:custom-demo' })
  })

  /**
   * ARRANGE: valid build variables with API base URL ending in one slash.
   * ACTION: load config.
   * ASSERT: only the trailing API slash is normalized; RPC/chain/program strings remain exact.
   * FAILURE MEANS: client may rewrite deployment identity in a way tests do not observe.
   */
  it('normalizes only API trailing slash and preserves deployment identity values', async () => {
    await expect(loadConfig()).resolves.toEqual({
      apiBaseUrl: 'https://api.lastro.example',
      solanaRpcUrl: validEnvironment.VITE_SOLANA_RPC_URL,
      solanaChain: validEnvironment.VITE_SOLANA_CHAIN,
      lastroProgramId: validEnvironment.VITE_LASTRO_PROGRAM_ID,
    })
  })

  /**
   * ARRANGE: inspect build config contract and the resulting public configuration object.
   * ACTION: enumerate the accepted VITE_* names and serialized config values.
   * ASSERT: no Agent token, wallet secret, database credential or Station private key is exposed.
   * FAILURE MEANS: a server/device secret crossed the public browser configuration boundary.
   */
  it('contains only public deployment values and never exposes server or private-key secrets', async () => {
    const config = await loadConfig()
    expect(Object.keys(validEnvironment).sort()).toEqual([
      'VITE_API_BASE_URL',
      'VITE_LASTRO_PROGRAM_ID',
      'VITE_SOLANA_CHAIN',
      'VITE_SOLANA_RPC_URL',
    ])
    expect(Object.keys(config).sort()).toEqual(['apiBaseUrl', 'lastroProgramId', 'solanaChain', 'solanaRpcUrl'])
    expect(JSON.stringify(config)).not.toMatch(/token|secret|private.?key|database|password/i)
  })
})

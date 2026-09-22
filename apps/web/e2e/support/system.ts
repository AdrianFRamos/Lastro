import { expect, test as base, type Page } from '@playwright/test'
import { address, getProgramDerivedAddress } from '@solana/kit'
import { decodeStationEvent } from '../../src/protocol/stationEvent'
import { createHash, createPrivateKey, createPublicKey, randomBytes, sign as signEd25519, type KeyObject } from 'node:crypto'
import { Buffer } from 'node:buffer'
import process from 'node:process'
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { readFile } from 'node:fs/promises'
import { createInterface } from 'node:readline'

export type Action = 'ORIGIN' | 'TRANSFER' | 'REIDENTIFY'
export type WalletName = 'Wallet A' | 'Wallet B' | 'Wallet C'

interface ControllerReady {
  controlUrl: string
  wallets: Record<WalletName, string>
}

interface WalletActor {
  address: string
  publicKey: Buffer
  privateKey: KeyObject
}

interface RpcAccountValue {
  data: [string, 'base64']
  executable: boolean
  owner: string
}

interface EvidenceEvent {
  eventBytesBase64: string
  observedRfidHex: string
  stationPubkeyHex: string
  stationSignatureHex: string
  txSignature: string | null
}

export interface EvidencePackageDto {
  version: 1
  deploymentId: string
  animalId: string
  events: EvidenceEvent[]
}

interface AnimalProjectionDto {
  animalId: string
  visualRecoveryId: string
  currentRfidHash: string | null
  currentCustodian: string | null
  identityRevision: number
  eventSequence: number
  lastEventHash: string | null
}

interface ControllerMetrics {
  agentEvidencePosts: number
  droppedEvidenceResponses: number
  armedEvidenceResponseDrops: number
}

interface ControllerProcess {
  ready: ControllerReady
  process: ChildProcessWithoutNullStreams
  stop(): Promise<void>
}

export class BrowserSystemHarness {
  readonly apiUrl: string
  readonly rpcUrl: string
  readonly programId: string
  readonly deploymentId: Buffer
  readonly chain: string
  readonly wallets: Record<WalletName, WalletActor>
  private readonly walletSignatureCounts: Record<WalletName, number> = { 'Wallet A': 0, 'Wallet B': 0, 'Wallet C': 0 }

  constructor(
    readonly controlUrl: string,
    readyWallets: Record<WalletName, string>,
    wallets: Record<WalletName, WalletActor>,
  ) {
    this.apiUrl = requiredEnv('LASTRO_SYSTEM_API_URL').replace(/\/$/, '')
    this.rpcUrl = requiredEnv('LASTRO_SYSTEM_SOLANA_RPC_URL')
    this.programId = requiredEnv('LASTRO_PROGRAM_ID')
    this.deploymentId = decodeHex(requiredEnv('LASTRO_DEPLOYMENT_ID_HEX'), 32, 'deployment ID')
    this.chain = process.env.LASTRO_E2E_SOLANA_CHAIN ?? 'solana:localnet'
    this.wallets = wallets
    for (const name of walletNames) {
      if (readyWallets[name] !== wallets[name].address) {
        throw new Error(`${name} address from E2E controller does not match its configured keypair`)
      }
    }
  }

  walletAddress(name: WalletName): string {
    return this.wallets[name].address
  }

  walletCustodianHex(name: WalletName): string {
    return this.wallets[name].publicKey.toString('hex')
  }

  walletSignatureCount(name: WalletName): number {
    return this.walletSignatureCounts[name]
  }

  signWalletTransaction(name: WalletName, wire: Buffer): Buffer {
    const signed = signLegacyTransaction(this.wallets[name], wire)
    this.walletSignatureCounts[name] += 1
    return signed
  }

  freshVisualRecoveryId(): string {
    return `E2E-${Date.now()}-${randomBytes(6).toString('hex')}`
  }

  freshRfidHex(): string {
    let value = randomBytes(8)
    while (value.equals(Buffer.alloc(8))) value = randomBytes(8)
    return value.toString('hex')
  }

  rfidHashHex(rfidHex: string): string {
    const rfid = decodeHex(rfidHex, 8, 'RFID')
    return createHash('sha256').update(Buffer.concat([Buffer.from('LASTRO_RFID\0'), rfid])).digest('hex')
  }

  async observe(action: Action, rfidHex: string): Promise<void> {
    await jsonRequest(`${this.controlUrl}/__lastro_e2e/observe`, 'POST', { action, rfidHex })
  }

  async dropNextEvidenceResponse(): Promise<void> {
    await jsonRequest(`${this.controlUrl}/__lastro_e2e/fault/drop-next-evidence-response`, 'POST', {})
  }

  async metrics(): Promise<ControllerMetrics> {
    return await jsonRequest(`${this.controlUrl}/__lastro_e2e/metrics`, 'GET') as ControllerMetrics
  }

  async apiGet<T = unknown>(path: string): Promise<T> {
    return await jsonRequest(`${this.apiUrl}${path}`, 'GET') as T
  }

  async apiPost<T = unknown>(path: string, body: unknown): Promise<T> {
    return await jsonRequest(`${this.apiUrl}${path}`, 'POST', body) as T
  }

  async animal(animalId: string): Promise<AnimalProjectionDto> {
    return this.apiGet(`/api/animals/${animalId}`)
  }

  async animalByRecovery(visualRecoveryId: string): Promise<AnimalProjectionDto> {
    return this.apiGet(`/api/animals/by-recovery/${encodeURIComponent(visualRecoveryId)}`)
  }

  async animalByRfid(rfidHash: string): Promise<AnimalProjectionDto> {
    return this.apiGet(`/api/animals/by-rfid/${rfidHash}`)
  }

  async evidencePackage(animalId: string): Promise<EvidencePackageDto> {
    return this.apiGet(`/api/animals/${animalId}/evidence-package`)
  }

  async animalAccountSnapshot(animalId: string): Promise<{ address: string; dataBase64: string }> {
    const animalBytes = decodeHex(animalId, 32, 'AnimalID')
    const [animalAddress] = await getProgramDerivedAddress({
      programAddress: address(this.programId),
      seeds: [Buffer.from('animal'), this.deploymentId, animalBytes],
    })
    const account = await this.rpcAccount(animalAddress)
    return { address: animalAddress, dataBase64: account.data[0] }
  }

  async canonicalAnimal(animalId: string): Promise<{
    animalId: string
    currentRfidHash: string
    currentCustodian: string
    identityRevision: number
    eventSequence: number
    lastEventHash: string
  }> {
    const snapshot = await this.animalAccountSnapshot(animalId)
    const account = await this.rpcAccount(snapshot.address)
    const raw = decodeAccountData(account, this.programId, 149)
    const eventSequence = Number(raw.readBigUInt64LE(108))
    if (!Number.isSafeInteger(eventSequence)) throw new Error('Canonical event sequence exceeds JavaScript safe integer range')
    return {
      animalId: raw.subarray(8, 40).toString('hex'),
      currentRfidHash: raw.subarray(40, 72).toString('hex'),
      currentCustodian: raw.subarray(72, 104).toString('hex'),
      identityRevision: raw.readUInt32LE(104),
      eventSequence,
      lastEventHash: raw.subarray(116, 148).toString('hex'),
    }
  }

  async latestReidentifyBindingStatuses(animalId: string): Promise<{ oldStatus: number; newStatus: number }> {
    const pkg = await this.evidencePackage(animalId)
    const reidentify = [...pkg.events].reverse().find((event) => stationAction(event.eventBytesBase64) === 3)
    if (!reidentify) throw new Error('EvidencePackage has no REIDENTIFY event')
    const event = decodeStationEvent(decodeCanonicalStationEvent(reidentify.eventBytesBase64))
    const programAddress = address(this.programId)
    const [oldAddress] = await getProgramDerivedAddress({
      programAddress,
      seeds: [Buffer.from('rfid'), this.deploymentId, event.oldRfidHash],
    })
    const [newAddress] = await getProgramDerivedAddress({
      programAddress,
      seeds: [Buffer.from('rfid'), this.deploymentId, event.newRfidHash],
    })
    const oldBinding = decodeAccountData(await this.rpcAccount(oldAddress), this.programId, 74)
    const newBinding = decodeAccountData(await this.rpcAccount(newAddress), this.programId, 74)
    return { oldStatus: oldBinding[72]!, newStatus: newBinding[72]! }
  }

  async rpcAccount(address: string): Promise<RpcAccountValue> {
    const response = await this.rpc('getAccountInfo', [address, { commitment: 'finalized', encoding: 'base64' }]) as {
      value?: RpcAccountValue | null
    }
    if (!response.value) throw new Error(`Canonical Solana account does not exist: ${address}`)
    return response.value
  }

  async rpc(method: string, params: unknown[]): Promise<unknown> {
    const response = await fetch(this.rpcUrl, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
    })
    if (!response.ok) throw new Error(`Solana RPC ${method} failed with HTTP ${response.status}`)
    const body = await response.json() as { error?: unknown; result?: unknown }
    if (body.error !== undefined) throw new Error(`Solana RPC ${method} returned ${JSON.stringify(body.error)}`)
    if (!Object.hasOwn(body, 'result')) throw new Error(`Solana RPC ${method} response is missing result`)
    return body.result
  }
}

const walletNames = ['Wallet A', 'Wallet B', 'Wallet C'] as const
const fullStackEnabled = process.env.LASTRO_E2E_SYSTEM === '1'

export const test = base.extend<
  { walletHarness: void },
  { system: BrowserSystemHarness; controller: ControllerProcess }
>({
  controller: [async ({}, use) => {
    if (!fullStackEnabled) throw new Error('LASTRO_E2E_SYSTEM=1 is required for the full-stack Playwright fixture')
    const controller = await startController()
    try {
      await use(controller)
    } finally {
      await controller.stop()
    }
  }, { scope: 'worker' }],

  system: [async ({ controller }, use) => {
    const wallets: Record<WalletName, WalletActor> = {
      'Wallet A': await loadWalletActor(requiredEnv('LASTRO_SYSTEM_WALLET_A_KEYPAIR')),
      'Wallet B': await loadWalletActor(requiredEnv('LASTRO_SYSTEM_WALLET_B_KEYPAIR')),
      'Wallet C': await loadWalletActor(requiredEnv('LASTRO_SYSTEM_WALLET_C_KEYPAIR')),
    }
    await use(new BrowserSystemHarness(controller.ready.controlUrl, controller.ready.wallets, wallets))
  }, { scope: 'worker' }],

  walletHarness: [async ({ page, system }, use) => {
    await installWalletStandardHarness(page, system)
    await use()
  }, { auto: true }],
})

export { expect }
export const requireFullStack = !fullStackEnabled

export async function connectWallet(page: Page, system: BrowserSystemHarness, name: WalletName): Promise<void> {
  const selector = page.getByLabel('Wallet Standard wallet')
  await expect(selector.locator('option')).toHaveCount(4)
  await selector.selectOption(name)
  await page.getByRole('button', { name: 'Connect wallet' }).click()
  await expect(page.locator('.wallet-status code')).toHaveText(system.walletAddress(name))
}

export async function createAnimal(page: Page, system: BrowserSystemHarness, visualRecoveryId = system.freshVisualRecoveryId()): Promise<AnimalProjectionDto> {
  await page.getByLabel('Visual recovery ID').fill(visualRecoveryId)
  await page.getByRole('button', { name: 'Create Animal' }).click()
  await expect(page.getByText('Animal created. ORIGIN still requires physical RFID evidence.')).toBeVisible()
  const projection = await system.animalByRecovery(visualRecoveryId)
  await expect(definitionValue(page, 'AnimalID')).toHaveText(projection.animalId)
  return projection
}

export async function recoverAnimal(page: Page, system: BrowserSystemHarness, visualRecoveryId: string): Promise<AnimalProjectionDto> {
  await page.getByLabel('Visual recovery ID').fill(visualRecoveryId)
  await page.getByRole('button', { name: 'Find by visual recovery ID' }).click()
  const projection = await system.animalByRecovery(visualRecoveryId)
  await expect(definitionValue(page, 'AnimalID')).toHaveText(projection.animalId)
  await expect(page.getByText(`Recovered AnimalID ${projection.animalId} from the independent visual recovery identifier.`)).toBeVisible()
  return projection
}

export async function runAction(
  page: Page,
  system: BrowserSystemHarness,
  action: Action,
  rfidHex: string,
  options: { nextWallet?: WalletName } = {},
): Promise<void> {
  if (action === 'TRANSFER') {
    const next = options.nextWallet
    if (!next) throw new Error('TRANSFER browser helper requires nextWallet')
    await page.getByLabel('Next custodian wallet').fill(system.walletAddress(next))
  }
  await system.observe(action, rfidHex)
  await page.getByRole('button', { name: actionButtonName(action), exact: true }).click()
  await expect(page.getByText(`${action} finalized and canonical state verified.`)).toBeVisible({ timeout: 90_000 })
}

export async function setWalletRejection(page: Page, name: WalletName, rejected: boolean): Promise<void> {
  await page.evaluate(({ walletName, shouldReject }) => {
    const state = window as typeof window & { __lastroE2EWalletReject?: Record<string, boolean> }
    state.__lastroE2EWalletReject ??= {}
    state.__lastroE2EWalletReject[walletName] = shouldReject
  }, { walletName: name, shouldReject: rejected })
}

export function definitionValue(page: Page, label: string) {
  const card = page.getByLabel('Animal state')
  return card.locator('dt', { hasText: label }).locator('xpath=following-sibling::dd[1]')
}

export function timelineItems(page: Page) {
  return page.getByLabel('Custody timeline').locator('li')
}

async function installWalletStandardHarness(page: Page, system: BrowserSystemHarness): Promise<void> {
  await page.exposeBinding('lastroE2ESignTransaction', async (_source, walletName: string, bytes: number[]) => {
    if (!walletNames.includes(walletName as WalletName)) throw new Error(`Unknown test wallet: ${walletName}`)
    return Array.from(system.signWalletTransaction(walletName as WalletName, Buffer.from(bytes)))
  })

  const publicWallets = walletNames.map((name) => ({
    name,
    address: system.wallets[name].address,
    publicKey: Array.from(system.wallets[name].publicKey),
  }))
  await page.addInitScript(({ wallets, chain }) => {
    type RegistrationApi = { register(wallet: unknown): () => void }
    type SignInput = { transaction: Uint8Array }
    type E2EWindow = typeof window & {
      lastroE2ESignTransaction: (walletName: string, bytes: number[]) => Promise<number[]>
      __lastroE2EWalletReject?: Record<string, boolean>
    }

    const testWindow = window as E2EWindow
    testWindow.__lastroE2EWalletReject = {}
    const icon = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9ZQmcAAAAASUVORK5CYII='
    const registered = wallets.map(({ name, address, publicKey }) => {
      const account = Object.freeze({
        address,
        publicKey: new Uint8Array(publicKey),
        chains: Object.freeze([chain]),
        features: Object.freeze(['solana:signTransaction']),
        label: `${name} account`,
        icon,
      })
      const listeners = new Set<(properties: unknown) => void>()
      const wallet = Object.freeze({
        version: '1.0.0',
        name,
        icon,
        chains: Object.freeze([chain]),
        accounts: Object.freeze([account]),
        features: Object.freeze({
          'standard:connect': Object.freeze({
            version: '1.0.0',
            connect: async () => ({ accounts: [account] }),
          }),
          'standard:events': Object.freeze({
            version: '1.0.0',
            on: (event: string, listener: (properties: unknown) => void) => {
              if (event === 'change') listeners.add(listener)
              return () => listeners.delete(listener)
            },
          }),
          'solana:signTransaction': Object.freeze({
            version: '1.0.0',
            supportedTransactionVersions: Object.freeze(['legacy']),
            signTransaction: async (...inputs: SignInput[]) => {
              if (testWindow.__lastroE2EWalletReject?.[name]) {
                throw new Error(`${name} rejected transaction signing`)
              }
              return Promise.all(inputs.map(async ({ transaction }) => ({
                signedTransaction: new Uint8Array(await testWindow.lastroE2ESignTransaction(name, Array.from(transaction))),
              })))
            },
          }),
        }),
      })
      return wallet
    })

    const register = (api: RegistrationApi) => {
      for (const wallet of registered) api.register(wallet)
    }
    window.addEventListener('wallet-standard:app-ready', (event) => {
      register((event as CustomEvent<RegistrationApi>).detail)
    })
    window.dispatchEvent(new CustomEvent('wallet-standard:register-wallet', { detail: register }))
  }, { wallets: publicWallets, chain: system.chain })
}

function signLegacyTransaction(actor: WalletActor, wire: Buffer): Buffer {
  const signatureCount = decodeShortVec(wire, 0)
  if (signatureCount.value !== 1) throw new Error(`Lastro browser E2E wallet expected one transaction signature, got ${signatureCount.value}`)
  const signatureOffset = signatureCount.nextOffset
  const messageOffset = signatureOffset + 64
  if (wire.length <= messageOffset + 3) throw new Error('Serialized Solana transaction is truncated')
  const message = wire.subarray(messageOffset)
  if ((message[0]! & 0x80) !== 0) throw new Error('Lastro browser E2E wallet accepts only legacy transactions')
  if (message[0] !== 1) throw new Error(`Lastro browser E2E wallet expected one required signer, got ${message[0]}`)
  const accountCount = decodeShortVec(message, 3)
  if (accountCount.value < 1 || accountCount.nextOffset + 32 > message.length) {
    throw new Error('Serialized Solana message does not contain a fee-payer account')
  }
  const feePayer = message.subarray(accountCount.nextOffset, accountCount.nextOffset + 32)
  if (!feePayer.equals(actor.publicKey)) throw new Error('Test wallet refused a transaction for a different fee payer')
  const signature = signEd25519(null, message, actor.privateKey)
  if (signature.length !== 64) throw new Error('Ed25519 signer returned a non-64-byte signature')
  return Buffer.concat([wire.subarray(0, signatureOffset), signature, wire.subarray(signatureOffset + 64)])
}

async function startController(): Promise<ControllerProcess> {
  const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../../../..')
  const python = process.env.PYTHON ?? 'python'
  const child = spawn(python, ['-m', 'tests.system.e2e_controller'], {
    cwd: repoRoot,
    env: { ...process.env, PYTHONUNBUFFERED: '1' },
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  const stderr: string[] = []
  child.stderr.setEncoding('utf8')
  child.stderr.on('data', (chunk: string) => stderr.push(chunk))

  const lines = createInterface({ input: child.stdout })
  const ready = await new Promise<ControllerReady>((resolveReady, rejectReady) => {
    const timer = setTimeout(() => rejectReady(new Error(`Timed out waiting for E2E controller. ${stderr.join('')}`)), 20_000)
    const fail = (error: Error) => {
      clearTimeout(timer)
      rejectReady(error)
    }
    child.once('error', fail)
    child.once('exit', (code) => fail(new Error(`E2E controller exited before ready with status ${code}. ${stderr.join('')}`)))
    lines.once('line', (line) => {
      clearTimeout(timer)
      try {
        const parsed = JSON.parse(line) as Partial<ControllerReady>
        if (typeof parsed.controlUrl !== 'string' || !parsed.wallets) throw new Error('controller ready payload is incomplete')
        resolveReady(parsed as ControllerReady)
      } catch (error) {
        rejectReady(new Error(`Invalid E2E controller ready payload: ${line}; ${String(error)}`))
      }
    })
  })
  lines.close()

  return {
    ready,
    process: child,
    async stop() {
      if (child.exitCode !== null || child.signalCode !== null) return
      child.kill('SIGTERM')
      await new Promise<void>((resolveStopped) => {
        const timer = setTimeout(() => {
          if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL')
        }, 5_000)
        child.once('exit', () => {
          clearTimeout(timer)
          resolveStopped()
        })
      })
    },
  }
}

async function loadWalletActor(path: string): Promise<WalletActor> {
  const parsed = JSON.parse(await readFile(path, 'utf8')) as unknown
  if (!Array.isArray(parsed) || parsed.length !== 64 || parsed.some((value) => !Number.isInteger(value) || value < 0 || value > 255)) {
    throw new Error(`Solana keypair must be a JSON array of 64 bytes: ${path}`)
  }
  const raw = Buffer.from(parsed as number[])
  const seed = raw.subarray(0, 32)
  const expectedPublic = raw.subarray(32)
  const privateKey = createPrivateKey({
    key: Buffer.concat([Buffer.from('302e020100300506032b657004220420', 'hex'), seed]),
    format: 'der',
    type: 'pkcs8',
  })
  const publicDer = createPublicKey(privateKey).export({ format: 'der', type: 'spki' }) as Buffer
  const publicKey = publicDer.subarray(-32)
  if (!publicKey.equals(expectedPublic)) throw new Error(`Solana keypair public key does not match private seed: ${path}`)
  return { address: base58Encode(publicKey), publicKey, privateKey }
}

async function jsonRequest(url: string, method: 'GET' | 'POST', body?: unknown): Promise<unknown> {
  const response = await fetch(url, {
    method,
    headers: { accept: 'application/json', ...(body === undefined ? {} : { 'content-type': 'application/json' }) },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  const text = await response.text()
  if (!response.ok) throw new Error(`HTTP ${response.status} for ${url}: ${text}`)
  if (text === '') return null
  try {
    return JSON.parse(text)
  } catch {
    throw new Error(`Endpoint returned invalid JSON: ${url}`)
  }
}

function decodeAccountData(account: RpcAccountValue, programId: string, expectedLength: number): Buffer {
  if (account.owner !== programId || account.executable !== false) throw new Error('Canonical account owner/executable flag is invalid')
  if (!Array.isArray(account.data) || account.data[1] !== 'base64') throw new Error('Canonical account data is not base64')
  const raw = Buffer.from(account.data[0], 'base64')
  if (raw.length !== expectedLength || raw.toString('base64') !== account.data[0]) {
    throw new Error(`Canonical account must contain exactly ${expectedLength} bytes of canonical base64`)
  }
  return raw
}

function stationAction(eventBytesBase64: string): number {
  return decodeCanonicalStationEvent(eventBytesBase64)[36]!
}

function decodeCanonicalStationEvent(eventBytesBase64: string): Buffer {
  const raw = Buffer.from(eventBytesBase64, 'base64')
  if (raw.length !== 276 || raw.toString('base64') !== eventBytesBase64) throw new Error('StationEvent is not canonical 276-byte base64')
  return raw
}

function decodeShortVec(bytes: Uint8Array, offset: number): { value: number; nextOffset: number } {
  let value = 0
  let shift = 0
  let cursor = offset
  for (;;) {
    if (cursor >= bytes.length || shift > 28) throw new Error('Invalid Solana shortvec')
    const byte = bytes[cursor++]!
    value |= (byte & 0x7f) << shift
    if ((byte & 0x80) === 0) return { value, nextOffset: cursor }
    shift += 7
  }
}

function decodeHex(value: string, bytes: number, label: string): Buffer {
  if (!new RegExp(`^[0-9a-f]{${bytes * 2}}$`).test(value)) throw new Error(`${label} must be exactly ${bytes} lowercase hex bytes`)
  return Buffer.from(value, 'hex')
}

function base58Encode(data: Uint8Array): string {
  const alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
  let value = 0n
  for (const byte of data) value = (value << 8n) | BigInt(byte)
  let encoded = ''
  while (value > 0n) {
    const remainder = Number(value % 58n)
    value /= 58n
    encoded = alphabet[remainder]! + encoded
  }
  let leadingZeros = 0
  while (leadingZeros < data.length && data[leadingZeros] === 0) leadingZeros += 1
  return '1'.repeat(leadingZeros) + (encoded || (leadingZeros === 0 ? '1' : ''))
}

function actionButtonName(action: Action): string {
  if (action === 'ORIGIN') return 'Origin'
  if (action === 'TRANSFER') return 'Transfer'
  return 'Reidentify'
}

function requiredEnv(name: string): string {
  const value = process.env[name]
  if (!value?.trim()) throw new Error(`Missing required E2E environment variable: ${name}`)
  return value.trim()
}

/**
 * Live status of the services the browser talks to: the Lastro API and the Solana RPC.
 *
 * One poller is shared by every component that shows status (header, footer, home). It starts
 * with the first subscriber and stops when the last one unmounts, so pages that do not show
 * status never hit the network for it.
 */
import { onBeforeUnmount, onMounted, readonly, ref } from 'vue'
import { webConfig } from '../config'

export type ServiceState = 'checking' | 'online' | 'offline'

export interface ServiceStatus {
  state: ServiceState
  /** Round trip of the last successful check, in milliseconds. */
  latencyMs: number | null
  checkedAt: Date | null
}

const POLL_MS = 6000
const TIMEOUT_MS = 4000

function initial(): ServiceStatus {
  return { state: 'checking', latencyMs: null, checkedAt: null }
}

const api = ref<ServiceStatus>(initial())
const rpc = ref<ServiceStatus>(initial())
const slot = ref<number | null>(null)

let subscribers = 0
let timer: ReturnType<typeof setInterval> | null = null

async function timed<T>(run: (signal: AbortSignal) => Promise<T>): Promise<[T, number]> {
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), TIMEOUT_MS)
  const started = performance.now()
  try {
    const value = await run(controller.signal)
    return [value, Math.round(performance.now() - started)]
  } finally {
    clearTimeout(timeout)
  }
}

async function rpcCall(method: string, signal: AbortSignal): Promise<unknown> {
  const response = await fetch(webConfig.solanaRpcUrl, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params: [] }),
    signal,
  })
  if (!response.ok) throw new Error(`RPC ${method} returned ${response.status}`)
  const body = (await response.json()) as { result?: unknown; error?: unknown }
  if (body.error !== undefined) throw new Error(`RPC ${method} failed`)
  return body.result
}

async function checkApi(): Promise<void> {
  try {
    const [, latencyMs] = await timed(async (signal) => {
      const response = await fetch(`${webConfig.apiBaseUrl}/api/health`, { signal })
      if (!response.ok) throw new Error(`health returned ${response.status}`)
    })
    api.value = { state: 'online', latencyMs, checkedAt: new Date() }
  } catch {
    api.value = { state: 'offline', latencyMs: null, checkedAt: new Date() }
  }
}

async function checkRpc(): Promise<void> {
  try {
    const [current, latencyMs] = await timed((signal) => rpcCall('getSlot', signal))
    slot.value = typeof current === 'number' ? current : null
    rpc.value = { state: 'online', latencyMs, checkedAt: new Date() }
  } catch {
    rpc.value = { state: 'offline', latencyMs: null, checkedAt: new Date() }
  }
}

export function refreshStatus(): Promise<void> {
  return Promise.all([checkApi(), checkRpc()]).then(() => undefined)
}

function start(): void {
  subscribers += 1
  if (subscribers > 1) return
  void refreshStatus()
  timer = setInterval(() => void refreshStatus(), POLL_MS)
}

function stop(): void {
  subscribers = Math.max(0, subscribers - 1)
  if (subscribers === 0 && timer) {
    clearInterval(timer)
    timer = null
  }
}

/** Subscribe the calling component to the shared status poller. */
export function useSystemStatus() {
  onMounted(start)
  onBeforeUnmount(stop)
  return { api: readonly(api), rpc: readonly(rpc), slot: readonly(slot), refresh: refreshStatus }
}

/** Short human name for the configured Solana cluster (`solana:devnet` → `devnet`). */
export function clusterName(): string {
  const name = webConfig.solanaChain.slice('solana:'.length)
  return name === 'localnet' || webConfig.solanaRpcUrl.includes('127.0.0.1') ? 'localnet' : name
}

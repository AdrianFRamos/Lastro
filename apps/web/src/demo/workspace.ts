/**
 * State of the simulated workspace: who is signed in (a demo profile, no real account) and the
 * records each participant keeps. Both live in this browser's storage so a presentation survives
 * a reload; malformed stored data fails closed back to the example rows. Storage may be
 * unavailable (private mode, sandboxed frames): the workspace then still works for the visit.
 */
import { ref } from 'vue'
import {
  findResource,
  findRole,
  resources,
  seedRecords,
  type DemoRecord,
  type RoleDef,
} from './roles'

const SESSION_KEY = 'lastro.demo.session.v1'
const RECORDS_KEY = 'lastro.demo.records.v1'

export interface DemoSession {
  role: RoleDef['id']
  email: string
}

type RecordTable = Record<string, DemoRecord[]>

function readStorage(key: string): unknown {
  try {
    const raw = window.localStorage.getItem(key)
    return raw === null ? null : JSON.parse(raw)
  } catch {
    return null
  }
}

function writeStorage(key: string, value: unknown): void {
  try {
    if (value === null) window.localStorage.removeItem(key)
    else window.localStorage.setItem(key, JSON.stringify(value))
  } catch {
    // The change still applies for this visit.
  }
}

function seedCopy(): RecordTable {
  return Object.fromEntries(
    resources.map((resource) => [
      resource.id,
      (seedRecords[resource.id] ?? []).map((record) => ({ ...record })),
    ]),
  )
}

function isRecord(value: unknown): value is DemoRecord {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false
  const entries = Object.entries(value)
  return (
    typeof (value as { id?: unknown }).id === 'string' &&
    entries.every(([, field]) => typeof field === 'string')
  )
}

function loadRecords(): RecordTable {
  const stored = readStorage(RECORDS_KEY)
  const table = seedCopy()
  if (stored === null) return table
  if (typeof stored !== 'object' || Array.isArray(stored)) {
    writeStorage(RECORDS_KEY, null)
    return table
  }
  for (const resource of resources) {
    const rows = (stored as Record<string, unknown>)[resource.id]
    if (rows === undefined) continue
    if (!Array.isArray(rows) || !rows.every(isRecord)) {
      writeStorage(RECORDS_KEY, null)
      return seedCopy()
    }
    table[resource.id] = rows.map((row) => ({ ...row }))
  }
  return table
}

function loadSession(): DemoSession | null {
  const stored = readStorage(SESSION_KEY)
  if (stored === null) return null
  const candidate = stored as Partial<DemoSession>
  if (findRole(candidate?.role) && typeof candidate.email === 'string') {
    return { role: candidate.role as DemoSession['role'], email: candidate.email }
  }
  writeStorage(SESSION_KEY, null)
  return null
}

export const session = ref<DemoSession | null>(loadSession())
export const records = ref<RecordTable>(loadRecords())

export function signIn(role: DemoSession['role'], email: string): void {
  session.value = { role, email }
  writeStorage(SESSION_KEY, session.value)
}

export function signOut(): void {
  session.value = null
  writeStorage(SESSION_KEY, null)
}

/** Re-read both from storage, as a fresh page load would. */
export function reloadWorkspace(): void {
  session.value = loadSession()
  records.value = loadRecords()
}

export function listRecords(resourceId: string): DemoRecord[] {
  return records.value[resourceId] ?? []
}

let sequence = 0

function newId(resourceId: string): string {
  const random =
    typeof crypto !== 'undefined' && 'randomUUID' in crypto
      ? crypto.randomUUID().slice(0, 8)
      : Math.random().toString(16).slice(2, 10)
  sequence += 1
  return `${resourceId.split('.')[1]}-${random}${sequence}`
}

function persist(): void {
  writeStorage(RECORDS_KEY, records.value)
}

/** Form values as text; number inputs hand back numbers. */
export function asText(value: unknown): string {
  return value === undefined || value === null ? '' : String(value).trim()
}

/** Create a record, or update it when `id` is given. Unknown fields are dropped. */
export function saveRecord(
  resourceId: string,
  values: Record<string, unknown>,
  id?: string,
): DemoRecord {
  const resource = findResource(resourceId)
  if (!resource) throw new Error(`Unknown demo resource: ${resourceId}`)
  const clean: Record<string, string> = {}
  for (const field of resource.fields) clean[field.key] = asText(values[field.key])

  const rows = listRecords(resourceId)
  const saved: DemoRecord = { ...clean, id: id ?? newId(resourceId) }
  records.value = {
    ...records.value,
    [resourceId]: id ? rows.map((row) => (row.id === id ? saved : row)) : [...rows, saved],
  }
  persist()
  return saved
}

export function deleteRecord(resourceId: string, id: string): void {
  records.value = {
    ...records.value,
    [resourceId]: listRecords(resourceId).filter((row) => row.id !== id),
  }
  persist()
}

/** Put the example rows back, discarding every change made in this browser. */
export function resetRecords(): void {
  records.value = seedCopy()
  writeStorage(RECORDS_KEY, null)
}

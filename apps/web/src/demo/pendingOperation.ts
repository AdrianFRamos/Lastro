import type { CaptureAction, Hex32 } from '../api/types'

const STORAGE_KEY = 'lastro.pending-operation'

export interface PendingOperation {
  animalId: Hex32
  captureId: string
  action: CaptureAction
  nextCustodian: Hex32 | null
  eventHash: Hex32 | null
  txSignature: string | null
}

export function readPendingOperation(): PendingOperation | null {
  const raw = window.localStorage.getItem(STORAGE_KEY)
  if (raw === null) return null
  try {
    return parsePendingOperation(JSON.parse(raw) as Record<string, unknown>)
  } catch {
    window.localStorage.removeItem(STORAGE_KEY)
    return null
  }
}

export function writePendingOperation(operation: PendingOperation): void {
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(operation))
}

export function clearPendingOperation(): void {
  window.localStorage.removeItem(STORAGE_KEY)
}

function parsePendingOperation(value: Record<string, unknown>): PendingOperation {
  const animalId = hex32(value.animalId, 'animalId')
  const captureId = uuid(value.captureId, 'captureId')
  const action = value.action
  if (action !== 'ORIGIN' && action !== 'TRANSFER' && action !== 'REIDENTIFY') {
    throw new Error('action is invalid')
  }
  const nextCustodian = nullableHex32(value.nextCustodian, 'nextCustodian')
  const eventHash = nullableHex32(value.eventHash, 'eventHash')
  const txSignature = value.txSignature === null ? null : solanaSignature(value.txSignature, 'txSignature')
  return { animalId, captureId, action, nextCustodian, eventHash, txSignature }
}

function hex32(value: unknown, name: string): Hex32 {
  if (typeof value !== 'string' || !/^[0-9a-f]{64}$/.test(value)) {
    throw new Error(`${name} must be lowercase 32-byte hex`)
  }
  return value
}

function nullableHex32(value: unknown, name: string): Hex32 | null {
  return value === null ? null : hex32(value, name)
}

function uuid(value: unknown, name: string): string {
  if (typeof value !== 'string' || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value)) {
    throw new Error(`${name} must be a lowercase UUID`)
  }
  return value
}

function solanaSignature(value: unknown, name: string): string {
  if (
    typeof value !== 'string' ||
    value.length < 64 ||
    value.length > 88 ||
    !/^[1-9A-HJ-NP-Za-km-z]+$/.test(value)
  ) {
    throw new Error(`${name} must be base58 text for a 64-byte Solana signature`)
  }
  return value
}

import { webConfig } from '../config'
import { parseEvidencePackage, type EvidencePackage } from '../protocol/evidence'
import type {
  AnimalProjection,
  Capture,
  CaptureAction,
  CaptureAuthorizationChallenge,
  CaptureAuthorizationProof,
  EventStatus,
  EventSubmission,
  Hex32,
  InstructionDto,
  SubmissionStatus,
  TransactionData,
} from './types'

const DEFAULT_TIMEOUT_MS = 10_000

export class ApiClientError extends Error {
  constructor(
    message: string,
    readonly status: number | null,
    readonly responseBody: string | null,
  ) {
    super(message)
    this.name = 'ApiClientError'
  }
}

async function request<T>(
  path: string,
  parser: (value: unknown) => T,
  init?: RequestInit,
): Promise<T> {
  const controller = new AbortController()
  const timeout = window.setTimeout(
    () => controller.abort(new DOMException('API request timed out', 'TimeoutError')),
    DEFAULT_TIMEOUT_MS,
  )
  const externalSignal = init?.signal
  const abortFromExternal = () => controller.abort(externalSignal?.reason)
  externalSignal?.addEventListener('abort', abortFromExternal, { once: true })

  try {
    let response: Response
    try {
      response = await fetch(`${webConfig.apiBaseUrl}${path}`, {
        ...init,
        signal: controller.signal,
        headers: { 'content-type': 'application/json', ...(init?.headers ?? {}) },
      })
    } catch (error) {
      if (controller.signal.aborted) {
        throw new ApiClientError(
          externalSignal?.aborted ? 'API request was aborted' : 'API request timed out',
          null,
          null,
        )
      }
      throw new ApiClientError(
        error instanceof Error ? error.message : 'API network request failed',
        null,
        null,
      )
    }

    const text = await response.text()
    if (!response.ok) {
      throw new ApiClientError(
        `API request failed with HTTP ${response.status}`,
        response.status,
        text,
      )
    }

    let value: unknown
    try {
      value = text === '' ? null : JSON.parse(text)
    } catch {
      throw new ApiClientError('API returned invalid JSON', response.status, text)
    }
    try {
      return parser(value)
    } catch (error) {
      throw new ApiClientError(
        `API response failed validation: ${error instanceof Error ? error.message : 'invalid response'}`,
        response.status,
        text,
      )
    }
  } finally {
    window.clearTimeout(timeout)
    externalSignal?.removeEventListener('abort', abortFromExternal)
  }
}

export const api = {
  createAnimal: (visualRecoveryId: string) =>
    request('/api/animals', parseAnimal, {
      method: 'POST',
      body: JSON.stringify({ visualRecoveryId }),
    }),
  getAnimal: (id: Hex32) => request(`/api/animals/${id}`, parseAnimal),
  getAnimalByRecovery: (id: string) =>
    request(`/api/animals/by-recovery/${encodeURIComponent(id)}`, parseAnimal),
  getAnimalByRfid: (hash: Hex32) => request(`/api/animals/by-rfid/${hash}`, parseAnimal),
  getCaptureAuthorizationChallenge: (
    action: CaptureAction,
    animalId: Hex32,
    nextCustodian?: Hex32 | null,
    supersedeCaptureId?: string | null,
  ) =>
    request('/api/captures/authorization-challenge', parseCaptureAuthorizationChallenge, {
      method: 'POST',
      body: JSON.stringify({
        action,
        animalId,
        nextCustodian: nextCustodian ?? null,
        supersedeCaptureId: supersedeCaptureId ?? null,
      }),
    }),
  createCapture: (
    action: CaptureAction,
    animalId: Hex32,
    nextCustodian: Hex32 | null | undefined,
    authorization: CaptureAuthorizationProof,
    supersedeCaptureId?: string | null,
  ) =>
    request('/api/captures', parseCapture, {
      method: 'POST',
      body: JSON.stringify({
        action,
        animalId,
        nextCustodian: nextCustodian ?? null,
        authorization,
        supersedeCaptureId: supersedeCaptureId ?? null,
      }),
    }),
  getCapture: (captureId: string) =>
    request(`/api/captures/${encodeURIComponent(captureId)}`, parseCapture),
  getEvidencePackage: (animalId: Hex32) =>
    request(`/api/animals/${animalId}/evidence-package`, parseEvidencePackage),
  getTransactionData: (eventHash: Hex32) =>
    request(`/api/events/${eventHash}/transaction-data`, parseTransactionData),
  submit: (eventHash: Hex32, txSignature: string) =>
    request(`/api/events/${eventHash}/submit`, parseEventSubmission, {
      method: 'POST',
      body: JSON.stringify({ txSignature }),
    }),
  confirm: (eventHash: Hex32, txSignature: string) =>
    request(`/api/events/${eventHash}/confirm`, parseAnimal, {
      method: 'POST',
      body: JSON.stringify({ txSignature }),
    }),
}

function parseAnimal(value: unknown): AnimalProjection {
  const record = requireRecord(value, 'AnimalProjection')
  return {
    animalId: requireHex32(record.animalId, 'animalId'),
    visualRecoveryId: requireString(record.visualRecoveryId, 'visualRecoveryId'),
    currentRfidHash: nullableHex32(record.currentRfidHash, 'currentRfidHash'),
    currentCustodian: nullableHex32(record.currentCustodian, 'currentCustodian'),
    identityRevision: requireUint32(record.identityRevision, 'identityRevision'),
    eventSequence: requireSafeInteger(record.eventSequence, 'eventSequence'),
    lastEventHash: nullableHex32(record.lastEventHash, 'lastEventHash'),
  }
}

function parseCaptureAuthorizationChallenge(value: unknown): CaptureAuthorizationChallenge {
  const record = requireRecord(value, 'CaptureAuthorizationChallenge')
  const expiresAtUnix = requireSafeInteger(record.expiresAtUnix, 'expiresAtUnix')
  if (expiresAtUnix === 0) throw new Error('expiresAtUnix must be positive')
  return {
    challengeId: requireUuid(record.challengeId, 'challengeId'),
    deploymentId: requireHex32(record.deploymentId, 'deploymentId'),
    requiredSigner: requireSolanaAddress(record.requiredSigner, 'requiredSigner'),
    messageBase64: requireCanonicalBase64(record.messageBase64, 'messageBase64', 1024),
    expiresAtUnix,
  }
}

function parseCapture(value: unknown): Capture {
  const record = requireRecord(value, 'Capture')
  const action = record.action
  const status = record.status
  if (action !== 'ORIGIN' && action !== 'TRANSFER' && action !== 'REIDENTIFY')
    throw new Error('invalid capture action')
  if (
    status !== 'PENDING' &&
    status !== 'DISPATCHED' &&
    status !== 'EVIDENCE_ACCEPTED' &&
    status !== 'EXPIRED' &&
    status !== 'CANCELLED'
  ) {
    throw new Error('invalid capture status')
  }
  const eventHash = nullableHex32(record.eventHash, 'eventHash')
  const eventStatus = nullableEventStatus(record.eventStatus)
  const txSignature = nullableSolanaSignature(record.txSignature, 'txSignature')
  if (eventHash === null) {
    if (eventStatus !== null || txSignature !== null)
      throw new Error('capture event metadata requires eventHash')
  } else if (eventStatus === null) {
    throw new Error('eventStatus is required when eventHash is present')
  }
  if (eventStatus === 'EVIDENCE_ACCEPTED' && txSignature !== null) {
    throw new Error('EVIDENCE_ACCEPTED capture must not contain txSignature')
  }
  if ((eventStatus === 'SUBMITTED' || eventStatus === 'FINALIZED') && txSignature === null) {
    throw new Error(`${eventStatus} capture requires txSignature`)
  }
  return {
    captureId: requireUuid(record.captureId, 'captureId'),
    action,
    animalId: requireHex32(record.animalId, 'animalId'),
    status,
    eventHash,
    eventStatus,
    txSignature,
  }
}

function parseEventSubmission(value: unknown): EventSubmission {
  const record = requireRecord(value, 'EventSubmission')
  return {
    status: requireSubmissionStatus(record.status),
    txSignature: requireSolanaSignature(record.txSignature, 'txSignature'),
  }
}

function nullableEventStatus(value: unknown): EventStatus | null {
  if (value === null) return null
  return requireEventStatus(value)
}

function requireEventStatus(value: unknown): EventStatus {
  if (
    value !== 'EVIDENCE_ACCEPTED' &&
    value !== 'SUBMITTED' &&
    value !== 'FINALIZED' &&
    value !== 'REJECTED'
  ) {
    throw new Error('invalid event status')
  }
  return value
}

function requireSubmissionStatus(value: unknown): SubmissionStatus {
  if (value !== 'SUBMITTED' && value !== 'FINALIZED') throw new Error('invalid submission status')
  return value
}

function nullableSolanaSignature(value: unknown, name: string): string | null {
  if (value === null) return null
  return requireSolanaSignature(value, name)
}

function parseTransactionData(value: unknown): TransactionData {
  const record = requireRecord(value, 'TransactionData')
  if (record.transactionVersion !== 'legacy' && record.transactionVersion !== 'v0')
    throw new Error('invalid transactionVersion')
  if (!Array.isArray(record.instructions) || record.instructions.length !== 2)
    throw new Error('instructions must contain exactly two entries')
  return {
    requiredSigner: requireString(record.requiredSigner, 'requiredSigner'),
    lastroProgramId: requireString(record.lastroProgramId, 'lastroProgramId'),
    instructions: [
      parseInstruction(record.instructions[0]),
      parseInstruction(record.instructions[1]),
    ],
    measuredSerializedBytes: requireSafeInteger(
      record.measuredSerializedBytes,
      'measuredSerializedBytes',
    ),
    transactionVersion: record.transactionVersion,
  }
}

function parseInstruction(value: unknown): InstructionDto {
  const record = requireRecord(value, 'Instruction')
  if (!Array.isArray(record.accounts)) throw new Error('instruction accounts must be an array')
  return {
    programId: requireString(record.programId, 'programId'),
    dataBase64: requireString(record.dataBase64, 'dataBase64'),
    accounts: record.accounts.map((entry) => {
      const account = requireRecord(entry, 'AccountMeta')
      if (typeof account.isSigner !== 'boolean' || typeof account.isWritable !== 'boolean')
        throw new Error('invalid account role flags')
      return {
        address: requireString(account.address, 'address'),
        isSigner: account.isSigner,
        isWritable: account.isWritable,
      }
    }),
  }
}

function requireRecord(value: unknown, name: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value))
    throw new Error(`${name} must be an object`)
  return value as Record<string, unknown>
}

function requireString(value: unknown, name: string): string {
  if (typeof value !== 'string' || value.length === 0)
    throw new Error(`${name} must be a non-empty string`)
  return value
}

function requireUuid(value: unknown, name: string): string {
  if (
    typeof value !== 'string' ||
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value)
  ) {
    throw new Error(`${name} must be a lowercase UUID`)
  }
  return value
}

function requireSolanaAddress(value: unknown, name: string): string {
  if (
    typeof value !== 'string' ||
    value.length < 32 ||
    value.length > 44 ||
    !/^[1-9A-HJ-NP-Za-km-z]+$/.test(value)
  ) {
    throw new Error(`${name} must be a base58 Solana address`)
  }
  return value
}

function requireCanonicalBase64(value: unknown, name: string, maxDecodedBytes: number): string {
  if (
    typeof value !== 'string' ||
    value.length === 0 ||
    value.length > Math.ceil(maxDecodedBytes / 3) * 4
  ) {
    throw new Error(`${name} must be bounded canonical base64`)
  }
  let decoded: string
  try {
    decoded = atob(value)
  } catch {
    throw new Error(`${name} must be bounded canonical base64`)
  }
  if (decoded.length === 0 || decoded.length > maxDecodedBytes || btoa(decoded) !== value) {
    throw new Error(`${name} must be bounded canonical base64`)
  }
  return value
}

function requireSolanaSignature(value: unknown, name: string): string {
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

function requireHex32(value: unknown, name: string): Hex32 {
  if (typeof value !== 'string' || !/^[0-9a-f]{64}$/.test(value))
    throw new Error(`${name} must be 32-byte lowercase hex`)
  return value
}

function nullableHex32(value: unknown, name: string): Hex32 | null {
  return value === null ? null : requireHex32(value, name)
}

function requireSafeInteger(value: unknown, name: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0)
    throw new Error(`${name} must be a non-negative safe integer`)
  return value
}

function requireUint32(value: unknown, name: string): number {
  const parsed = requireSafeInteger(value, name)
  if (parsed > 0xffff_ffff) throw new Error(`${name} must fit u32`)
  return parsed
}

export type { EvidencePackage }

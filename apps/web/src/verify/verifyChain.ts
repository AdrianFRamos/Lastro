import {
  address,
  getAddressDecoder,
  getBase58Decoder,
  getBase58Encoder,
  getProgramDerivedAddress,
  type Address,
} from '@solana/kit'
import type { EvidencePackage } from '../protocol/evidence'
import { decodeStationEvent } from '../protocol/stationEvent'
import { eventHash } from '../protocol/hash'
import { webConfig } from '../config'
import { SECP256R1_PROGRAM_ID } from '../solana/constants'
import type { VerificationLayerResult } from './types'

export interface CanonicalVerificationOptions {
  rpcUrl?: string
  programId?: string
  fetchFn?: typeof fetch
  rpcTimeoutMs?: number
  trustedAuthority?: string
  trustedDeploymentId?: string
}

interface RpcAccount {
  data: [string, string]
  executable: boolean
  owner: string
}

interface ExpectedAccountMeta {
  address: string
  isSigner: boolean
  isWritable: boolean
}

interface ExpectedInstruction {
  programId: string
  accounts: ExpectedAccountMeta[]
  data: Uint8Array
}

const SYSTEM_PROGRAM_ID = '11111111111111111111111111111111'
const INSTRUCTIONS_SYSVAR_ID = 'Sysvar1nstructions1111111111111111111111111'
const CONFIG_SEED = new TextEncoder().encode('config')
const ANIMAL_SEED = new TextEncoder().encode('animal')
const RFID_SEED = new TextEncoder().encode('rfid')
const DEFAULT_RPC_TIMEOUT_MS = 10_000
const MAX_VERIFICATION_DURATION_MS = 30_000

export async function verifyCanonicalChainState(
  pkg: EvidencePackage,
  options: CanonicalVerificationOptions = {},
): Promise<VerificationLayerResult> {
  try {
    if (pkg.events.length === 0 || pkg.events.length > 128) {
      return invalid('Evidence history exceeds the 128-event verification limit')
    }
    if (pkg.deploymentId !== (options.trustedDeploymentId ?? webConfig.lastroDeploymentId)) {
      return invalid('EvidencePackage deployment differs from the trusted browser deployment')
    }
    const trustedAuthority = options.trustedAuthority ?? webConfig.lastroAuthority
    if (!trustedAuthority) {
      return {
        layer: 'ON_CHAIN_STATE',
        status: 'NOT_CHECKED',
        detail:
          'A trusted deployment authority must be configured independently before canonical verification',
      }
    }
    const expectedAuthority = decodeCanonicalBase58(trustedAuthority, 32, 'deployment authority')
    const deployment = decodeHex(pkg.deploymentId, 32)
    const animalId = decodeHex(pkg.animalId, 32)
    const events = pkg.events.map((entry) => ({
      raw: decodeBase64(entry.eventBytesBase64, 276),
      stationPubkey: decodeHex(entry.stationPubkeyHex, 33),
      stationSignature: decodeHex(entry.stationSignatureHex, 64),
      txSignature: entry.txSignature,
    }))
    const last = decodeStationEvent(events.at(-1)!.raw)
    const programId = options.programId ?? webConfig.lastroProgramId
    const rpcUrl = options.rpcUrl ?? webConfig.solanaRpcUrl
    const fetchFn = withTimeout(
      options.fetchFn ?? fetch,
      options.rpcTimeoutMs ?? DEFAULT_RPC_TIMEOUT_MS,
      MAX_VERIFICATION_DURATION_MS,
    )
    const programAddress = address(programId)

    const [configPda, configBump] = await getProgramDerivedAddress({
      programAddress,
      seeds: [new TextEncoder().encode('config'), deployment],
    })
    const configData = await getOwnedAccount(configPda, 106, programId, rpcUrl, fetchFn)
    await requireDiscriminator(configData, 'ProtocolConfig')
    if (!equalBytes(configData.subarray(8, 40), expectedAuthority)) {
      return invalid(
        'Canonical ProtocolConfig authority differs from the trusted deployment authority',
      )
    }
    if (!equalBytes(configData.subarray(40, 72), deployment))
      return invalid('ProtocolConfig deployment_id does not match the package')
    if (configData[105] !== configBump)
      return invalid('Canonical ProtocolConfig PDA bump is invalid')
    const canonicalStation = configData.subarray(72, 105)
    if (events.some((entry) => !equalBytes(entry.stationPubkey, canonicalStation))) {
      return invalid('Package contains a Station key not authorized by canonical ProtocolConfig')
    }

    const transactionSignatureCount = events.filter((entry) => entry.txSignature !== null).length
    if (transactionSignatureCount !== 0 && transactionSignatureCount !== events.length) {
      return invalid(
        'EvidencePackage must contain transaction signatures for every event or for none of them',
      )
    }
    if (transactionSignatureCount === events.length) {
      for (const entry of events) {
        await verifyFinalizedEventTransaction(
          entry.txSignature!,
          entry.raw,
          entry.stationPubkey,
          entry.stationSignature,
          programId,
          rpcUrl,
          fetchFn,
        )
      }
    }

    const [animalPda, animalBump] = await getProgramDerivedAddress({
      programAddress,
      seeds: [new TextEncoder().encode('animal'), deployment, animalId],
    })
    const animalData = await getOwnedAccount(animalPda, 149, programId, rpcUrl, fetchFn)
    await requireDiscriminator(animalData, 'AnimalState')
    if (!equalBytes(animalData.subarray(8, 40), animalId))
      return invalid('Canonical AnimalState AnimalID does not match the package')
    if (animalData[148] !== animalBump) return invalid('Canonical AnimalState PDA bump is invalid')
    if (!equalBytes(animalData.subarray(40, 72), last.newRfidHash))
      return invalid('Canonical current RFID differs from package terminal state')
    if (!equalBytes(animalData.subarray(72, 104), last.toCustodian))
      return invalid('Canonical custodian differs from package terminal state')
    if (readU32(animalData, 104) !== last.identityRevision)
      return invalid('Canonical identity revision differs from package terminal state')
    if (readU64(animalData, 108) !== last.eventSequence)
      return invalid('Canonical event sequence differs from package terminal state')
    if (!equalBytes(animalData.subarray(116, 148), await eventHash(events.at(-1)!.raw))) {
      return invalid('Canonical last event hash differs from package terminal event')
    }

    const currentBinding = await loadBinding(
      programAddress,
      deployment,
      last.newRfidHash,
      programId,
      rpcUrl,
      fetchFn,
    )
    if (
      !equalBytes(currentBinding.subarray(8, 40), animalId) ||
      !equalBytes(currentBinding.subarray(40, 72), last.newRfidHash) ||
      currentBinding[72] !== 1
    ) {
      return invalid('Canonical current RfidBinding is not ACTIVE for this AnimalID')
    }

    for (const entry of events) {
      const event = decodeStationEvent(entry.raw)
      if (event.action !== 3) continue
      const retired = await loadBinding(
        programAddress,
        deployment,
        event.oldRfidHash,
        programId,
        rpcUrl,
        fetchFn,
      )
      if (
        !equalBytes(retired.subarray(8, 40), animalId) ||
        !equalBytes(retired.subarray(40, 72), event.oldRfidHash) ||
        retired[72] !== 2
      ) {
        return invalid('A previous RFID from REIDENTIFY is not RETIRED canonically')
      }
    }

    if (transactionSignatureCount === 0) {
      return {
        layer: 'ON_CHAIN_STATE',
        status: 'NOT_CHECKED',
        detail:
          'Canonical ProtocolConfig, AnimalState, and RFID bindings match, but transaction signatures are absent so the historical Solana envelopes were not checked',
      }
    }
    return {
      layer: 'ON_CHAIN_STATE',
      status: 'VALID',
      detail:
        'Canonical ProtocolConfig, AnimalState, and RFID bindings match the independently verified history, and every supplied transaction is finalized with the exact Lastro envelope',
    }
  } catch (error) {
    if (error instanceof RpcUnavailableError) {
      return { layer: 'ON_CHAIN_STATE', status: 'NOT_CHECKED', detail: error.message }
    }
    return invalid(error instanceof Error ? error.message : 'Canonical Solana verification failed')
  }
}

async function verifyFinalizedEventTransaction(
  txSignature: string,
  rawEvent: Uint8Array,
  stationPubkey: Uint8Array,
  stationSignature: Uint8Array,
  programId: string,
  rpcUrl: string,
  fetchFn: typeof fetch,
): Promise<void> {
  decodeCanonicalBase58(txSignature, 64, 'transaction signature')
  const result = await rpcResult(rpcUrl, fetchFn, 'getTransaction', [
    txSignature,
    { commitment: 'finalized', encoding: 'json', maxSupportedTransactionVersion: 0 },
  ])
  if (result === null) throw new Error('Evidence transaction is not finalized on canonical Solana')
  if (!isRecord(result))
    throw new Error('Finalized Solana transaction response has an invalid shape')
  if (result.version !== undefined && result.version !== null && result.version !== 'legacy') {
    throw new Error('Evidence transaction is not the required legacy transaction version')
  }
  if (!isRecord(result.meta) || result.meta.err !== null) {
    throw new Error('Evidence transaction did not finalize successfully')
  }
  if (!isRecord(result.transaction))
    throw new Error('Finalized Solana transaction payload is invalid')
  const transaction = result.transaction
  if (
    !Array.isArray(transaction.signatures) ||
    transaction.signatures.length !== 1 ||
    transaction.signatures[0] !== txSignature
  ) {
    throw new Error('Finalized Solana transaction signature does not match EvidencePackage')
  }
  if (!isRecord(transaction.message))
    throw new Error('Finalized Solana transaction message is invalid')

  const expected = await expectedEventEnvelope(rawEvent, stationPubkey, stationSignature, programId)
  verifyCompiledMessage(transaction.message, expected.requiredSigner, expected.instructions)
}

async function expectedEventEnvelope(
  rawEvent: Uint8Array,
  stationPubkey: Uint8Array,
  stationSignature: Uint8Array,
  programId: string,
): Promise<{ requiredSigner: string; instructions: [ExpectedInstruction, ExpectedInstruction] }> {
  const event = decodeStationEvent(rawEvent)
  const programAddress = address(programId)
  const [config] = await getProgramDerivedAddress({
    programAddress,
    seeds: [CONFIG_SEED, event.deploymentId],
  })
  const [animal] = await getProgramDerivedAddress({
    programAddress,
    seeds: [ANIMAL_SEED, event.deploymentId, event.animalId],
  })
  const requiredSigner = getAddressDecoder().decode(
    event.action === 1 ? event.toCustodian : event.fromCustodian,
  )

  let accounts: ExpectedAccountMeta[]
  if (event.action === 1) {
    const [binding] = await getProgramDerivedAddress({
      programAddress,
      seeds: [RFID_SEED, event.deploymentId, event.newRfidHash],
    })
    accounts = [
      expectedMeta(requiredSigner, true, true),
      expectedMeta(config, false, false),
      expectedMeta(animal, false, true),
      expectedMeta(binding, false, true),
      expectedMeta(INSTRUCTIONS_SYSVAR_ID, false, false),
      expectedMeta(SYSTEM_PROGRAM_ID, false, false),
    ]
  } else if (event.action === 2) {
    const [binding] = await getProgramDerivedAddress({
      programAddress,
      seeds: [RFID_SEED, event.deploymentId, event.oldRfidHash],
    })
    accounts = [
      expectedMeta(requiredSigner, true, false),
      expectedMeta(config, false, false),
      expectedMeta(animal, false, true),
      expectedMeta(binding, false, false),
      expectedMeta(INSTRUCTIONS_SYSVAR_ID, false, false),
    ]
  } else {
    const [oldBinding] = await getProgramDerivedAddress({
      programAddress,
      seeds: [RFID_SEED, event.deploymentId, event.oldRfidHash],
    })
    const [newBinding] = await getProgramDerivedAddress({
      programAddress,
      seeds: [RFID_SEED, event.deploymentId, event.newRfidHash],
    })
    accounts = [
      expectedMeta(requiredSigner, true, true),
      expectedMeta(config, false, false),
      expectedMeta(animal, false, true),
      expectedMeta(oldBinding, false, true),
      expectedMeta(newBinding, false, true),
      expectedMeta(INSTRUCTIONS_SYSVAR_ID, false, false),
      expectedMeta(SYSTEM_PROGRAM_ID, false, false),
    ]
  }

  const secpData = new Uint8Array(113)
  secpData[0] = 1
  const descriptor = new DataView(secpData.buffer)
  for (const [index, value] of [16, 0, 80, 0, 8, 276, 1].entries()) {
    descriptor.setUint16(2 + index * 2, value, true)
  }
  secpData.set(stationSignature, 16)
  secpData.set(stationPubkey, 80)

  const actionName = event.action === 1 ? 'origin' : event.action === 2 ? 'transfer' : 'reidentify'
  const discriminator = await hashDiscriminator(`global:${actionName}`)
  const lastroData = new Uint8Array(8 + rawEvent.length)
  lastroData.set(discriminator, 0)
  lastroData.set(rawEvent, 8)

  return {
    requiredSigner,
    instructions: [
      { programId: SECP256R1_PROGRAM_ID, accounts: [], data: secpData },
      { programId, accounts, data: lastroData },
    ],
  }
}

function verifyCompiledMessage(
  message: Record<string, unknown>,
  requiredSigner: string,
  expectedInstructions: readonly ExpectedInstruction[],
): void {
  if (
    message.addressTableLookups !== undefined &&
    (!Array.isArray(message.addressTableLookups) || message.addressTableLookups.length !== 0)
  ) {
    throw new Error('Evidence transaction unexpectedly uses address table lookups')
  }
  if (
    !Array.isArray(message.accountKeys) ||
    !message.accountKeys.every((value) => typeof value === 'string')
  ) {
    throw new Error('Evidence transaction account keys are invalid')
  }
  const accountKeys = message.accountKeys as string[]
  if (!isRecord(message.header)) throw new Error('Evidence transaction header is invalid')
  const requiredSignatures = integerField(
    message.header.numRequiredSignatures,
    'numRequiredSignatures',
  )
  const readonlySigned = integerField(
    message.header.numReadonlySignedAccounts,
    'numReadonlySignedAccounts',
  )
  const readonlyUnsigned = integerField(
    message.header.numReadonlyUnsignedAccounts,
    'numReadonlyUnsignedAccounts',
  )
  if (
    requiredSignatures !== 1 ||
    readonlySigned > requiredSignatures ||
    accountKeys[0] !== requiredSigner
  ) {
    throw new Error(
      'Evidence transaction signer header does not match canonical custodian authority',
    )
  }
  if (readonlyUnsigned > accountKeys.length - requiredSignatures) {
    throw new Error('Evidence transaction account header is inconsistent')
  }

  const roles = new Map<string, { signer: boolean; writable: boolean }>()
  roles.set(requiredSigner, { signer: true, writable: true })
  for (const instruction of expectedInstructions) {
    mergeRole(roles, instruction.programId, false, false)
    for (const account of instruction.accounts) {
      mergeRole(roles, account.address, account.isSigner, account.isWritable)
    }
  }
  if (roles.size !== accountKeys.length)
    throw new Error('Evidence transaction contains unexpected accounts')
  for (let index = 0; index < accountKeys.length; index += 1) {
    const signer = index < requiredSignatures
    const writable = signer
      ? index < requiredSignatures - readonlySigned
      : index < accountKeys.length - readonlyUnsigned
    const expected = roles.get(accountKeys[index]!)
    if (!expected || expected.signer !== signer || expected.writable !== writable) {
      throw new Error('Evidence transaction account privileges do not match the Lastro envelope')
    }
  }

  if (
    !Array.isArray(message.instructions) ||
    message.instructions.length !== expectedInstructions.length
  ) {
    throw new Error(
      'Evidence transaction must contain exactly the frozen two-instruction Lastro envelope',
    )
  }
  for (
    let instructionIndex = 0;
    instructionIndex < expectedInstructions.length;
    instructionIndex += 1
  ) {
    const actual = message.instructions[instructionIndex]
    const expected = expectedInstructions[instructionIndex]!
    if (!isRecord(actual))
      throw new Error('Evidence transaction contains an invalid compiled instruction')
    const programIdIndex = integerField(actual.programIdIndex, 'programIdIndex')
    if (accountKeys[programIdIndex] !== expected.programId) {
      throw new Error(
        `Evidence transaction instruction ${instructionIndex} targets the wrong program`,
      )
    }
    if (!Array.isArray(actual.accounts) || actual.accounts.length !== expected.accounts.length) {
      throw new Error(
        `Evidence transaction instruction ${instructionIndex} has the wrong account count`,
      )
    }
    for (let accountIndex = 0; accountIndex < expected.accounts.length; accountIndex += 1) {
      const keyIndex = integerField(actual.accounts[accountIndex], 'account index')
      if (accountKeys[keyIndex] !== expected.accounts[accountIndex]!.address) {
        throw new Error(
          `Evidence transaction instruction ${instructionIndex} account order is invalid`,
        )
      }
    }
    if (typeof actual.data !== 'string')
      throw new Error('Evidence transaction instruction data is invalid')
    const actualData = decodeCanonicalBase58(actual.data, expected.data.length, 'instruction data')
    if (!equalBytes(actualData, expected.data)) {
      throw new Error(
        `Evidence transaction instruction ${instructionIndex} bytes do not match the exact StationEvent envelope`,
      )
    }
  }
}

function withTimeout(
  fetchFn: typeof fetch,
  timeoutMs: number,
  totalBudgetMs: number,
): typeof fetch {
  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0)
    throw new Error('RPC timeout must be a positive finite number')
  const deadline = Date.now() + totalBudgetMs
  return (async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const remainingMs = deadline - Date.now()
    if (remainingMs <= 0) {
      throw new RpcUnavailableError('Solana RPC verification exceeded its total time budget')
    }
    // The signal remains live while the caller reads the response body.
    return fetchFn(input, {
      ...init,
      signal: AbortSignal.timeout(Math.min(timeoutMs, remainingMs)),
    })
  }) as typeof fetch
}

async function rpcResult(
  rpcUrl: string,
  fetchFn: typeof fetch,
  method: string,
  params: unknown[],
): Promise<unknown> {
  let response: Response
  try {
    response = await fetchFn(rpcUrl, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
    })
  } catch {
    throw new RpcUnavailableError('Solana RPC is unavailable; canonical state was not checked')
  }
  if (!response.ok)
    throw new RpcUnavailableError(
      'Solana RPC returned an HTTP error; canonical state was not checked',
    )
  let body: unknown
  try {
    body = await response.json()
  } catch {
    throw new RpcUnavailableError(
      'Solana RPC returned invalid JSON; canonical state was not checked',
    )
  }
  if (!isRecord(body) || body.error !== undefined || !('result' in body)) {
    throw new RpcUnavailableError('Solana RPC returned an error; canonical state was not checked')
  }
  return body.result
}

function expectedMeta(
  value: Address | string,
  isSigner: boolean,
  isWritable: boolean,
): ExpectedAccountMeta {
  return { address: String(value), isSigner, isWritable }
}

function mergeRole(
  roles: Map<string, { signer: boolean; writable: boolean }>,
  account: string,
  signer: boolean,
  writable: boolean,
): void {
  const current = roles.get(account) ?? { signer: false, writable: false }
  roles.set(account, { signer: current.signer || signer, writable: current.writable || writable })
}

function integerField(value: unknown, name: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0)
    throw new Error(`Evidence transaction ${name} is invalid`)
  return value as number
}

function decodeCanonicalBase58(value: string, expectedBytes: number, label: string): Uint8Array {
  let decoded: Uint8Array
  try {
    decoded = new Uint8Array(getBase58Encoder().encode(value))
  } catch {
    throw new Error(`Evidence ${label} is not valid base58`)
  }
  if (decoded.length !== expectedBytes || getBase58Decoder().decode(decoded) !== value) {
    throw new Error(`Evidence ${label} is not canonical base58 for ${expectedBytes} bytes`)
  }
  return decoded
}

async function hashDiscriminator(value: string): Promise<Uint8Array> {
  return new Uint8Array(
    await crypto.subtle.digest('SHA-256', new TextEncoder().encode(value)),
  ).subarray(0, 8)
}

async function loadBinding(
  programAddress: Address,
  deployment: Uint8Array,
  rfidHash: Uint8Array,
  programId: string,
  rpcUrl: string,
  fetchFn: typeof fetch,
): Promise<Uint8Array> {
  const [pda, bump] = await getProgramDerivedAddress({
    programAddress,
    seeds: [new TextEncoder().encode('rfid'), deployment, rfidHash],
  })
  const data = await getOwnedAccount(pda, 74, programId, rpcUrl, fetchFn)
  await requireDiscriminator(data, 'RfidBinding')
  if (data[73] !== bump) throw new Error('Canonical RfidBinding PDA bump is invalid')
  if (data[72] !== 1 && data[72] !== 2)
    throw new Error('Canonical RfidBinding has an invalid status')
  return data
}

async function getOwnedAccount(
  accountAddress: string,
  expectedLength: number,
  programId: string,
  rpcUrl: string,
  fetchFn: typeof fetch,
): Promise<Uint8Array> {
  let response: Response
  try {
    response = await fetchFn(rpcUrl, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: 1,
        method: 'getAccountInfo',
        params: [accountAddress, { commitment: 'finalized', encoding: 'base64' }],
      }),
    })
  } catch {
    throw new RpcUnavailableError('Solana RPC is unavailable; canonical state was not checked')
  }
  if (!response.ok)
    throw new RpcUnavailableError(
      'Solana RPC returned an HTTP error; canonical state was not checked',
    )
  let body: unknown
  try {
    body = await response.json()
  } catch {
    throw new RpcUnavailableError(
      'Solana RPC returned invalid JSON; canonical state was not checked',
    )
  }
  if (!isRecord(body) || body.error !== undefined)
    throw new RpcUnavailableError('Solana RPC returned an error; canonical state was not checked')
  const result = body.result
  if (!isRecord(result) || !isRecord(result.value))
    throw new Error('Canonical Solana account does not exist')
  const account = result.value as unknown as RpcAccount
  if (account.owner !== programId || account.executable !== false) {
    throw new Error('Canonical account owner/layout does not match the configured Lastro program')
  }
  if (!Array.isArray(account.data) || account.data.length !== 2 || account.data[1] !== 'base64')
    throw new Error('Canonical account data is not base64 encoded')
  const data = decodeBase64(account.data[0], expectedLength)
  return data
}

async function requireDiscriminator(data: Uint8Array, accountName: string): Promise<void> {
  const expected = new Uint8Array(
    await crypto.subtle.digest('SHA-256', new TextEncoder().encode(`account:${accountName}`)),
  ).subarray(0, 8)
  if (!equalBytes(data.subarray(0, 8), expected))
    throw new Error(`Canonical ${accountName} discriminator is invalid`)
}

function readU32(data: Uint8Array, offset: number): number {
  return new DataView(data.buffer, data.byteOffset, data.byteLength).getUint32(offset, true)
}

function readU64(data: Uint8Array, offset: number): bigint {
  return new DataView(data.buffer, data.byteOffset, data.byteLength).getBigUint64(offset, true)
}

function decodeHex(value: string, bytes: number): Uint8Array {
  if (!new RegExp(`^[0-9a-f]{${bytes * 2}}$`).test(value))
    throw new Error(`Expected ${bytes} lowercase hex bytes`)
  const out = new Uint8Array(bytes)
  for (let index = 0; index < bytes; index += 1)
    out[index] = Number.parseInt(value.slice(index * 2, index * 2 + 2), 16)
  return out
}

function decodeBase64(value: string, bytes: number): Uint8Array {
  let binary: string
  try {
    binary = atob(value)
  } catch {
    throw new Error('Canonical account contains invalid base64')
  }
  if (btoa(binary) !== value || binary.length !== bytes)
    throw new Error(`Canonical account must contain exactly ${bytes} bytes`)
  return Uint8Array.from(binary, (character) => character.charCodeAt(0))
}

function equalBytes(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((byte, index) => byte === right[index])
}

function invalid(detail: string): VerificationLayerResult {
  return { layer: 'ON_CHAIN_STATE', status: 'INVALID', detail }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

class RpcUnavailableError extends Error {}

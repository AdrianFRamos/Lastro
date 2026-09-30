/**
 * Canonical Solana comparison for a v2 EvidencePackage. Runs only after local verification:
 * every account and transaction is read at finalized commitment and compared with bytes the
 * browser derives itself. The deployment authority must be pinned independently of the API.
 */
import { getAddressDecoder, getBase58Decoder, getBase58Encoder } from '@solana/kit'
import type { EvidencePackage } from '../protocol/evidence'
import { equalBytes, fromBase64, fromHex, rfidHash } from '../protocol/hash'
import { decodeV2DomainEvent, V2EventType } from '../protocol/v2/domainEvent'
import { webConfig } from '../config'
import {
  COMPUTE_BUDGET_PROGRAM_ID,
  MAX_TRAILING_COMPUTE_BUDGET_INSTRUCTIONS,
  SECP256R1_PROGRAM_ID,
} from '../solana/constants'
import {
  assetPda,
  configPda,
  discriminator,
  eventPda,
  rfidBindingPda,
  stationPda,
  stationRegistryPda,
} from '../solana/pda'
import { secpHeader } from '../solana/transaction'
import type { VerificationLayerResult } from './types'

export interface CanonicalVerificationOptions {
  rpcUrl?: string
  programId?: string
  fetchFn?: typeof fetch
  rpcTimeoutMs?: number
  trustedAuthority?: string
  trustedDeploymentId?: string
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

interface Rpc {
  programId: string
  rpcUrl: string
  fetchFn: typeof fetch
}

const SYSTEM_PROGRAM_ID = '11111111111111111111111111111111'
const INSTRUCTIONS_SYSVAR_ID = 'Sysvar1nstructions1111111111111111111111111'
const DEFAULT_RPC_TIMEOUT_MS = 10_000
// Sized for MAX_PACKAGE_EVENTS with bounded parallel RPC; unreachable RPC still ends NOT_CHECKED.
const MAX_VERIFICATION_DURATION_MS = 120_000
const RPC_CONCURRENCY = 8
const RFID_STATUS_ACTIVE = 1
const RFID_STATUS_RETIRED = 2
const ZERO32 = new Uint8Array(32)
const INSTRUCTION_NAMES: Partial<Record<number, string>> = {
  [V2EventType.ObservationRecorded]: 'record_observation',
  [V2EventType.IdentifierBound]: 'bind_identifier',
  [V2EventType.IdentifierReplaced]: 'replace_identifier',
}

export async function verifyCanonicalChainState(
  pkg: EvidencePackage,
  options: CanonicalVerificationOptions = {},
): Promise<VerificationLayerResult> {
  try {
    if (pkg.deploymentId !== (options.trustedDeploymentId ?? webConfig.lastroDeploymentId)) {
      return invalid('EvidencePackage deployment differs from the trusted browser deployment')
    }
    const programId = options.programId ?? webConfig.lastroProgramId
    if (pkg.lastroProgramId !== programId) {
      return invalid('EvidencePackage program differs from the trusted browser program')
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
    const authority = decodeCanonicalBase58(trustedAuthority, 32, 'deployment authority')
    const deployment = fromHex(pkg.deploymentId, 32)
    const assetId = fromHex(pkg.asset.assetId, 32)
    const rpc: Rpc = {
      programId,
      rpcUrl: options.rpcUrl ?? webConfig.solanaRpcUrl,
      fetchFn: withTimeout(
        options.fetchFn ?? fetch,
        options.rpcTimeoutMs ?? DEFAULT_RPC_TIMEOUT_MS,
        MAX_VERIFICATION_DURATION_MS,
      ),
    }

    const [configAddress, configBump] = await configPda(programId, deployment)
    const config = await getAccount(rpc, configAddress, 'ProtocolConfigV2', 221)
    if (!equalBytes(config.subarray(8, 40), authority))
      return invalid(
        'Canonical ProtocolConfig authority differs from the trusted deployment authority',
      )
    if (!equalBytes(config.subarray(40, 72), deployment) || config[220] !== configBump)
      return invalid('Canonical ProtocolConfig does not match its deployment PDA')

    const [assetAddress, assetBump] = await assetPda(programId, deployment, assetId)
    const asset = await getAccount(rpc, assetAddress, 'AssetState', 341)
    if (
      !equalBytes(asset.subarray(8, 40), assetId) ||
      !equalBytes(asset.subarray(42, 74), deployment) ||
      asset[340] !== assetBump
    )
      return invalid('Canonical AssetState does not match its asset PDA')
    if (!equalBytes(asset.subarray(74, 106), fromHex(pkg.asset.custodian, 32)))
      return invalid('Canonical custodian differs from the package')
    if (readU64(asset, 226) !== BigInt(pkg.asset.stateVersion))
      return invalid(
        'Canonical state version differs from the package (asset changed since export?)',
      )
    if (!equalBytes(asset.subarray(234, 266), fromHex(pkg.asset.lastEventHash, 32)))
      return invalid('Canonical last event hash differs from the package')
    const currentRfid = pkg.asset.currentRfidHash ? fromHex(pkg.asset.currentRfidHash, 32) : ZERO32
    if (!equalBytes(asset.subarray(308, 340), currentRfid))
      return invalid('Canonical current RFID differs from the package')

    let previousRfid: Uint8Array = ZERO32
    // Per-event RPC checks are independent; run them with bounded concurrency so long
    // histories (many presence proofs) fit the total RPC budget. The RFID replay itself is
    // sequential and local.
    const eventChecks: Array<() => Promise<string | null>> = []
    for (const [index, evidence] of pkg.events.entries()) {
      const label = `Event ${index + 1}`
      const envelopeBytes = fromBase64(evidence.envelopeBytesBase64, 220)
      const envelope = decodeV2DomainEvent(envelopeBytes)
      const eventId = fromHex(envelope.eventId, 32)
      const instructionName = INSTRUCTION_NAMES[envelope.eventType]
      if (!instructionName) return invalid(`${label} has no Station instruction to verify`)

      let rfidArgs: Uint8Array[] = []
      let retiredRfid: Uint8Array | null = null
      if (evidence.observedRfidHex !== null) {
        const observed = await rfidHash(fromHex(evidence.observedRfidHex, 8))
        if (envelope.eventType === V2EventType.IdentifierBound) rfidArgs = [observed]
        if (envelope.eventType === V2EventType.IdentifierReplaced) {
          rfidArgs = [previousRfid, observed]
          retiredRfid = previousRfid
        }
        previousRfid = observed
      }

      eventChecks.push(async () => {
        const [anchorAddress, anchorBump] = await eventPda(programId, deployment, eventId)
        const anchor = await getAccount(rpc, anchorAddress, 'EventAnchor', 259)
        if (
          !equalBytes(anchor.subarray(8, 40), eventId) ||
          !equalBytes(anchor.subarray(40, 72), deployment) ||
          !equalBytes(anchor.subarray(72, 104), assetId) ||
          !equalBytes(anchor.subarray(104, 136), fromHex(envelope.sourceId, 32)) ||
          !equalBytes(anchor.subarray(194, 226), fromHex(envelope.payloadHash, 32)) ||
          !equalBytes(anchor.subarray(226, 258), fromHex(evidence.eventHash, 32)) ||
          anchor[258] !== anchorBump
        )
          return `${label} is not the EventAnchor Solana finalized`
        if (retiredRfid) {
          const retired = await getRfidBinding(rpc, deployment, retiredRfid)
          if (!equalBytes(retired.subarray(8, 40), assetId) || retired[72] !== RFID_STATUS_RETIRED)
            return `${label} replaced RFID is not RETIRED for this asset on-chain`
        }
        const station = fromHex(envelope.sourceId, 32)
        const [registry] = await stationRegistryPda(programId, deployment)
        const [stationAccount] = await stationPda(programId, deployment, station)
        const bindings = await Promise.all(
          rfidArgs.map(async (hash) =>
            String((await rfidBindingPda(programId, deployment, hash))[0]),
          ),
        )
        const secp = new Uint8Array(113)
        secp.set(secpHeader())
        secp.set(fromHex(evidence.stationSignatureHex, 64), 16)
        secp.set(fromHex(evidence.stationPubkeyHex, 33), 80)
        const lastroData = concat(
          await discriminator('global', instructionName),
          assetId,
          eventId,
          station,
          envelopeBytes,
          ...rfidArgs,
        )
        await verifyFinalizedTransaction(rpc, evidence.txSignature, label, (signer) => {
          return [
            { programId: SECP256R1_PROGRAM_ID, accounts: [], data: secp },
            {
              programId,
              data: lastroData,
              accounts: [
                meta(signer, true, rfidArgs.length > 0),
                meta(configAddress, false, false),
                meta(registry, false, false),
                meta(stationAccount, false, false),
                meta(assetAddress, false, true),
                meta(anchorAddress, false, true),
                ...bindings.map((binding) => meta(binding, false, true)),
                meta(INSTRUCTIONS_SYSVAR_ID, false, false),
                meta(SYSTEM_PROGRAM_ID, false, false),
              ],
            },
          ]
        })
        return null
      })
    }
    const failure = (await runLimited(eventChecks, RPC_CONCURRENCY)).find((detail) => detail)
    if (failure) return invalid(failure)

    if (!equalBytes(currentRfid, ZERO32)) {
      const active = await getRfidBinding(rpc, deployment, currentRfid)
      if (!equalBytes(active.subarray(8, 40), assetId) || active[72] !== RFID_STATUS_ACTIVE)
        return invalid('Canonical current RfidBinding is not ACTIVE for this asset')
    }

    const acceptDiscriminator = await discriminator('global', 'accept_custody_transfer')
    for (const [index, transfer] of pkg.custodyTransfers.entries()) {
      const recipient = getAddressDecoder().decode(fromHex(transfer.newCustodian, 32))
      await verifyFinalizedTransaction(
        rpc,
        transfer.txSignature,
        `Custody transfer ${index + 1}`,
        (signer, message) => {
          if (signer !== recipient)
            throw new Error(`Custody transfer ${index + 1} was not accepted by its recipient`)
          const accepted = acceptInstruction(message, programId, acceptDiscriminator)
          if (!accepted.includes(String(assetAddress)))
            throw new Error(`Custody transfer ${index + 1} does not accept this asset`)
          return null
        },
      )
    }

    return {
      layer: 'ON_CHAIN_STATE',
      status: 'VALID',
      detail:
        'Canonical ProtocolConfig, AssetState, EventAnchors and RFID bindings match the verified history, and every supplied transaction is finalized with the exact Lastro instructions',
    }
  } catch (error) {
    if (error instanceof RpcUnavailableError) {
      return { layer: 'ON_CHAIN_STATE', status: 'NOT_CHECKED', detail: error.message }
    }
    return invalid(error instanceof Error ? error.message : 'Canonical Solana verification failed')
  }
}

/**
 * Fetch a finalized, successful single-signer transaction and compare it with the expected
 * instructions (or, when `expect` returns null, leave the instruction check to the caller).
 */
async function verifyFinalizedTransaction(
  rpc: Rpc,
  txSignature: string,
  label: string,
  expect: (signer: string, message: Record<string, unknown>) => ExpectedInstruction[] | null,
): Promise<void> {
  decodeCanonicalBase58(txSignature, 64, 'transaction signature')
  const result = await rpcResult(rpc, 'getTransaction', [
    txSignature,
    { commitment: 'finalized', encoding: 'json', maxSupportedTransactionVersion: 0 },
  ])
  if (result === null) throw new Error(`${label} transaction is not finalized on canonical Solana`)
  if (!isRecord(result) || !isRecord(result.meta) || result.meta.err !== null)
    throw new Error(`${label} transaction did not finalize successfully`)
  if (result.version !== undefined && result.version !== null && result.version !== 'legacy')
    throw new Error(`${label} transaction is not a legacy transaction`)
  const transaction = result.transaction
  if (
    !isRecord(transaction) ||
    !Array.isArray(transaction.signatures) ||
    transaction.signatures.length !== 1 ||
    transaction.signatures[0] !== txSignature ||
    !isRecord(transaction.message)
  )
    throw new Error(`${label} transaction signature does not match the package`)
  const message = transaction.message
  if (!Array.isArray(message.accountKeys) || typeof message.accountKeys[0] !== 'string')
    throw new Error(`${label} transaction account keys are invalid`)
  const expected = expect(message.accountKeys[0], message)
  if (expected) verifyCompiledMessage(message, message.accountKeys[0], expected, label)
}

/** Accounts of the one Lastro `accept_custody_transfer` instruction in `message`. */
function acceptInstruction(
  message: Record<string, unknown>,
  programId: string,
  acceptDiscriminator: Uint8Array,
): string[] {
  const keys = message.accountKeys as string[]
  const instructions = Array.isArray(message.instructions) ? message.instructions : []
  const matches = instructions.filter(
    (instruction) =>
      isRecord(instruction) &&
      keys[integerField(instruction.programIdIndex, 'programIdIndex')] === programId &&
      typeof instruction.data === 'string' &&
      equalBytes(
        decodeCanonicalBase58(instruction.data, 16, 'instruction data').subarray(0, 8),
        acceptDiscriminator,
      ),
  ) as Record<string, unknown>[]
  if (matches.length !== 1 || !Array.isArray(matches[0]!.accounts))
    throw new Error('Custody transaction must contain exactly one accept_custody_transfer')
  return (matches[0]!.accounts as unknown[]).map(
    (index) => keys[integerField(index, 'account index')]!,
  )
}

async function getRfidBinding(rpc: Rpc, deployment: Uint8Array, hash: Uint8Array) {
  const [bindingAddress, bump] = await rfidBindingPda(rpc.programId, deployment, hash)
  const binding = await getAccount(rpc, bindingAddress, 'RfidBinding', 74)
  if (!equalBytes(binding.subarray(40, 72), hash) || binding[73] !== bump)
    throw new Error('Canonical RfidBinding does not match its PDA')
  return binding
}

async function getAccount(
  rpc: Rpc,
  accountAddress: string,
  accountName: string,
  expectedLength: number,
): Promise<Uint8Array> {
  const result = await rpcResult(rpc, 'getAccountInfo', [
    String(accountAddress),
    { commitment: 'finalized', encoding: 'base64' },
  ])
  if (!isRecord(result) || !isRecord(result.value))
    throw new Error(`Canonical ${accountName} account does not exist`)
  const account = result.value
  if (account.owner !== rpc.programId || account.executable !== false)
    throw new Error(`Canonical ${accountName} is not owned by the configured Lastro program`)
  if (
    !Array.isArray(account.data) ||
    account.data.length !== 2 ||
    account.data[1] !== 'base64' ||
    typeof account.data[0] !== 'string'
  )
    throw new Error('Canonical account data is not base64 encoded')
  const data = fromBase64(account.data[0], expectedLength)
  if (!equalBytes(data.subarray(0, 8), await discriminator('account', accountName)))
    throw new Error(`Canonical ${accountName} discriminator is invalid`)
  return data
}

function verifyCompiledMessage(
  message: Record<string, unknown>,
  requiredSigner: string,
  expectedInstructions: readonly ExpectedInstruction[],
  label: string,
): void {
  if (
    message.addressTableLookups !== undefined &&
    (!Array.isArray(message.addressTableLookups) || message.addressTableLookups.length !== 0)
  ) {
    throw new Error(`${label} transaction unexpectedly uses address table lookups`)
  }
  if (
    !Array.isArray(message.accountKeys) ||
    !message.accountKeys.every((value) => typeof value === 'string')
  ) {
    throw new Error(`${label} transaction account keys are invalid`)
  }
  const accountKeys = message.accountKeys as string[]
  if (!isRecord(message.header)) throw new Error(`${label} transaction header is invalid`)
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
    throw new Error(`${label} transaction signer header does not match a single Lastro signer`)
  }
  if (readonlyUnsigned > accountKeys.length - requiredSignatures) {
    throw new Error(`${label} transaction account header is inconsistent`)
  }

  if (!Array.isArray(message.instructions)) {
    throw new Error(`${label} transaction instructions are invalid`)
  }
  // Up to two account-less ComputeBudget instructions (priority fees) may follow the
  // frozen pair; the program itself rejects anything else after the Lastro instruction.
  const trailingCount = message.instructions.length - expectedInstructions.length
  if (trailingCount < 0 || trailingCount > MAX_TRAILING_COMPUTE_BUDGET_INSTRUCTIONS) {
    throw new Error(
      `${label} transaction must contain the Lastro instructions plus at most two ComputeBudget instructions`,
    )
  }

  const roles = new Map<string, { signer: boolean; writable: boolean }>()
  roles.set(requiredSigner, { signer: true, writable: true })
  if (trailingCount > 0) mergeRole(roles, COMPUTE_BUDGET_PROGRAM_ID, false, false)
  for (const instruction of expectedInstructions) {
    mergeRole(roles, instruction.programId, false, false)
    for (const account of instruction.accounts) {
      mergeRole(roles, account.address, account.isSigner, account.isWritable)
    }
  }
  if (roles.size !== accountKeys.length)
    throw new Error(`${label} transaction contains unexpected accounts`)
  for (let index = 0; index < accountKeys.length; index += 1) {
    const signer = index < requiredSignatures
    const writable = signer
      ? index < requiredSignatures - readonlySigned
      : index < accountKeys.length - readonlyUnsigned
    const expected = roles.get(accountKeys[index]!)
    if (!expected || expected.signer !== signer || expected.writable !== writable) {
      throw new Error(`${label} transaction account privileges do not match the Lastro envelope`)
    }
  }

  for (const actual of message.instructions.slice(expectedInstructions.length)) {
    if (
      !isRecord(actual) ||
      accountKeys[integerField(actual.programIdIndex, 'programIdIndex')] !==
        COMPUTE_BUDGET_PROGRAM_ID ||
      !Array.isArray(actual.accounts) ||
      actual.accounts.length !== 0
    ) {
      throw new Error(`${label} transaction contains a non-ComputeBudget trailing instruction`)
    }
  }
  for (
    let instructionIndex = 0;
    instructionIndex < expectedInstructions.length;
    instructionIndex += 1
  ) {
    const actual = message.instructions[instructionIndex]
    const expected = expectedInstructions[instructionIndex]!
    if (!isRecord(actual))
      throw new Error(`${label} transaction contains an invalid compiled instruction`)
    const programIdIndex = integerField(actual.programIdIndex, 'programIdIndex')
    if (accountKeys[programIdIndex] !== expected.programId) {
      throw new Error(
        `${label} transaction instruction ${instructionIndex} targets the wrong program`,
      )
    }
    if (!Array.isArray(actual.accounts) || actual.accounts.length !== expected.accounts.length) {
      throw new Error(
        `${label} transaction instruction ${instructionIndex} has the wrong account count`,
      )
    }
    for (let accountIndex = 0; accountIndex < expected.accounts.length; accountIndex += 1) {
      const keyIndex = integerField(actual.accounts[accountIndex], 'account index')
      if (accountKeys[keyIndex] !== expected.accounts[accountIndex]!.address) {
        throw new Error(
          `${label} transaction instruction ${instructionIndex} account order is invalid`,
        )
      }
    }
    if (typeof actual.data !== 'string')
      throw new Error(`${label} transaction instruction data is invalid`)
    const actualData = decodeCanonicalBase58(actual.data, expected.data.length, 'instruction data')
    if (!equalBytes(actualData, expected.data)) {
      throw new Error(
        `${label} transaction instruction ${instructionIndex} bytes do not match the exact Station envelope`,
      )
    }
  }
}

/** Run async tasks with at most `limit` in flight, preserving result order. */
async function runLimited<T>(tasks: Array<() => Promise<T>>, limit: number): Promise<T[]> {
  const results: T[] = new Array(tasks.length)
  let next = 0
  const worker = async () => {
    while (next < tasks.length) {
      const index = next++
      results[index] = await tasks[index]!()
    }
  }
  await Promise.all(Array.from({ length: Math.min(limit, tasks.length) }, worker))
  return results
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

async function rpcResult(rpc: Rpc, method: string, params: unknown[]): Promise<unknown> {
  let response: Response
  try {
    response = await rpc.fetchFn(rpc.rpcUrl, {
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

function meta(value: string, isSigner: boolean, isWritable: boolean): ExpectedAccountMeta {
  return { address: String(value), isSigner, isWritable }
}

function concat(...parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((total, part) => total + part.length, 0))
  let offset = 0
  for (const part of parts) {
    out.set(part, offset)
    offset += part.length
  }
  return out
}

function readU64(data: Uint8Array, offset: number): bigint {
  return new DataView(data.buffer, data.byteOffset, data.byteLength).getBigUint64(offset, true)
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

function invalid(detail: string): VerificationLayerResult {
  return { layer: 'ON_CHAIN_STATE', status: 'INVALID', detail }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

class RpcUnavailableError extends Error {}

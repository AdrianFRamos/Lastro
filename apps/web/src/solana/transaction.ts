import {
  AccountRole,
  address,
  appendTransactionMessageInstructions,
  compileTransaction,
  createTransactionMessage,
  getAddressDecoder,
  getBase64EncodedWireTransaction,
  getSignatureFromTransaction,
  getTransactionSize,
  pipe,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
  type Address,
  type Base64EncodedWireTransaction,
  type Instruction,
} from '@solana/kit'
import type {
  AccountMetaDto,
  CaptureAction,
  Hex32,
  InstructionDto,
  TransactionData,
} from '../api/types'
import { webConfig } from '../config'
import { equalBytes, fromBase64, fromHex, sha256, stationId } from '../protocol/hash'
import { verifyStationSignature } from '../protocol/p256'
import {
  decodeV2DomainEvent,
  V2EventType,
  v2EventHashOfBytes,
  v2IdentifierPayloadHash,
} from '../protocol/v2/domainEvent'
import { solanaClient } from './client'
import {
  COMPUTE_BUDGET_PROGRAM_ID,
  DEFAULT_COMPUTE_UNIT_LIMIT,
  MAX_PRIORITY_FEE_LAMPORTS,
  MAX_TRAILING_COMPUTE_BUDGET_INSTRUCTIONS,
  SECP256R1_PROGRAM_ID,
  SOLANA_LEGACY_V0_MAX_TRANSACTION_BYTES,
} from './constants'
import {
  assetPda,
  configPda,
  custodyIntentId,
  discriminator,
  eventPda,
  intentPda,
  rfidBindingPda,
  stationPda,
  stationRegistryPda,
} from './pda'

const SYSTEM_PROGRAM_ID = '11111111111111111111111111111111'
const INSTRUCTIONS_SYSVAR_ID = 'Sysvar1nstructions1111111111111111111111111'
const SECP256R1_DATA_LENGTH = 113
/** discriminator(8) + subject(32) + event_id(32) + station_id(32): the envelope starts here. */
export const STATION_EVENT_ENVELOPE_OFFSET = 104
const ENVELOPE_LENGTH = 220
const ZERO32 = new Uint8Array(32)
const INTENT_TYPE_CUSTODY_TRANSFER = 2

const STATION_INSTRUCTIONS: Record<
  CaptureAction,
  { name: string; type: V2EventType; hashes: number }
> = {
  BIND_IDENTIFIER: { name: 'bind_identifier', type: V2EventType.IdentifierBound, hashes: 1 },
  REPLACE_IDENTIFIER: {
    name: 'replace_identifier',
    type: V2EventType.IdentifierReplaced,
    hashes: 2,
  },
  OBSERVE_PRESENCE: {
    name: 'record_observation',
    type: V2EventType.ObservationRecorded,
    hashes: 0,
  },
}

/** What the browser independently expects the API-prepared transaction to do. */
export type ExpectedTransaction =
  | {
      kind: 'STATION_EVENT'
      action: CaptureAction
      assetId: Hex32
      eventHash: Hex32
      /** Wallet that must authorize: the asset's current custodian (from the capture challenge). */
      requiredSigner: string
    }
  | {
      kind: 'REGISTER_ASSET'
      assetId: Hex32
      custodian: Hex32
      assetType: number
      availableWeightGrams: number
    }
  | { kind: 'CUSTODY_PROPOSE'; assetId: Hex32; transferId: string; recipient: Hex32 }
  | { kind: 'CUSTODY_ACCEPT'; assetId: Hex32; transferId: string; recipient: Hex32 }

interface ValidatedTransaction {
  /** Protocol instructions followed by any validated priority-fee instructions. */
  instructions: readonly Instruction[]
}

export interface SignedLastroTransaction {
  signature: string
  wireTransactionBase64: Base64EncodedWireTransaction
  lastValidBlockHeight?: bigint
}

export type SignedLastroTransactionObserver = (signed: SignedLastroTransaction) => void

export interface WalletSigningContext {
  accountAddress: string
  supportedTransactionVersions: ReadonlySet<'legacy' | 0 | 1>
  canSignTransactions: boolean
}

/** Validate the connected wallet authority/capability before any signing request is created. */
export function validateWalletSigningContext(
  transactionData: TransactionData,
  context: WalletSigningContext,
): 'legacy' | 0 {
  if (context.accountAddress !== transactionData.requiredSigner) {
    throw new Error('Connected wallet is not the authority required for this transition')
  }
  const requestedVersion = transactionData.transactionVersion === 'legacy' ? 'legacy' : 0
  if (!context.supportedTransactionVersions.has(requestedVersion)) {
    throw new Error(
      `Connected wallet does not support ${transactionData.transactionVersion} transactions`,
    )
  }
  if (!context.canSignTransactions) {
    throw new Error(
      'Connected wallet must support signTransaction so Lastro can verify the signed envelope before broadcast',
    )
  }
  return requestedVersion
}

/**
 * Independently validate the complete API transaction descriptor before any wallet popup:
 * program, instruction order, every account, every data byte and the signer all follow from
 * what the browser expects, never from what the API claims.
 */
export async function validateLastroTransactionData(
  transactionData: TransactionData,
  expected: ExpectedTransaction,
): Promise<ValidatedTransaction> {
  const programId = webConfig.lastroProgramId
  if (transactionData.lastroProgramId !== programId) {
    throw new Error('Lastro program id does not match browser deployment configuration')
  }
  const protocolCount = expected.kind === 'STATION_EVENT' ? 2 : 1
  const trailing = transactionData.instructions.slice(protocolCount)
  if (
    transactionData.instructions.length < protocolCount ||
    trailing.length > MAX_TRAILING_COMPUTE_BUDGET_INSTRUCTIONS
  ) {
    throw new Error(
      `Lastro transaction must contain ${protocolCount} protocol instruction(s) and at most two priority-fee instructions`,
    )
  }
  validatePriorityFeeInstructions(trailing)
  if (transactionData.measuredSerializedBytes > SOLANA_LEGACY_V0_MAX_TRANSACTION_BYTES) {
    throw new Error('API transaction measurement exceeds Solana legacy/v0 size limit')
  }
  if (
    transactionData.transactionVersion !== 'legacy' &&
    transactionData.transactionVersion !== 'v0'
  ) {
    throw new Error('Unsupported transaction version requested by API')
  }
  const lastro = transactionData.instructions[protocolCount - 1]!
  if (lastro.programId !== programId) {
    throw new Error(`Instruction ${protocolCount - 1} must be the configured Lastro program`)
  }

  const deployment = fromHex(webConfig.lastroDeploymentId, 32)
  const assetId = fromHex(expected.assetId, 32)
  const [config] = await configPda(programId, deployment)
  const [asset] = await assetPda(programId, deployment, assetId)
  const data = decodeCanonicalBase64(lastro.dataBase64)
  let signer: string
  let accounts: AccountMetaDto[]
  let wanted: Uint8Array

  switch (expected.kind) {
    case 'STATION_EVENT': {
      signer = expected.requiredSigner
      const spec = STATION_INSTRUCTIONS[expected.action]
      if (data.length !== STATION_EVENT_ENVELOPE_OFFSET + ENVELOPE_LENGTH + 32 * spec.hashes) {
        throw new Error('Lastro instruction data has the wrong length for this event type')
      }
      const envelopeBytes = data.slice(
        STATION_EVENT_ENVELOPE_OFFSET,
        STATION_EVENT_ENVELOPE_OFFSET + ENVELOPE_LENGTH,
      )
      const envelope = decodeV2DomainEvent(envelopeBytes)
      if (envelope.eventType !== spec.type)
        throw new Error('Envelope event type differs from the capture')
      if (
        envelope.deploymentId !== webConfig.lastroDeploymentId ||
        envelope.subjectId !== expected.assetId
      )
        throw new Error('Envelope does not belong to this deployment and asset')
      if ((await v2EventHashOfBytes(envelopeBytes)) !== expected.eventHash)
        throw new Error('Envelope hash differs from the accepted Station evidence')
      const hashes = Array.from({ length: spec.hashes }, (_, index) =>
        data.slice(STATION_EVENT_ENVELOPE_OFFSET + ENVELOPE_LENGTH + 32 * index).subarray(0, 32),
      )
      const [oldHash, newHash] =
        spec.hashes === 1
          ? [ZERO32, hashes[0]!]
          : spec.hashes === 2
            ? [hashes[0]!, hashes[1]!]
            : [null, null]
      if (
        oldHash &&
        newHash &&
        (await v2IdentifierPayloadHash(oldHash, newHash)) !== envelope.payloadHash
      )
        throw new Error('RFID arguments do not match the payload the Station signed')

      const source = fromHex(envelope.sourceId, 32)
      const station = await verifySecpDescriptor(
        transactionData.instructions[0]!,
        envelopeBytes,
        source,
      )
      wanted = concat(
        await discriminator('global', spec.name),
        assetId,
        fromHex(envelope.eventId, 32),
        station,
        envelopeBytes,
        ...hashes,
      )
      const [registry] = await stationRegistryPda(programId, deployment)
      const [stationAccount] = await stationPda(programId, deployment, station)
      const [anchor] = await eventPda(programId, deployment, fromHex(envelope.eventId, 32))
      accounts = [
        meta(signer, true, spec.hashes > 0),
        meta(config, false, false),
        meta(registry, false, false),
        meta(stationAccount, false, false),
        meta(asset, false, true),
        meta(anchor, false, true),
      ]
      for (const hash of hashes)
        accounts.push(meta((await rfidBindingPda(programId, deployment, hash))[0], false, true))
      accounts.push(
        meta(INSTRUCTIONS_SYSVAR_ID, false, false),
        meta(SYSTEM_PROGRAM_ID, false, false),
      )
      break
    }
    case 'REGISTER_ASSET': {
      signer = transactionData.requiredSigner
      if (webConfig.lastroAuthority && signer !== webConfig.lastroAuthority)
        throw new Error('Asset registration must be signed by the pinned deployment authority')
      const weight = new Uint8Array(8)
      new DataView(weight.buffer).setBigUint64(0, BigInt(expected.availableWeightGrams), true)
      wanted = concat(
        await discriminator('global', 'register_asset'),
        assetId,
        Uint8Array.of(expected.assetType),
        fromHex(expected.custodian, 32),
        ZERO32,
        assetId,
        weight,
      )
      accounts = [
        meta(signer, true, true),
        meta(config, false, false),
        meta(asset, false, true),
        meta(SYSTEM_PROGRAM_ID, false, false),
      ]
      break
    }
    case 'CUSTODY_PROPOSE': {
      signer = transactionData.requiredSigner
      const intentId = await custodyIntentId(expected.transferId)
      const [intent] = await intentPda(programId, deployment, assetId, intentId)
      // disc | intent_id | asset_id | intent type u16 | expected_state_version u64 | nonce u64 | expires_at i64 | payload
      if (data.length !== 8 + 32 + 32 + 2 + 8 + 8 + 8 + 32)
        throw new Error('create_intent data has the wrong length')
      const view = new DataView(data.buffer, data.byteOffset, data.byteLength)
      const nonce = view.getBigUint64(82, true)
      if (
        nonce !==
        new DataView(fromHex(expected.transferId.replaceAll('-', ''), 16).buffer).getBigUint64(
          0,
          true,
        )
      )
        throw new Error('Custody intent nonce is not derived from this transfer')
      const payload = concat(
        deployment,
        assetId,
        fromHex(expected.recipient, 32),
        data.slice(74, 82),
        data.slice(82, 90),
      )
      const payloadHash = await sha256(new TextEncoder().encode('LASTRO_V2_INTENT\0'), payload)
      wanted = concat(
        await discriminator('global', 'create_intent'),
        intentId,
        assetId,
        Uint8Array.of(INTENT_TYPE_CUSTODY_TRANSFER, 0),
        data.slice(74, 98),
        payloadHash,
      )
      accounts = [
        meta(signer, true, true),
        meta(config, false, false),
        meta(asset, false, false),
        meta(intent, false, true),
        meta(SYSTEM_PROGRAM_ID, false, false),
      ]
      break
    }
    case 'CUSTODY_ACCEPT': {
      signer = getAddressDecoder().decode(fromHex(expected.recipient, 32))
      const [intent] = await intentPda(
        programId,
        deployment,
        assetId,
        await custodyIntentId(expected.transferId),
      )
      if (data.length !== 16) throw new Error('accept_custody_transfer data has the wrong length')
      wanted = concat(await discriminator('global', 'accept_custody_transfer'), data.slice(8, 16))
      accounts = [
        meta(signer, true, false),
        meta(config, false, false),
        meta(asset, false, true),
        meta(intent, false, true),
      ]
      break
    }
  }

  if (transactionData.requiredSigner !== signer) {
    throw new Error('Required signer does not match the authority this transition needs')
  }
  if (!equalBytes(data, wanted)) {
    throw new Error('Lastro instruction data differs from the independently derived bytes')
  }
  if (lastro.accounts.length !== accounts.length) {
    throw new Error('Lastro instruction contains an unexpected account count')
  }
  accounts.forEach((want, index) => {
    const actual = lastro.accounts[index]!
    if (
      actual.address !== want.address ||
      actual.isSigner !== want.isSigner ||
      actual.isWritable !== want.isWritable
    ) {
      throw new Error(
        `Lastro instruction account ${index} does not match the derived account contract`,
      )
    }
  })

  return { instructions: transactionData.instructions.map(toInstruction) }
}

/** Custodian wallet address for 32-byte custodian hex. */
export function custodianAddress(hex: Hex32): string {
  return getAddressDecoder().decode(fromHex(hex, 32))
}

/**
 * Validate, construct, sign, and submit a Lastro v2 transaction.
 * Signing remains inside the connected Wallet Standard account; private key material never enters Lastro code.
 */
export async function submitLastroTransaction(
  transactionData: TransactionData,
  expected: ExpectedTransaction,
  onSigned?: SignedLastroTransactionObserver,
): Promise<string> {
  const validated = await validateLastroTransactionData(transactionData, expected)
  const walletState = solanaClient.wallet.getState()
  const connected = walletState.connected
  if (!connected?.signer)
    throw new Error('A signing-capable Wallet Standard account must be connected')
  const requestedVersion = validateWalletSigningContext(transactionData, {
    accountAddress: connected.account.address,
    supportedTransactionVersions: connected.supportedTransactionVersions,
    canSignTransactions: 'modifyAndSignTransactions' in connected.signer,
  })

  const latest = await solanaClient.rpc.getLatestBlockhash({ commitment: 'finalized' }).send()
  const message = pipe(
    createTransactionMessage({ version: requestedVersion }),
    (value) => setTransactionMessageFeePayerSigner(connected.signer!, value),
    (value) => setTransactionMessageLifetimeUsingBlockhash(latest.value, value),
    (value) => appendTransactionMessageInstructions(validated.instructions, value),
  )

  const unsignedTransaction = compileTransaction(message)
  const unsignedSize = getTransactionSize(unsignedTransaction)
  if (unsignedSize !== transactionData.measuredSerializedBytes) {
    throw new Error(
      `Locally compiled transaction size ${unsignedSize} does not match API measurement ${transactionData.measuredSerializedBytes}`,
    )
  }
  if (unsignedSize > SOLANA_LEGACY_V0_MAX_TRANSACTION_BYTES) {
    throw new Error(
      `Serialized Lastro transaction is ${unsignedSize} bytes and exceeds the 1232-byte limit`,
    )
  }

  const signedTransaction = await signTransactionMessageWithSigners(message)
  if (!equalBytes(unsignedTransaction.messageBytes, signedTransaction.messageBytes)) {
    throw new Error('Wallet changed the validated Lastro transaction message')
  }
  const signedSize = getTransactionSize(signedTransaction)
  if (signedSize !== unsignedSize || signedSize > SOLANA_LEGACY_V0_MAX_TRANSACTION_BYTES) {
    throw new Error('Signed Lastro transaction size differs from the validated transaction')
  }

  const wire = getBase64EncodedWireTransaction(signedTransaction)
  const signature = String(getSignatureFromTransaction(signedTransaction))
  if (onSigned) {
    onSigned({
      signature,
      wireTransactionBase64: wire,
      lastValidBlockHeight: latest.value.lastValidBlockHeight,
    })
  }

  const rpcSignature = String(
    await solanaClient.rpc
      .sendTransaction(wire, {
        encoding: 'base64',
        preflightCommitment: 'finalized',
      })
      .send(),
  )
  if (rpcSignature !== signature) {
    throw new Error(
      'Solana RPC returned a transaction signature different from the signed envelope',
    )
  }
  return signature
}

/** Re-submit the exact signed bytes retained before an ambiguous RPC result. */
export async function rebroadcastSignedLastroTransaction(
  signed: SignedLastroTransaction,
): Promise<string> {
  const rpcSignature = String(
    await solanaClient.rpc
      .sendTransaction(signed.wireTransactionBase64, {
        encoding: 'base64',
        preflightCommitment: 'finalized',
      })
      .send(),
  )
  if (rpcSignature !== signed.signature) {
    throw new Error(
      'Solana RPC returned a transaction signature different from the retained signed envelope',
    )
  }
  return signed.signature
}

/**
 * Only ComputeBudget unit-limit (tag 2) and unit-price (tag 3) instructions may follow the
 * protocol instructions, at most one of each, and the implied priority fee is capped so a
 * compromised API cannot make the wallet sign an arbitrary fee.
 */
export function validatePriorityFeeInstructions(trailing: readonly InstructionDto[]): bigint {
  let unitLimit: bigint | undefined
  let unitPrice: bigint | undefined
  for (const instruction of trailing) {
    if (instruction.programId !== COMPUTE_BUDGET_PROGRAM_ID || instruction.accounts.length !== 0) {
      throw new Error(
        'Only account-less ComputeBudget instructions may follow the Lastro instruction',
      )
    }
    const data = decodeCanonicalBase64(instruction.dataBase64)
    const view = new DataView(data.buffer, data.byteOffset, data.byteLength)
    if (data[0] === 2 && data.length === 5 && unitLimit === undefined) {
      unitLimit = BigInt(view.getUint32(1, true))
      if (unitLimit === 0n || unitLimit > 1_400_000n) {
        throw new Error('Compute unit limit is outside the Solana range')
      }
    } else if (data[0] === 3 && data.length === 9 && unitPrice === undefined) {
      unitPrice = view.getBigUint64(1, true)
    } else {
      throw new Error('Unsupported or repeated ComputeBudget instruction')
    }
  }
  const fee =
    ((unitLimit ?? DEFAULT_COMPUTE_UNIT_LIMIT) * (unitPrice ?? 0n) + 999_999n) / 1_000_000n
  if (fee > MAX_PRIORITY_FEE_LAMPORTS) {
    throw new Error(
      `Priority fee ${fee} lamports exceeds the ${MAX_PRIORITY_FEE_LAMPORTS} lamport cap`,
    )
  }
  return fee
}

/**
 * The Secp256r1 descriptor must reference exactly the 220 envelope bytes inside instruction 1
 * and carry a valid low-S Station signature whose key derives the envelope's sourceId.
 * Returns the StationId.
 */
async function verifySecpDescriptor(
  instruction: InstructionDto,
  envelopeBytes: Uint8Array,
  sourceId: Uint8Array,
): Promise<Uint8Array> {
  if (instruction.programId !== SECP256R1_PROGRAM_ID || instruction.accounts.length !== 0) {
    throw new Error('Instruction 0 must be the official account-less Secp256r1 precompile')
  }
  const descriptor = decodeCanonicalBase64(instruction.dataBase64)
  if (
    descriptor.length !== SECP256R1_DATA_LENGTH ||
    !equalBytes(descriptor.subarray(0, 16), secpHeader())
  ) {
    throw new Error('Secp256r1 descriptor does not reference the exact envelope in instruction 1')
  }
  const signature = descriptor.subarray(16, 80)
  const publicKey = descriptor.subarray(80, 113)
  if (!verifyStationSignature(envelopeBytes, publicKey, signature)) {
    throw new Error('Secp256r1 descriptor contains an invalid Station signature for this envelope')
  }
  const derived = await stationId(publicKey)
  if (!equalBytes(derived, sourceId)) {
    throw new Error('Secp256r1 Station public key does not match the envelope sourceId')
  }
  return derived
}

/** One signature; signature/pubkey in instruction 0; envelope at offset 104 of instruction 1. */
export function secpHeader(): Uint8Array {
  const header = new Uint8Array(16)
  header[0] = 1
  const view = new DataView(header.buffer)
  ;[16, 0, 80, 0, STATION_EVENT_ENVELOPE_OFFSET, ENVELOPE_LENGTH, 1].forEach((value, index) =>
    view.setUint16(2 + index * 2, value, true),
  )
  return header
}

function toInstruction(value: InstructionDto): Instruction {
  return {
    programAddress: address(value.programId),
    accounts: value.accounts.map((accountMeta) => ({
      address: address(accountMeta.address),
      role: accountRole(accountMeta),
    })),
    data: decodeCanonicalBase64(value.dataBase64),
  }
}

function accountRole(value: AccountMetaDto): AccountRole {
  if (value.isSigner && value.isWritable) return AccountRole.WRITABLE_SIGNER
  if (value.isSigner) return AccountRole.READONLY_SIGNER
  if (value.isWritable) return AccountRole.WRITABLE
  return AccountRole.READONLY
}

function meta(value: Address | string, isSigner: boolean, isWritable: boolean): AccountMetaDto {
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

function decodeCanonicalBase64(value: string): Uint8Array {
  try {
    return fromBase64(value)
  } catch {
    throw new Error('Instruction data must use canonical base64')
  }
}

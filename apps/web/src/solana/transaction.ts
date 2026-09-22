import {
  AccountRole,
  address,
  appendTransactionMessageInstruction,
  compileTransaction,
  createTransactionMessage,
  getAddressDecoder,
  getBase64EncodedWireTransaction,
  getProgramDerivedAddress,
  getTransactionSize,
  pipe,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
  type Address,
  type Instruction,
} from '@solana/kit'
import type { AccountMetaDto, InstructionDto, TransactionData } from '../api/types'
import { webConfig } from '../config'
import { stationId } from '../protocol/hash'
import { verifyStationSignature } from '../protocol/p256'
import { decodeStationEvent, type StationEvent } from '../protocol/stationEvent'
import { solanaClient } from './client'
import {
  LASTRO_PROTOCOL_INSTRUCTION_COUNT,
  SECP256R1_PROGRAM_ID,
  SOLANA_LEGACY_V0_MAX_TRANSACTION_BYTES,
} from './constants'

const SYSTEM_PROGRAM_ID = '11111111111111111111111111111111'
const INSTRUCTIONS_SYSVAR_ID = 'Sysvar1nstructions1111111111111111111111111'
const CONFIG_SEED = new TextEncoder().encode('config')
const ANIMAL_SEED = new TextEncoder().encode('animal')
const RFID_SEED = new TextEncoder().encode('rfid')
const LASTRO_INSTRUCTION_DATA_LENGTH = 8 + 276
const SECP256R1_DATA_LENGTH = 113

interface ValidatedTransaction {
  instructions: readonly [Instruction, Instruction]
  rawEvent: Uint8Array
  event: StationEvent
}

export interface WalletSigningContext {
  accountAddress: string
  supportedTransactionVersions: ReadonlySet<'legacy' | 0>
  canSignTransactions: boolean
}

/** Validate the connected wallet authority/capability before any signing request is created. */
export function validateWalletSigningContext(
  transactionData: TransactionData,
  context: WalletSigningContext,
): 'legacy' | 0 {
  if (context.accountAddress !== transactionData.requiredSigner) {
    throw new Error('Connected wallet is not the current custodian required for this transition')
  }
  const requestedVersion = transactionData.transactionVersion === 'legacy' ? 'legacy' : 0
  if (!context.supportedTransactionVersions.has(requestedVersion)) {
    throw new Error(`Connected wallet does not support ${transactionData.transactionVersion} transactions`)
  }
  if (!context.canSignTransactions) {
    throw new Error('Connected wallet must support signTransaction so Lastro can verify the signed envelope before broadcast')
  }
  return requestedVersion
}

/**
 * Independently validate the complete API transaction descriptor before any wallet popup.
 * The raw 276-byte StationEvent is retained byte-for-byte and becomes instruction 1 data.
 */
export async function validateLastroTransactionData(transactionData: TransactionData): Promise<ValidatedTransaction> {
  if (transactionData.lastroProgramId !== webProgramId()) {
    throw new Error('Lastro program id does not match browser deployment configuration')
  }
  if (transactionData.instructions.length !== LASTRO_PROTOCOL_INSTRUCTION_COUNT) {
    throw new Error('Lastro transaction must contain exactly two protocol instructions')
  }
  if (transactionData.instructions[0].programId !== SECP256R1_PROGRAM_ID) {
    throw new Error('Instruction 0 must be the official Secp256r1 precompile')
  }
  if (transactionData.instructions[0].accounts.length !== 0) {
    throw new Error('Secp256r1 instruction must not contain account metas')
  }
  if (transactionData.instructions[1].programId !== transactionData.lastroProgramId) {
    throw new Error('Instruction 1 must be the configured Lastro program')
  }
  if (transactionData.measuredSerializedBytes > SOLANA_LEGACY_V0_MAX_TRANSACTION_BYTES) {
    throw new Error('API transaction measurement exceeds Solana legacy/v0 size limit')
  }
  if (transactionData.transactionVersion !== 'legacy' && transactionData.transactionVersion !== 'v0') {
    throw new Error('Unsupported transaction version requested by API')
  }

  const secpDescriptor = decodeCanonicalBase64(transactionData.instructions[0].dataBase64)
  const lastroData = decodeCanonicalBase64(transactionData.instructions[1].dataBase64)
  if (secpDescriptor.length !== SECP256R1_DATA_LENGTH) {
    throw new Error('Secp256r1 instruction must use the frozen 113-byte Lastro descriptor')
  }
  if (lastroData.length !== LASTRO_INSTRUCTION_DATA_LENGTH) {
    throw new Error('Lastro instruction must contain one discriminator and one exact StationEvent')
  }

  const rawEvent = lastroData.slice(8)
  const event = decodeStationEvent(rawEvent)
  await verifyAnchorDiscriminator(lastroData.subarray(0, 8), event.action)
  await verifySecpDescriptor(secpDescriptor, rawEvent, event)
  await verifyExpectedAccounts(transactionData.instructions[1], transactionData.lastroProgramId, event)

  const expectedSigner = custodianAddress(event.action === 1 ? event.toCustodian : event.fromCustodian)
  if (transactionData.requiredSigner !== expectedSigner) {
    throw new Error('Required signer does not match the custodian authority encoded by StationEvent')
  }

  return {
    instructions: [toInstruction(transactionData.instructions[0]), toInstruction(transactionData.instructions[1])],
    rawEvent,
    event,
  }
}

/**
 * Validate, construct, sign, and submit the frozen Lastro envelope.
 * Signing remains inside the connected Wallet Standard account; private key material never enters Lastro code.
 */
export async function submitLastroTransaction(transactionData: TransactionData): Promise<string> {
  const validated = await validateLastroTransactionData(transactionData)
  const walletState = solanaClient.wallet.getState()
  const connected = walletState.connected
  if (!connected?.signer) throw new Error('A signing-capable Wallet Standard account must be connected')
  const requestedVersion = validateWalletSigningContext(transactionData, {
    accountAddress: connected.account.address,
    supportedTransactionVersions: connected.supportedTransactionVersions,
    canSignTransactions: 'modifyAndSignTransactions' in connected.signer,
  })

  const latest = await solanaClient.rpc.getLatestBlockhash({ commitment: 'finalized' }).send()
  let message = pipe(
    createTransactionMessage({ version: requestedVersion }),
    (value) => setTransactionMessageFeePayerSigner(connected.signer!, value),
    (value) => setTransactionMessageLifetimeUsingBlockhash(latest.value, value),
  )
  message = appendTransactionMessageInstruction(validated.instructions[0], message)
  message = appendTransactionMessageInstruction(validated.instructions[1], message)

  const unsignedTransaction = compileTransaction(message)
  const unsignedSize = getTransactionSize(unsignedTransaction)
  if (unsignedSize !== transactionData.measuredSerializedBytes) {
    throw new Error(`Locally compiled transaction size ${unsignedSize} does not match API measurement ${transactionData.measuredSerializedBytes}`)
  }
  if (unsignedSize > SOLANA_LEGACY_V0_MAX_TRANSACTION_BYTES) {
    throw new Error(`Serialized Lastro transaction is ${unsignedSize} bytes and exceeds the 1232-byte limit`)
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
  const signature = await solanaClient.rpc.sendTransaction(wire, {
    encoding: 'base64',
    preflightCommitment: 'finalized',
  }).send()
  return String(signature)
}

async function verifySecpDescriptor(descriptor: Uint8Array, rawEvent: Uint8Array, event: StationEvent): Promise<void> {
  if (descriptor[0] !== 1 || descriptor[1] !== 0) {
    throw new Error('Secp256r1 descriptor must contain exactly one signature and zero padding')
  }
  const view = new DataView(descriptor.buffer, descriptor.byteOffset, descriptor.byteLength)
  const expected = [16, 0, 80, 0, 8, 276, 1]
  for (let index = 0; index < expected.length; index += 1) {
    if (view.getUint16(2 + index * 2, true) !== expected[index]) {
      throw new Error('Secp256r1 descriptor does not reference the exact StationEvent in instruction 1')
    }
  }
  const signature = descriptor.subarray(16, 80)
  const publicKey = descriptor.subarray(80, 113)
  if (publicKey[0] !== 0x02 && publicKey[0] !== 0x03) {
    throw new Error('Secp256r1 descriptor contains an invalid compressed Station public key')
  }
  if (!verifyStationSignature(rawEvent, publicKey, signature)) {
    throw new Error('Secp256r1 descriptor contains an invalid Station signature for this StationEvent')
  }
  const derivedStationId = await stationId(publicKey)
  if (!equalBytes(derivedStationId, event.stationId)) {
    throw new Error('Secp256r1 Station public key does not match StationEvent StationID')
  }
  if (rawEvent.length !== 276) {
    throw new Error('Secp256r1 message reference must cover exactly 276 StationEvent bytes')
  }
}

async function verifyAnchorDiscriminator(actual: Uint8Array, action: StationEvent['action']): Promise<void> {
  const actionName = action === 1 ? 'origin' : action === 2 ? 'transfer' : 'reidentify'
  const input = new TextEncoder().encode(`global:${actionName}`)
  const expected = new Uint8Array(await crypto.subtle.digest('SHA-256', input)).subarray(0, 8)
  if (!equalBytes(actual, expected)) throw new Error('Lastro instruction discriminator does not match StationEvent action')
}

async function verifyExpectedAccounts(instruction: InstructionDto, programId: string, event: StationEvent): Promise<void> {
  const programAddress = address(programId)
  const [config] = await getProgramDerivedAddress({ programAddress, seeds: [CONFIG_SEED, event.deploymentId] })
  const [animal] = await getProgramDerivedAddress({ programAddress, seeds: [ANIMAL_SEED, event.deploymentId, event.animalId] })
  const signer = custodianAddress(event.action === 1 ? event.toCustodian : event.fromCustodian)

  let expected: AccountMetaDto[]
  if (event.action === 1) {
    const [binding] = await getProgramDerivedAddress({ programAddress, seeds: [RFID_SEED, event.deploymentId, event.newRfidHash] })
    expected = [
      meta(signer, true, true), meta(config, false, false), meta(animal, false, true),
      meta(binding, false, true), meta(INSTRUCTIONS_SYSVAR_ID, false, false), meta(SYSTEM_PROGRAM_ID, false, false),
    ]
  } else if (event.action === 2) {
    const [binding] = await getProgramDerivedAddress({ programAddress, seeds: [RFID_SEED, event.deploymentId, event.oldRfidHash] })
    expected = [
      meta(signer, true, false), meta(config, false, false), meta(animal, false, true),
      meta(binding, false, false), meta(INSTRUCTIONS_SYSVAR_ID, false, false),
    ]
  } else {
    const [oldBinding] = await getProgramDerivedAddress({ programAddress, seeds: [RFID_SEED, event.deploymentId, event.oldRfidHash] })
    const [newBinding] = await getProgramDerivedAddress({ programAddress, seeds: [RFID_SEED, event.deploymentId, event.newRfidHash] })
    expected = [
      meta(signer, true, true), meta(config, false, false), meta(animal, false, true),
      meta(oldBinding, false, true), meta(newBinding, false, true),
      meta(INSTRUCTIONS_SYSVAR_ID, false, false), meta(SYSTEM_PROGRAM_ID, false, false),
    ]
  }

  if (instruction.accounts.length !== expected.length) {
    throw new Error('Lastro instruction contains an unexpected account count')
  }
  for (let index = 0; index < expected.length; index += 1) {
    const actual = instruction.accounts[index]!
    const wanted = expected[index]!
    if (actual.address !== wanted.address || actual.isSigner !== wanted.isSigner || actual.isWritable !== wanted.isWritable) {
      throw new Error(`Lastro instruction account ${index} does not match the StationEvent-derived account contract`)
    }
  }
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

function custodianAddress(bytes: Uint8Array): string {
  if (bytes.length !== 32) throw new Error('Custodian must be exactly 32 bytes')
  return getAddressDecoder().decode(bytes)
}

function meta(value: Address | string, isSigner: boolean, isWritable: boolean): AccountMetaDto {
  return { address: String(value), isSigner, isWritable }
}

function decodeCanonicalBase64(value: string): Uint8Array {
  let binary: string
  try {
    binary = atob(value)
  } catch {
    throw new Error('Instruction data must use canonical base64')
  }
  if (btoa(binary) !== value) throw new Error('Instruction data must use canonical base64')
  return Uint8Array.from(binary, (character) => character.charCodeAt(0))
}

function equalBytes(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((byte, index) => byte === right[index])
}

function webProgramId(): string {
  return webConfig.lastroProgramId
}

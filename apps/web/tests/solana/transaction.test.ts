import { address, getAddressDecoder, getProgramDerivedAddress } from '@solana/kit'
import { describe, expect, it } from 'vitest'
import vectors from '../../../../test-vectors/vectors.json'
import type { AccountMetaDto, TransactionData } from '../../src/api/types'
import { eventHash } from '../../src/protocol/hash'
import { decodeStationEvent } from '../../src/protocol/stationEvent'
import { SECP256R1_PROGRAM_ID } from '../../src/solana/constants'
import {
  validateLastroTransactionData,
  validateWalletSigningContext,
  type ExpectedTransitionIntent,
} from '../../src/solana/transaction'

const PROGRAM_ID = 'Vote111111111111111111111111111111111111111'
const SYSTEM_PROGRAM_ID = '11111111111111111111111111111111'
const INSTRUCTIONS_SYSVAR_ID = 'Sysvar1nstructions1111111111111111111111111'
const text = new TextEncoder()

function decodeHex(value: string): Uint8Array {
  return Uint8Array.from(value.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16))
}

function encodeHex(value: Uint8Array): string {
  return Array.from(value, (byte) => byte.toString(16).padStart(2, '0')).join('')
}

function base64(value: Uint8Array): string {
  return btoa(String.fromCharCode(...value))
}

function meta(addressValue: string, isSigner: boolean, isWritable: boolean): AccountMetaDto {
  return { address: addressValue, isSigner, isWritable }
}

async function anchorDiscriminator(name: string): Promise<Uint8Array> {
  return new Uint8Array(
    await crypto.subtle.digest('SHA-256', text.encode(`global:${name}`)),
  ).subarray(0, 8)
}

function secpDescriptor(eventBytes: Uint8Array): Uint8Array {
  const out = new Uint8Array(113)
  out[0] = 1
  const view = new DataView(out.buffer)
  const fields = [16, 0, 80, 0, 8, 276, 1]
  fields.forEach((value, index) => view.setUint16(2 + index * 2, value, true))
  out.set(decodeHex(vectors.events.origin.station_signature_hex), 16)
  out.set(decodeHex(vectors.station.pubkey_compressed_hex), 80)
  expect(eventBytes).toHaveLength(276)
  return out
}

async function validTransactionData(): Promise<TransactionData> {
  const eventBytes = decodeHex(vectors.events.origin.event_bytes_hex)
  const event = decodeStationEvent(eventBytes)
  const programAddress = address(PROGRAM_ID)
  const [config] = await getProgramDerivedAddress({
    programAddress,
    seeds: [text.encode('config'), event.deploymentId],
  })
  const [animal] = await getProgramDerivedAddress({
    programAddress,
    seeds: [text.encode('animal'), event.deploymentId, event.animalId],
  })
  const [binding] = await getProgramDerivedAddress({
    programAddress,
    seeds: [text.encode('rfid'), event.deploymentId, event.newRfidHash],
  })
  const signer = getAddressDecoder().decode(event.toCustodian)
  const discriminator = await anchorDiscriminator('origin')
  const lastroData = new Uint8Array(8 + eventBytes.length)
  lastroData.set(discriminator, 0)
  lastroData.set(eventBytes, 8)

  return {
    requiredSigner: signer,
    lastroProgramId: PROGRAM_ID,
    measuredSerializedBytes: 900,
    transactionVersion: 'legacy',
    instructions: [
      {
        programId: SECP256R1_PROGRAM_ID,
        accounts: [],
        dataBase64: base64(secpDescriptor(eventBytes)),
      },
      {
        programId: PROGRAM_ID,
        accounts: [
          meta(signer, true, true),
          meta(String(config), false, false),
          meta(String(animal), false, true),
          meta(String(binding), false, true),
          meta(INSTRUCTIONS_SYSVAR_ID, false, false),
          meta(SYSTEM_PROGRAM_ID, false, false),
        ],
        dataBase64: base64(lastroData),
      },
    ],
  }
}

async function validExpectedIntent(): Promise<ExpectedTransitionIntent> {
  const eventBytes = decodeHex(vectors.events.origin.event_bytes_hex)
  const event = decodeStationEvent(eventBytes)
  return {
    action: 'ORIGIN',
    animalId: vectors.animal_id_hex,
    deploymentId: vectors.deployment_id_hex,
    toCustodian: encodeHex(event.toCustodian),
    eventHash: encodeHex(await eventHash(eventBytes)),
  }
}

describe('solana/transaction validation before wallet signing', () => {
  /**
   * ARRANGE: start from valid frozen transaction data and replace only the declared Lastro program id.
   * ACTION: validate the API descriptor before any wallet interaction.
   * ASSERT: validation rejects the descriptor before signing.
   * FAILURE MEANS: a compromised backend could redirect custodian authority to another program.
   */
  it('rejects transaction data targeting a program id different from configured Lastro', async () => {
    const value = await validTransactionData()
    value.lastroProgramId = SYSTEM_PROGRAM_ID
    await expect(validateLastroTransactionData(value)).rejects.toThrow('program id')
  })

  /**
   * ARRANGE: keep the API descriptor internally valid while independently changing one field of
   *          the operation the user actually initiated.
   * ACTION: validate the descriptor with the expected transition intent before wallet interaction.
   * ASSERT: action, AnimalID, deployment, destination and event hash must all match independently.
   * FAILURE MEANS: a compromised API could substitute another valid Station event before the wallet popup.
   */
  it('rejects an internally valid transaction when it differs from the independently expected intent', async () => {
    const value = await validTransactionData()
    const expected = await validExpectedIntent()
    await expect(validateLastroTransactionData(value, expected)).resolves.toBeDefined()

    const mismatches: ExpectedTransitionIntent[] = [
      { ...expected, action: 'TRANSFER' },
      { ...expected, animalId: 'ff'.repeat(32) },
      { ...expected, deploymentId: 'ee'.repeat(32) },
      { ...expected, toCustodian: 'dd'.repeat(32) },
      { ...expected, eventHash: 'cc'.repeat(32) },
    ]

    for (const mismatch of mismatches) {
      await expect(validateLastroTransactionData(value, mismatch)).rejects.toThrow(
        'expected transition intent',
      )
    }
  })

  /**
   * ARRANGE: use a valid two-instruction envelope, then reverse the two protocol instructions.
   * ACTION: validate each instruction order.
   * ASSERT: only Secp256r1 at index 0 followed by Lastro at index 1 is accepted.
   * FAILURE MEANS: on-chain precompile binding and client review could refer to different instructions.
   */
  it('requires Secp256r1 verification immediately before the bound Lastro instruction', async () => {
    const valid = await validTransactionData()
    await expect(validateLastroTransactionData(valid)).resolves.toBeDefined()

    const reversed = await validTransactionData()
    reversed.instructions = [reversed.instructions[1], reversed.instructions[0]]
    await expect(validateLastroTransactionData(reversed)).rejects.toThrow('Instruction 0')
  })

  /**
   * ARRANGE: build valid API instruction data from the frozen ORIGIN event bytes.
   * ACTION: validate the descriptor and inspect the returned raw event/instruction data.
   * ASSERT: the exact 276 signed bytes are preserved and remain embedded unchanged after the discriminator.
   * FAILURE MEANS: the browser could invalidate the Station signature by reserializing signed evidence.
   */
  it('preserves validated raw instruction bytes without reserializing StationEvent data', async () => {
    const value = await validTransactionData()
    const validated = await validateLastroTransactionData(value)
    const expected = decodeHex(vectors.events.origin.event_bytes_hex)
    expect(validated.rawEvent).toEqual(expected)
    expect(validated.instructions[1].data?.slice(8)).toEqual(expected)
  })

  /**
   * ARRANGE: mutate only the Secp256r1 message offset in an otherwise valid descriptor.
   * ACTION: validate the transaction preview.
   * ASSERT: any offset other than byte 8 for exactly 276 bytes in instruction 1 is rejected.
   * FAILURE MEANS: the backend could verify different bytes than the Lastro instruction processes.
   */
  it('rejects Secp256r1 offsets that do not reference the exact serialized StationEvent', async () => {
    const value = await validTransactionData()
    const descriptor = Uint8Array.from(atob(value.instructions[0].dataBase64), (character) =>
      character.charCodeAt(0),
    )
    new DataView(descriptor.buffer).setUint16(10, 9, true)
    value.instructions[0].dataBase64 = base64(descriptor)
    await expect(validateLastroTransactionData(value)).rejects.toThrow('exact StationEvent')
  })

  /**
   * ARRANGE: use otherwise valid transaction data with API-reported sizes of 1233 and 1232 bytes.
   * ACTION: validate both descriptors before wallet interaction.
   * ASSERT: 1233 is rejected and 1232 remains eligible for later exact local serialization checks.
   * FAILURE MEANS: the browser could knowingly submit an envelope above Solana's network limit.
   */
  it('rejects measured transaction size above 1232 bytes', async () => {
    const tooLarge = await validTransactionData()
    tooLarge.measuredSerializedBytes = 1233
    await expect(validateLastroTransactionData(tooLarge)).rejects.toThrow('size limit')

    const boundary = await validTransactionData()
    boundary.measuredSerializedBytes = 1232
    await expect(validateLastroTransactionData(boundary)).resolves.toBeDefined()
  })

  /**
   * ARRANGE: expected current custodian A; connect wallet A then wallet B.
   * ACTION: request signing eligibility for the same transition.
   * ASSERT: A is eligible; B is blocked before signing and receives an explicit authority error.
   * FAILURE MEANS: the UI can solicit signatures from an authority the chain must reject.
   */
  it('requires connected wallet to equal the transition authority encoded by current state', async () => {
    const value = await validTransactionData()
    expect(() =>
      validateWalletSigningContext(value, {
        accountAddress: value.requiredSigner,
        supportedTransactionVersions: new Set(['legacy']),
        canSignTransactions: true,
      }),
    ).not.toThrow()
    expect(() =>
      validateWalletSigningContext(value, {
        accountAddress: SYSTEM_PROGRAM_ID,
        supportedTransactionVersions: new Set(['legacy']),
        canSignTransactions: true,
      }),
    ).toThrow('current custodian')
  })

  /**
   * ARRANGE: connected wallet advertises supportedTransactionVersions; API requests legacy or v0.
   * ACTION: compare requested version with wallet capability before signing.
   * ASSERT: unsupported version is rejected before popup; supported version proceeds to exact serialization.
   * FAILURE MEANS: the demo can fail after approval because the wallet cannot sign the chosen format.
   */
  it('requires connected wallet to support the requested transaction version', async () => {
    const value = await validTransactionData()
    expect(() =>
      validateWalletSigningContext(value, {
        accountAddress: value.requiredSigner,
        supportedTransactionVersions: new Set([0]),
        canSignTransactions: true,
      }),
    ).toThrow('does not support legacy')
    expect(
      validateWalletSigningContext(value, {
        accountAddress: value.requiredSigner,
        supportedTransactionVersions: new Set(['legacy']),
        canSignTransactions: true,
      }),
    ).toBe('legacy')
  })
})

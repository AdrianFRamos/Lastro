import { getAddressDecoder } from '@solana/kit'
import { describe, expect, it, vi } from 'vitest'
import vectors from '../../../../test-vectors/v2-capture.json'
import type { AccountMetaDto, TransactionData } from '../../src/api/types'
import { fromHex } from '../../src/protocol/hash'
import { COMPUTE_BUDGET_PROGRAM_ID, SECP256R1_PROGRAM_ID } from '../../src/solana/constants'
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
} from '../../src/solana/pda'
import {
  secpHeader,
  validateLastroTransactionData,
  validatePriorityFeeInstructions,
  validateWalletSigningContext,
  type ExpectedTransaction,
} from '../../src/solana/transaction'

const PROGRAM_ID = 'Vote111111111111111111111111111111111111111'
const SYSTEM_PROGRAM_ID = '11111111111111111111111111111111'
const INSTRUCTIONS_SYSVAR_ID = 'Sysvar1nstructions1111111111111111111111111'
const CUSTODIAN = getAddressDecoder().decode(new Uint8Array(32).fill(0xc1))

vi.mock('../../src/config', () => ({
  webConfig: {
    lastroProgramId: 'Vote111111111111111111111111111111111111111',
    lastroDeploymentId: 'd0'.repeat(32),
    lastroAuthority: null,
    solanaChain: 'solana:localnet',
    solanaRpcUrl: 'http://127.0.0.1:8899',
  },
}))

const deployment = fromHex(vectors.deployment_id_hex, 32)
const assetId = fromHex(vectors.asset_id_hex, 32)
const tagA = fromHex(vectors.tags.a.hash_hex, 32)

function base64(value: Uint8Array): string {
  return btoa(String.fromCharCode(...value))
}

function meta(address: string, isSigner: boolean, isWritable: boolean): AccountMetaDto {
  return { address: String(address), isSigner, isWritable }
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

/** The bind_identifier transaction the API builds for the bind vector. */
async function bindTransaction(): Promise<TransactionData> {
  const envelope = fromHex(vectors.captures.bind.envelope_hex, 220)
  const eventId = envelope.slice(68, 100)
  const station = fromHex(vectors.station.station_id_hex, 32)
  const secp = concat(
    secpHeader(),
    fromHex(vectors.captures.bind.station_signature_hex, 64),
    fromHex(vectors.station.pubkey33_hex, 33),
  )
  const data = concat(
    await discriminator('global', 'bind_identifier'),
    assetId,
    eventId,
    station,
    envelope,
    tagA,
  )
  return {
    requiredSigner: CUSTODIAN,
    lastroProgramId: PROGRAM_ID,
    measuredSerializedBytes: 900,
    transactionVersion: 'legacy',
    instructions: [
      { programId: SECP256R1_PROGRAM_ID, accounts: [], dataBase64: base64(secp) },
      {
        programId: PROGRAM_ID,
        accounts: [
          meta(CUSTODIAN, true, true),
          meta((await configPda(PROGRAM_ID, deployment))[0], false, false),
          meta((await stationRegistryPda(PROGRAM_ID, deployment))[0], false, false),
          meta((await stationPda(PROGRAM_ID, deployment, station))[0], false, false),
          meta((await assetPda(PROGRAM_ID, deployment, assetId))[0], false, true),
          meta((await eventPda(PROGRAM_ID, deployment, eventId))[0], false, true),
          meta((await rfidBindingPda(PROGRAM_ID, deployment, tagA))[0], false, true),
          meta(INSTRUCTIONS_SYSVAR_ID, false, false),
          meta(SYSTEM_PROGRAM_ID, false, false),
        ],
        dataBase64: base64(data),
      },
    ],
  }
}

const bindIntent: ExpectedTransaction = {
  kind: 'STATION_EVENT',
  action: 'BIND_IDENTIFIER',
  assetId: vectors.asset_id_hex,
  eventHash: vectors.captures.bind.event_hash_hex,
  requiredSigner: CUSTODIAN,
}

function mutateData(value: TransactionData, index: number, offset: number): void {
  const instruction = value.instructions[index]!
  const bytes = Uint8Array.from(atob(instruction.dataBase64), (c) => c.charCodeAt(0))
  bytes[offset] = bytes[offset]! ^ 1
  instruction.dataBase64 = base64(bytes)
}

describe('solana/transaction validation before wallet signing', () => {
  /**
   * ARRANGE: the exact bind_identifier transaction for the signed bind vector.
   * ACTION: validate it against the independently expected capture.
   * ASSERT: it passes and keeps both instructions.
   * FAILURE MEANS: genuine captures could not be signed.
   */
  it('accepts the exact Station-event transaction the API builds', async () => {
    const validated = await validateLastroTransactionData(await bindTransaction(), bindIntent)
    expect(validated.instructions).toHaveLength(2)
  })

  /**
   * ARRANGE: valid transaction data with a different declared program id.
   * ACTION: validate before any wallet interaction.
   * ASSERT: rejected.
   * FAILURE MEANS: a compromised backend could redirect wallet authority to another program.
   */
  it('rejects transaction data targeting a program id different from configured Lastro', async () => {
    const value = await bindTransaction()
    value.lastroProgramId = SYSTEM_PROGRAM_ID
    await expect(validateLastroTransactionData(value, bindIntent)).rejects.toThrow('program id')
  })

  /**
   * ARRANGE: a valid transaction and an expectation that differs in action, asset or event.
   * ACTION: validate.
   * ASSERT: each mismatch is rejected.
   * FAILURE MEANS: a compromised API could substitute another valid Station event.
   */
  it('rejects an internally valid transaction that differs from the expected capture', async () => {
    const value = await bindTransaction()
    for (const expected of [
      { ...bindIntent, action: 'REPLACE_IDENTIFIER' as const },
      { ...bindIntent, assetId: '22'.repeat(32) },
      { ...bindIntent, eventHash: vectors.captures.replace.event_hash_hex },
      { ...bindIntent, requiredSigner: SYSTEM_PROGRAM_ID },
    ]) {
      await expect(validateLastroTransactionData(value, expected)).rejects.toThrow()
    }
  })

  /**
   * ARRANGE: flip one byte of the RFID argument, the Station signature and the descriptor offsets.
   * ACTION: validate each.
   * ASSERT: all are rejected.
   * FAILURE MEANS: the wallet could sign a different tag than the Station read, or an unbound signature.
   */
  it('binds the RFID argument, Station signature and precompile offsets to the envelope', async () => {
    for (const [index, offset] of [
      [1, 104 + 220], // new RFID hash
      [0, 20], // Station signature
      [0, 10], // message offset in the descriptor
    ] as const) {
      const value = await bindTransaction()
      mutateData(value, index, offset)
      await expect(validateLastroTransactionData(value, bindIntent)).rejects.toThrow()
    }
  })

  /**
   * ARRANGE: swap the RFID binding account for another tag's binding.
   * ACTION: validate.
   * ASSERT: rejected by the derived account contract.
   * FAILURE MEANS: the wallet could create a binding PDA for a different tag.
   */
  it('derives every account independently', async () => {
    const value = await bindTransaction()
    value.instructions[1]!.accounts[6] = meta(
      (await rfidBindingPda(PROGRAM_ID, deployment, fromHex(vectors.tags.b.hash_hex, 32)))[0],
      false,
      true,
    )
    await expect(validateLastroTransactionData(value, bindIntent)).rejects.toThrow('account 6')
  })

  /**
   * ARRANGE: register_asset and accept_custody_transfer transactions built like the API.
   * ACTION: validate with matching and mismatching expectations.
   * ASSERT: matching passes; a different custodian or recipient is rejected.
   * FAILURE MEANS: the authority could register an asset to the wrong custodian, or a recipient
   *                could accept a transfer of another asset.
   */
  it('validates wallet-only registration and custody acceptance instructions', async () => {
    const [config] = await configPda(PROGRAM_ID, deployment)
    const [asset] = await assetPda(PROGRAM_ID, deployment, assetId)
    const weight = new Uint8Array(8)
    new DataView(weight.buffer).setBigUint64(0, 450_000n, true)
    const register: TransactionData = {
      requiredSigner: CUSTODIAN,
      lastroProgramId: PROGRAM_ID,
      measuredSerializedBytes: 300,
      transactionVersion: 'legacy',
      instructions: [
        {
          programId: PROGRAM_ID,
          accounts: [
            meta(CUSTODIAN, true, true),
            meta(config, false, false),
            meta(asset, false, true),
            meta(SYSTEM_PROGRAM_ID, false, false),
          ],
          dataBase64: base64(
            concat(
              await discriminator('global', 'register_asset'),
              assetId,
              Uint8Array.of(1),
              new Uint8Array(32).fill(0xc1),
              new Uint8Array(32),
              assetId,
              weight,
            ),
          ),
        },
      ],
    }
    const registration = {
      kind: 'REGISTER_ASSET' as const,
      assetId: vectors.asset_id_hex,
      custodian: 'c1'.repeat(32),
      assetType: 1,
      availableWeightGrams: 450_000,
    }
    await expect(validateLastroTransactionData(register, registration)).resolves.toBeTruthy()
    await expect(
      validateLastroTransactionData(register, { ...registration, custodian: 'c2'.repeat(32) }),
    ).rejects.toThrow('derived bytes')

    const transferId = '00000000-0000-4000-8000-000000000001'
    const [intent] = await intentPda(
      PROGRAM_ID,
      deployment,
      assetId,
      await custodyIntentId(transferId),
    )
    const accept: TransactionData = {
      ...register,
      instructions: [
        {
          programId: PROGRAM_ID,
          accounts: [
            meta(CUSTODIAN, true, false),
            meta(config, false, false),
            meta(asset, false, true),
            meta(intent, false, true),
          ],
          dataBase64: base64(
            concat(await discriminator('global', 'accept_custody_transfer'), new Uint8Array(8)),
          ),
        },
      ],
    }
    const acceptance = {
      kind: 'CUSTODY_ACCEPT' as const,
      assetId: vectors.asset_id_hex,
      transferId,
      recipient: 'c1'.repeat(32),
    }
    await expect(validateLastroTransactionData(accept, acceptance)).resolves.toBeTruthy()
    await expect(
      validateLastroTransactionData(accept, {
        ...acceptance,
        transferId: '00000000-0000-4000-8000-000000000002',
      }),
    ).rejects.toThrow('account 3')
  })

  /**
   * ARRANGE: expected authority A; connect wallet A then another address.
   * ACTION: request signing eligibility for the same transition.
   * ASSERT: A is eligible; the other is blocked before signing.
   * FAILURE MEANS: the UI can solicit signatures from an authority the chain must reject.
   */
  it('requires connected wallet to equal the transition authority', async () => {
    const value = await bindTransaction()
    const context = {
      supportedTransactionVersions: new Set(['legacy'] as const),
      canSignTransactions: true,
    }
    expect(validateWalletSigningContext(value, { ...context, accountAddress: CUSTODIAN })).toBe(
      'legacy',
    )
    expect(() =>
      validateWalletSigningContext(value, { ...context, accountAddress: SYSTEM_PROGRAM_ID }),
    ).toThrow('authority required')
    expect(() =>
      validateWalletSigningContext(value, {
        accountAddress: CUSTODIAN,
        supportedTransactionVersions: new Set([0] as const),
        canSignTransactions: true,
      }),
    ).toThrow('does not support legacy')
  })

  /**
   * ARRANGE: unit-limit and unit-price ComputeBudget descriptors as the API appends them.
   * ACTION: validate them alone, over the cap, repeated, and from another program.
   * ASSERT: a fee within 0.0002 SOL passes; excess, repetition, accounts or other programs throw.
   * FAILURE MEANS: a compromised API could make the wallet sign an arbitrary priority fee.
   */
  it('bounds the priority fee a wallet is asked to sign', () => {
    const budget = (bytes: number[]) => ({
      programId: COMPUTE_BUDGET_PROGRAM_ID,
      accounts: [],
      dataBase64: base64(Uint8Array.from(bytes)),
    })
    const limit = budget([2, 0x40, 0x0d, 0x03, 0x00])
    const price = (microLamports: number) => {
      const bytes = new Uint8Array(9)
      bytes[0] = 3
      new DataView(bytes.buffer).setBigUint64(1, BigInt(microLamports), true)
      return budget(Array.from(bytes))
    }
    expect(validatePriorityFeeInstructions([])).toBe(0n)
    expect(validatePriorityFeeInstructions([limit, price(5_000)])).toBe(1_000n)
    expect(() => validatePriorityFeeInstructions([limit, price(1_000_001)])).toThrow('cap')
    expect(() => validatePriorityFeeInstructions([price(1), price(1)])).toThrow('repeated')
    expect(() =>
      validatePriorityFeeInstructions([{ ...limit, accounts: [meta(PROGRAM_ID, false, false)] }]),
    ).toThrow('account-less')
    expect(() =>
      validatePriorityFeeInstructions([{ ...limit, programId: SYSTEM_PROGRAM_ID }]),
    ).toThrow('ComputeBudget')
  })
})

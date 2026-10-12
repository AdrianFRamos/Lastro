import { getAddressDecoder } from '@solana/kit'
import { describe, expect, it, vi } from 'vitest'
import vectors from '../../../../test-vectors/v2-capture.json'
import type { AccountMetaDto, TransactionData } from '../../src/api/types'
import { fromHex } from '../../src/protocol/hash'
import { sessionId as deriveSessionId } from '../../src/protocol/v2/session'
import { COMPUTE_BUDGET_PROGRAM_ID, SECP256R1_PROGRAM_ID } from '../../src/solana/constants'
import {
  assetPda,
  batchPda,
  configPda,
  facilityPda,
  custodyIntentId,
  discriminator,
  eventPda,
  intentPda,
  rfidBindingPda,
  sessionPda,
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
const SPONSOR = getAddressDecoder().decode(new Uint8Array(32).fill(0x5b))

vi.mock('../../src/config', () => ({
  webConfig: {
    lastroProgramId: 'Vote111111111111111111111111111111111111111',
    lastroDeploymentId: 'd0'.repeat(32),
    lastroAuthority: null,
    lastroSponsor: '79cu2QNvp92chEmBhBgcUfERDT9KHnUcr8eiJb5WzKML',
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
    feePayer: CUSTODIAN,
    lastroProgramId: PROGRAM_ID,
    measuredSerializedBytes: 900,
    transactionVersion: 'legacy',
    instructions: [
      { programId: SECP256R1_PROGRAM_ID, accounts: [], dataBase64: base64(secp) },
      {
        programId: PROGRAM_ID,
        accounts: [
          meta(CUSTODIAN, true, false),
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
    value.instructions[1]!.accounts[7] = meta(
      (await rfidBindingPda(PROGRAM_ID, deployment, fromHex(vectors.tags.b.hash_hex, 32)))[0],
      false,
      true,
    )
    await expect(validateLastroTransactionData(value, bindIntent)).rejects.toThrow('account 7')
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
      feePayer: CUSTODIAN,
      lastroProgramId: PROGRAM_ID,
      measuredSerializedBytes: 300,
      transactionVersion: 'legacy',
      instructions: [
        {
          programId: PROGRAM_ID,
          accounts: [
            meta(CUSTODIAN, true, false),
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
            meta(SPONSOR, false, true),
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
    const proposalFundedBySponsor = { intentRentPayer: async () => SPONSOR }
    await expect(
      validateLastroTransactionData(accept, acceptance, proposalFundedBySponsor),
    ).resolves.toBeTruthy()
    await expect(
      validateLastroTransactionData(
        accept,
        { ...acceptance, transferId: '00000000-0000-4000-8000-000000000002' },
        proposalFundedBySponsor,
      ),
    ).rejects.toThrow('account 3')
    // The refund must go to the payer the on-chain proposal recorded, and only while it is open.
    await expect(
      validateLastroTransactionData(accept, acceptance, { intentRentPayer: async () => CUSTODIAN }),
    ).rejects.toThrow('account 4')
    await expect(
      validateLastroTransactionData(accept, acceptance, { intentRentPayer: async () => null }),
    ).rejects.toThrow('not open')
  })

  /**
   * ARRANGE: the bind transaction prepared with the pinned sponsor, and with another key, as
   *          fee payer.
   * ACTION: validate both.
   * ASSERT: the pinned sponsor passes as the separate writable payer; any other payer is
   *         rejected before the wallet is asked to sign.
   * FAILURE MEANS: a compromised API could attribute the wallet's transaction to an arbitrary
   *                fee payer.
   */
  it('accepts only the pinned sponsor as a separate fee payer', async () => {
    const sponsored = await bindTransaction()
    sponsored.feePayer = SPONSOR
    sponsored.instructions[1]!.accounts[1] = meta(SPONSOR, true, true)
    await expect(validateLastroTransactionData(sponsored, bindIntent)).resolves.toBeTruthy()

    const stranger = getAddressDecoder().decode(new Uint8Array(32).fill(0x99))
    const foreign = await bindTransaction()
    foreign.feePayer = stranger
    foreign.instructions[1]!.accounts[1] = meta(stranger, true, true)
    await expect(validateLastroTransactionData(foreign, bindIntent)).rejects.toThrow(
      'pinned platform sponsor',
    )

    // The payer account inside the instruction must be the declared fee payer.
    const mismatched = await bindTransaction()
    mismatched.feePayer = SPONSOR
    await expect(validateLastroTransactionData(mismatched, bindIntent)).rejects.toThrow('account 1')
  })

  /**
   * ARRANGE: open_session and anchor_observation_batch transactions built like the API, with
   *          the pinned sponsor as payer.
   * ACTION: validate against the session terms the browser holds, then against changed terms.
   * ASSERT: the exact terms pass; another capacity, Station, root or out-of-window batch fails.
   * FAILURE MEANS: one custodian signature could authorize terms other than those displayed.
   */
  it('validates session opening and batch anchoring against the displayed terms', async () => {
    const station = fromHex(vectors.station.station_id_hex, 32)
    // The program accepts only the id derived from custodian, Station and terms.
    const sessionId = await deriveSessionId(
      deployment,
      fromHex('c1'.repeat(32), 32),
      station,
      1_000,
      4_600,
      500,
    )
    const sessionHex = Array.from(sessionId, (byte) => byte.toString(16).padStart(2, '0')).join('')
    const [config] = await configPda(PROGRAM_ID, deployment)
    const [session] = await sessionPda(PROGRAM_ID, deployment, sessionId)
    const u16 = (value: number) => {
      const bytes = new Uint8Array(2)
      new DataView(bytes.buffer).setUint16(0, value, true)
      return bytes
    }
    const u32 = (value: number) => {
      const bytes = new Uint8Array(4)
      new DataView(bytes.buffer).setUint32(0, value, true)
      return bytes
    }
    const i64 = (value: number) => {
      const bytes = new Uint8Array(8)
      new DataView(bytes.buffer).setBigInt64(0, BigInt(value), true)
      return bytes
    }
    const open: TransactionData = {
      requiredSigner: CUSTODIAN,
      feePayer: SPONSOR,
      lastroProgramId: PROGRAM_ID,
      measuredSerializedBytes: 400,
      transactionVersion: 'legacy',
      instructions: [
        {
          programId: PROGRAM_ID,
          accounts: [
            meta(CUSTODIAN, true, false),
            meta(SPONSOR, true, true),
            meta(config, false, false),
            meta((await stationRegistryPda(PROGRAM_ID, deployment))[0], false, false),
            meta((await stationPda(PROGRAM_ID, deployment, station))[0], false, false),
            meta(
              (await facilityPda(PROGRAM_ID, deployment, fromHex('fa'.repeat(32), 32)))[0],
              false,
              false,
            ),
            meta(session, false, true),
            meta(SYSTEM_PROGRAM_ID, false, false),
          ],
          dataBase64: base64(
            concat(
              await discriminator('global', 'open_session'),
              sessionId,
              station,
              u16(1),
              u32(500),
              i64(1_000),
              i64(4_600),
            ),
          ),
        },
      ],
    }
    const opening = {
      kind: 'SESSION_OPEN' as const,
      sessionId: sessionHex,
      custodian: 'c1'.repeat(32),
      stationId: vectors.station.station_id_hex,
      facilityId: 'fa'.repeat(32),
      maxEvents: 500,
      validFrom: 1_000,
      expiresAt: 4_600,
    }
    await expect(validateLastroTransactionData(open, opening)).resolves.toBeTruthy()
    await expect(
      validateLastroTransactionData(open, { ...opening, maxEvents: 501 }),
    ).rejects.toThrow('derived from its custodian')
    await expect(
      validateLastroTransactionData(open, { ...opening, sessionId: '5e'.repeat(32) }),
    ).rejects.toThrow('derived from its custodian')
    await expect(
      validateLastroTransactionData(open, { ...opening, stationId: '22'.repeat(32) }),
    ).rejects.toThrow()
    await expect(
      validateLastroTransactionData(open, { ...opening, facilityId: 'fb'.repeat(32) }),
    ).rejects.toThrow('account 5')
    await expect(
      validateLastroTransactionData(open, { ...opening, expiresAt: 1_000 + 12 * 3_600 + 1 }),
    ).rejects.toThrow('protocol limits')

    const root = new Uint8Array(32).fill(0x77)
    const batch: TransactionData = {
      ...open,
      instructions: [
        {
          programId: PROGRAM_ID,
          accounts: [
            meta(CUSTODIAN, true, false),
            meta(SPONSOR, true, true),
            meta(config, false, false),
            meta(session, false, true),
            meta((await stationPda(PROGRAM_ID, deployment, station))[0], false, false),
            meta((await batchPda(PROGRAM_ID, deployment, sessionId, 0))[0], false, true),
            meta(SYSTEM_PROGRAM_ID, false, false),
          ],
          dataBase64: base64(
            concat(
              await discriminator('global', 'anchor_observation_batch'),
              root,
              u32(300),
              i64(1_100),
              i64(2_000),
            ),
          ),
        },
      ],
    }
    const anchoring = {
      kind: 'SESSION_BATCH' as const,
      sessionId: sessionHex,
      custodian: 'c1'.repeat(32),
      stationId: vectors.station.station_id_hex,
      batchIndex: 0,
      merkleRoot: '77'.repeat(32),
      leafCount: 300,
      firstObservedAt: 1_100,
      lastObservedAt: 2_000,
      validFrom: 1_000,
      expiresAt: 4_600,
    }
    await expect(validateLastroTransactionData(batch, anchoring)).resolves.toBeTruthy()
    await expect(
      validateLastroTransactionData(batch, { ...anchoring, merkleRoot: '78'.repeat(32) }),
    ).rejects.toThrow('derived bytes')
    await expect(
      validateLastroTransactionData(batch, { ...anchoring, batchIndex: 1 }),
    ).rejects.toThrow('account 5')
    await expect(
      validateLastroTransactionData(batch, { ...anchoring, stationId: '22'.repeat(32) }),
    ).rejects.toThrow('account 4')
    await expect(
      validateLastroTransactionData(batch, { ...anchoring, lastObservedAt: 4_601 }),
    ).rejects.toThrow('session terms')
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

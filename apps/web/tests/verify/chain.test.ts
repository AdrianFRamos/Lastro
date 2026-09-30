import { getAddressDecoder, getBase58Decoder } from '@solana/kit'
import { describe, expect, it, vi } from 'vitest'
import vectors from '../../../../test-vectors/v2-capture.json'
import validFixture from '../../../../test-vectors/v2-evidence-package.valid.json'
import { parseEvidencePackage, type EvidencePackage } from '../../src/protocol/evidence'
import { fromHex } from '../../src/protocol/hash'
import { SECP256R1_PROGRAM_ID } from '../../src/solana/constants'
import {
  assetPda,
  configPda,
  discriminator,
  eventPda,
  rfidBindingPda,
  stationPda,
  stationRegistryPda,
} from '../../src/solana/pda'
import { secpHeader } from '../../src/solana/transaction'
import { verifyCanonicalChainState } from '../../src/verify/verifyChain'

vi.mock('../../src/config', () => ({
  webConfig: {
    lastroProgramId: 'Vote111111111111111111111111111111111111111',
    lastroDeploymentId: 'd0'.repeat(32),
    lastroAuthority: null,
    solanaChain: 'solana:localnet',
    solanaRpcUrl: 'http://127.0.0.1:8899',
  },
}))

const PROGRAM_ID = 'Vote111111111111111111111111111111111111111'
const SYSTEM_PROGRAM_ID = '11111111111111111111111111111111'
const INSTRUCTIONS_SYSVAR_ID = 'Sysvar1nstructions1111111111111111111111111'
const AUTHORITY_BYTES = new Uint8Array(32).fill(0xaa)
const AUTHORITY = getAddressDecoder().decode(AUTHORITY_BYTES)
const CUSTODIAN = getAddressDecoder().decode(new Uint8Array(32).fill(0xc1))
const deployment = fromHex(vectors.deployment_id_hex, 32)
const assetId = fromHex(vectors.asset_id_hex, 32)
const station = fromHex(vectors.station.station_id_hex, 32)
const tagA = fromHex(vectors.tags.a.hash_hex, 32)
const tagB = fromHex(vectors.tags.b.hash_hex, 32)

type Meta = { address: string; isSigner: boolean; isWritable: boolean }
type Ix = { programId: string; accounts: Meta[]; data: Uint8Array }

const base64 = (bytes: Uint8Array) => btoa(String.fromCharCode(...bytes))
const base58 = (bytes: Uint8Array) => getBase58Decoder().decode(bytes)
const meta = (address: unknown, isSigner: boolean, isWritable: boolean): Meta => ({
  address: String(address),
  isSigner,
  isWritable,
})

function concat(...parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((total, part) => total + part.length, 0))
  let offset = 0
  for (const part of parts) {
    out.set(part, offset)
    offset += part.length
  }
  return out
}

async function account(name: string, length: number, fill: (data: Uint8Array) => void) {
  const data = new Uint8Array(length)
  data.set(await discriminator('account', name))
  fill(data)
  return { owner: PROGRAM_ID, executable: false, data: [base64(data), 'base64'] }
}

/** Legacy compiled message for one signer, as `getTransaction(encoding: json)` returns it. */
function compile(signer: string, instructions: Ix[]) {
  const roles = new Map<string, { signer: boolean; writable: boolean }>([
    [signer, { signer: true, writable: true }],
  ])
  const merge = (key: string, s: boolean, w: boolean) => {
    const current = roles.get(key) ?? { signer: false, writable: false }
    roles.set(key, { signer: current.signer || s, writable: current.writable || w })
  }
  for (const ix of instructions) {
    merge(ix.programId, false, false)
    for (const entry of ix.accounts) merge(entry.address, entry.isSigner, entry.isWritable)
  }
  const keys = [...roles.entries()]
  const order = [
    ...keys.filter(([, r]) => r.signer),
    ...keys.filter(([, r]) => !r.signer && r.writable),
    ...keys.filter(([, r]) => !r.signer && !r.writable),
  ].map(([key]) => key)
  return {
    header: {
      numRequiredSignatures: 1,
      numReadonlySignedAccounts: 0,
      numReadonlyUnsignedAccounts: keys.filter(([, r]) => !r.signer && !r.writable).length,
    },
    accountKeys: order,
    recentBlockhash: '11111111111111111111111111111111',
    instructions: instructions.map((ix) => ({
      programIdIndex: order.indexOf(ix.programId),
      accounts: ix.accounts.map((entry) => order.indexOf(entry.address)),
      data: base58(ix.data),
    })),
  }
}

async function stationTransaction(name: 'bind' | 'replace' | 'observe', signer: string) {
  const capture = vectors.captures[name]
  const envelope = fromHex(capture.envelope_hex, 220)
  const eventId = envelope.slice(68, 100)
  const [instruction, rfids] =
    name === 'bind'
      ? ['bind_identifier', [tagA]]
      : name === 'replace'
        ? ['replace_identifier', [tagA, tagB]]
        : ['record_observation', []]
  const secp = concat(
    secpHeader(),
    fromHex(capture.station_signature_hex, 64),
    fromHex(vectors.station.pubkey33_hex, 33),
  )
  const bindings = await Promise.all(
    (rfids as Uint8Array[]).map(
      async (hash) => (await rfidBindingPda(PROGRAM_ID, deployment, hash))[0],
    ),
  )
  return compile(signer, [
    { programId: SECP256R1_PROGRAM_ID, accounts: [], data: secp },
    {
      programId: PROGRAM_ID,
      data: concat(
        await discriminator('global', instruction as string),
        assetId,
        eventId,
        station,
        envelope,
        ...(rfids as Uint8Array[]),
      ),
      accounts: [
        meta(signer, true, (rfids as Uint8Array[]).length > 0),
        meta((await configPda(PROGRAM_ID, deployment))[0], false, false),
        meta((await stationRegistryPda(PROGRAM_ID, deployment))[0], false, false),
        meta((await stationPda(PROGRAM_ID, deployment, station))[0], false, false),
        meta((await assetPda(PROGRAM_ID, deployment, assetId))[0], false, true),
        meta((await eventPda(PROGRAM_ID, deployment, eventId))[0], false, true),
        ...bindings.map((binding) => meta(binding, false, true)),
        meta(INSTRUCTIONS_SYSVAR_ID, false, false),
        meta(SYSTEM_PROGRAM_ID, false, false),
      ],
    },
  ])
}

/** Finalized canonical state that matches the fixture exactly; tests override single entries. */
async function canonicalState() {
  const accounts = new Map<string, unknown>()
  const [config, configBump] = await configPda(PROGRAM_ID, deployment)
  accounts.set(
    String(config),
    await account('ProtocolConfigV2', 221, (data) => {
      data.set(AUTHORITY_BYTES, 8)
      data.set(deployment, 40)
      data[220] = configBump
    }),
  )
  const [asset, assetBump] = await assetPda(PROGRAM_ID, deployment, assetId)
  accounts.set(
    String(asset),
    await account('AssetState', 341, (data) => {
      data.set(assetId, 8)
      data.set(deployment, 42)
      data.set(new Uint8Array(32).fill(0xc1), 74)
      new DataView(data.buffer).setBigUint64(226, 3n, true)
      data.set(fromHex(validFixture.asset.lastEventHash, 32), 234)
      data.set(tagB, 308)
      data[340] = assetBump
    }),
  )
  for (const event of validFixture.events) {
    const eventId = fromHex(event.eventId, 32)
    const [anchor, bump] = await eventPda(PROGRAM_ID, deployment, eventId)
    accounts.set(
      String(anchor),
      await account('EventAnchor', 259, (data) => {
        data.set(eventId, 8)
        data.set(deployment, 40)
        data.set(assetId, 72)
        data.set(fromHex(event.sourceId, 32), 104)
        data.set(fromHex(event.payloadHash, 32), 194)
        data.set(fromHex(event.eventHash, 32), 226)
        data[258] = bump
      }),
    )
  }
  for (const [hash, status] of [
    [tagA, 2],
    [tagB, 1],
  ] as const) {
    const [binding, bump] = await rfidBindingPda(PROGRAM_ID, deployment, hash)
    accounts.set(
      String(binding),
      await account('RfidBinding', 74, (data) => {
        data.set(assetId, 8)
        data.set(hash, 40)
        data[72] = status
        data[73] = bump
      }),
    )
  }
  const transactions = new Map<string, unknown>()
  const names = ['bind', 'replace', 'observe'] as const
  for (const [index, event] of validFixture.events.entries()) {
    const signer = names[index] === 'observe' ? AUTHORITY : CUSTODIAN
    const message = await stationTransaction(names[index]!, signer)
    transactions.set(event.txSignature, {
      meta: { err: null },
      transaction: { signatures: [event.txSignature], message },
    })
  }
  return { accounts, transactions }
}

function rpc(state: Awaited<ReturnType<typeof canonicalState>>): typeof fetch {
  return (async (_url: unknown, init?: RequestInit) => {
    const body = JSON.parse(String(init?.body)) as { method: string; params: [string] }
    const result =
      body.method === 'getAccountInfo'
        ? { value: state.accounts.get(body.params[0]) ?? null }
        : (state.transactions.get(body.params[0]) ?? null)
    return new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result }))
  }) as typeof fetch
}

const pkg = (): EvidencePackage => parseEvidencePackage(structuredClone(validFixture))
const verify = (value: EvidencePackage, fetchFn: typeof fetch, trustedAuthority = AUTHORITY) =>
  verifyCanonicalChainState(value, { fetchFn, trustedAuthority, programId: PROGRAM_ID })

describe('verify/final canonical Solana comparison', () => {
  /**
   * ARRANGE: finalized accounts and transactions that match the signed history exactly.
   * ACTION: verify the fixture against them.
   * ASSERT: ON_CHAIN_STATE is VALID.
   * FAILURE MEANS: genuine evidence could never reach a canonical VALID verdict.
   */
  it('accepts a package that matches finalized accounts and exact transactions', async () => {
    const result = await verify(pkg(), rpc(await canonicalState()))
    expect(result).toMatchObject({ status: 'VALID' })
  })

  /**
   * ARRANGE: no pinned authority; then a ProtocolConfig whose authority differs.
   * ACTION: verify.
   * ASSERT: NOT_CHECKED without a trust anchor; INVALID with a substituted authority.
   * FAILURE MEANS: a look-alike deployment could vouch for itself.
   */
  it('refuses an unpinned deployment authority and rejects an authority substituted on-chain', async () => {
    const state = await canonicalState()
    await expect(
      verifyCanonicalChainState(pkg(), { fetchFn: rpc(state), programId: PROGRAM_ID }),
    ).resolves.toMatchObject({ status: 'NOT_CHECKED' })
    const other = getAddressDecoder().decode(new Uint8Array(32).fill(0xbb))
    await expect(verify(pkg(), rpc(state), other)).resolves.toMatchObject({
      status: 'INVALID',
      detail: expect.stringContaining('authority'),
    })
  })

  /**
   * ARRANGE: package claims a different current RFID / custodian than the chain.
   * ACTION: verify.
   * ASSERT: INVALID.
   * FAILURE MEANS: an exported package could contradict the canonical asset state.
   */
  it('rejects a package whose terminal state differs from canonical AssetState', async () => {
    const state = await canonicalState()
    const wrongRfid = pkg()
    wrongRfid.asset.currentRfidHash = vectors.tags.a.hash_hex
    await expect(verify(wrongRfid, rpc(state))).resolves.toMatchObject({ status: 'INVALID' })
    const wrongCustodian = pkg()
    wrongCustodian.asset.custodian = 'c2'.repeat(32)
    await expect(verify(wrongCustodian, rpc(state))).resolves.toMatchObject({ status: 'INVALID' })
  })

  /**
   * ARRANGE: the replaced tag is still ACTIVE on-chain.
   * ACTION: verify.
   * ASSERT: INVALID.
   * FAILURE MEANS: a retired tag could keep identifying the animal as current.
   */
  it('requires the replaced RFID binding to be RETIRED', async () => {
    const state = await canonicalState()
    const [binding] = await rfidBindingPda(PROGRAM_ID, deployment, tagA)
    const [, bump] = await rfidBindingPda(PROGRAM_ID, deployment, tagA)
    state.accounts.set(
      String(binding),
      await account('RfidBinding', 74, (data) => {
        data.set(assetId, 8)
        data.set(tagA, 40)
        data[72] = 1
        data[73] = bump
      }),
    )
    await expect(verify(pkg(), rpc(state))).resolves.toMatchObject({
      status: 'INVALID',
      detail: expect.stringContaining('RETIRED'),
    })
  })

  /**
   * ARRANGE: one transaction missing (not finalized), one failed, one with other instruction bytes.
   * ACTION: verify each.
   * ASSERT: all INVALID.
   * FAILURE MEANS: evidence could claim chain anchoring it never had.
   */
  it('binds every txSignature to the exact finalized Lastro transaction', async () => {
    const txSignature = validFixture.events[1]!.txSignature
    const missing = await canonicalState()
    missing.transactions.delete(txSignature)
    await expect(verify(pkg(), rpc(missing))).resolves.toMatchObject({ status: 'INVALID' })

    const failed = await canonicalState()
    ;(failed.transactions.get(txSignature) as any).meta.err = { InstructionError: [1, 'Custom'] }
    await expect(verify(pkg(), rpc(failed))).resolves.toMatchObject({ status: 'INVALID' })

    const tampered = await canonicalState()
    const other = await stationTransaction('bind', CUSTODIAN)
    ;(tampered.transactions.get(txSignature) as any).transaction.message = other
    await expect(verify(pkg(), rpc(tampered))).resolves.toMatchObject({ status: 'INVALID' })
  })

  /**
   * ARRANGE: the presence proof was anchored by the asset's custodian instead of the authority.
   * ACTION: verify.
   * ASSERT: still VALID — the program accepts either signer and the exact bytes still match.
   * FAILURE MEANS: legitimate custodian-signed presence proofs would be reported as forged.
   */
  it('accepts presence proofs anchored by the custodian', async () => {
    const state = await canonicalState()
    const txSignature = validFixture.events[2]!.txSignature
    ;(state.transactions.get(txSignature) as any).transaction.message = await stationTransaction(
      'observe',
      CUSTODIAN,
    )
    await expect(verify(pkg(), rpc(state))).resolves.toMatchObject({ status: 'VALID' })
  })

  /**
   * ARRANGE: an RPC that is unreachable.
   * ACTION: verify.
   * ASSERT: NOT_CHECKED, never VALID or INVALID.
   * FAILURE MEANS: network failure would be reported as a verdict about the evidence.
   */
  it('represents RPC unavailability as NOT_CHECKED', async () => {
    const offline = (async () => {
      throw new TypeError('network down')
    }) as unknown as typeof fetch
    await expect(verify(pkg(), offline)).resolves.toMatchObject({ status: 'NOT_CHECKED' })
  })
})

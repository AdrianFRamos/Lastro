import {
  address,
  getAddressDecoder,
  getBase58Decoder,
  getBase58Encoder,
  getProgramDerivedAddress,
} from '@solana/kit'
import { describe, expect, it } from 'vitest'
import validFixture from '../../../../test-vectors/evidence-package.valid.json'
import { parseEvidencePackage } from '../../src/protocol/evidence'
import { eventHash } from '../../src/protocol/hash'
import { decodeStationEvent } from '../../src/protocol/stationEvent'
import { SECP256R1_PROGRAM_ID } from '../../src/solana/constants'
import { verifyCanonicalChainState } from '../../src/verify/verifyChain'

const PROGRAM_ID = '11111111111111111111111111111111'
const TX_PROGRAM_ID = 'Vote111111111111111111111111111111111111111'
const SYSTEM_PROGRAM_ID = '11111111111111111111111111111111'
const INSTRUCTIONS_SYSVAR_ID = 'Sysvar1nstructions1111111111111111111111111'
const RPC_URL = 'https://rpc.invalid.test'
const TRUSTED_AUTHORITY = 'Vote111111111111111111111111111111111111111'
const text = new TextEncoder()

type FixtureMutation = (accounts: RpcFixtureAccounts) => void

interface RpcFixtureAccounts {
  config: Uint8Array
  animal: Uint8Array
  currentBinding: Uint8Array
  retiredBindings: Uint8Array[]
}

function bytesHex(value: string): Uint8Array {
  return Uint8Array.from(value.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16))
}

function bytesBase64(value: string): Uint8Array {
  return Uint8Array.from(atob(value), (character) => character.charCodeAt(0))
}

function writeU32(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).setUint32(offset, value, true)
}

function writeU64(bytes: Uint8Array, offset: number, value: bigint): void {
  new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).setBigUint64(offset, value, true)
}

async function discriminator(name: string): Promise<Uint8Array> {
  return new Uint8Array(
    await crypto.subtle.digest('SHA-256', text.encode(`account:${name}`)),
  ).subarray(0, 8)
}

async function instructionDiscriminator(name: string): Promise<Uint8Array> {
  return new Uint8Array(
    await crypto.subtle.digest('SHA-256', text.encode(`global:${name}`)),
  ).subarray(0, 8)
}

async function canonicalAccounts(programId = PROGRAM_ID): Promise<RpcFixtureAccounts> {
  const pkg = parseEvidencePackage(structuredClone(validFixture))
  const deployment = bytesHex(pkg.deploymentId)
  const animalId = bytesHex(pkg.animalId)
  const decoded = pkg.events.map((entry) => ({
    raw: bytesBase64(entry.eventBytesBase64),
    event: decodeStationEvent(bytesBase64(entry.eventBytesBase64)),
  }))
  const last = decoded.at(-1)!
  const programAddress = address(programId)

  const [, configBump] = await getProgramDerivedAddress({
    programAddress,
    seeds: [text.encode('config'), deployment],
  })
  const config = new Uint8Array(106)
  config.set(await discriminator('ProtocolConfig'), 0)
  config.set(getBase58Encoder().encode(TRUSTED_AUTHORITY), 8)
  config.set(deployment, 40)
  config.set(bytesHex(pkg.events[0]!.stationPubkeyHex), 72)
  config[105] = configBump

  const [, animalBump] = await getProgramDerivedAddress({
    programAddress,
    seeds: [text.encode('animal'), deployment, animalId],
  })
  const animal = new Uint8Array(149)
  animal.set(await discriminator('AnimalState'), 0)
  animal.set(animalId, 8)
  animal.set(last.event.newRfidHash, 40)
  animal.set(last.event.toCustodian, 72)
  writeU32(animal, 104, last.event.identityRevision)
  writeU64(animal, 108, last.event.eventSequence)
  animal.set(await eventHash(last.raw), 116)
  animal[148] = animalBump

  const binding = async (rfidHash: Uint8Array, status: 1 | 2): Promise<Uint8Array> => {
    const [, bump] = await getProgramDerivedAddress({
      programAddress,
      seeds: [text.encode('rfid'), deployment, rfidHash],
    })
    const data = new Uint8Array(74)
    data.set(await discriminator('RfidBinding'), 0)
    data.set(animalId, 8)
    data.set(rfidHash, 40)
    data[72] = status
    data[73] = bump
    return data
  }

  const retired = decoded
    .filter((entry) => entry.event.action === 3)
    .map((entry) => binding(entry.event.oldRfidHash, 2))

  return {
    config,
    animal,
    currentBinding: await binding(last.event.newRfidHash, 1),
    retiredBindings: await Promise.all(retired),
  }
}

function accountResponse(data: Uint8Array, owner = PROGRAM_ID): Response {
  const encoded = btoa(String.fromCharCode(...data))
  return new Response(
    JSON.stringify({
      jsonrpc: '2.0',
      id: 1,
      result: {
        context: { slot: 1 },
        value: {
          data: [encoded, 'base64'],
          executable: false,
          lamports: 1,
          owner,
          rentEpoch: 0,
          space: data.length,
        },
      },
    }),
    { status: 200, headers: { 'content-type': 'application/json' } },
  )
}

async function rpcFetch(mutate?: FixtureMutation): Promise<typeof fetch> {
  const accounts = await canonicalAccounts()
  mutate?.(accounts)
  const queue = [
    accounts.config,
    accounts.animal,
    accounts.currentBinding,
    ...accounts.retiredBindings,
  ]
  return (async () => {
    const data = queue.shift()
    if (!data) throw new Error('unexpected RPC request')
    return accountResponse(data)
  }) as typeof fetch
}

async function verify(mutate?: FixtureMutation) {
  return verifyCanonicalChainState(parseEvidencePackage(structuredClone(validFixture)), {
    programId: PROGRAM_ID,
    trustedAuthority: TRUSTED_AUTHORITY,
    rpcUrl: RPC_URL,
    fetchFn: await rpcFetch(mutate),
  })
}

interface CompiledMessageFixture {
  header: {
    numRequiredSignatures: number
    numReadonlySignedAccounts: number
    numReadonlyUnsignedAccounts: number
  }
  accountKeys: string[]
  recentBlockhash: string
  instructions: Array<{ programIdIndex: number; accounts: number[]; data: string }>
}

interface FinalizedTransactionFixture {
  version: 'legacy'
  meta: { err: unknown }
  transaction: {
    signatures: string[]
    message: CompiledMessageFixture
  }
}

type TransactionMutation = (transactions: Array<FinalizedTransactionFixture | null>) => void

function base58(bytes: Uint8Array): string {
  return getBase58Decoder().decode(bytes)
}

function fromBase58(value: string): Uint8Array {
  return Uint8Array.from(getBase58Encoder().encode(value))
}

function withTransactionSignatures() {
  const pkg = parseEvidencePackage(structuredClone(validFixture))
  pkg.events.forEach((entry, index) => {
    entry.txSignature = base58(new Uint8Array(64).fill(index + 1))
  })
  return pkg
}

async function finalizedTransaction(
  pkg: ReturnType<typeof withTransactionSignatures>,
  eventIndex: number,
): Promise<FinalizedTransactionFixture> {
  const entry = pkg.events[eventIndex]!
  const raw = bytesBase64(entry.eventBytesBase64)
  const event = decodeStationEvent(raw)
  const programAddress = address(TX_PROGRAM_ID)
  const [config] = await getProgramDerivedAddress({
    programAddress,
    seeds: [text.encode('config'), event.deploymentId],
  })
  const [animal] = await getProgramDerivedAddress({
    programAddress,
    seeds: [text.encode('animal'), event.deploymentId, event.animalId],
  })
  const signer = getAddressDecoder().decode(
    event.action === 1 ? event.toCustodian : event.fromCustodian,
  )

  let accounts: Array<{ address: string; isWritable: boolean }>
  if (event.action === 1) {
    const [binding] = await getProgramDerivedAddress({
      programAddress,
      seeds: [text.encode('rfid'), event.deploymentId, event.newRfidHash],
    })
    accounts = [
      { address: signer, isWritable: true },
      { address: String(config), isWritable: false },
      { address: String(animal), isWritable: true },
      { address: String(binding), isWritable: true },
      { address: INSTRUCTIONS_SYSVAR_ID, isWritable: false },
      { address: SYSTEM_PROGRAM_ID, isWritable: false },
    ]
  } else if (event.action === 2) {
    const [binding] = await getProgramDerivedAddress({
      programAddress,
      seeds: [text.encode('rfid'), event.deploymentId, event.oldRfidHash],
    })
    accounts = [
      { address: signer, isWritable: false },
      { address: String(config), isWritable: false },
      { address: String(animal), isWritable: true },
      { address: String(binding), isWritable: false },
      { address: INSTRUCTIONS_SYSVAR_ID, isWritable: false },
    ]
  } else {
    const [oldBinding] = await getProgramDerivedAddress({
      programAddress,
      seeds: [text.encode('rfid'), event.deploymentId, event.oldRfidHash],
    })
    const [newBinding] = await getProgramDerivedAddress({
      programAddress,
      seeds: [text.encode('rfid'), event.deploymentId, event.newRfidHash],
    })
    accounts = [
      { address: signer, isWritable: true },
      { address: String(config), isWritable: false },
      { address: String(animal), isWritable: true },
      { address: String(oldBinding), isWritable: true },
      { address: String(newBinding), isWritable: true },
      { address: INSTRUCTIONS_SYSVAR_ID, isWritable: false },
      { address: SYSTEM_PROGRAM_ID, isWritable: false },
    ]
  }

  const unique = (values: string[]) => [...new Set(values)]
  const writableUnsigned = unique(
    accounts
      .filter((account) => account.address !== signer && account.isWritable)
      .map((account) => account.address),
  )
  const readonlyUnsigned = unique([
    ...accounts
      .filter((account) => account.address !== signer && !account.isWritable)
      .map((account) => account.address),
    SECP256R1_PROGRAM_ID,
    TX_PROGRAM_ID,
  ]).filter((account) => !writableUnsigned.includes(account))
  const accountKeys = [signer, ...writableUnsigned, ...readonlyUnsigned]

  const secpData = new Uint8Array(113)
  secpData[0] = 1
  const descriptor = new DataView(secpData.buffer)
  ;[16, 0, 80, 0, 8, 276, 1].forEach((value, fieldIndex) => {
    descriptor.setUint16(2 + fieldIndex * 2, value, true)
  })
  secpData.set(bytesHex(entry.stationSignatureHex), 16)
  secpData.set(bytesHex(entry.stationPubkeyHex), 80)

  const actionName = event.action === 1 ? 'origin' : event.action === 2 ? 'transfer' : 'reidentify'
  const lastroData = new Uint8Array(8 + raw.length)
  lastroData.set(await instructionDiscriminator(actionName), 0)
  lastroData.set(raw, 8)

  return {
    version: 'legacy',
    meta: { err: null },
    transaction: {
      signatures: [entry.txSignature!],
      message: {
        header: {
          numRequiredSignatures: 1,
          numReadonlySignedAccounts: 0,
          numReadonlyUnsignedAccounts: readonlyUnsigned.length,
        },
        accountKeys,
        recentBlockhash: '11111111111111111111111111111111',
        instructions: [
          {
            programIdIndex: accountKeys.indexOf(SECP256R1_PROGRAM_ID),
            accounts: [],
            data: base58(secpData),
          },
          {
            programIdIndex: accountKeys.indexOf(TX_PROGRAM_ID),
            accounts: accounts.map((account) => accountKeys.indexOf(account.address)),
            data: base58(lastroData),
          },
        ],
      },
    },
  }
}

async function transactionRpcFixture(mutate?: TransactionMutation) {
  const pkg = withTransactionSignatures()
  const accounts = await canonicalAccounts(TX_PROGRAM_ID)
  const transactions: Array<FinalizedTransactionFixture | null> = await Promise.all(
    pkg.events.map((_, eventIndex) => finalizedTransaction(pkg, eventIndex)),
  )
  mutate?.(transactions)
  const accountQueue = [
    accounts.config,
    accounts.animal,
    accounts.currentBinding,
    ...accounts.retiredBindings,
  ]

  const fetchFn = (async (_input: RequestInfo | URL, init?: RequestInit) => {
    const request = JSON.parse(String(init?.body)) as { method?: string; params?: unknown[] }
    if (request.method === 'getTransaction') {
      const signature = request.params?.[0]
      const eventIndex = pkg.events.findIndex((entry) => entry.txSignature === signature)
      if (eventIndex < 0) throw new Error('unexpected transaction signature')
      return new Response(
        JSON.stringify({
          jsonrpc: '2.0',
          id: 1,
          result: transactions[eventIndex],
        }),
        { status: 200, headers: { 'content-type': 'application/json' } },
      )
    }
    if (request.method === 'getAccountInfo') {
      const data = accountQueue.shift()
      if (!data) throw new Error('unexpected account RPC request')
      return accountResponse(data, TX_PROGRAM_ID)
    }
    throw new Error('unexpected RPC method')
  }) as typeof fetch

  return { pkg, fetchFn }
}

describe('verify/final canonical Solana comparison', () => {
  /**
   * ARRANGE: Use canonical fixture accounts and an independent trusted authority.
   * ACTION: Remove the authority anchor, then substitute the on-chain authority.
   * ASSERT: An absent anchor is NOT_CHECKED; a substituted authority is INVALID.
   * FAILURE MEANS: an impostor deployment can be presented as independent proof.
   */
  it('refuses an unpinned deployment authority and rejects an authority substituted on-chain', async () => {
    const pkg = parseEvidencePackage(structuredClone(validFixture))
    const withoutAnchor = await verifyCanonicalChainState(pkg, { trustedAuthority: '' })
    expect(withoutAnchor.status).toBe('NOT_CHECKED')

    const changedAuthority = await verify((accounts) => {
      accounts.config[8] = (accounts.config[8] ?? 0) ^ 1
    })
    expect(changedAuthority.status).toBe('INVALID')
    expect(changedAuthority.detail).toContain('trusted deployment authority')
  })
  /**
   * ARRANGE: build canonical ProtocolConfig, AnimalState, current RFID binding, and retired RFID binding from the valid package.
   * ACTION: compare the independently reconstructed terminal state to those RPC account bytes.
   * ASSERT: ON_CHAIN_STATE is VALID only when every required canonical field agrees.
   * FAILURE MEANS: independent verification cannot establish agreement with canonical Solana state.
   */
  it('checks canonical accounts but does not mark transactionless evidence as final valid proof', async () => {
    const result = await verify()
    expect(result).toMatchObject({ layer: 'ON_CHAIN_STATE', status: 'NOT_CHECKED' })
    expect(result.detail).toContain('transaction signatures are absent')
  })

  /**
   * ARRANGE: start from canonical RPC fixtures and change only the current custodian byte.
   * ACTION: compare terminal package state to canonical AnimalState.
   * ASSERT: ON_CHAIN_STATE is INVALID.
   * FAILURE MEANS: stale or fabricated custody could be presented as current.
   */
  it('rejects a package whose terminal custodian differs from canonical RPC state', async () => {
    const result = await verify(({ animal }) => {
      animal[72] = animal[72]! ^ 1
    })
    expect(result.status).toBe('INVALID')
    expect(result.detail).toContain('custodian')
  })

  /**
   * ARRANGE: independently mutate canonical current RFID, identity revision, sequence, and last event hash.
   * ACTION: compare each mutated account to the same locally valid package.
   * ASSERT: every individual terminal-field mismatch yields INVALID.
   * FAILURE MEANS: the verifier is not comparing the complete canonical AnimalState contract.
   */
  it('rejects every terminal RFID revision sequence or last-event-hash mismatch', async () => {
    const mutations: FixtureMutation[] = [
      ({ animal }) => {
        animal[40] = animal[40]! ^ 1
      },
      ({ animal }) => {
        writeU32(animal, 104, new DataView(animal.buffer).getUint32(104, true) + 1)
      },
      ({ animal }) => {
        writeU64(animal, 108, new DataView(animal.buffer).getBigUint64(108, true) + 1n)
      },
      ({ animal }) => {
        animal[116] = animal[116]! ^ 1
      },
    ]
    for (const mutate of mutations) expect((await verify(mutate)).status).toBe('INVALID')
  })

  /**
   * ARRANGE: provide a locally valid package but make the RPC transport throw before returning any account.
   * ACTION: perform canonical Solana verification.
   * ASSERT: ON_CHAIN_STATE is NOT_CHECKED, never VALID or INVALID solely because the network is unavailable.
   * FAILURE MEANS: network unavailability can be confused with proof validity.
   */
  it('represents RPC unavailability as NOT_CHECKED and never as canonical validity', async () => {
    const fetchFn = (async () => {
      throw new TypeError('network unavailable')
    }) as typeof fetch
    const result = await verifyCanonicalChainState(
      parseEvidencePackage(structuredClone(validFixture)),
      {
        programId: PROGRAM_ID,
        trustedAuthority: TRUSTED_AUTHORITY,
        rpcUrl: RPC_URL,
        fetchFn,
      },
    )
    expect(result.status).toBe('NOT_CHECKED')
  })

  /**
   * ARRANGE: return canonical-looking account bytes once with the wrong owner and once with a malformed layout.
   * ACTION: decode and compare canonical state.
   * ASSERT: both substitutions yield INVALID rather than trusting bytes at the expected address.
   * FAILURE MEANS: RPC account substitution could forge the canonical comparison.
   */
  it('requires the AnimalState account owner and binary layout to match the Lastro program', async () => {
    const accounts = await canonicalAccounts()
    const wrongOwnerQueue = [
      accounts.config,
      accounts.animal,
      accounts.currentBinding,
      ...accounts.retiredBindings,
    ]
    let request = 0
    const wrongOwnerFetch = (async () => {
      const data = wrongOwnerQueue.shift()!
      const response = accountResponse(
        data,
        request++ === 1 ? 'Vote111111111111111111111111111111111111111' : PROGRAM_ID,
      )
      return response
    }) as typeof fetch
    const wrongOwner = await verifyCanonicalChainState(
      parseEvidencePackage(structuredClone(validFixture)),
      {
        programId: PROGRAM_ID,
        trustedAuthority: TRUSTED_AUTHORITY,
        rpcUrl: RPC_URL,
        fetchFn: wrongOwnerFetch,
      },
    )
    expect(wrongOwner.status).toBe('INVALID')

    const malformedQueue = [accounts.config, accounts.animal.slice(0, 148)]
    const malformedFetch = (async () => accountResponse(malformedQueue.shift()!)) as typeof fetch
    const malformed = await verifyCanonicalChainState(
      parseEvidencePackage(structuredClone(validFixture)),
      {
        programId: PROGRAM_ID,
        trustedAuthority: TRUSTED_AUTHORITY,
        rpcUrl: RPC_URL,
        fetchFn: malformedFetch,
      },
    )
    expect(malformed.status).toBe('INVALID')
  })

  /**
   * ARRANGE: start from valid local evidence and add a transaction signature to only one event.
   * ACTION: compare it to canonical Solana state.
   * ASSERT: the package is INVALID before it can masquerade as a partially proven history.
   * FAILURE MEANS: a verifier could mix transaction-bound and unbound events in one accepted proof.
   */
  it('rejects a mixed set of present and missing transaction signatures', async () => {
    const pkg = parseEvidencePackage(structuredClone(validFixture))
    pkg.events[0]!.txSignature = base58(new Uint8Array(64).fill(7))
    const fetchFn = await rpcFetch()
    const result = await verifyCanonicalChainState(pkg, {
      programId: PROGRAM_ID,
      trustedAuthority: TRUSTED_AUTHORITY,
      rpcUrl: RPC_URL,
      fetchFn,
    })
    expect(result.status).toBe('INVALID')
    expect(result.detail).toContain('every event or for none')
  })

  /**
   * ARRANGE: attach one deterministic transaction signature to every valid evidence event and return exact finalized RPC envelopes.
   * ACTION: independently verify canonical accounts and each finalized transaction.
   * ASSERT: ON_CHAIN_STATE is VALID only when every txSignature maps to the exact StationEvent/program/account envelope.
   * FAILURE MEANS: the verifier could accept evidence without proving its claimed canonical transactions.
   */
  it('binds every txSignature to the exact finalized Lastro transaction envelope', async () => {
    const fixture = await transactionRpcFixture()
    const result = await verifyCanonicalChainState(fixture.pkg, {
      programId: TX_PROGRAM_ID,
      trustedAuthority: TRUSTED_AUTHORITY,
      rpcUrl: RPC_URL,
      fetchFn: fixture.fetchFn,
    })
    expect(result.status).toBe('VALID')
    expect(result.detail).toContain('every supplied transaction is finalized')
  })

  /**
   * ARRANGE: use otherwise valid evidence, then return either no finalized transaction or a finalized transaction with meta.err.
   * ACTION: verify canonical chain evidence.
   * ASSERT: both cases are INVALID.
   * FAILURE MEANS: pending or failed Solana execution could be presented as accepted canonical history.
   */
  it('rejects non-finalized and failed evidence transactions', async () => {
    const nonFinalized = await transactionRpcFixture((transactions) => {
      transactions[0] = null
    })
    expect(
      (
        await verifyCanonicalChainState(nonFinalized.pkg, {
          programId: TX_PROGRAM_ID,
          trustedAuthority: TRUSTED_AUTHORITY,
          rpcUrl: RPC_URL,
          fetchFn: nonFinalized.fetchFn,
        })
      ).status,
    ).toBe('INVALID')

    const failed = await transactionRpcFixture((transactions) => {
      transactions[0]!.meta.err = { InstructionError: [1, { Custom: 6000 }] }
    })
    expect(
      (
        await verifyCanonicalChainState(failed.pkg, {
          programId: TX_PROGRAM_ID,
          trustedAuthority: TRUSTED_AUTHORITY,
          rpcUrl: RPC_URL,
          fetchFn: failed.fetchFn,
        })
      ).status,
    ).toBe('INVALID')
  })

  /**
   * ARRANGE: keep the requested txSignature but return a finalized transaction carrying a different signature.
   * ACTION: verify the EvidencePackage.
   * ASSERT: ON_CHAIN_STATE is INVALID.
   * FAILURE MEANS: an unrelated finalized transaction could satisfy an evidence entry.
   */
  it('rejects a finalized transaction whose signature differs from EvidencePackage', async () => {
    const fixture = await transactionRpcFixture((transactions) => {
      transactions[0]!.transaction.signatures[0] = base58(new Uint8Array(64).fill(99))
    })
    const result = await verifyCanonicalChainState(fixture.pkg, {
      programId: TX_PROGRAM_ID,
      trustedAuthority: TRUSTED_AUTHORITY,
      rpcUrl: RPC_URL,
      fetchFn: fixture.fetchFn,
    })
    expect(result.status).toBe('INVALID')
    expect(result.detail).toContain('signature')
  })

  /**
   * ARRANGE: start from an exact finalized ORIGIN envelope and independently alter instruction data, program selection, or account order.
   * ACTION: verify each tampered transaction against the unchanged signed evidence.
   * ASSERT: every mutation is INVALID.
   * FAILURE MEANS: txSignature verification is not bound to exact StationEvent bytes, Lastro program, and derived accounts.
   */
  it('rejects tampered StationEvent bytes program or accounts in finalized transactions', async () => {
    const mutations: TransactionMutation[] = [
      (transactions) => {
        const instruction = transactions[0]!.transaction.message.instructions[1]!
        const data = fromBase58(instruction.data)
        data[data.length - 1] = data[data.length - 1]! ^ 1
        instruction.data = base58(data)
      },
      (transactions) => {
        const message = transactions[0]!.transaction.message
        const systemIndex = message.accountKeys.indexOf(SYSTEM_PROGRAM_ID)
        if (systemIndex < 0) throw new Error('ORIGIN fixture is missing System Program')
        message.instructions[1]!.programIdIndex = systemIndex
      },
      (transactions) => {
        const accounts = transactions[0]!.transaction.message.instructions[1]!.accounts
        ;[accounts[1], accounts[2]] = [accounts[2]!, accounts[1]!]
      },
    ]
    for (const mutate of mutations) {
      const fixture = await transactionRpcFixture(mutate)
      const result = await verifyCanonicalChainState(fixture.pkg, {
        programId: TX_PROGRAM_ID,
        trustedAuthority: TRUSTED_AUTHORITY,
        rpcUrl: RPC_URL,
        fetchFn: fixture.fetchFn,
      })
      expect(result.status).toBe('INVALID')
    }
  })

  /**
   * ARRANGE: a fetch implementation that stalls until its AbortSignal is fired.
   * ACTION: verify a locally valid package with a 1 ms RPC deadline.
   * ASSERT: timeout maps to NOT_CHECKED rather than VALID or INVALID.
   * FAILURE MEANS: canonical verification can hang indefinitely or misclassify network unavailability.
   */
  it('times out stalled RPC as NOT_CHECKED', async () => {
    const stalledFetch = (async (_input: RequestInfo | URL, init?: RequestInit) => {
      await new Promise<never>((_resolve, reject) => {
        const signal = init?.signal
        if (!signal) {
          reject(new Error('expected verifier RPC AbortSignal'))
          return
        }
        signal.addEventListener('abort', () => reject(new DOMException('aborted', 'AbortError')), {
          once: true,
        })
      })
      throw new Error('unreachable')
    }) as typeof fetch

    const result = await verifyCanonicalChainState(
      parseEvidencePackage(structuredClone(validFixture)),
      {
        programId: PROGRAM_ID,
        trustedAuthority: TRUSTED_AUTHORITY,
        rpcUrl: RPC_URL,
        fetchFn: stalledFetch,
        rpcTimeoutMs: 1,
      },
    )
    expect(result.status).toBe('NOT_CHECKED')
    expect(result.detail).toContain('unavailable')
  })
})

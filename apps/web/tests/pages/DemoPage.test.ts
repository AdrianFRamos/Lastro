import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { AnimalProjection } from '../../src/api/types'
import vectors from '../../../../test-vectors/vectors.json'
import DemoPage from '../../src/pages/DemoPage.vue'

const WALLET_A = 'wallet-a'
const WALLET_B = 'wallet-b'
const WALLET_C = 'wallet-c'
const CUSTODIAN_A = 'aa'.repeat(32)
const CUSTODIAN_B = 'bb'.repeat(32)
const CUSTODIAN_C = 'cc'.repeat(32)

const mocks = vi.hoisted(() => ({
  walletAddress: null as string | null,
  api: {
    createAnimal: vi.fn(),
    getAnimal: vi.fn(),
    getAnimalByRecovery: vi.fn(),
    getEvidencePackage: vi.fn(),
    getCaptureAuthorizationChallenge: vi.fn(),
    createCapture: vi.fn(),
    getCapture: vi.fn(),
    getTransactionData: vi.fn(),
    submit: vi.fn(),
    confirm: vi.fn(),
  },
  connect: vi.fn(),
  connectByName: vi.fn(),
  signCaptureAuthorization: vi.fn(),
  walletChoices: [] as Array<{ name: string; address: string }>,
  submit: vi.fn(),
  rebroadcast: vi.fn(),
  getBlockHeight: vi.fn(),
  getSignatureStatuses: vi.fn(),
}))

vi.mock('../../src/api/client', () => ({
  api: mocks.api,
  ApiClientError: class ApiClientError extends Error {
    constructor(
      message: string,
      readonly status: number | null,
      readonly responseBody: string | null,
    ) {
      super(message)
    }
  },
}))

vi.mock('../../src/solana/wallet', () => ({
  availableWalletChoices: () => mocks.walletChoices,
  connectFirstAvailableWallet: mocks.connect,
  connectWalletByName: mocks.connectByName,
  currentWalletAddress: () => mocks.walletAddress,
  signCaptureAuthorization: mocks.signCaptureAuthorization,
  walletAddressToCustodianHex: (address: string) => {
    if (address === WALLET_A) return CUSTODIAN_A
    if (address === WALLET_B) return CUSTODIAN_B
    if (address === WALLET_C) return CUSTODIAN_C
    throw new Error('invalid test wallet')
  },
}))

vi.mock('../../src/solana/transaction', () => ({
  submitLastroTransaction: mocks.submit,
  rebroadcastSignedLastroTransaction: mocks.rebroadcast,
}))

vi.mock('../../src/solana/client', () => ({
  solanaClient: {
    rpc: {
      getBlockHeight: mocks.getBlockHeight,
      getSignatureStatuses: mocks.getSignatureStatuses,
    },
  },
}))

const unoriginated: AnimalProjection = {
  animalId: '11'.repeat(32),
  visualRecoveryId: 'VIS-0042',
  currentRfidHash: null,
  currentCustodian: null,
  identityRevision: 0,
  eventSequence: 0,
  lastEventHash: null,
}

const withCustodianB: AnimalProjection = {
  ...unoriginated,
  currentRfidHash: '22'.repeat(32),
  currentCustodian: CUSTODIAN_B,
  eventSequence: 2,
  lastEventHash: '33'.repeat(32),
}

function eventBase64(eventHex: string): string {
  const bytes = Uint8Array.from(eventHex.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16))
  return btoa(String.fromCharCode(...bytes))
}

const twoEventPackage = {
  version: 1 as const,
  deploymentId: vectors.deployment_id_hex,
  animalId: vectors.animal_id_hex,
  events: [vectors.events.origin, vectors.events.transfer].map((event) => ({
    eventBytesBase64: eventBase64(event.event_bytes_hex),
    observedRfidHex: vectors.rfid.a_canonical_hex,
    stationPubkeyHex: vectors.station.pubkey_compressed_hex,
    stationSignatureHex: event.station_signature_hex,
    txSignature: 'test-signature',
  })),
}

function mountPage(): VueWrapper {
  return mount(DemoPage, {
    global: {
      stubs: {
        RouterLink: { template: '<a><slot /></a>' },
      },
    },
  })
}

function button(wrapper: VueWrapper, label: string) {
  const match = wrapper.findAll('button').find((candidate) => candidate.text() === label)
  if (!match) throw new Error(`button not found: ${label}`)
  return match
}

async function recover(wrapper: VueWrapper, animal: AnimalProjection): Promise<void> {
  mocks.api.getAnimalByRecovery.mockResolvedValueOnce(animal)
  await wrapper.get('input[aria-label="Visual recovery ID"]').setValue(animal.visualRecoveryId)
  await button(wrapper, 'Find by visual recovery ID').trigger('click')
  await flushPromises()
}

async function refreshWallet(wrapper: VueWrapper, address: string): Promise<void> {
  mocks.walletAddress = address
  mocks.connect.mockResolvedValueOnce(undefined)
  await button(wrapper, 'Connect wallet').trigger('click')
  await flushPromises()
}

beforeEach(() => {
  window.history.replaceState(null, '', '/demo')
  window.localStorage.clear()
  mocks.walletAddress = null
  mocks.walletChoices = []
  for (const fn of Object.values(mocks.api)) fn.mockReset()
  mocks.api.getEvidencePackage.mockResolvedValue(twoEventPackage)
  mocks.connect.mockReset()
  mocks.connectByName.mockReset()
  mocks.signCaptureAuthorization.mockReset()
  mocks.signCaptureAuthorization.mockResolvedValue({
    challengeId: 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee',
    signatureBase64: btoa(String.fromCharCode(...new Uint8Array(64).fill(7))),
  })
  mocks.api.getCaptureAuthorizationChallenge.mockResolvedValue({
    challengeId: 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee',
    deploymentId: vectors.deployment_id_hex,
    requiredSigner: WALLET_B,
    messageBase64: btoa('test capture authorization'),
    expiresAtUnix: 2_000_000_000,
  })
  mocks.submit.mockReset()
  mocks.rebroadcast.mockReset()
  mocks.getBlockHeight.mockReset()
  mocks.getSignatureStatuses.mockReset()
  mocks.getBlockHeight.mockReturnValue({ send: async () => 1n })
  mocks.getSignatureStatuses.mockReturnValue({ send: async () => ({ value: [null] }) })
})

describe('pages/DemoPage state machine', () => {
  /**
   * ARRANGE: vary selected-animal origin state and wallet connection independently. Station readiness is not
   *          fabricated because the protocol exposes no live readiness telemetry; physical proof is required after capture.
   * ACTION: render and recover/create animals while refreshing wallet state.
   * ASSERT: ORIGIN is enabled only for an unoriginated animal with an intended custodian wallet connected.
   * FAILURE MEANS: UI can start ORIGIN without the browser-known protocol prerequisites.
   */
  it('enables ORIGIN only for an unoriginated animal with a connected intended custodian wallet', async () => {
    const wrapper = mountPage()
    expect(button(wrapper, 'Origin').attributes('disabled')).toBeDefined()

    await recover(wrapper, unoriginated)
    expect(button(wrapper, 'Origin').attributes('disabled')).toBeDefined()

    await refreshWallet(wrapper, WALLET_A)
    expect(button(wrapper, 'Origin').attributes('disabled')).toBeUndefined()

    await recover(wrapper, withCustodianB)
    expect(button(wrapper, 'Origin').attributes('disabled')).toBeDefined()
    wrapper.unmount()
  })

  /**
   * ARRANGE: canonical projection says custodian B; connect wallet A then B and provide a next custodian.
   * ACTION: render TRANSFER eligibility.
   * ASSERT: A cannot submit normal transfer; B can reach the capture path.
   * FAILURE MEANS: UI authority does not reflect canonical custodian state.
   */
  it('normal TRANSFER action is available only to the current custodian wallet', async () => {
    mocks.walletAddress = WALLET_A
    const wrapper = mountPage()
    await recover(wrapper, withCustodianB)
    await wrapper.get('input[aria-label="Next custodian wallet"]').setValue(WALLET_C)
    expect(button(wrapper, 'Transfer').attributes('disabled')).toBeDefined()

    await refreshWallet(wrapper, WALLET_B)
    expect(button(wrapper, 'Transfer').attributes('disabled')).toBeUndefined()
    wrapper.unmount()
  })

  /**
   * ARRANGE: RFID A is unavailable; visual ID resolves the existing AnimalID with custodian B.
   * ACTION: recover identity under wallet A, then switch to wallet B.
   * ASSERT: lookup only selects identity; B wallet remains required before REIDENTIFY can start a capture.
   * FAILURE MEANS: visual recovery identifier has accidentally become an authorization factor.
   */
  it('REIDENTIFY uses visual ID for recovery but still requires current custodian authority', async () => {
    mocks.walletAddress = WALLET_A
    const wrapper = mountPage()
    await recover(wrapper, withCustodianB)
    expect(mocks.api.createCapture).not.toHaveBeenCalled()
    expect(button(wrapper, 'Reidentify').attributes('disabled')).toBeDefined()

    await refreshWallet(wrapper, WALLET_B)
    expect(button(wrapper, 'Reidentify').attributes('disabled')).toBeUndefined()
    expect(mocks.api.createCapture).not.toHaveBeenCalled()
    wrapper.unmount()
  })

  /**
   * ARRANGE: after A→B, first connect B and then preserve A as the deliberate adversarial actor.
   * ACTION: render the explicit stale-custodian demo control.
   * ASSERT: the control is absent for B and appears for stale wallet A without replacing normal TRANSFER authority checks.
   * FAILURE MEANS: demo-only adversarial behavior can leak into regular product actions.
   */
  it('exposes the stale-custodian attack only in the explicit stale-authority demo state', async () => {
    mocks.walletAddress = WALLET_B
    const wrapper = mountPage()
    await recover(wrapper, withCustodianB)
    expect(
      wrapper
        .findAll('button')
        .some((candidate) => candidate.text() === 'Run stale-custodian attempt'),
    ).toBe(false)

    await refreshWallet(wrapper, WALLET_A)
    expect(button(wrapper, 'Run stale-custodian attempt').exists()).toBe(true)
    expect(button(wrapper, 'Transfer').attributes('disabled')).toBeDefined()
    expect(button(wrapper, 'Reidentify').attributes('disabled')).toBeDefined()
    wrapper.unmount()
  })

  /**
   * PURPOSE: Prove wallet control before reserving physical Station work.
   * ARRANGE: Current custodian B requests a B->C transfer and the API issues one authorization challenge.
   * ACTION: Start TRANSFER, sign the challenge, and stop immediately after the capture reservation call.
   * ASSERT: challenge issuance and wallet proof both happen before createCapture, which receives that exact proof.
   * FAILURE MEANS: an unauthenticated public client can enqueue physical Station work before proving custody authority.
   */
  it('authorizes the exact capture intent before reserving Station work', async () => {
    mocks.walletAddress = WALLET_B
    const challenge = {
      challengeId: 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee',
      deploymentId: vectors.deployment_id_hex,
      requiredSigner: WALLET_B,
      messageBase64: btoa('test capture authorization'),
      expiresAtUnix: 2_000_000_000,
    }
    const proof = {
      challengeId: challenge.challengeId,
      signatureBase64: btoa(String.fromCharCode(...new Uint8Array(64).fill(7))),
    }
    mocks.api.getCaptureAuthorizationChallenge.mockResolvedValueOnce(challenge)
    mocks.signCaptureAuthorization.mockResolvedValueOnce(proof)
    mocks.api.createCapture.mockRejectedValueOnce(new Error('reservation boundary reached'))

    const wrapper = mountPage()
    await recover(wrapper, withCustodianB)
    await wrapper.get('input[aria-label="Next custodian wallet"]').setValue(WALLET_C)
    await button(wrapper, 'Transfer').trigger('click')
    await flushPromises()

    expect(mocks.api.getCaptureAuthorizationChallenge).toHaveBeenCalledWith(
      'TRANSFER',
      withCustodianB.animalId,
      CUSTODIAN_C,
      null,
    )
    expect(mocks.signCaptureAuthorization).toHaveBeenCalledWith(challenge, {
      action: 'TRANSFER',
      animalId: withCustodianB.animalId,
      nextCustodian: CUSTODIAN_C,
      supersedeCaptureId: null,
    })
    expect(mocks.api.createCapture).toHaveBeenCalledWith(
      'TRANSFER',
      withCustodianB.animalId,
      CUSTODIAN_C,
      proof,
      null,
    )
    expect(mocks.api.getCaptureAuthorizationChallenge.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.signCaptureAuthorization.mock.invocationCallOrder[0]!,
    )
    expect(mocks.signCaptureAuthorization.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.api.createCapture.mock.invocationCallOrder[0]!,
    )
    expect(wrapper.text()).toContain('reservation boundary reached')
    wrapper.unmount()
  })

  /**
   * ARRANGE: current custodian B reaches the TRANSFER wallet prompt and the wallet rejects signing.
   * ACTION: run the normal transfer path.
   * ASSERT: API confirmation is never called, the displayed projection remains at custodian B/sequence 2, and rejection is visible.
   * FAILURE MEANS: wallet rejection can be displayed or persisted as a successful custody transition.
   */
  it('wallet signature rejection leaves canonical and projected state unchanged', async () => {
    mocks.walletAddress = WALLET_B
    const wrapper = mountPage()
    await recover(wrapper, withCustodianB)
    await wrapper.get('input[aria-label="Next custodian wallet"]').setValue(WALLET_C)
    mocks.api.createCapture.mockResolvedValueOnce({
      captureId: 'capture-1',
      action: 'TRANSFER',
      animalId: withCustodianB.animalId,
      status: 'EVIDENCE_ACCEPTED',
      eventHash: '44'.repeat(32),
      eventStatus: 'EVIDENCE_ACCEPTED',
      txSignature: null,
    })
    mocks.api.getTransactionData.mockResolvedValueOnce({ requiredSigner: WALLET_B })
    mocks.submit.mockRejectedValueOnce(new Error('User rejected signing request'))

    await button(wrapper, 'Transfer').trigger('click')
    await flushPromises()

    expect(mocks.api.confirm).not.toHaveBeenCalled()
    expect(wrapper.text()).toContain(CUSTODIAN_B)
    expect(wrapper.text()).toContain('Sequence')
    expect(wrapper.text()).toContain('2')
    expect(wrapper.text()).toContain('User rejected signing request')
    expect(wrapper.findAll('[aria-label="Custody timeline"] li')).toHaveLength(2)
    wrapper.unmount()
  })

  /**
   * PURPOSE: Preserve the exact signed Solana transaction before an ambiguous RPC result.
   * ARRANGE: The Station evidence is accepted, the wallet signs, and the transaction helper reports
   *          the signed signature/wire through its pre-broadcast callback before simulating a lost RPC response.
   * ACTION: Run TRANSFER and let the submit helper fail after invoking that callback.
   * ASSERT: localStorage already contains the exact signature and signed wire, while API submission is not fabricated.
   * FAILURE MEANS: an RPC timeout after signing can lose the only retryable copy and tempt the UI to create a second transaction.
   */
  it('persists signed transaction bytes before an ambiguous RPC send failure', async () => {
    const pendingEventHash = '66'.repeat(32)
    const transactionSignature = '8'.repeat(88)
    const wireTransactionBase64 = btoa('signed-wire')
    mocks.walletAddress = WALLET_B
    const wrapper = mountPage()
    await recover(wrapper, withCustodianB)
    await wrapper.get('input[aria-label="Next custodian wallet"]').setValue(WALLET_C)

    mocks.api.createCapture.mockResolvedValueOnce({
      captureId: '44444444-4444-4444-4444-444444444444',
      action: 'TRANSFER',
      animalId: withCustodianB.animalId,
      status: 'EVIDENCE_ACCEPTED',
      eventHash: pendingEventHash,
      eventStatus: 'EVIDENCE_ACCEPTED',
      txSignature: null,
    })
    mocks.api.getTransactionData.mockResolvedValueOnce({ requiredSigner: WALLET_B })
    mocks.submit.mockImplementationOnce(
      async (
        _transactionData: unknown,
        _expectedIntent: unknown,
        onSigned: (signed: { signature: string; wireTransactionBase64: string }) => void,
      ) => {
        onSigned({ signature: transactionSignature, wireTransactionBase64 })
        throw new Error('RPC transport response lost after signing')
      },
    )

    await button(wrapper, 'Transfer').trigger('click')
    await flushPromises()

    const stored = JSON.parse(
      window.localStorage.getItem('lastro.pending-operation') ?? '{}',
    ) as Record<string, unknown>
    expect(stored.txSignature).toBe(transactionSignature)
    expect(stored.wireTransactionBase64).toBe(wireTransactionBase64)
    expect(stored.eventHash).toBe(pendingEventHash)
    expect(mocks.api.submit).not.toHaveBeenCalled()
    expect(wrapper.text()).toContain('RPC transport response lost after signing')
    wrapper.unmount()
  })

  /**
   * ARRANGE: animal recovery fails because the API projection is unavailable.
   * ACTION: attempt recovery and render the page error boundary.
   * ASSERT: no animal state is fabricated and no canonical-success message appears.
   * FAILURE MEANS: source-of-truth boundaries become ambiguous during outages.
   */
  it('does not display fabricated canonical success when API projection is unavailable', async () => {
    const wrapper = mountPage()
    mocks.api.getAnimalByRecovery.mockRejectedValueOnce(new Error('projection unavailable'))
    await wrapper.get('input[aria-label="Visual recovery ID"]').setValue('VIS-0042')
    await button(wrapper, 'Find by visual recovery ID').trigger('click')
    await flushPromises()

    expect(wrapper.text()).toContain('No animal selected.')
    expect(wrapper.text()).toContain('projection unavailable')
    expect(wrapper.text()).not.toContain('finalized and canonical state verified')
    wrapper.unmount()
  })

  /**
   * ARRANGE: a durable AnimalID query parameter points at an already originated projection with two finalized events.
   * ACTION: mount the demo page as a fresh browser load with no component memory.
   * ASSERT: projection and timeline are reconstructed from API projection plus EvidencePackage, and the visual recovery id is restored.
   * FAILURE MEANS: browser refresh correctness still depends on ephemeral Vue state.
   */
  it('restores selected animal and timeline from durable sources after page reload', async () => {
    window.history.replaceState(null, '', `/demo?animalId=${withCustodianB.animalId}`)
    mocks.api.getAnimal.mockResolvedValueOnce(withCustodianB)
    const wrapper = mountPage()
    await flushPromises()

    expect(mocks.api.getAnimal).toHaveBeenCalledWith(withCustodianB.animalId)
    expect(mocks.api.getEvidencePackage).toHaveBeenCalledWith(withCustodianB.animalId)
    expect(wrapper.get('input[aria-label="Visual recovery ID"]').element).toHaveProperty(
      'value',
      withCustodianB.visualRecoveryId,
    )
    expect(wrapper.findAll('[aria-label="Custody timeline"] li')).toHaveLength(2)
    expect(wrapper.text()).toContain(`Restored AnimalID ${withCustodianB.animalId}`)
    wrapper.unmount()
  })

  /**
   * ARRANGE: two Wallet Standard wallets are discovered and Wallet B is selected explicitly.
   * ACTION: connect from the operator console.
   * ASSERT: the named wallet connection path is used and the displayed authority becomes Wallet B.
   * FAILURE MEANS: the A→B→C demo cannot intentionally switch custodian actors.
   */
  it('connects the explicitly selected Wallet Standard wallet', async () => {
    mocks.walletChoices = [
      { name: 'Wallet A', address: WALLET_A },
      { name: 'Wallet B', address: WALLET_B },
    ]
    mocks.walletAddress = WALLET_B
    mocks.connectByName.mockResolvedValueOnce(undefined)
    const wrapper = mountPage()
    await flushPromises()
    await wrapper.get('select[aria-label="Wallet Standard wallet"]').setValue('Wallet B')
    await button(wrapper, 'Connect wallet').trigger('click')
    await flushPromises()

    expect(mocks.connectByName).toHaveBeenCalledWith('Wallet B')
    expect(wrapper.text()).toContain(WALLET_B)
    wrapper.unmount()
  })

  /**
   * ARRANGE: no animal is selected and neither physical identifier has resolved an identity.
   * ACTION: render the recovery section without entering a visual identifier.
   * ASSERT: continuity is explicitly UNRESOLVED and no capture action becomes available.
   * FAILURE MEANS: the UI can imply identity continuity when both independent physical identifiers are unavailable.
   */
  it('reports physical identity continuity as unresolved when no identifier resolves an animal', () => {
    const wrapper = mountPage()
    expect(wrapper.text()).toContain('Physical identity continuity: UNRESOLVED')
    expect(button(wrapper, 'Origin').attributes('disabled')).toBeDefined()
    expect(button(wrapper, 'Transfer').attributes('disabled')).toBeDefined()
    expect(button(wrapper, 'Reidentify').attributes('disabled')).toBeDefined()
    wrapper.unmount()
  })

  /**
   * PURPOSE: Resume a server-verified SUBMITTED transaction after browser reload without another wallet signature.
   * ARRANGE: URL remembers the animal, localStorage remembers the capture, and GET capture reports SUBMITTED with the verified transaction signature.
   * ACTION: Mount DemoPage as a fresh page load.
   * ASSERT: The page confirms the stored signature to finality without requesting transaction data or invoking the wallet signer again.
   * FAILURE MEANS: a recoverable reload can cause duplicate wallet authorization or lose a valid in-flight transition.
   */
  it('reload resumes a durable SUBMITTED transaction without signing again', async () => {
    const pendingEventHash = '44'.repeat(32)
    const transactionSignature = '5'.repeat(88)
    const finalized = {
      ...withCustodianB,
      currentCustodian: CUSTODIAN_C,
      eventSequence: 3,
      lastEventHash: pendingEventHash,
    }
    window.history.replaceState(null, '', `/demo?animalId=${withCustodianB.animalId}`)
    window.localStorage.setItem(
      'lastro.pending-operation',
      JSON.stringify({
        animalId: withCustodianB.animalId,
        captureId: '11111111-1111-1111-1111-111111111111',
        action: 'TRANSFER',
        nextCustodian: CUSTODIAN_C,
        expectedToCustodian: CUSTODIAN_C,
        eventHash: pendingEventHash,
        txSignature: transactionSignature,
      }),
    )
    mocks.api.getAnimal.mockResolvedValueOnce(withCustodianB)
    mocks.api.getCapture.mockResolvedValueOnce({
      captureId: '11111111-1111-1111-1111-111111111111',
      action: 'TRANSFER',
      animalId: withCustodianB.animalId,
      status: 'EVIDENCE_ACCEPTED',
      eventHash: pendingEventHash,
      eventStatus: 'SUBMITTED',
      txSignature: transactionSignature,
    })
    mocks.api.confirm.mockResolvedValueOnce(finalized)

    const wrapper = mountPage()
    await flushPromises()

    expect(mocks.submit).not.toHaveBeenCalled()
    expect(mocks.api.getTransactionData).not.toHaveBeenCalled()
    expect(mocks.api.submit).not.toHaveBeenCalled()
    expect(mocks.api.confirm).toHaveBeenCalledWith(pendingEventHash, transactionSignature)
    expect(window.localStorage.getItem('lastro.pending-operation')).toBeNull()
    expect(wrapper.text()).toContain('finalized and canonical state verified after reload')
    wrapper.unmount()
  })

  /**
   * PURPOSE: Retry an ambiguously sent transaction using the exact previously signed bytes.
   * ARRANGE: localStorage contains the signature and signed wire; the first API verification says the
   *          transaction is not confirmed yet, then succeeds after an exact rebroadcast.
   * ACTION: Reload the pending operation.
   * ASSERT: The same signature/wire is rebroadcast once and no wallet signing or new transaction data is requested.
   * FAILURE MEANS: recovery can create a second transaction instead of replaying the immutable signed envelope.
   */
  it('reload rebroadcasts the exact remembered signed transaction when confirmation is initially absent', async () => {
    const pendingEventHash = '88'.repeat(32)
    const transactionSignature = '9'.repeat(88)
    const wireTransactionBase64 = btoa('exact-signed-wire')
    const finalized = { ...withCustodianB, eventSequence: 3, lastEventHash: pendingEventHash }
    const { ApiClientError } = await import('../../src/api/client')

    window.history.replaceState(null, '', `/demo?animalId=${withCustodianB.animalId}`)
    window.localStorage.setItem(
      'lastro.pending-operation',
      JSON.stringify({
        animalId: withCustodianB.animalId,
        captureId: '55555555-5555-5555-5555-555555555555',
        action: 'TRANSFER',
        nextCustodian: CUSTODIAN_C,
        expectedToCustodian: CUSTODIAN_C,
        eventHash: pendingEventHash,
        txSignature: transactionSignature,
        wireTransactionBase64,
      }),
    )
    mocks.api.getAnimal.mockResolvedValueOnce(withCustodianB)
    mocks.api.getCapture.mockResolvedValueOnce({
      captureId: '55555555-5555-5555-5555-555555555555',
      action: 'TRANSFER',
      animalId: withCustodianB.animalId,
      status: 'EVIDENCE_ACCEPTED',
      eventHash: pendingEventHash,
      eventStatus: 'EVIDENCE_ACCEPTED',
      txSignature: null,
    })
    mocks.api.submit
      .mockRejectedValueOnce(
        new ApiClientError(
          'pending',
          409,
          JSON.stringify({
            message: 'transaction is not a confirmed exact Lastro transaction for this event',
          }),
        ),
      )
      .mockResolvedValueOnce({ status: 'SUBMITTED', txSignature: transactionSignature })
    mocks.rebroadcast.mockResolvedValueOnce(transactionSignature)
    mocks.api.confirm.mockResolvedValueOnce(finalized)

    const wrapper = mountPage()
    await flushPromises()

    expect(mocks.rebroadcast).toHaveBeenCalledWith({
      signature: transactionSignature,
      wireTransactionBase64,
    })
    expect(mocks.submit).not.toHaveBeenCalled()
    expect(mocks.api.getTransactionData).not.toHaveBeenCalled()
    expect(mocks.api.submit).toHaveBeenCalledTimes(2)
    expect(mocks.api.confirm).toHaveBeenCalledWith(pendingEventHash, transactionSignature)
    wrapper.unmount()
  })

  /**
   * ARRANGE: Restore an accepted capture with an expired blockhash, absent signature history and retained wire.
   * ACTION: Ask the API to verify the transaction and check finalized height/signature status.
   * ASSERT: Only the signed envelope is cleared; the capture and signed physical event remain.
   * FAILURE MEANS: expiration can strand evidence or trigger an unsafe second physical read.
   */
  it('retains physical evidence while clearing only a provably expired signed envelope', async () => {
    const eventHash = '88'.repeat(32)
    const txSignature = '9'.repeat(88)
    const { ApiClientError } = await import('../../src/api/client')
    window.history.replaceState(null, '', `/demo?animalId=${withCustodianB.animalId}`)
    window.localStorage.setItem(
      'lastro.pending-operation',
      JSON.stringify({
        animalId: withCustodianB.animalId,
        captureId: '55555555-5555-5555-5555-555555555555',
        action: 'TRANSFER',
        nextCustodian: CUSTODIAN_C,
        expectedToCustodian: CUSTODIAN_C,
        eventHash,
        txSignature,
        wireTransactionBase64: btoa('signed-wire'),
        lastValidBlockHeight: '100',
      }),
    )
    mocks.api.getAnimal.mockResolvedValueOnce(withCustodianB)
    mocks.api.getCapture.mockResolvedValueOnce({
      captureId: '55555555-5555-5555-5555-555555555555',
      action: 'TRANSFER',
      animalId: withCustodianB.animalId,
      status: 'EVIDENCE_ACCEPTED',
      eventHash,
      eventStatus: 'EVIDENCE_ACCEPTED',
      txSignature: null,
    })
    mocks.api.submit.mockRejectedValueOnce(
      new ApiClientError(
        'pending',
        409,
        JSON.stringify({
          message: 'transaction is not a confirmed exact Lastro transaction for this event',
        }),
      ),
    )
    mocks.getBlockHeight.mockReturnValue({ send: async () => 101n })

    const wrapper = mountPage()
    await flushPromises()

    const stored = JSON.parse(window.localStorage.getItem('lastro.pending-operation')!)
    expect(stored.captureId).toBe('55555555-5555-5555-5555-555555555555')
    expect(stored.eventHash).toBe(eventHash)
    expect(stored.txSignature).toBeNull()
    expect(stored.wireTransactionBase64).toBeNull()
    expect(mocks.getSignatureStatuses).toHaveBeenCalledWith([txSignature], {
      searchTransactionHistory: true,
    })
    expect(mocks.rebroadcast).not.toHaveBeenCalled()
    wrapper.unmount()
  })

  /**
   * ARRANGE: Restore an accepted REIDENTIFY capture that has not been signed on Solana.
   * ACTION: Click the explicit Retry RFID scan control with the current custodian connected.
   * ASSERT: The prior capture ID is bound to both the wallet challenge and reservation request.
   * FAILURE MEANS: the page can silently reuse the wrong tag or replace unrelated evidence.
   */
  it('requests an explicitly signed same-action RFID rescan', async () => {
    const captureId = '55555555-5555-5555-5555-555555555555'
    const accepted = {
      captureId,
      action: 'REIDENTIFY',
      animalId: withCustodianB.animalId,
      status: 'EVIDENCE_ACCEPTED',
      eventHash: '44'.repeat(32),
      eventStatus: 'EVIDENCE_ACCEPTED',
      txSignature: null,
    }
    mocks.walletAddress = WALLET_B
    window.history.replaceState(null, '', `/demo?animalId=${withCustodianB.animalId}`)
    window.localStorage.setItem(
      'lastro.pending-operation',
      JSON.stringify({
        animalId: withCustodianB.animalId,
        captureId,
        action: 'REIDENTIFY',
        nextCustodian: null,
        expectedToCustodian: CUSTODIAN_B,
        eventHash: accepted.eventHash,
        txSignature: null,
        wireTransactionBase64: null,
      }),
    )
    mocks.api.getAnimal.mockResolvedValueOnce(withCustodianB)
    mocks.api.getCapture.mockResolvedValueOnce(accepted).mockResolvedValueOnce(accepted)
    mocks.api.createCapture.mockRejectedValueOnce(new Error('stopped before dispatch'))

    const wrapper = mountPage()
    await flushPromises()
    await button(wrapper, 'Retry RFID scan').trigger('click')
    await flushPromises()

    expect(mocks.api.getCaptureAuthorizationChallenge).toHaveBeenCalledWith(
      'REIDENTIFY',
      withCustodianB.animalId,
      null,
      captureId,
    )
    expect(mocks.signCaptureAuthorization).toHaveBeenCalledWith(
      expect.anything(),
      expect.objectContaining({ supersedeCaptureId: captureId }),
    )
    expect(mocks.api.createCapture).toHaveBeenCalledWith(
      'REIDENTIFY',
      withCustodianB.animalId,
      null,
      expect.anything(),
      captureId,
    )
    wrapper.unmount()
  })

  /**
   * PURPOSE: Recover the narrower crash window after wallet broadcast but before the API has persisted SUBMITTED.
   * ARRANGE: localStorage has the broadcast signature while durable capture state still reports only EVIDENCE_ACCEPTED.
   * ACTION: Reload the page and let it re-register that same signature through confirmed RPC verification.
   * ASSERT: api.submit and confirm use the cached signature, while transaction preparation and wallet signing are never repeated.
   * FAILURE MEANS: a reload in the confirmed-wait window can create a second wallet transaction for the same immutable StationEvent.
   */
  it('reload re-verifies a locally remembered broadcast signature without signing a second transaction', async () => {
    const pendingEventHash = '55'.repeat(32)
    const transactionSignature = '6'.repeat(88)
    const finalized = { ...withCustodianB, eventSequence: 3, lastEventHash: pendingEventHash }
    window.history.replaceState(null, '', `/demo?animalId=${withCustodianB.animalId}`)
    window.localStorage.setItem(
      'lastro.pending-operation',
      JSON.stringify({
        animalId: withCustodianB.animalId,
        captureId: '22222222-2222-2222-2222-222222222222',
        action: 'TRANSFER',
        nextCustodian: CUSTODIAN_C,
        expectedToCustodian: CUSTODIAN_C,
        eventHash: pendingEventHash,
        txSignature: transactionSignature,
      }),
    )
    mocks.api.getAnimal.mockResolvedValueOnce(withCustodianB)
    mocks.api.getCapture.mockResolvedValueOnce({
      captureId: '22222222-2222-2222-2222-222222222222',
      action: 'TRANSFER',
      animalId: withCustodianB.animalId,
      status: 'EVIDENCE_ACCEPTED',
      eventHash: pendingEventHash,
      eventStatus: 'EVIDENCE_ACCEPTED',
      txSignature: null,
    })
    mocks.api.submit.mockResolvedValueOnce({
      status: 'SUBMITTED',
      txSignature: transactionSignature,
    })
    mocks.api.confirm.mockResolvedValueOnce(finalized)

    const wrapper = mountPage()
    await flushPromises()

    expect(mocks.submit).not.toHaveBeenCalled()
    expect(mocks.api.getTransactionData).not.toHaveBeenCalled()
    expect(mocks.api.submit).toHaveBeenCalledWith(pendingEventHash, transactionSignature)
    expect(mocks.api.confirm).toHaveBeenCalledWith(pendingEventHash, transactionSignature)
    expect(window.localStorage.getItem('lastro.pending-operation')).toBeNull()
    wrapper.unmount()
  })

  /**
   * PURPOSE: Pending-operation recovery must not depend on exporting an all-finalized evidence package.
   * ARRANGE: the URL and localStorage identify a SUBMITTED transfer while the projection still reflects
   *          the last finalized state.
   * ACTION: mount the page and resume the durable capture.
   * ASSERT: capture recovery begins before the evidence-package timeline is loaded.
   * FAILURE MEANS: the backend's correct 409 for a non-finalized history can prevent reload recovery.
   */
  it('recovers a pending transfer before requesting the finalized evidence timeline', async () => {
    const pendingEventHash = '77'.repeat(32)
    const transactionSignature = '7'.repeat(88)
    const finalized = {
      ...withCustodianB,
      currentCustodian: CUSTODIAN_C,
      eventSequence: 3,
      lastEventHash: pendingEventHash,
    }
    window.history.replaceState(null, '', `/demo?animalId=${withCustodianB.animalId}`)
    window.localStorage.setItem(
      'lastro.pending-operation',
      JSON.stringify({
        animalId: withCustodianB.animalId,
        captureId: '33333333-3333-3333-3333-333333333333',
        action: 'TRANSFER',
        nextCustodian: CUSTODIAN_C,
        expectedToCustodian: CUSTODIAN_C,
        eventHash: pendingEventHash,
        txSignature: transactionSignature,
      }),
    )
    mocks.api.getAnimal.mockResolvedValueOnce(withCustodianB)
    mocks.api.getCapture.mockResolvedValueOnce({
      captureId: '33333333-3333-3333-3333-333333333333',
      action: 'TRANSFER',
      animalId: withCustodianB.animalId,
      status: 'EVIDENCE_ACCEPTED',
      eventHash: pendingEventHash,
      eventStatus: 'SUBMITTED',
      txSignature: transactionSignature,
    })
    mocks.api.confirm.mockResolvedValueOnce(finalized)
    mocks.api.getEvidencePackage.mockResolvedValueOnce(twoEventPackage)

    const wrapper = mountPage()
    await flushPromises()

    expect(mocks.api.getCapture).toHaveBeenCalledWith('33333333-3333-3333-3333-333333333333')
    expect(mocks.api.getEvidencePackage).toHaveBeenCalled()
    expect(mocks.api.getCapture.mock.invocationCallOrder[0]!).toBeLessThan(
      mocks.api.getEvidencePackage.mock.invocationCallOrder[0]!,
    )
    expect(mocks.api.confirm).toHaveBeenCalledWith(pendingEventHash, transactionSignature)
    wrapper.unmount()
  })
})

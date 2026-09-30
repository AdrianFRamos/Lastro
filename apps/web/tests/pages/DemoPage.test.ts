import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { AssetProjection, Capture } from '../../src/api/types'
import validFixture from '../../../../test-vectors/v2-evidence-package.valid.json'
import DemoPage from '../../src/pages/DemoPage.vue'

const WALLET_A = 'wallet-a'
const WALLET_B = 'wallet-b'
const CUSTODIAN_A = 'aa'.repeat(32)
const CUSTODIAN_B = 'bb'.repeat(32)
const ASSET_ID = '11'.repeat(32)
const EVENT_HASH = '44'.repeat(32)
const CAPTURE_ID = '00112233-4455-6677-8899-aabbccddeeff'
const TX = '2AXDGYSE4f2sz7tvMMzyHvUfcoJmxudvdhBcmiUSo6ijwfYmfZYsKRxboQMPh3R4kUhXRVdtSXFXMheka4Rc4P2'

const mocks = vi.hoisted(() => ({
  walletAddress: null as string | null,
  api: {
    getRegisterAssetTransaction: vi.fn(),
    getAsset: vi.fn(),
    getAssetByRfid: vi.fn(),
    getEvidencePackage: vi.fn(),
    getCaptureAuthorizationChallenge: vi.fn(),
    createCapture: vi.fn(),
    getCapture: vi.fn(),
    getTransactionData: vi.fn(),
    submit: vi.fn(),
    confirm: vi.fn(),
    createCustodyTransfer: vi.fn(),
    getCustodyTransferTransaction: vi.fn(),
    acceptCustodyTransfer: vi.fn(),
  },
  connect: vi.fn(),
  signCaptureAuthorization: vi.fn(),
  submit: vi.fn(),
  rebroadcast: vi.fn(),
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
  availableWalletChoices: () => [],
  connectFirstAvailableWallet: mocks.connect,
  connectWalletByName: vi.fn(),
  currentWalletAddress: () => mocks.walletAddress,
  signCaptureAuthorization: mocks.signCaptureAuthorization,
  walletAddressToCustodianHex: (address: string) => {
    if (address === WALLET_A) return CUSTODIAN_A
    if (address === WALLET_B) return CUSTODIAN_B
    throw new Error('invalid test wallet')
  },
}))

vi.mock('../../src/solana/transaction', () => ({
  submitLastroTransaction: mocks.submit,
  rebroadcastSignedLastroTransaction: mocks.rebroadcast,
  custodianAddress: (hex: string) => `address-of-${hex.slice(0, 4)}`,
}))

vi.mock('../../src/solana/client', () => ({
  solanaClient: {
    rpc: {
      getBlockHeight: () => ({ send: async () => 1n }),
      getSignatureStatuses: () => ({ send: async () => ({ value: [null] }) }),
    },
  },
}))

const untagged: AssetProjection = {
  assetId: ASSET_ID,
  assetType: 1,
  status: 1,
  custodian: CUSTODIAN_A,
  stateVersion: 0,
  eventSequence: 0,
  lastEventHash: '00'.repeat(32),
  currentRfidHash: null,
  availableWeightGrams: 450000,
}
const tagged: AssetProjection = {
  ...untagged,
  stateVersion: 1,
  eventSequence: 1,
  currentRfidHash: '22'.repeat(32),
}

function capture(overrides: Partial<Capture> = {}): Capture {
  return {
    captureId: CAPTURE_ID,
    action: 'BIND_IDENTIFIER',
    assetId: ASSET_ID,
    stateVersion: 1,
    status: 'EVIDENCE_ACCEPTED',
    eventHash: EVENT_HASH,
    eventStatus: 'EVIDENCE_ACCEPTED',
    txSignature: null,
    ...overrides,
  }
}

function mountPage(): VueWrapper {
  return mount(DemoPage, {
    global: { stubs: { RouterLink: { template: '<a><slot /></a>' } } },
  })
}

function button(wrapper: VueWrapper, label: string) {
  const match = wrapper.findAll('button').find((candidate) => candidate.text() === label)
  if (!match) throw new Error(`button not found: ${label}`)
  return match
}

async function load(wrapper: VueWrapper, asset: AssetProjection): Promise<void> {
  mocks.api.getAsset.mockResolvedValueOnce(asset)
  await wrapper.get('input[aria-label="AssetID or RFID hash"]').setValue(asset.assetId)
  await button(wrapper, 'Load by AssetID').trigger('click')
  await flushPromises()
}

beforeEach(() => {
  window.history.replaceState(null, '', '/demo')
  window.localStorage.clear()
  mocks.walletAddress = null
  for (const fn of Object.values(mocks.api)) fn.mockReset()
  mocks.api.getEvidencePackage.mockResolvedValue(structuredClone(validFixture))
  mocks.api.getCaptureAuthorizationChallenge.mockResolvedValue({
    challengeId: 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee',
    deploymentId: 'd0'.repeat(32),
    requiredSigner: WALLET_A,
    messageBase64: btoa('test capture authorization'),
    expiresAtUnix: 2_000_000_000,
  })
  mocks.signCaptureAuthorization.mockReset()
  mocks.signCaptureAuthorization.mockResolvedValue({
    challengeId: 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee',
    signatureBase64: btoa(String.fromCharCode(...new Uint8Array(64).fill(7))),
  })
  mocks.submit.mockReset()
  mocks.rebroadcast.mockReset()
  mocks.connect.mockReset()
})

describe('pages/DemoPage state machine', () => {
  /**
   * ARRANGE: an untagged asset and the custodian wallet; then a different wallet.
   * ACTION: render the action cards.
   * ASSERT: only BIND is available to the custodian; nothing identity-changing for others.
   * FAILURE MEANS: the console could ask a wallet for signatures the chain must reject.
   */
  it('offers BIND only to the custodian of an untagged asset', async () => {
    mocks.walletAddress = WALLET_A
    const wrapper = mountPage()
    await load(wrapper, untagged)
    expect(button(wrapper, 'Bind RFID').attributes('disabled')).toBeUndefined()
    expect(button(wrapper, 'Replace RFID').attributes('disabled')).toBeDefined()
    expect(button(wrapper, 'Record presence').attributes('disabled')).toBeDefined()

    mocks.walletAddress = WALLET_B
    mocks.connect.mockResolvedValueOnce(undefined)
    await button(wrapper, 'Connect wallet').trigger('click')
    await flushPromises()
    expect(button(wrapper, 'Bind RFID').attributes('disabled')).toBeDefined()
    expect(wrapper.text()).toContain('Connect the current custodian wallet.')
    wrapper.unmount()
  })

  /**
   * ARRANGE: a tagged asset and its custodian.
   * ACTION: render the action cards.
   * ASSERT: REPLACE and presence proof are available; BIND is not.
   * FAILURE MEANS: an animal could get a second active tag.
   */
  it('offers REPLACE and presence proof once the asset is tagged', async () => {
    mocks.walletAddress = WALLET_A
    const wrapper = mountPage()
    await load(wrapper, tagged)
    expect(button(wrapper, 'Bind RFID').attributes('disabled')).toBeDefined()
    expect(button(wrapper, 'Replace RFID').attributes('disabled')).toBeUndefined()
    expect(button(wrapper, 'Record presence').attributes('disabled')).toBeUndefined()
    wrapper.unmount()
  })

  /**
   * ARRANGE: the custodian starts a BIND; capture creation is the observed boundary.
   * ACTION: click Bind RFID.
   * ASSERT: challenge → wallet signature (bound to the next state version) → capture, in order.
   * FAILURE MEANS: Station work could be reserved before proving wallet control.
   */
  it('authorizes the exact capture intent before reserving Station work', async () => {
    mocks.walletAddress = WALLET_A
    mocks.api.createCapture.mockRejectedValueOnce(new Error('reservation boundary reached'))
    const wrapper = mountPage()
    await load(wrapper, untagged)
    await button(wrapper, 'Bind RFID').trigger('click')
    await flushPromises()

    expect(mocks.api.getCaptureAuthorizationChallenge).toHaveBeenCalledWith(
      'BIND_IDENTIFIER',
      ASSET_ID,
    )
    expect(mocks.signCaptureAuthorization).toHaveBeenCalledWith(expect.anything(), {
      action: 'BIND_IDENTIFIER',
      assetId: ASSET_ID,
      stateVersion: 1,
    })
    const order = [
      mocks.api.getCaptureAuthorizationChallenge,
      mocks.signCaptureAuthorization,
      mocks.api.createCapture,
    ].map((fn) => fn.mock.invocationCallOrder[0]!)
    expect(order).toEqual([...order].sort((a, b) => a - b))
    expect(wrapper.text()).toContain('reservation boundary reached')
    wrapper.unmount()
  })

  /**
   * ARRANGE: accepted Station evidence; the wallet rejects signing.
   * ACTION: run BIND.
   * ASSERT: submit/confirm are never called and the asset stays untagged.
   * FAILURE MEANS: a rejected signature could be displayed as a completed binding.
   */
  it('wallet signature rejection leaves canonical state unchanged', async () => {
    mocks.walletAddress = WALLET_A
    mocks.api.createCapture.mockResolvedValueOnce(capture())
    mocks.api.getTransactionData.mockResolvedValueOnce({})
    mocks.submit.mockRejectedValueOnce(new Error('User rejected the request'))
    const wrapper = mountPage()
    await load(wrapper, untagged)
    await button(wrapper, 'Bind RFID').trigger('click')
    await flushPromises()

    expect(mocks.api.submit).not.toHaveBeenCalled()
    expect(mocks.api.confirm).not.toHaveBeenCalled()
    expect(wrapper.text()).toContain('User rejected the request')
    expect(wrapper.text()).toContain('No RFID bound')
    wrapper.unmount()
  })

  /**
   * ARRANGE: accepted evidence, a signed transaction, submitted and finalized API responses.
   * ACTION: run BIND end to end.
   * ASSERT: the wallet validates the exact STATION_EVENT expectation, the signed bytes are kept
   *         before broadcast, and the asset is re-read from the chain only after FINALIZED.
   * FAILURE MEANS: the demo could skip browser-side validation or show state before finality.
   */
  it('runs a BIND capture to finalized canonical state', async () => {
    mocks.walletAddress = WALLET_A
    mocks.api.createCapture.mockResolvedValueOnce(capture())
    mocks.api.getTransactionData.mockResolvedValueOnce({ requiredSigner: WALLET_A })
    mocks.submit.mockImplementationOnce(async (_data, _expected, onSigned) => {
      onSigned({ signature: TX, wireTransactionBase64: 'AQID', lastValidBlockHeight: 99n })
      expect(JSON.parse(window.localStorage.getItem('lastro.pending-operation')!)).toMatchObject({
        txSignature: TX,
        wireTransactionBase64: 'AQID',
      })
      return TX
    })
    mocks.api.submit.mockResolvedValueOnce({
      eventHash: EVENT_HASH,
      status: 'SUBMITTED',
      txSignature: TX,
    })
    mocks.api.confirm.mockResolvedValueOnce({
      eventHash: EVENT_HASH,
      status: 'FINALIZED',
      txSignature: TX,
    })
    const wrapper = mountPage()
    await load(wrapper, untagged)
    mocks.api.getAsset.mockResolvedValueOnce(tagged)
    await button(wrapper, 'Bind RFID').trigger('click')
    await flushPromises()

    expect(mocks.submit).toHaveBeenCalledWith(
      { requiredSigner: WALLET_A },
      {
        kind: 'STATION_EVENT',
        action: 'BIND_IDENTIFIER',
        assetId: ASSET_ID,
        eventHash: EVENT_HASH,
        requiredSigner: WALLET_A,
      },
      expect.any(Function),
    )
    expect(mocks.api.confirm).toHaveBeenCalledWith(EVENT_HASH, TX)
    expect(wrapper.text()).toContain('RFID binding finalized and canonical state verified.')
    expect(wrapper.text()).toContain(tagged.currentRfidHash!)
    expect(window.localStorage.getItem('lastro.pending-operation')).toBeNull()
    wrapper.unmount()
  })

  /**
   * ARRANGE: a reload with a remembered, already SUBMITTED capture.
   * ACTION: mount the page with ?assetId=.
   * ASSERT: it confirms the same signature without signing again.
   * FAILURE MEANS: a reload could ask for a second wallet signature for the same evidence.
   */
  it('reload resumes a durable SUBMITTED transaction without signing again', async () => {
    window.history.replaceState(null, '', `/demo?assetId=${ASSET_ID}`)
    window.localStorage.setItem(
      'lastro.pending-operation',
      JSON.stringify({
        assetId: ASSET_ID,
        captureId: CAPTURE_ID,
        action: 'BIND_IDENTIFIER',
        eventHash: EVENT_HASH,
        txSignature: TX,
        wireTransactionBase64: null,
        lastValidBlockHeight: null,
      }),
    )
    mocks.api.getAsset.mockResolvedValueOnce(untagged).mockResolvedValueOnce(tagged)
    mocks.api.getCapture.mockResolvedValueOnce(
      capture({ eventStatus: 'SUBMITTED', txSignature: TX }),
    )
    mocks.api.confirm.mockResolvedValueOnce({
      eventHash: EVENT_HASH,
      status: 'FINALIZED',
      txSignature: TX,
    })
    const wrapper = mountPage()
    await flushPromises()
    await flushPromises()

    expect(mocks.submit).not.toHaveBeenCalled()
    expect(mocks.api.confirm).toHaveBeenCalledWith(EVENT_HASH, TX)
    expect(wrapper.text()).toContain('RFID binding finalized')
    wrapper.unmount()
  })

  /**
   * ARRANGE: the authority wallet registers an animal; the asset appears after one 404.
   * ACTION: click Register animal.
   * ASSERT: the wallet validates a REGISTER_ASSET expectation for the requested custodian and the
   *         page waits for the finalized AssetState.
   * FAILURE MEANS: the demo could show an asset that the chain does not have.
   */
  it('registers an animal and waits for its finalized AssetState', async () => {
    vi.useFakeTimers()
    const { ApiClientError } = await import('../../src/api/client')
    mocks.walletAddress = WALLET_A
    mocks.api.getRegisterAssetTransaction.mockResolvedValueOnce({})
    mocks.submit.mockResolvedValueOnce(TX)
    mocks.api.getAsset
      .mockRejectedValueOnce(new ApiClientError('not found', 404, '{}'))
      .mockImplementationOnce(async (id: string) => ({ ...untagged, assetId: id }))
    const wrapper = mountPage()
    await wrapper.get('#custodian-wallet').setValue(WALLET_B)
    await button(wrapper, 'Register animal').trigger('click')
    await vi.runAllTimersAsync()
    await flushPromises()
    vi.useRealTimers()

    const [, expected] = mocks.submit.mock.calls[0]!
    expect(expected).toMatchObject({ kind: 'REGISTER_ASSET', custodian: CUSTODIAN_B, assetType: 1 })
    expect(expected.assetId).toMatch(/^[0-9a-f]{64}$/)
    expect(wrapper.text()).toContain('registered on Solana')
    wrapper.unmount()
  })

  /**
   * ARRANGE: an RFID lookup that resolves to a RETIRED binding.
   * ACTION: look it up.
   * ASSERT: the animal loads and the message says the tag is retired.
   * FAILURE MEANS: a replaced tag found in the field would no longer lead to its animal.
   */
  it('resolves a retired RFID to its animal', async () => {
    mocks.api.getAssetByRfid.mockResolvedValueOnce({
      rfidHash: '99'.repeat(32),
      bindingStatus: 'RETIRED',
      asset: tagged,
    })
    const wrapper = mountPage()
    await wrapper.get('input[aria-label="AssetID or RFID hash"]').setValue('99'.repeat(32))
    await button(wrapper, 'Find by RFID hash').trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('RETIRED tag')
    expect(wrapper.text()).toContain(ASSET_ID)
    wrapper.unmount()
  })

  /**
   * ARRANGE: the custodian proposes a transfer; the recipient wallet accepts it.
   * ACTION: run both phases.
   * ASSERT: each phase is validated as its own custody expectation and the custodian changes
   *         only after the API confirms finalized acceptance.
   * FAILURE MEANS: custody could move without the recipient's on-chain acceptance.
   */
  it('runs a two-phase custody transfer', async () => {
    mocks.walletAddress = WALLET_A
    const wrapper = mountPage()
    await load(wrapper, tagged)
    await wrapper.get('#operator-token').setValue('operator-secret')
    await wrapper.get('#recipient-party').setValue('55'.repeat(32))
    await wrapper.get('#recipient-facility').setValue('66'.repeat(32))
    await wrapper.get('#recipient-wallet').setValue(WALLET_B)
    mocks.api.createCustodyTransfer.mockResolvedValueOnce({})
    mocks.api.getCustodyTransferTransaction.mockResolvedValue({})
    mocks.submit.mockResolvedValue(TX)
    await button(wrapper, 'Propose (current custodian)').trigger('click')
    await flushPromises()
    const [, proposal] = mocks.submit.mock.calls[0]!
    expect(proposal).toMatchObject({
      kind: 'CUSTODY_PROPOSE',
      assetId: ASSET_ID,
      recipient: CUSTODIAN_B,
    })

    mocks.walletAddress = WALLET_B
    mocks.connect.mockResolvedValueOnce(undefined)
    await button(wrapper, 'Connect wallet').trigger('click')
    await flushPromises()
    mocks.api.acceptCustodyTransfer.mockResolvedValueOnce({})
    mocks.api.getAsset.mockResolvedValueOnce({ ...tagged, custodian: CUSTODIAN_B })
    await button(wrapper, 'Accept (recipient wallet)').trigger('click')
    await flushPromises()
    const [, acceptance] = mocks.submit.mock.calls[1]!
    expect(acceptance).toMatchObject({
      kind: 'CUSTODY_ACCEPT',
      transferId: proposal.transferId,
      recipient: CUSTODIAN_B,
    })
    expect(mocks.api.acceptCustodyTransfer).toHaveBeenCalledWith(proposal.transferId, TX)
    expect(wrapper.text()).toContain('Custody transfer finalized')
    wrapper.unmount()
  })

  /**
   * ARRANGE: a proposal made elsewhere; the recipient opens the console on their own device.
   * ACTION: load the asset, paste the shared transfer ID and accept with the connected wallet.
   * ASSERT: the acceptance is validated for the connected wallet and that transfer ID.
   * FAILURE MEANS: a recipient on another device could never accept custody.
   */
  it('lets a recipient on another device accept a shared transfer ID', async () => {
    const transferId = '00000000-0000-4000-8000-000000000042'
    mocks.walletAddress = WALLET_B
    const wrapper = mountPage()
    await load(wrapper, tagged)
    await wrapper.get('#transfer-id').setValue(transferId)
    mocks.api.getCustodyTransferTransaction.mockResolvedValueOnce({})
    mocks.submit.mockResolvedValueOnce(TX)
    mocks.api.acceptCustodyTransfer.mockResolvedValueOnce({})
    mocks.api.getAsset.mockResolvedValueOnce({ ...tagged, custodian: CUSTODIAN_B })
    await button(wrapper, 'Accept (recipient wallet)').trigger('click')
    await flushPromises()
    expect(mocks.submit.mock.calls[0]![1]).toMatchObject({
      kind: 'CUSTODY_ACCEPT',
      transferId,
      recipient: CUSTODIAN_B,
    })
    expect(wrapper.text()).toContain('Custody transfer finalized')
    wrapper.unmount()
  })

  /**
   * ARRANGE: a wallet that is not the custodian.
   * ACTION: run the explicit stale-authority check.
   * ASSERT: rejected with no capture, evidence or transaction.
   * FAILURE MEANS: the demo could imply a previous custodian still has authority.
   */
  it('exposes the stale-custodian check without creating work', async () => {
    mocks.walletAddress = WALLET_B
    const wrapper = mountPage()
    await load(wrapper, tagged)
    await button(wrapper, 'Run stale-custodian attempt').trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('REJECTED')
    expect(mocks.api.createCapture).not.toHaveBeenCalled()
    wrapper.unmount()
  })
})

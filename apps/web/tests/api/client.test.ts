import { afterEach, describe, expect, it, vi } from 'vitest'
import validFixture from '../../../../test-vectors/v2-evidence-package.valid.json'
import { api, ApiClientError } from '../../src/api/client'

const CAPTURE_ID = '00112233-4455-6677-8899-aabbccddeeff'
const TX_SIGNATURE =
  '2AXDGYSE4f2sz7tvMMzyHvUfcoJmxudvdhBcmiUSo6ijwfYmfZYsKRxboQMPh3R4kUhXRVdtSXFXMheka4Rc4P2'
const ASSET_ID = '11'.repeat(32)

const asset = {
  assetId: ASSET_ID,
  assetType: 1,
  status: 1,
  custodian: 'c1'.repeat(32),
  stateVersion: 0,
  eventSequence: 0,
  lastEventHash: '00'.repeat(32),
  currentRfidHash: null,
  availableWeightGrams: 450000,
}

function capture(overrides: Record<string, unknown> = {}) {
  return {
    captureId: CAPTURE_ID,
    action: 'BIND_IDENTIFIER',
    assetId: ASSET_ID,
    stateVersion: 1,
    status: 'PENDING',
    eventHash: null,
    eventStatus: null,
    txSignature: null,
    ...overrides,
  }
}

function jsonResponse(value: unknown, status = 200): Response {
  return new Response(JSON.stringify(value), {
    status,
    headers: { 'content-type': 'application/json' },
  })
}

afterEach(() => {
  vi.restoreAllMocks()
  vi.useRealTimers()
})

describe('api/client', () => {
  /**
   * ARRANGE: canonical AssetState and RFID lookup responses.
   * ACTION: read them through the typed client.
   * ASSERT: parsed exactly; a RETIRED tag still resolves to its asset.
   * FAILURE MEANS: the console could show a projection that differs from the chain.
   */
  it('reads canonical assets and resolves active or retired RFIDs', async () => {
    vi.spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(jsonResponse(asset))
      .mockResolvedValueOnce(
        jsonResponse({ rfidHash: '22'.repeat(32), bindingStatus: 'RETIRED', asset }),
      )
    await expect(api.getAsset(ASSET_ID)).resolves.toEqual(asset)
    await expect(api.getAssetByRfid('22'.repeat(32))).resolves.toMatchObject({
      bindingStatus: 'RETIRED',
      asset,
    })
  })

  /**
   * ARRANGE: representative 4xx/409/5xx API responses with explicit bodies.
   * ACTION: call a typed API operation for each status.
   * ASSERT: every non-success rejects with ApiClientError preserving status and body.
   * FAILURE MEANS: transport failure could be mistaken for canonical state.
   */
  it('maps every non-success API response to an explicit typed client error', async () => {
    for (const status of [400, 409, 500]) {
      const body = JSON.stringify({ message: `failure-${status}` })
      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(new Response(body, { status }))
      const error = await api.getAsset(ASSET_ID).catch((value: unknown) => value)
      expect(error).toBeInstanceOf(ApiClientError)
      expect(error).toMatchObject({ status, responseBody: body })
    }
  })

  /**
   * ARRANGE: the committed v2 EvidencePackage fixture.
   * ACTION: fetch it through the client.
   * ASSERT: returned unchanged for the verifier.
   * FAILURE MEANS: the client could rewrite proof bytes before verification.
   */
  it('preserves evidence package payload exactly for the verifier layer', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(validFixture))
    await expect(api.getEvidencePackage(ASSET_ID)).resolves.toEqual(validFixture)
    expect(String(fetchMock.mock.calls[0]![0])).toContain(
      `/api/v2/assets/${ASSET_ID}/evidence-package`,
    )
  })

  /**
   * ARRANGE: a fetch that never resolves.
   * ACTION: call the client with fake timers past the deadline.
   * ASSERT: rejects with a timeout error.
   * FAILURE MEANS: a stalled API could leave the UI in an optimistic state.
   */
  it('network timeout never produces an optimistic success result', async () => {
    vi.useFakeTimers()
    vi.spyOn(globalThis, 'fetch').mockImplementation(
      (_input, init) =>
        new Promise((_resolve, reject) =>
          init?.signal?.addEventListener('abort', () => reject(new DOMException('aborted'))),
        ),
    )
    const pending = api.getAsset(ASSET_ID).catch((error: unknown) => error)
    await vi.advanceTimersByTimeAsync(10_001)
    await expect(pending).resolves.toMatchObject({ message: 'API request timed out' })
  })

  /**
   * ARRANGE: one challenge, then one capture after receiving its proof.
   * ACTION: request the challenge and submit the signed proof.
   * ASSERT: the challenge carries only action + asset; creation carries the exact proof.
   * FAILURE MEANS: the browser could reserve Station work before proving wallet control.
   */
  it('requests capture authorization before sending the signed one-time proof', async () => {
    const challenge = {
      challengeId: 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee',
      deploymentId: 'd0'.repeat(32),
      requiredSigner: 'Vote111111111111111111111111111111111111111',
      messageBase64: btoa('Lastro capture authorization v2'),
      expiresAtUnix: 2_000_000_000,
    }
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(jsonResponse(challenge, 201))
      .mockResolvedValueOnce(jsonResponse(capture(), 201))

    await expect(
      api.getCaptureAuthorizationChallenge('BIND_IDENTIFIER', ASSET_ID),
    ).resolves.toEqual(challenge)
    const proof = {
      challengeId: challenge.challengeId,
      signatureBase64: btoa(String.fromCharCode(...new Uint8Array(64).fill(7))),
    }
    await expect(api.createCapture('BIND_IDENTIFIER', ASSET_ID, proof)).resolves.toEqual(capture())

    expect(JSON.parse(String(fetchMock.mock.calls[0]![1]?.body))).toEqual({
      action: 'BIND_IDENTIFIER',
      assetId: ASSET_ID,
    })
    expect(JSON.parse(String(fetchMock.mock.calls[1]![1]?.body))).toEqual({
      action: 'BIND_IDENTIFIER',
      assetId: ASSET_ID,
      authorization: proof,
    })
  })

  /**
   * ARRANGE: capture responses with inconsistent lifecycle metadata.
   * ACTION: parse them.
   * ASSERT: rejected; a consistent SUBMITTED capture is accepted.
   * FAILURE MEANS: reload recovery could mistake accepted evidence for a submitted transaction.
   */
  it('rejects inconsistent capture event lifecycle metadata', async () => {
    const submitted = capture({
      status: 'EVIDENCE_ACCEPTED',
      eventHash: '22'.repeat(32),
      eventStatus: 'SUBMITTED',
      txSignature: TX_SIGNATURE,
    })
    vi.spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(jsonResponse(submitted))
      .mockResolvedValueOnce(jsonResponse({ ...submitted, txSignature: null }))
      .mockResolvedValueOnce(jsonResponse({ ...submitted, action: 'ORIGIN' }))
    await expect(api.getCapture(CAPTURE_ID)).resolves.toEqual(submitted)
    await expect(api.getCapture(CAPTURE_ID)).rejects.toBeInstanceOf(ApiClientError)
    await expect(api.getCapture(CAPTURE_ID)).rejects.toBeInstanceOf(ApiClientError)
  })

  /**
   * ARRANGE: a SUBMITTED v2 event anchor response.
   * ACTION: call api.submit.
   * ASSERT: POSTs only txSignature to the v2 endpoint and parses the lifecycle.
   * FAILURE MEANS: recovery could bypass the confirmed-commitment trust boundary.
   */
  it('submits only the transaction signature to the v2 confirmed-verification endpoint', async () => {
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(
        jsonResponse({ ...validFixture.events[0], status: 'SUBMITTED', txSignature: TX_SIGNATURE }),
      )
    await expect(api.submit('22'.repeat(32), TX_SIGNATURE)).resolves.toEqual({
      eventHash: validFixture.events[0]!.eventHash,
      status: 'SUBMITTED',
      txSignature: TX_SIGNATURE,
    })
    const [url, init] = fetchMock.mock.calls[0]!
    expect(String(url)).toContain(`/api/v2/events/${'22'.repeat(32)}/submit`)
    expect(JSON.parse(String(init?.body))).toEqual({ txSignature: TX_SIGNATURE })
  })

  /**
   * ARRANGE: malformed identifiers in responses.
   * ACTION: parse them.
   * ASSERT: rejected.
   * FAILURE MEANS: oversized or malformed durable identifiers reach wallet/verifier code.
   */
  it('rejects malformed or oversized durable identifiers', async () => {
    vi.spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(jsonResponse({ ...asset, assetId: 'zz' }))
      .mockResolvedValueOnce(jsonResponse(capture({ captureId: 'not-a-uuid' })))
    await expect(api.getAsset(ASSET_ID)).rejects.toBeInstanceOf(ApiClientError)
    await expect(api.getCapture(CAPTURE_ID)).rejects.toBeInstanceOf(ApiClientError)
  })
})

import { afterEach, describe, expect, it, vi } from 'vitest'
import validFixture from '../../../../test-vectors/evidence-package.valid.json'
import { api, ApiClientError } from '../../src/api/client'

const CAPTURE_ID = '00112233-4455-6677-8899-aabbccddeeff'
const TX_SIGNATURE = '2AXDGYSE4f2sz7tvMMzyHvUfcoJmxudvdhBcmiUSo6ijwfYmfZYsKRxboQMPh3R4kUhXRVdtSXFXMheka4Rc4P2'

const animal = {
  animalId: '11'.repeat(32),
  visualRecoveryId: 'VIS-001',
  currentRfidHash: null,
  currentCustodian: null,
  identityRevision: 0,
  eventSequence: 0,
  lastEventHash: null,
}

function jsonResponse(value: unknown, status = 200): Response {
  return new Response(JSON.stringify(value), { status, headers: { 'content-type': 'application/json' } })
}

afterEach(() => {
  vi.restoreAllMocks()
  vi.useRealTimers()
})

describe('api/client', () => {
  /**
   * ARRANGE: mock a successful POST /api/animals response and provide one visual recovery identifier.
   * ACTION: call createAnimal through the browser API client.
   * ASSERT: the JSON request body contains only visualRecoveryId and the response is strictly parsed.
   * FAILURE MEANS: browser scope could silently expand animal registration or send invented identity fields.
   */
  it('createAnimal sends only the visual recovery identifier required by hackathon scope', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(animal))
    await expect(api.createAnimal('VIS-001')).resolves.toEqual(animal)
    const [, init] = fetchMock.mock.calls[0]!
    expect(init?.method).toBe('POST')
    expect(JSON.parse(String(init?.body))).toEqual({ visualRecoveryId: 'VIS-001' })
  })

  /**
   * ARRANGE: return representative 4xx/409/5xx API responses with explicit bodies.
   * ACTION: call a typed API operation for each status.
   * ASSERT: every non-success rejects with ApiClientError preserving HTTP status and response body.
   * FAILURE MEANS: transport failure could be mistaken for successful canonical/projection state.
   */
  it('maps every non-success API response to an explicit typed client error', async () => {
    for (const status of [400, 409, 500]) {
      const body = JSON.stringify({ message: `failure-${status}` })
      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(new Response(body, { status }))
      const error = await api.getAnimal('11'.repeat(32)).catch((value: unknown) => value)
      expect(error).toBeInstanceOf(ApiClientError)
      expect(error).toMatchObject({ status, responseBody: body })
    }
  })

  /**
   * ARRANGE: return the committed EvidencePackage fixture from the evidence endpoint.
   * ACTION: fetch and parse it through the API client.
   * ASSERT: event order and every cryptographic transport field remain exactly unchanged.
   * FAILURE MEANS: the API client could rewrite proof bytes before independent verification.
   */
  it('preserves evidence package payload exactly for the verifier layer', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(validFixture))
    await expect(api.getEvidencePackage(validFixture.animalId)).resolves.toEqual(validFixture)
  })

  /**
   * ARRANGE: make fetch remain pending until the client AbortSignal fires at its timeout.
   * ACTION: advance fake time through the configured request timeout.
   * ASSERT: the operation rejects with ApiClientError and never resolves an optimistic value.
   * FAILURE MEANS: network timeout could be presented as a completed API action.
   */
  it('network timeout or abort never produces an optimistic success result', async () => {
    vi.useFakeTimers()
    vi.spyOn(globalThis, 'fetch').mockImplementation((_input, init) => new Promise((_resolve, reject) => {
      init?.signal?.addEventListener('abort', () => reject(init.signal?.reason ?? new DOMException('aborted', 'AbortError')), { once: true })
    }))
    const expectation = expect(api.getAnimal('11'.repeat(32))).rejects.toMatchObject({ name: 'ApiClientError', status: null, message: 'API request timed out' })
    await vi.advanceTimersByTimeAsync(10_000)
    await expectation
  })
  /**
   * PURPOSE: Preserve the separate physical-capture and event-submission lifecycles exposed by the API.
   * ARRANGE: Return one accepted capture whose exact transaction was already verified at confirmed commitment.
   * ACTION: Fetch the capture through the typed browser client.
   * ASSERT: eventStatus and txSignature are parsed without being conflated with capture status.
   * FAILURE MEANS: reload recovery cannot distinguish accepted evidence from a verified submitted transaction.
   */
  it('parses submitted transaction metadata separately from capture status', async () => {
    const capture = {
      captureId: CAPTURE_ID,
      action: 'TRANSFER',
      animalId: '11'.repeat(32),
      status: 'EVIDENCE_ACCEPTED',
      eventHash: '22'.repeat(32),
      eventStatus: 'SUBMITTED',
      txSignature: TX_SIGNATURE,
    }
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(capture))
    await expect(api.getCapture(CAPTURE_ID)).resolves.toEqual(capture)
  })

  /**
   * PURPOSE: Register a wallet broadcast with the API only through the confirmed-transaction verification endpoint.
   * ARRANGE: Mock a SUBMITTED response for a known event/signature pair.
   * ACTION: Call api.submit.
   * ASSERT: The client POSTs only txSignature and strictly parses the returned durable submission state.
   * FAILURE MEANS: browser recovery can bypass or misread the server-side confirmed-commitment trust boundary.
   */
  it('submits only the transaction signature to the confirmed-verification endpoint', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse({
      status: 'SUBMITTED',
      txSignature: TX_SIGNATURE,
    }))
    await expect(api.submit('22'.repeat(32), TX_SIGNATURE)).resolves.toEqual({
      status: 'SUBMITTED',
      txSignature: TX_SIGNATURE,
    })
    const [url, init] = fetchMock.mock.calls[0]!
    expect(String(url)).toContain(`/api/events/${'22'.repeat(32)}/submit`)
    expect(init?.method).toBe('POST')
    expect(JSON.parse(String(init?.body))).toEqual({ txSignature: TX_SIGNATURE })
  })

  /**
   * PURPOSE: Reject impossible combinations of capture/event lifecycle metadata at the browser trust boundary.
   * ARRANGE: Return capture responses that claim submitted/finalized state without a signature or accepted state with one.
   * ACTION: Parse each response through api.getCapture.
   * ASSERT: Every inconsistent lifecycle combination is rejected before page logic can act on it.
   * FAILURE MEANS: malformed or compromised API metadata could bypass the browser recovery state machine.
   */
  it('rejects inconsistent capture event lifecycle metadata', async () => {
    const base = {
      captureId: CAPTURE_ID,
      action: 'TRANSFER',
      animalId: '11'.repeat(32),
      status: 'EVIDENCE_ACCEPTED',
      eventHash: '22'.repeat(32),
    }
    for (const invalid of [
      { ...base, eventStatus: 'SUBMITTED', txSignature: null },
      { ...base, eventStatus: 'FINALIZED', txSignature: null },
      { ...base, eventStatus: 'EVIDENCE_ACCEPTED', txSignature: TX_SIGNATURE },
      { ...base, eventHash: null, eventStatus: 'SUBMITTED', txSignature: TX_SIGNATURE },
    ]) {
      vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(jsonResponse(invalid))
      await expect(api.getCapture(CAPTURE_ID)).rejects.toMatchObject({ name: 'ApiClientError' })
    }
  })

  /**
   * PURPOSE: Accept only durable transaction-submission states from the confirmed-verification endpoint.
   * ARRANGE: Return an impossible EVIDENCE_ACCEPTED status together with a transaction signature.
   * ACTION: Parse the response through api.submit.
   * ASSERT: The client rejects it instead of treating evidence acceptance as confirmed transaction registration.
   * FAILURE MEANS: the page could advance to finalized polling without a server-verified SUBMITTED state.
   */
  it('rejects non-submitted statuses from the transaction submission endpoint', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse({
      status: 'EVIDENCE_ACCEPTED',
      txSignature: TX_SIGNATURE,
    }))
    await expect(api.submit('22'.repeat(32), TX_SIGNATURE)).rejects.toMatchObject({ name: 'ApiClientError' })
  })

  /**
   * PURPOSE: Treat durable API identifiers as untrusted input at the browser boundary.
   * ARRANGE: Return malformed/oversized capture and Solana transaction identifiers.
   * ACTION: Parse them through normal API client calls.
   * ASSERT: Invalid UUID/base58 shapes are rejected before recovery/page logic can persist them.
   * FAILURE MEANS: hostile API metadata can create oversized or ambiguous browser recovery state.
   */
  it('rejects malformed or oversized durable identifiers', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(jsonResponse({
      captureId: 'x'.repeat(4096),
      action: 'TRANSFER',
      animalId: '11'.repeat(32),
      status: 'EVIDENCE_ACCEPTED',
      eventHash: null,
      eventStatus: null,
      txSignature: null,
    }))
    await expect(api.getCapture(CAPTURE_ID)).rejects.toMatchObject({ name: 'ApiClientError' })

    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(jsonResponse({
      status: 'SUBMITTED',
      txSignature: '1'.repeat(89),
    }))
    await expect(api.submit('22'.repeat(32), TX_SIGNATURE)).rejects.toMatchObject({ name: 'ApiClientError' })
  })

})

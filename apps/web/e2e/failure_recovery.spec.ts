import {
  connectWallet,
  createAnimal,
  definitionValue,
  expect,
  requireFullStack,
  runAction,
  setWalletRejection,
  test,
  timelineItems,
} from './support/system'

test.describe('Retry and failure recovery', () => {
  test.skip(requireFullStack, 'Set LASTRO_E2E_SYSTEM=1 only with the declared API/PostgreSQL/Solana/Agent environment')

  test('lost API response followed by Agent retry does not create duplicate evidence or transitions', async ({ page, system }) => {
    // PURPOSE: Prove ambiguous network delivery is recovered by immutable Agent/API idempotency instead of duplicating evidence.
    // ARRANGE: Create an unoriginated animal and arm the Agent-only proxy to drop the next evidence HTTP response after API acceptance.
    await page.goto('/demo')
    await connectWallet(page, system, 'Wallet A')
    const created = await createAnimal(page, system)
    const rfidA = system.freshRfidHex()
    const beforeMetrics = await system.metrics()
    await system.dropNextEvidenceResponse()

    // ACTION: Execute ORIGIN normally through the browser; the Agent must retry the same durable evidence after the dropped response.
    await runAction(page, system, 'ORIGIN', rfidA)
    await expect.poll(async () => (await system.metrics()).agentEvidencePosts, { timeout: 30_000 }).toBeGreaterThanOrEqual(beforeMetrics.agentEvidencePosts + 2)

    // ASSERT: Exactly one response was faulted, canonical sequence advanced once, and exported finalized history contains one event.
    const afterMetrics = await system.metrics()
    expect(afterMetrics.droppedEvidenceResponses).toBe(beforeMetrics.droppedEvidenceResponses + 1)
    const projection = await system.animal(created.animalId)
    expect(projection.eventSequence).toBe(1)
    expect(projection.identityRevision).toBe(1)
    expect((await system.evidencePackage(created.animalId)).events).toHaveLength(1)
    await expect(timelineItems(page)).toHaveCount(1)

    // FAILURE MEANS: normal network retry can fork, overwrite, or consume a physical Station event more than once.
  })

  test('page reload reconstructs UI from durable projection/evidence rather than browser memory', async ({ page, system }) => {
    // PURPOSE: Ensure a browser refresh cannot be required to remember canonical-looking state that exists only in Vue memory.
    // ARRANGE: Complete ORIGIN and A->B, then snapshot API and canonical Solana state.
    await page.goto('/demo')
    await connectWallet(page, system, 'Wallet A')
    const created = await createAnimal(page, system)
    const rfidA = system.freshRfidHex()
    await runAction(page, system, 'ORIGIN', rfidA)
    await runAction(page, system, 'TRANSFER', rfidA, { nextWallet: 'Wallet B' })
    const projection = await system.animal(created.animalId)
    const canonical = await system.canonicalAnimal(created.animalId)
    const restoredRequests: string[] = []
    page.on('request', (request) => {
      if (request.url().includes(`/api/animals/${created.animalId}`)) restoredRequests.push(request.url())
    })

    // ACTION: Hard reload the URL, preserving only the AnimalID query parameter written by the app.
    await page.reload()

    // ASSERT: The page refetches projection/history, restores the exact state/timeline, and still agrees with canonical Solana state.
    await expect(page.getByText(`Restored AnimalID ${created.animalId} from durable API projection and evidence history.`)).toBeVisible()
    await expect(definitionValue(page, 'AnimalID')).toHaveText(created.animalId)
    await expect(definitionValue(page, 'Current RFID')).toHaveText(projection.currentRfidHash!)
    await expect(definitionValue(page, 'Custodian')).toHaveText(projection.currentCustodian!)
    await expect(definitionValue(page, 'Revision')).toHaveText(String(projection.identityRevision))
    await expect(definitionValue(page, 'Sequence')).toHaveText(String(projection.eventSequence))
    await expect(timelineItems(page)).toHaveCount(2)
    expect(restoredRequests.some((url) => url.endsWith(`/api/animals/${created.animalId}`))).toBe(true)
    expect(restoredRequests.some((url) => url.endsWith(`/api/animals/${created.animalId}/evidence-package`))).toBe(true)
    expect(projection).toMatchObject(canonical)

    // FAILURE MEANS: demo correctness depends on ephemeral browser state instead of durable API projection plus canonical chain state.
  })

  test('wallet rejection leaves accepted physical evidence non-finalized and canonical state unchanged', async ({ page, system }) => {
    // PURPOSE: Prove wallet authorization is mandatory after valid physical evidence and before any canonical/projection transition.
    // ARRANGE: Originate with Wallet A, snapshot state, then prepare a valid A->B physical capture while making Wallet A reject signing.
    await page.goto('/demo')
    await connectWallet(page, system, 'Wallet A')
    const created = await createAnimal(page, system)
    const rfidA = system.freshRfidHex()
    await runAction(page, system, 'ORIGIN', rfidA)
    const beforeProjection = await system.animal(created.animalId)
    const beforeCanonical = await system.animalAccountSnapshot(created.animalId)
    let submitRequests = 0
    let confirmRequests = 0
    page.on('request', (request) => {
      if (request.method() !== 'POST') return
      const pathname = new URL(request.url()).pathname
      if (/\/api\/events\/[0-9a-f]{64}\/submit$/.test(pathname)) submitRequests += 1
      if (/\/api\/events\/[0-9a-f]{64}\/confirm$/.test(pathname)) confirmRequests += 1
    })
    await setWalletRejection(page, 'Wallet A', true)
    await page.getByLabel('Next custodian wallet').fill(system.walletAddress('Wallet B'))
    await system.observe('TRANSFER', rfidA)

    // ACTION: Submit TRANSFER and reject the signing request at the Wallet Standard boundary.
    await page.getByRole('button', { name: 'Transfer', exact: true }).click()
    await expect(page.getByText(/Wallet A rejected transaction signing/i)).toBeVisible({ timeout: 30_000 })

    // ASSERT: Physical evidence reached EVIDENCE_ACCEPTED, but no confirm call, projection update, or Solana mutation occurred.
    await expect(page.getByLabel('Station status')).toContainText('EVIDENCE_ACCEPTED')
    expect(submitRequests).toBe(0)
    expect(confirmRequests).toBe(0)
    expect(await system.animal(created.animalId)).toEqual(beforeProjection)
    expect(await system.animalAccountSnapshot(created.animalId)).toEqual(beforeCanonical)
    expect((await system.evidencePackage(created.animalId)).events).toHaveLength(1)

    // FAILURE MEANS: unsigned/rejected evidence can be presented or persisted as a canonical custody transition.
  })
  test('reload after wallet broadcast but before SUBMITTED registration reuses the same transaction signature', async ({ page, system }) => {
    // PURPOSE: Close the reload window after the wallet broadcasts but before confirmed RPC registration reaches the API.
    // ARRANGE: Start ORIGIN and abort exactly the first browser POST to /submit after Wallet A has signed the real transaction.
    await page.goto('/demo')
    await connectWallet(page, system, 'Wallet A')
    const created = await createAnimal(page, system)
    const rfidA = system.freshRfidHex()
    const signaturesBefore = system.walletSignatureCount('Wallet A')
    let droppedSubmit = false
    await page.route(/\/api\/events\/[0-9a-f]{64}\/submit$/, async (route) => {
      if (!droppedSubmit && route.request().method() === 'POST') {
        droppedSubmit = true
        await route.abort('connectionfailed')
        return
      }
      await route.continue()
    })
    await system.observe('ORIGIN', rfidA)

    // ACTION: Authorize once, lose the submission-registration request, then reload the same browser storage/session.
    await page.getByRole('button', { name: 'Origin', exact: true }).click()
    await expect.poll(() => system.walletSignatureCount('Wallet A'), { timeout: 30_000 }).toBe(signaturesBefore + 1)
    await expect(page.getByLabel('Station status')).toContainText('EVIDENCE_ACCEPTED')
    const captureId = await captureIdFromStatus(page)
    await expect.poll(async () => (await system.apiGet<{ eventStatus: string | null }>(`/api/captures/${captureId}`)).eventStatus).toBe('EVIDENCE_ACCEPTED')
    expect(droppedSubmit).toBe(true)
    await page.reload()

    // ASSERT: local durable browser metadata is revalidated through /submit, finalization completes, and Wallet A never signs twice.
    await expect(page.getByText('ORIGIN finalized and canonical state verified after reload.')).toBeVisible({ timeout: 90_000 })
    expect(system.walletSignatureCount('Wallet A')).toBe(signaturesBefore + 1)
    const projection = await system.animal(created.animalId)
    expect(projection.eventSequence).toBe(1)
    expect((await system.evidencePackage(created.animalId)).events).toHaveLength(1)

    // FAILURE MEANS: a reload can lose the broadcast signature and produce a second wallet transaction for one StationEvent.
  })

  test('reload after durable SUBMITTED state resumes finalization without a second wallet signature', async ({ page, system }) => {
    // PURPOSE: Close the reload window after confirmed RPC registration but before finalized projection confirmation.
    // ARRANGE: Start ORIGIN, let /submit succeed, and abort exactly the first browser POST to /confirm before it reaches the API.
    await page.goto('/demo')
    await connectWallet(page, system, 'Wallet A')
    const created = await createAnimal(page, system)
    const rfidA = system.freshRfidHex()
    const signaturesBefore = system.walletSignatureCount('Wallet A')
    let droppedConfirm = false
    await page.route(/\/api\/events\/[0-9a-f]{64}\/confirm$/, async (route) => {
      if (!droppedConfirm && route.request().method() === 'POST') {
        droppedConfirm = true
        await route.abort('connectionfailed')
        return
      }
      await route.continue()
    })
    await system.observe('ORIGIN', rfidA)

    // ACTION: Let the API persist SUBMITTED, lose the first finalization request, and reload.
    await page.getByRole('button', { name: 'Origin', exact: true }).click()
    await expect.poll(() => system.walletSignatureCount('Wallet A'), { timeout: 30_000 }).toBe(signaturesBefore + 1)
    await expect(page.getByLabel('Station status')).toContainText('EVIDENCE_ACCEPTED')
    const captureId = await captureIdFromStatus(page)
    await expect.poll(async () => (await system.apiGet<{ eventStatus: string | null }>(`/api/captures/${captureId}`)).eventStatus, { timeout: 30_000 }).toBe('SUBMITTED')
    expect(droppedConfirm).toBe(true)
    await page.reload()

    // ASSERT: server-stored verified signature drives recovery and finalization; no transaction preparation/signing repeats.
    await expect(page.getByText('ORIGIN finalized and canonical state verified after reload.')).toBeVisible({ timeout: 90_000 })
    expect(system.walletSignatureCount('Wallet A')).toBe(signaturesBefore + 1)
    const projection = await system.animal(created.animalId)
    expect(projection.eventSequence).toBe(1)
    expect((await system.evidencePackage(created.animalId)).events).toHaveLength(1)

    // FAILURE MEANS: durable SUBMITTED state cannot survive browser loss without asking the custodian to sign again.
  })

})


async function captureIdFromStatus(page: import('@playwright/test').Page): Promise<string> {
  const text = await page.getByLabel('Station status').textContent()
  const match = text?.match(/:\s*([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\s*$/i)
  if (!match) throw new Error(`Station status does not contain a capture UUID: ${text ?? '<null>'}`)
  return match[1]!
}

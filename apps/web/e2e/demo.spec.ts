import {
  connectWallet,
  createAnimal,
  definitionValue,
  expect,
  recoverAnimal,
  requireFullStack,
  runAction,
  test,
  timelineItems,
} from './support/system'

test.describe('Lastro full browser demo', () => {
  test.skip(
    requireFullStack,
    'Set LASTRO_E2E_SYSTEM=1 only with the declared API/PostgreSQL/Solana/Agent environment',
  )

  test('full ORIGIN -> A->B -> REIDENTIFY -> B->C flow preserves one AnimalID', async ({
    page,
    system,
  }) => {
    // PURPOSE: Prove the complete hackathon journey through the real browser, Agent, API, wallet, and Solana boundaries.
    // ARRANGE: Create one fresh animal, two unique physical RFIDs, and deterministic Wallet A/B/C actors.
    await page.goto('/demo')
    await connectWallet(page, system, 'Wallet A')
    const created = await createAnimal(page, system)
    const animalId = created.animalId
    const visualRecoveryId = created.visualRecoveryId
    const rfidA = system.freshRfidHex()
    const rfidB = system.freshRfidHex()

    // ACTION: Execute ORIGIN with RFID A and Wallet A, then transfer custody from A to B.
    await runAction(page, system, 'ORIGIN', rfidA)
    let projection = await system.animal(animalId)
    let canonical = await system.canonicalAnimal(animalId)
    expect(projection).toMatchObject({
      animalId,
      currentRfidHash: system.rfidHashHex(rfidA),
      currentCustodian: system.walletCustodianHex('Wallet A'),
      identityRevision: 1,
      eventSequence: 1,
    })
    expectProjectionMatchesCanonical(projection, canonical)

    await runAction(page, system, 'TRANSFER', rfidA, { nextWallet: 'Wallet B' })
    projection = await system.animal(animalId)
    canonical = await system.canonicalAnimal(animalId)
    expect(projection).toMatchObject({
      animalId,
      currentRfidHash: system.rfidHashHex(rfidA),
      currentCustodian: system.walletCustodianHex('Wallet B'),
      identityRevision: 1,
      eventSequence: 2,
    })
    expectProjectionMatchesCanonical(projection, canonical)

    // ACTION: Switch back to stale Wallet A and exercise the explicit rejected-authority demo path.
    const beforeStaleProjection = structuredClone(projection)
    const beforeStaleCanonical = await system.animalAccountSnapshot(animalId)
    await connectWallet(page, system, 'Wallet A')
    await page.getByRole('button', { name: 'Run stale-custodian attempt' }).click()
    await expect(
      page.getByText(
        'REJECTED: connected wallet is not the current custodian. No capture, evidence, or transaction was created.',
      ),
    ).toBeVisible()
    expect(await system.animal(animalId)).toEqual(beforeStaleProjection)
    expect(await system.animalAccountSnapshot(animalId)).toEqual(beforeStaleCanonical)

    // ACTION: Resolve the same AnimalID through the independent visual identifier, reidentify to RFID B, then transfer B to C.
    const recovered = await recoverAnimal(page, system, visualRecoveryId)
    expect(recovered.animalId).toBe(animalId)
    await connectWallet(page, system, 'Wallet B')
    await runAction(page, system, 'REIDENTIFY', rfidB)
    projection = await system.animal(animalId)
    canonical = await system.canonicalAnimal(animalId)
    expect(projection).toMatchObject({
      animalId,
      currentRfidHash: system.rfidHashHex(rfidB),
      currentCustodian: system.walletCustodianHex('Wallet B'),
      identityRevision: 2,
      eventSequence: 3,
    })
    expectProjectionMatchesCanonical(projection, canonical)

    await runAction(page, system, 'TRANSFER', rfidB, { nextWallet: 'Wallet C' })
    projection = await system.animal(animalId)
    canonical = await system.canonicalAnimal(animalId)

    // ASSERT: One AnimalID reaches sequence 4/revision 2 with RFID B and Wallet C, matching canonical Solana state exactly.
    expect(projection).toMatchObject({
      animalId,
      visualRecoveryId,
      currentRfidHash: system.rfidHashHex(rfidB),
      currentCustodian: system.walletCustodianHex('Wallet C'),
      identityRevision: 2,
      eventSequence: 4,
    })
    expectProjectionMatchesCanonical(projection, canonical)
    await expect(definitionValue(page, 'AnimalID')).toHaveText(animalId)
    await expect(definitionValue(page, 'Current RFID')).toHaveText(system.rfidHashHex(rfidB))
    await expect(definitionValue(page, 'Custodian')).toHaveText(
      system.walletCustodianHex('Wallet C'),
    )
    await expect(definitionValue(page, 'Revision')).toHaveText('2')
    await expect(definitionValue(page, 'Sequence')).toHaveText('4')
    await expect(timelineItems(page)).toHaveCount(4)
    await expect(timelineItems(page).nth(0)).toContainText('#1 ORIGIN')
    await expect(timelineItems(page).nth(1)).toContainText('#2 TRANSFER')
    await expect(timelineItems(page).nth(2)).toContainText('#3 REIDENTIFY')
    await expect(timelineItems(page).nth(3)).toContainText('#4 TRANSFER')

    // FAILURE MEANS: the primary physical evidence -> custody -> reidentification proof cannot complete without violating canonical state.
  })

  test('stale Wallet A attack after A->B is rejected and cannot change canonical state', async ({
    page,
    system,
  }) => {
    // PURPOSE: Protect the current-custodian authority invariant through the operator-facing stale-wallet demonstration.
    // ARRANGE: Complete ORIGIN and A->B, then snapshot both PostgreSQL projection and raw canonical AnimalState.
    await page.goto('/demo')
    await connectWallet(page, system, 'Wallet A')
    const created = await createAnimal(page, system)
    const rfidA = system.freshRfidHex()
    await runAction(page, system, 'ORIGIN', rfidA)
    await runAction(page, system, 'TRANSFER', rfidA, { nextWallet: 'Wallet B' })
    const beforeProjection = await system.animal(created.animalId)
    const beforeCanonical = await system.animalAccountSnapshot(created.animalId)
    const beforePackage = await system.evidencePackage(created.animalId)

    // ACTION: Reconnect stale Wallet A and request the explicit invalid attempt.
    await connectWallet(page, system, 'Wallet A')
    await page.getByRole('button', { name: 'Run stale-custodian attempt' }).click()

    // ASSERT: The rejection is explicit and no capture/evidence/canonical/projection mutation occurs.
    await expect(page.getByText(/^REJECTED:/)).toBeVisible()
    expect(await system.animal(created.animalId)).toEqual(beforeProjection)
    expect(await system.animalAccountSnapshot(created.animalId)).toEqual(beforeCanonical)
    expect(await system.evidencePackage(created.animalId)).toEqual(beforePackage)

    // FAILURE MEANS: stale custody can consume sequence/state or the UI rejection is merely cosmetic.
  })
})

function expectProjectionMatchesCanonical(
  projection: {
    animalId: string
    currentRfidHash: string | null
    currentCustodian: string | null
    identityRevision: number
    eventSequence: number
    lastEventHash: string | null
  },
  canonical: {
    animalId: string
    currentRfidHash: string
    currentCustodian: string
    identityRevision: number
    eventSequence: number
    lastEventHash: string
  },
): void {
  expect(projection.animalId).toBe(canonical.animalId)
  expect(projection.currentRfidHash).toBe(canonical.currentRfidHash)
  expect(projection.currentCustodian).toBe(canonical.currentCustodian)
  expect(projection.identityRevision).toBe(canonical.identityRevision)
  expect(projection.eventSequence).toBe(canonical.eventSequence)
  expect(projection.lastEventHash).toBe(canonical.lastEventHash)
}

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

test.describe('Physical identity recovery and RFID replacement', () => {
  test.skip(
    requireFullStack,
    'Set LASTRO_E2E_SYSTEM=1 only with the declared API/PostgreSQL/Solana/Agent environment',
  )

  test('visual recovery resolves the existing AnimalID before a new RFID capture', async ({
    page,
    system,
  }) => {
    // PURPOSE: Prove visual recovery selects an existing digital identity rather than creating a replacement animal.
    // ARRANGE: Originate one animal on RFID A, transfer A->B, then open a fresh demo route with no selected animal.
    await page.goto('/demo')
    await connectWallet(page, system, 'Wallet A')
    const created = await createAnimal(page, system)
    const rfidA = system.freshRfidHex()
    const rfidB = system.freshRfidHex()
    await runAction(page, system, 'ORIGIN', rfidA)
    await runAction(page, system, 'TRANSFER', rfidA, { nextWallet: 'Wallet B' })
    const before = await system.animal(created.animalId)
    expect(before).toMatchObject({
      identityRevision: 1,
      eventSequence: 2,
      currentCustodian: system.walletCustodianHex('Wallet B'),
    })

    // ACTION: Drop browser selection state, resolve by the independent visual identifier, then REIDENTIFY with Wallet B and RFID B.
    await page.goto('/demo')
    await expect(page.getByText(/Physical identity continuity:/)).toContainText('UNRESOLVED')
    const recovered = await recoverAnimal(page, system, created.visualRecoveryId)
    expect(recovered.animalId).toBe(created.animalId)
    await connectWallet(page, system, 'Wallet B')
    await runAction(page, system, 'REIDENTIFY', rfidB)

    // ASSERT: AnimalID/custodian stay stable while RFID changes and identity revision advances exactly once.
    const after = await system.animal(created.animalId)
    expect(after).toMatchObject({
      animalId: created.animalId,
      visualRecoveryId: created.visualRecoveryId,
      currentRfidHash: system.rfidHashHex(rfidB),
      currentCustodian: system.walletCustodianHex('Wallet B'),
      identityRevision: 2,
      eventSequence: 3,
    })
    await expect(definitionValue(page, 'AnimalID')).toHaveText(created.animalId)
    await expect(timelineItems(page)).toHaveCount(3)

    // FAILURE MEANS: recovery can silently fork AnimalID or REIDENTIFY changes custody/revision incorrectly.
  })

  test('retired RFID A is no longer presented or resolved as current after REIDENTIFY to B', async ({
    page,
    system,
  }) => {
    // PURPOSE: Protect the one-current-RFID invariant while retaining immutable historical RfidBinding state.
    // ARRANGE: Complete ORIGIN, A->B, and REIDENTIFY from RFID A to fresh RFID B.
    await page.goto('/demo')
    await connectWallet(page, system, 'Wallet A')
    const created = await createAnimal(page, system)
    const rfidA = system.freshRfidHex()
    const rfidB = system.freshRfidHex()
    const rfidAHash = system.rfidHashHex(rfidA)
    const rfidBHash = system.rfidHashHex(rfidB)
    await runAction(page, system, 'ORIGIN', rfidA)
    await runAction(page, system, 'TRANSFER', rfidA, { nextWallet: 'Wallet B' })
    await connectWallet(page, system, 'Wallet B')
    await runAction(page, system, 'REIDENTIFY', rfidB)

    // ACTION: Query both physical identifiers and inspect both canonical binding accounts.
    const oldLookup = await fetch(`${system.apiUrl}/api/animals/by-rfid/${rfidAHash}`)
    const currentLookup = await system.animalByRfid(rfidBHash)
    const bindings = await system.latestReidentifyBindingStatuses(created.animalId)

    // ASSERT: RFID A is retired/non-current, RFID B is ACTIVE/current, and the UI exposes only RFID B as current.
    expect(oldLookup.status).toBe(404)
    expect(currentLookup.animalId).toBe(created.animalId)
    expect(bindings).toEqual({ oldStatus: 2, newStatus: 1 })
    await expect(definitionValue(page, 'Current RFID')).toHaveText(rfidBHash)
    await expect(timelineItems(page).nth(2)).toContainText('#3 REIDENTIFY')
    await expect(timelineItems(page).nth(2)).toContainText(rfidBHash)

    // FAILURE MEANS: physical binding history becomes ambiguous or the old RFID can still identify the animal as current.
  })

  test('missing both RFID and visual recovery identifier leaves identity UNRESOLVED', async ({
    page,
  }) => {
    // PURPOSE: Ensure the browser never invents biological/physical identity when both independent identifiers are unavailable.
    // ARRANGE: Open a fresh demo route without an AnimalID query parameter, RFID observation, or visual recovery identifier.
    const captureRequests: string[] = []
    page.on('request', (request) => {
      if (request.method() === 'POST' && request.url().includes('/api/captures'))
        captureRequests.push(request.url())
    })
    await page.goto('/demo')

    // ACTION: Leave both recovery inputs unavailable and inspect every canonical action entry point.
    const create = page.getByRole('button', { name: 'Create Animal' })
    const recover = page.getByRole('button', { name: 'Find by visual recovery ID' })
    const origin = page.getByRole('button', { name: 'Origin' })
    const transfer = page.getByRole('button', { name: 'Transfer' })
    const reidentify = page.getByRole('button', { name: 'Reidentify' })

    // ASSERT: Identity remains explicitly unresolved and no capture/transition can begin.
    await expect(page.getByText(/Physical identity continuity:/)).toContainText('UNRESOLVED')
    await expect(
      page.getByText(
        /Lastro does not guess identity when both physical identifiers are unavailable/,
      ),
    ).toBeVisible()
    await expect(create).toBeDisabled()
    await expect(recover).toBeDisabled()
    await expect(origin).toBeDisabled()
    await expect(transfer).toBeDisabled()
    await expect(reidentify).toBeDisabled()
    expect(captureRequests).toHaveLength(0)

    // FAILURE MEANS: descriptive/operator input can be promoted into an identity claim without a physical recovery identifier.
  })
})

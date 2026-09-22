import { Buffer } from 'node:buffer'
import {
  connectWallet,
  createAnimal,
  expect,
  requireFullStack,
  runAction,
  test,
  type EvidencePackageDto,
} from './support/system'

test.describe('Independent browser verifier', () => {
  test.skip(requireFullStack, 'Set LASTRO_E2E_SYSTEM=1 only with the declared API/PostgreSQL/Solana/Agent environment')

  test('exported package from a real completed flow verifies all five layers', async ({ page, system }) => {
    // PURPOSE: Prove portable evidence independently reproduces the complete live system validity result.
    // ARRANGE: Complete ORIGIN, A->B, REIDENTIFY, and B->C through the browser using two physical RFID observations.
    await page.goto('/demo')
    await connectWallet(page, system, 'Wallet A')
    const created = await createAnimal(page, system)
    const rfidA = system.freshRfidHex()
    const rfidB = system.freshRfidHex()
    await runAction(page, system, 'ORIGIN', rfidA)
    await runAction(page, system, 'TRANSFER', rfidA, { nextWallet: 'Wallet B' })
    await connectWallet(page, system, 'Wallet B')
    await runAction(page, system, 'REIDENTIFY', rfidB)
    await runAction(page, system, 'TRANSFER', rfidB, { nextWallet: 'Wallet C' })
    expect((await system.evidencePackage(created.animalId)).events).toHaveLength(4)

    // ACTION: Open the independent verifier and load the exported package by AnimalID.
    await page.goto(`/verify/${created.animalId}`)
    await page.getByRole('button', { name: 'Verify AnimalID' }).click()

    // ASSERT: All local layers plus direct canonical Solana comparison are VALID.
    await expectVerificationLayer(page, 'RFID EVIDENCE', 'VALID')
    await expectVerificationLayer(page, 'STATION SIGNATURE', 'VALID')
    await expectVerificationLayer(page, 'IDENTITY CONTINUITY', 'VALID')
    await expectVerificationLayer(page, 'CUSTODY', 'VALID')
    await expectVerificationLayer(page, 'ON CHAIN STATE', 'VALID')
    await expect(page.getByText('Overall: VALID')).toBeVisible()
    await expect(page.getByText('Evidence and canonical Solana state agree.')).toBeVisible()

    // FAILURE MEANS: a valid completed Lastro history cannot be independently reproduced from portable evidence and canonical state.
  })

  test('one-byte mutation of signed event data makes the verifier explicitly INVALID', async ({ page, system }) => {
    // PURPOSE: Prove one-byte tampering is detected locally without trusting any backend verdict.
    // ARRANGE: Create one valid finalized ORIGIN package and flip one byte inside its signed StationEvent while retaining signature/key metadata.
    await page.goto('/demo')
    await connectWallet(page, system, 'Wallet A')
    const created = await createAnimal(page, system)
    await runAction(page, system, 'ORIGIN', system.freshRfidHex())
    const valid = await system.evidencePackage(created.animalId)
    const tampered = mutateSignedEventByte(valid, 200)

    // ACTION: Load the tampered EvidencePackage through the local-file verifier path.
    await page.goto('/verify')
    await page.getByLabel('Evidence package file').setInputFiles({
      name: 'tampered-evidence.json',
      mimeType: 'application/json',
      buffer: Buffer.from(JSON.stringify(tampered)),
    })

    // ASSERT: At least one cryptographic/integrity layer and the overall result become explicitly INVALID.
    await expect(page.getByLabel('Verification result').getByText(/— INVALID/).first()).toBeVisible()
    await expect(page.getByText('Overall: INVALID')).toBeVisible()
    await expect(page.getByText('Evidence is invalid before canonical Solana comparison.')).toBeVisible()

    // FAILURE MEANS: signed evidence bytes can change without invalidating the portable proof.
  })

  test('local-file verification continues when the Lastro evidence API is unavailable', async ({ page, system }) => {
    // PURPOSE: Prove independent verification does not require Lastro API availability after the EvidencePackage has been exported.
    // ARRANGE: Finalize one ORIGIN, retain its package locally, then block the evidence-package HTTP endpoint in the verifier page.
    await page.goto('/demo')
    await connectWallet(page, system, 'Wallet A')
    const created = await createAnimal(page, system)
    await runAction(page, system, 'ORIGIN', system.freshRfidHex())
    const pkg = await system.evidencePackage(created.animalId)
    let blockedApiRequests = 0
    await page.route('**/api/animals/*/evidence-package', async (route) => {
      blockedApiRequests += 1
      await route.abort('failed')
    })

    // ACTION: Load the already exported package from a browser file while direct Solana RPC remains available.
    await page.goto('/verify')
    await page.getByLabel('Evidence package file').setInputFiles({
      name: 'evidence.json',
      mimeType: 'application/json',
      buffer: Buffer.from(JSON.stringify(pkg)),
    })

    // ASSERT: No blocked API fetch is attempted and all five independent verification layers remain VALID.
    expect(blockedApiRequests).toBe(0)
    await expectVerificationLayer(page, 'RFID EVIDENCE', 'VALID')
    await expectVerificationLayer(page, 'STATION SIGNATURE', 'VALID')
    await expectVerificationLayer(page, 'IDENTITY CONTINUITY', 'VALID')
    await expectVerificationLayer(page, 'CUSTODY', 'VALID')
    await expectVerificationLayer(page, 'ON CHAIN STATE', 'VALID')
    await expect(page.getByText('Overall: VALID')).toBeVisible()

    // FAILURE MEANS: the verifier still depends on backend availability instead of portable evidence plus direct canonical RPC reads.
  })
})

async function expectVerificationLayer(page: import('@playwright/test').Page, label: string, status: 'VALID' | 'INVALID'): Promise<void> {
  await expect(page.getByLabel('Verification result').locator('li').filter({ hasText: label })).toContainText(`— ${status}`)
}

function mutateSignedEventByte(pkg: EvidencePackageDto, offset: number): EvidencePackageDto {
  const copy = structuredClone(pkg)
  const first = copy.events[0]
  if (!first) throw new Error('EvidencePackage contains no events to tamper')
  const raw = Buffer.from(first.eventBytesBase64, 'base64')
  if (raw.length !== 276 || offset < 0 || offset >= raw.length) throw new Error('Tamper offset is outside StationEvent')
  raw[offset] ^= 0x01
  first.eventBytesBase64 = raw.toString('base64')
  return copy
}

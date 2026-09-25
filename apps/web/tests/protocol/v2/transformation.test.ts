import { describe, expect, it } from 'vitest'

import {
  V2LineageRole,
  encodeV2TransformationManifest,
  v2MerkleRoot,
  v2TransformationManifestHash,
  type V2LineageLeaf,
  type V2TransformationManifest,
} from '../../../src/protocol/v2/transformation'

const inputs: V2LineageLeaf[] = [
  {
    assetIdHex: '11'.repeat(32),
    role: V2LineageRole.Input,
    position: 0,
    quantity: 1n,
    weightGrams: 600n,
  },
  {
    assetIdHex: '12'.repeat(32),
    role: V2LineageRole.Input,
    position: 1,
    quantity: 1n,
    weightGrams: 400n,
  },
]

const outputs: V2LineageLeaf[] = [
  {
    assetIdHex: '21'.repeat(32),
    role: V2LineageRole.Output,
    position: 0,
    quantity: 1n,
    weightGrams: 800n,
  },
  {
    assetIdHex: '22'.repeat(32),
    role: V2LineageRole.Byproduct,
    position: 1,
    quantity: 1n,
    weightGrams: 150n,
  },
]

describe('Lastro protocol v2 transformation canonicalization', () => {
  it('matches the checked-in roots and fixed manifest length', async () => {
    const inputRootHex = await v2MerkleRoot(inputs)
    const outputRootHex = await v2MerkleRoot(outputs)
    const manifest: V2TransformationManifest = {
      transformationIdHex: '01'.repeat(32),
      facilityIdHex: '02'.repeat(32),
      transformationType: 1,
      inputRootHex,
      outputRootHex,
      inputCount: 2,
      outputCount: 2,
      mass: {
        inputWeightGrams: 1_000n,
        outputWeightGrams: 800n,
        byproductWeightGrams: 150n,
        lossWeightGrams: 50n,
        toleranceBasisPoints: 0,
      },
      manifestNonce: 9n,
      expiresAt: 100n,
    }

    expect(inputRootHex).toBe('7ca4e4b0f7428d351e5a00ef658d4371e09cfdba673cacb9781743cfc1fc67a6')
    expect(outputRootHex).toBe('fdc560ba8308cd5a5b07e8dd6a40f92ae61da18237b7b389552a40e39b43c99f')
    expect(encodeV2TransformationManifest(manifest).byteLength).toBe(188)
    await expect(v2TransformationManifestHash(manifest)).resolves.toBe(
      '977bc957839b450db6c2a43de720ba540393dee37678107fef525e740da61fbc',
    )
  })

  it('rejects duplicate positions and mass outside tolerance', async () => {
    await expect(v2MerkleRoot([inputs[0], { ...inputs[1], position: 0 }])).rejects.toThrow(
      'positions',
    )
    const inputRootHex = await v2MerkleRoot(inputs)
    const outputRootHex = await v2MerkleRoot(outputs)
    await expect(
      v2TransformationManifestHash({
        transformationIdHex: '01'.repeat(32),
        facilityIdHex: '02'.repeat(32),
        transformationType: 1,
        inputRootHex,
        outputRootHex,
        inputCount: 2,
        outputCount: 2,
        mass: {
          inputWeightGrams: 1_000n,
          outputWeightGrams: 700n,
          byproductWeightGrams: 100n,
          lossWeightGrams: 50n,
          toleranceBasisPoints: 100,
        },
        manifestNonce: 9n,
        expiresAt: 100n,
      }),
    ).rejects.toThrow('mass balance')
  })
})

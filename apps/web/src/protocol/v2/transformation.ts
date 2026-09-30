export const V2_LINEAGE_LEAF_LEN = 53
export const V2_TRANSFORMATION_MANIFEST_LEN = 188
export const V2_MAX_MASS_TOLERANCE_BASIS_POINTS = 1_000

export const V2_HASH_DOMAIN_LINEAGE_LEAF = 'LASTRO_V2_LINEAGE_LEAF\0'
export const V2_HASH_DOMAIN_LINEAGE_NODE = 'LASTRO_V2_LINEAGE_NODE\0'
export const V2_HASH_DOMAIN_TRANSFORMATION = 'LASTRO_V2_TRANSFORMATION\0'

export enum V2LineageRole {
  Input = 1,
  Output = 2,
  Byproduct = 3,
  Loss = 4,
}

export interface V2LineageLeaf {
  assetIdHex: string
  role: V2LineageRole
  position: number
  quantity: bigint
  weightGrams: bigint
}

export interface V2MassBalance {
  inputWeightGrams: bigint
  outputWeightGrams: bigint
  byproductWeightGrams: bigint
  lossWeightGrams: bigint
  toleranceBasisPoints: number
}

export interface V2TransformationManifest {
  transformationIdHex: string
  facilityIdHex: string
  transformationType: number
  inputRootHex: string
  outputRootHex: string
  inputCount: number
  outputCount: number
  mass: V2MassBalance
  manifestNonce: bigint
  expiresAt: bigint
}

function assertHex32(value: string, field: string): void {
  if (!/^[0-9a-f]{64}$/.test(value) || /^0{64}$/.test(value)) {
    throw new Error(`${field} must be a non-zero 32-byte lowercase hex value`)
  }
}

function hexToBytes(value: string, field: string): Uint8Array {
  assertHex32(value, field)
  return Uint8Array.from(value.match(/../g)!, (pair) => Number.parseInt(pair, 16))
}

function bytesToHex(bytes: Uint8Array): string {
  return [...bytes].map((byte) => byte.toString(16).padStart(2, '0')).join('')
}

function assertU64(value: bigint, field: string): void {
  if (value < 0n || value > 0xffffffffffffffffn)
    throw new Error(`${field} must be an unsigned 64-bit integer`)
}

function assertI64NonNegative(value: bigint, field: string): void {
  if (value < 0n || value > 0x7fffffffffffffffn)
    throw new Error(`${field} must be a non-negative signed 64-bit integer`)
}

async function sha256(bytes: Uint8Array): Promise<Uint8Array> {
  const input = new ArrayBuffer(bytes.byteLength)
  new Uint8Array(input).set(bytes)
  return new Uint8Array(await globalThis.crypto.subtle.digest('SHA-256', input))
}

async function domainHash(domain: string, bytes: Uint8Array): Promise<string> {
  const domainBytes = new TextEncoder().encode(domain)
  const input = new Uint8Array(domainBytes.byteLength + bytes.byteLength)
  input.set(domainBytes)
  input.set(bytes, domainBytes.byteLength)
  return bytesToHex(await sha256(input))
}

export function encodeV2LineageLeaf(leaf: V2LineageLeaf): Uint8Array {
  assertHex32(leaf.assetIdHex, 'assetIdHex')
  if (!Number.isInteger(leaf.position) || leaf.position < 0 || leaf.position > 0xffffffff) {
    throw new Error('position must be an unsigned 32-bit integer')
  }
  if (leaf.role < V2LineageRole.Input || leaf.role > V2LineageRole.Loss) {
    throw new Error('unknown lineage role')
  }
  assertU64(leaf.quantity, 'quantity')
  assertU64(leaf.weightGrams, 'weightGrams')
  if (leaf.quantity === 0n && leaf.weightGrams === 0n)
    throw new Error('lineage leaf cannot be empty')

  const bytes = new Uint8Array(V2_LINEAGE_LEAF_LEN)
  const view = new DataView(bytes.buffer)
  bytes.set(hexToBytes(leaf.assetIdHex, 'assetIdHex'), 0)
  view.setUint8(32, leaf.role)
  view.setUint32(33, leaf.position, true)
  view.setBigUint64(37, leaf.quantity, true)
  view.setBigUint64(45, leaf.weightGrams, true)
  return bytes
}

export async function v2LineageLeafHash(leaf: V2LineageLeaf): Promise<string> {
  return domainHash(V2_HASH_DOMAIN_LINEAGE_LEAF, encodeV2LineageLeaf(leaf))
}

export async function v2MerkleRoot(leaves: readonly V2LineageLeaf[]): Promise<string> {
  if (leaves.length === 0) throw new Error('lineage must contain at least one leaf')
  const ordered = [...leaves].sort((left, right) => left.position - right.position)
  for (let index = 1; index < ordered.length; index += 1) {
    const previous = ordered[index - 1]
    const current = ordered[index]
    if (!previous || !current) throw new Error('lineage ordering is unexpectedly empty')
    if (previous.position === current.position) throw new Error('lineage positions must be unique')
  }
  let level = await Promise.all(ordered.map(v2LineageLeafHash))
  while (level.length > 1) {
    const next: string[] = []
    for (let index = 0; index < level.length; index += 2) {
      const leftHash = level[index]
      if (!leftHash) throw new Error('merkle level is unexpectedly empty')
      const rightHash = level[index + 1] ?? leftHash
      const left = hexToBytes(leftHash, 'leftHash')
      const right = hexToBytes(rightHash, 'rightHash')
      const pair = new Uint8Array(64)
      pair.set(left, 0)
      pair.set(right, 32)
      next.push(await domainHash(V2_HASH_DOMAIN_LINEAGE_NODE, pair))
    }
    level = next
  }
  const root = level[0]
  if (!root) throw new Error('merkle root is unexpectedly empty')
  return root
}

export function validateV2MassBalance(mass: V2MassBalance): void {
  assertU64(mass.inputWeightGrams, 'inputWeightGrams')
  assertU64(mass.outputWeightGrams, 'outputWeightGrams')
  assertU64(mass.byproductWeightGrams, 'byproductWeightGrams')
  assertU64(mass.lossWeightGrams, 'lossWeightGrams')
  if (mass.inputWeightGrams === 0n) throw new Error('input weight must be positive')
  if (
    !Number.isInteger(mass.toleranceBasisPoints) ||
    mass.toleranceBasisPoints < 0 ||
    mass.toleranceBasisPoints > V2_MAX_MASS_TOLERANCE_BASIS_POINTS
  ) {
    throw new Error('mass tolerance is outside the configured bound')
  }
  const produced = mass.outputWeightGrams + mass.byproductWeightGrams + mass.lossWeightGrams
  const difference =
    mass.inputWeightGrams > produced
      ? mass.inputWeightGrams - produced
      : produced - mass.inputWeightGrams
  const allowed = (mass.inputWeightGrams * BigInt(mass.toleranceBasisPoints)) / 10_000n
  if (difference > allowed) throw new Error('mass balance is outside the allowed tolerance')
}

export function encodeV2TransformationManifest(manifest: V2TransformationManifest): Uint8Array {
  assertHex32(manifest.transformationIdHex, 'transformationIdHex')
  assertHex32(manifest.facilityIdHex, 'facilityIdHex')
  assertHex32(manifest.inputRootHex, 'inputRootHex')
  assertHex32(manifest.outputRootHex, 'outputRootHex')
  if (
    !Number.isInteger(manifest.transformationType) ||
    manifest.transformationType <= 0 ||
    manifest.transformationType > 0xffff
  )
    throw new Error('invalid transformation type')
  if (
    !Number.isInteger(manifest.inputCount) ||
    manifest.inputCount <= 0 ||
    manifest.inputCount > 0xffffffff
  )
    throw new Error('invalid input count')
  if (
    !Number.isInteger(manifest.outputCount) ||
    manifest.outputCount <= 0 ||
    manifest.outputCount > 0xffffffff
  )
    throw new Error('invalid output count')
  assertU64(manifest.manifestNonce, 'manifestNonce')
  assertI64NonNegative(manifest.expiresAt, 'expiresAt')
  validateV2MassBalance(manifest.mass)

  const bytes = new Uint8Array(V2_TRANSFORMATION_MANIFEST_LEN)
  const view = new DataView(bytes.buffer)
  let offset = 0
  bytes.set(hexToBytes(manifest.transformationIdHex, 'transformationIdHex'), offset)
  offset += 32
  bytes.set(hexToBytes(manifest.facilityIdHex, 'facilityIdHex'), offset)
  offset += 32
  view.setUint16(offset, manifest.transformationType, true)
  offset += 2
  bytes.set(hexToBytes(manifest.inputRootHex, 'inputRootHex'), offset)
  offset += 32
  bytes.set(hexToBytes(manifest.outputRootHex, 'outputRootHex'), offset)
  offset += 32
  view.setUint32(offset, manifest.inputCount, true)
  offset += 4
  view.setUint32(offset, manifest.outputCount, true)
  offset += 4
  view.setBigUint64(offset, manifest.mass.inputWeightGrams, true)
  offset += 8
  view.setBigUint64(offset, manifest.mass.outputWeightGrams, true)
  offset += 8
  view.setBigUint64(offset, manifest.mass.byproductWeightGrams, true)
  offset += 8
  view.setBigUint64(offset, manifest.mass.lossWeightGrams, true)
  offset += 8
  view.setUint16(offset, manifest.mass.toleranceBasisPoints, true)
  offset += 2
  view.setBigUint64(offset, manifest.manifestNonce, true)
  offset += 8
  view.setBigInt64(offset, manifest.expiresAt, true)
  return bytes
}

export async function v2TransformationManifestHash(
  manifest: V2TransformationManifest,
): Promise<string> {
  return domainHash(V2_HASH_DOMAIN_TRANSFORMATION, encodeV2TransformationManifest(manifest))
}

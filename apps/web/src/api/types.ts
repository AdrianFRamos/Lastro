/**
 * Browser DTOs mirror schemas/openapi.yaml only.
 *
 * Strings remain untrusted until protocol validators decode length/encoding. Do not use these
 * transport types as proof that a hash/address/signature is valid.
 */
export type Hex32 = string
export type CaptureAction = 'BIND_IDENTIFIER' | 'REPLACE_IDENTIFIER' | 'OBSERVE_PRESENCE'
export type EventStatus = 'EVIDENCE_ACCEPTED' | 'SUBMITTED' | 'FINALIZED' | 'REJECTED'

/** Canonical AssetState as read by the API at finalized commitment. */
export interface AssetProjection {
  assetId: Hex32
  assetType: number
  status: number
  custodian: Hex32
  stateVersion: number
  eventSequence: number
  lastEventHash: Hex32
  currentRfidHash: Hex32 | null
  availableWeightGrams: number
}

export interface RfidLookup {
  rfidHash: Hex32
  bindingStatus: 'ACTIVE' | 'RETIRED'
  asset: AssetProjection
}

export interface CaptureAuthorizationChallenge {
  challengeId: string
  deploymentId: Hex32
  requiredSigner: string
  messageBase64: string
  expiresAtUnix: number
}

export interface CaptureAuthorizationProof {
  challengeId: string
  signatureBase64: string
}

export interface CaptureAuthorizationIntent {
  action: CaptureAction
  assetId: Hex32
  /** state_version the capture will produce (current canonical version + 1). */
  stateVersion: number
}

export interface Capture {
  captureId: string
  action: CaptureAction
  assetId: Hex32
  stateVersion: number
  status: 'PENDING' | 'DISPATCHED' | 'EVIDENCE_ACCEPTED' | 'EXPIRED' | 'CANCELLED'
  eventHash: Hex32 | null
  eventStatus: EventStatus | null
  txSignature: string | null
}

/** Lifecycle fields of a v2 event the demo console needs after submit/confirm. */
export interface EventLifecycle {
  eventHash: Hex32
  status: EventStatus
  txSignature: string | null
}

export interface CustodyTransfer {
  transferId: string
  assetId: Hex32
  toPartyId: Hex32
  status: string
  txSignature: string | null
}

export interface AccountMetaDto {
  address: string
  isSigner: boolean
  isWritable: boolean
}
export interface InstructionDto {
  programId: string
  accounts: AccountMetaDto[]
  dataBase64: string
}
export interface TransactionData {
  requiredSigner: string
  lastroProgramId: string
  /**
   * Station events: Secp256r1 at 0, Lastro at 1. Wallet-only instructions: Lastro at 0.
   * Optional ComputeBudget priority-fee instructions follow.
   */
  instructions: [InstructionDto, ...InstructionDto[]]
  measuredSerializedBytes: number
  transactionVersion: 'legacy' | 'v0'
}

export type TransformationStatus = 'OPEN' | 'FINALIZING' | 'FINALIZED' | 'ABORTED' | 'EXPIRED'

export interface TransformationProjection {
  transformationId: Hex32
  deploymentId: Hex32
  facilityId: Hex32
  transformationType: number
  inputRoot: Hex32
  outputRoot: Hex32
  inputCount: number
  outputCount: number
  inputWeightGrams: number
  outputWeightGrams: number
  byproductWeightGrams: number
  lossWeightGrams: number
  toleranceBasisPoints: number
  manifestNonce: number
  manifestHash: Hex32
  manifestBytesBase64: string
  status: TransformationStatus
  sequence: number
  expiresAt: number
  txSignature: string | null
}

export interface LineageEdge {
  transformationId: Hex32
  parentAssetId: Hex32
  childAssetId: Hex32
  role: number
  position: number
  quantity: number
  weightGrams: number
}

export interface PartyProjection {
  partyId: Hex32
  deploymentId: Hex32
  legalName: string
  taxIdHash: Hex32 | null
  wallet: Hex32
  role: number
  status: string
}

export interface FacilityProjection {
  facilityId: Hex32
  deploymentId: Hex32
  ownerPartyId: Hex32
  facilityType: number
  displayName: string
  credentialHash: Hex32
  validFrom: number
  validUntil: number
  status: string
}

export interface LotAssetProjection {
  assetId: Hex32
  quantity: number
  weightGrams: number
  role: string
}

export interface LotProjection {
  lotId: Hex32
  deploymentId: Hex32
  facilityId: Hex32
  ownerPartyId: Hex32
  externalReference: string | null
  headCount: number
  liveWeightGrams: number
  status: string
  assets: LotAssetProjection[]
}

export interface ProcessingItemProjection {
  position: number
  assetId: Hex32 | null
  direction: string
  quantity: number
  weightGrams: number
}

export interface ProcessingProjection {
  operationId: Hex32
  deploymentId: Hex32
  facilityId: Hex32
  lotId: Hex32 | null
  transformationId: Hex32 | null
  operatorPartyId: Hex32
  operationKind: string
  status: string
  notes: string | null
  txSignature: string | null
  items: ProcessingItemProjection[]
}

export interface ShipmentItemProjection {
  position: number
  assetId: Hex32
  quantity: number
  weightGrams: number
}

export interface ShipmentProjection {
  shipmentId: Hex32
  deploymentId: Hex32
  originFacilityId: Hex32
  destinationFacilityId: Hex32
  carrierPartyId: Hex32
  createdByPartyId: Hex32
  status: string
  plannedDeparture: string | null
  departedAt: string | null
  deliveredAt: string | null
  notes: string | null
  txSignature: string | null
  items: ShipmentItemProjection[]
}

export interface RecallMemberProjection {
  assetId: Hex32
  traversalDepth: number
  relation: string
}

export interface RecallProjection {
  recallId: Hex32
  deploymentId: Hex32
  openedByPartyId: Hex32
  scopeType: string
  scopeId: Hex32
  reason: string
  status: string
  snapshotRoot: Hex32
  members: RecallMemberProjection[]
}

export interface AuthorityGrantProjection {
  grantId: string
  deploymentId: Hex32
  partyId: Hex32
  facilityId: Hex32 | null
  capability: string
  status: string
  grantedByPartyId: Hex32 | null
  validFrom: number
  validUntil: number
  reason: string | null
}

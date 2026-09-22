/**
 * Browser DTOs mirror schemas/openapi.yaml only.
 *
 * Strings remain untrusted until protocol validators decode length/encoding. Do not use these
 * transport types as proof that a hash/address/signature is valid.
 */
export type Hex32 = string
export type CaptureAction = 'ORIGIN' | 'TRANSFER' | 'REIDENTIFY'
export type EventStatus = 'EVIDENCE_ACCEPTED' | 'SUBMITTED' | 'FINALIZED' | 'REJECTED'
export type SubmissionStatus = 'SUBMITTED' | 'FINALIZED'

export interface AnimalProjection {
  animalId: Hex32
  visualRecoveryId: string
  currentRfidHash: Hex32 | null
  currentCustodian: Hex32 | null
  identityRevision: number
  eventSequence: number
  lastEventHash: Hex32 | null
}

export interface Capture {
  captureId: string
  action: CaptureAction
  animalId: Hex32
  status: 'PENDING' | 'DISPATCHED' | 'EVIDENCE_ACCEPTED' | 'EXPIRED' | 'CANCELLED'
  eventHash: Hex32 | null
  eventStatus: EventStatus | null
  txSignature: string | null
}

export interface EventSubmission {
  status: SubmissionStatus
  txSignature: string
}

export interface AccountMetaDto { address: string; isSigner: boolean; isWritable: boolean }
export interface InstructionDto { programId: string; accounts: AccountMetaDto[]; dataBase64: string }
export interface TransactionData {
  requiredSigner: string
  lastroProgramId: string
  instructions: [InstructionDto, InstructionDto]
  measuredSerializedBytes: number
  transactionVersion: 'legacy' | 'v0'
}

/** Verifier states. `NOT_CHECKED` is never treated as success. */
export type VerificationStatus = 'VALID' | 'INVALID' | 'NOT_CHECKED'

export type VerificationLayerName =
  | 'RFID_EVIDENCE'
  | 'STATION_SIGNATURE'
  | 'IDENTITY_CONTINUITY'
  | 'CUSTODY'
  | 'ON_CHAIN_STATE'

export interface VerificationLayerResult {
  layer: VerificationLayerName
  status: VerificationStatus
  /** Short technical explanation without secret material. */
  detail: string
}

export interface VerificationResult {
  animalId: string
  layers: VerificationLayerResult[]
  /** True only when every required layer is VALID. */
  valid: boolean
}

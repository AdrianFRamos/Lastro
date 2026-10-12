/** Verifier states. `NOT_CHECKED` is never treated as success. */
export type VerificationStatus = 'VALID' | 'INVALID' | 'NOT_CHECKED'

export type VerificationLayerName =
  | 'RFID_EVIDENCE'
  | 'STATION_SIGNATURE'
  | 'IDENTITY_CONTINUITY'
  | 'CUSTODY'
  /** v2.1 S5: each batch's facility and the boundary its on-chain commitment hashes. */
  | 'FACILITY_ORIGIN'
  | 'ON_CHAIN_STATE'

export interface VerificationLayerResult {
  layer: VerificationLayerName
  status: VerificationStatus
  /** Short technical explanation without secret material. */
  detail: string
}

export interface VerificationResult {
  assetId: string
  layers: VerificationLayerResult[]
  /** True only when every required layer is VALID. */
  valid: boolean
}

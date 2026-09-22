/** Strict EvidencePackage transport parser used before cryptographic verification. */
export interface EvidenceEvent {
  eventBytesBase64: string
  observedRfidHex: string
  stationPubkeyHex: string
  stationSignatureHex: string
  txSignature: string | null
}

export interface EvidencePackage {
  version: 1
  deploymentId: string
  animalId: string
  events: EvidenceEvent[]
}

const PACKAGE_KEYS = ['animalId', 'deploymentId', 'events', 'version'] as const
const STATION_EVENT_BASE64_LEN = Math.ceil(276 / 3) * 4

const EVENT_KEYS = [
  'eventBytesBase64',
  'observedRfidHex',
  'stationPubkeyHex',
  'stationSignatureHex',
  'txSignature',
] as const

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function hasExactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const actual = Object.keys(value).sort()
  const wanted = [...expected].sort()
  return actual.length === wanted.length && actual.every((key, index) => key === wanted[index])
}

function isLowerHex(value: unknown, bytes: number): value is string {
  return typeof value === 'string' && new RegExp(`^[0-9a-f]{${bytes * 2}}$`).test(value)
}

function decodedBase64Length(value: unknown): number | null {
  if (typeof value !== 'string' || value.length !== STATION_EVENT_BASE64_LEN) return null
  try {
    const decoded = atob(value)
    const canonical = btoa(decoded)
    if (canonical !== value) return null
    return decoded.length
  } catch {
    return null
  }
}

function isSolanaSignatureText(value: unknown): value is string {
  return typeof value === 'string'
    && value.length >= 64
    && value.length <= 88
    && /^[1-9A-HJ-NP-Za-km-z]+$/.test(value)
}

function parseEvent(value: unknown): EvidenceEvent {
  if (!isRecord(value) || !hasExactKeys(value, EVENT_KEYS)) {
    throw new Error('invalid EvidenceEvent shape')
  }
  if (decodedBase64Length(value.eventBytesBase64) !== 276) {
    throw new Error('EvidenceEvent StationEvent must decode to exactly 276 bytes')
  }
  if (!isLowerHex(value.observedRfidHex, 8)) throw new Error('observed RFID must be 8 lowercase hex bytes')
  if (!isLowerHex(value.stationPubkeyHex, 33)) throw new Error('Station public key must be 33 lowercase hex bytes')
  if (!isLowerHex(value.stationSignatureHex, 64)) throw new Error('Station signature must be 64 lowercase hex bytes')
  if (value.txSignature !== null && !isSolanaSignatureText(value.txSignature)) {
    throw new Error('transaction signature must be null or canonical-looking base58 for a 64-byte Solana signature')
  }
  return {
    eventBytesBase64: value.eventBytesBase64,
    observedRfidHex: value.observedRfidHex,
    stationPubkeyHex: value.stationPubkeyHex,
    stationSignatureHex: value.stationSignatureHex,
    txSignature: value.txSignature,
  }
}

export function parseEvidencePackage(value: unknown): EvidencePackage {
  if (!isRecord(value) || !hasExactKeys(value, PACKAGE_KEYS)) {
    throw new Error('invalid EvidencePackage shape')
  }
  if (value.version !== 1) throw new Error('unsupported EvidencePackage version')
  if (!isLowerHex(value.deploymentId, 32)) throw new Error('deploymentId must be 32 lowercase hex bytes')
  if (!isLowerHex(value.animalId, 32)) throw new Error('animalId must be 32 lowercase hex bytes')
  if (!Array.isArray(value.events) || value.events.length === 0) {
    throw new Error('EvidencePackage must contain at least one event')
  }

  return {
    version: 1,
    deploymentId: value.deploymentId,
    animalId: value.animalId,
    events: value.events.map(parseEvent),
  }
}

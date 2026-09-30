/** Browser hash and byte primitives for independent verification over canonical bytes only. */
const RFID_DOMAIN = new TextEncoder().encode('LASTRO_RFID\0')
const STATION_DOMAIN = new TextEncoder().encode('LASTRO_STATION\0')
export const CANONICAL_RFID_LENGTH = 8 as const

export async function sha256(...parts: Uint8Array[]): Promise<Uint8Array> {
  const input = new Uint8Array(parts.reduce((total, part) => total + part.length, 0))
  let offset = 0
  for (const part of parts) {
    input.set(part, offset)
    offset += part.length
  }
  return new Uint8Array(await crypto.subtle.digest('SHA-256', input))
}

export const rfidHash = async (canonicalRfid: Uint8Array) => {
  if (canonicalRfid.length !== CANONICAL_RFID_LENGTH)
    throw new Error('canonical RFID must be exactly 8 bytes')
  return sha256(RFID_DOMAIN, canonicalRfid)
}

export const stationId = async (compressedP256Pubkey: Uint8Array) => {
  if (
    compressedP256Pubkey.length !== 33 ||
    (compressedP256Pubkey[0] !== 0x02 && compressedP256Pubkey[0] !== 0x03)
  ) {
    throw new Error('Station public key must be a 33-byte compressed P-256 key')
  }
  return sha256(STATION_DOMAIN, compressedP256Pubkey)
}

export function toHex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')
}

export function fromHex(value: string, bytes: number): Uint8Array {
  if (!new RegExp(`^[0-9a-f]{${bytes * 2}}$`).test(value))
    throw new Error(`expected ${bytes} lowercase hex bytes`)
  return Uint8Array.from(value.match(/../g)!, (pair) => Number.parseInt(pair, 16))
}

/** Decode canonical (padded, non-malleable) base64 of an exact byte length. */
export function fromBase64(value: string, bytes?: number): Uint8Array {
  let binary: string
  try {
    binary = atob(value)
  } catch {
    throw new Error('value must be canonical base64')
  }
  if (btoa(binary) !== value || (bytes !== undefined && binary.length !== bytes))
    throw new Error(
      `value must be canonical base64${bytes === undefined ? '' : ` of ${bytes} bytes`}`,
    )
  return Uint8Array.from(binary, (character) => character.charCodeAt(0))
}

export function equalBytes(left: ArrayLike<number>, right: ArrayLike<number>): boolean {
  if (left.length !== right.length) return false
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return false
  }
  return true
}

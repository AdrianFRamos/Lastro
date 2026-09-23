/** Browser hash primitives for independent verification over canonical bytes only. */
import {
  CANONICAL_RFID_LENGTH,
  RFID_DOMAIN,
  STATION_DOMAIN,
  STATION_EVENT_LENGTH,
} from './constants'

async function sha256(bytes: Uint8Array): Promise<Uint8Array> {
  const input = new Uint8Array(bytes)
  return new Uint8Array(await crypto.subtle.digest('SHA-256', input))
}

function concat(a: Uint8Array, b: Uint8Array): Uint8Array {
  const out = new Uint8Array(a.length + b.length)
  out.set(a, 0)
  out.set(b, a.length)
  return out
}

export const eventHash = async (eventBytes: Uint8Array) => {
  if (eventBytes.length !== STATION_EVENT_LENGTH)
    throw new Error('StationEvent must be exactly 276 bytes')
  return sha256(eventBytes)
}

export const rfidHash = async (canonicalRfid: Uint8Array) => {
  if (canonicalRfid.length !== CANONICAL_RFID_LENGTH)
    throw new Error('canonical RFID must be exactly 8 bytes')
  return sha256(concat(RFID_DOMAIN, canonicalRfid))
}

export const stationId = async (compressedP256Pubkey: Uint8Array) => {
  if (
    compressedP256Pubkey.length !== 33 ||
    (compressedP256Pubkey[0] !== 0x02 && compressedP256Pubkey[0] !== 0x03)
  ) {
    throw new Error('Station public key must be a 33-byte compressed P-256 key')
  }
  return sha256(concat(STATION_DOMAIN, compressedP256Pubkey))
}

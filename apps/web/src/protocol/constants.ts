export const STATION_EVENT_LENGTH = 276 as const
export const CANONICAL_RFID_LENGTH = 8 as const
export const MAGIC = new Uint8Array([0x4c, 0x53, 0x54, 0x52]) // LSTR
export const VERSION = 1 as const
export const RFID_DOMAIN = new TextEncoder().encode('LASTRO_RFID\0')
export const STATION_DOMAIN = new TextEncoder().encode('LASTRO_STATION\0')

export const OFFSET = Object.freeze({
  magic: 0,
  version: 4,
  action: 5,
  reserved: 6,
  deploymentId: 8,
  animalId: 40,
  stationId: 72,
  eventSequence: 104,
  identityRevision: 112,
  previousEventHash: 116,
  oldRfidHash: 148,
  newRfidHash: 180,
  fromCustodian: 212,
  toCustodian: 244,
  end: 276,
})

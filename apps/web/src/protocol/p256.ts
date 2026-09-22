/**
 * Independent P-256 verifier used by the browser evidence verifier.
 *
 * Protocol contract:
 * - message: exactly the raw 276 StationEvent bytes;
 * - public key: SEC1 compressed P-256 point, exactly 33 bytes;
 * - signature: compact/IEEE-P1363 r||s, exactly 64 bytes;
 * - hash: SHA-256 over the raw message, performed once by P-256 ECDSA;
 * - signature must be low-S, matching Lastro/Solana acceptance semantics.
 *
 * Why @noble/curves instead of direct WebCrypto raw-key import:
 * WebCrypto support for compressed EC point import is runtime-dependent. Lastro's wire
 * format intentionally stores the 33-byte compressed key used by the Solana precompile.
 * @noble/curves accepts compressed P-256 keys and compact signatures directly, hashes
 * unhashed messages with SHA-256 for P-256, and uses low-S ECDSA semantics by default.
 * We still enforce low-S before calling the library so the protocol rule remains explicit.
 */

import { p256 } from '@noble/curves/nist.js'

const P256_ORDER = BigInt('0xffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551')
const P256_HALF_ORDER = P256_ORDER >> 1n

function bigEndianToBigInt(bytes: Uint8Array): bigint {
  let out = 0n
  for (const byte of bytes) out = (out << 8n) | BigInt(byte)
  return out
}

export function isLowSP256Signature(signature64: Uint8Array): boolean {
  if (signature64.length !== 64) return false

  const r = bigEndianToBigInt(signature64.subarray(0, 32))
  const s = bigEndianToBigInt(signature64.subarray(32, 64))

  return r > 0n && r < P256_ORDER && s > 0n && s <= P256_HALF_ORDER
}

export function verifyStationSignature(
  message276: Uint8Array,
  compressedPubkey33: Uint8Array,
  signature64: Uint8Array,
): boolean {
  if (message276.length !== 276) return false
  if (compressedPubkey33.length !== 33) return false
  if (compressedPubkey33[0] !== 0x02 && compressedPubkey33[0] !== 0x03) return false
  if (!isLowSP256Signature(signature64)) return false

  try {
    // noble-curves v2 receives the UNHASHED message for ECDSA and P-256 applies SHA-256.
    // Compact signatures and compressed public keys are the default byte formats.
    return p256.verify(signature64, message276, compressedPubkey33)
  } catch {
    // Malformed/off-curve public keys and malformed signatures are invalid evidence, not
    // exceptional verifier states. The caller can distinguish this INVALID result from
    // transport/RPC availability failures at a higher layer.
    return false
  }
}

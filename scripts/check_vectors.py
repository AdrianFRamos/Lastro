#!/usr/bin/env python3
"""Verify the frozen v2 interoperability vectors with an independent implementation.

Complements (never replaces) the Rust, C and TypeScript tests: RFID and StationId hashing,
low-S P-256 signatures over the 220-byte envelopes, the bind -> replace -> observe hash chain,
the identifier payload commitments and the serial fixtures.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import struct
from pathlib import Path

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives.asymmetric.utils import encode_dss_signature

ROOT = Path(__file__).resolve().parents[1]
VECTORS = ROOT / 'test-vectors' / 'v2-capture.json'
P256_ORDER = int('FFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551', 16)
ZERO32 = bytes(32)


def domain_hash(domain: bytes, data: bytes) -> bytes:
    return hashlib.sha256(domain + data).digest()


def main() -> int:
    argparse.ArgumentParser().add_argument('--verify-only', action='store_true')
    data = json.loads(VECTORS.read_text())
    tags = {}
    for label in ('a', 'b'):
        canonical = bytes.fromhex(data['tags'][label]['canonical_hex'])
        assert len(canonical) == 8, label
        tags[label] = domain_hash(b'LASTRO_RFID\0', canonical)
        assert tags[label].hex() == data['tags'][label]['hash_hex'], label

    pubkey = bytes.fromhex(data['station']['pubkey33_hex'])
    assert len(pubkey) == 33 and pubkey[0] in (2, 3)
    station_id = domain_hash(b'LASTRO_STATION\0', pubkey)
    assert station_id.hex() == data['station']['station_id_hex']
    public_key = ec.EllipticCurvePublicKey.from_encoded_point(ec.SECP256R1(), pubkey)

    payloads = {
        'bind': (ZERO32, tags['a']),
        'replace': (tags['a'], tags['b']),
        'observe': (tags['b'], tags['b']),
    }
    previous = ZERO32
    for version, name in enumerate(('bind', 'replace', 'observe'), start=1):
        capture = data['captures'][name]
        envelope = bytes.fromhex(capture['envelope_hex'])
        assert len(envelope) == 220, name
        assert envelope[4:36].hex() == data['deployment_id_hex'] and envelope[36:68].hex() == data['asset_id_hex']
        assert struct.unpack('<Q', envelope[100:108])[0] == version, name
        assert envelope[108:140] == previous, f'{name}: predecessor chain broken'
        payload = domain_hash(b'LASTRO_V2_PAYLOAD\0', b''.join(payloads[name]))
        assert envelope[140:172] == payload and payload.hex() == capture['payload_hash_hex'], name
        assert envelope[172:204] == station_id, name
        event_hash = domain_hash(b'LASTRO_V2_EVENT\0', envelope)
        assert event_hash.hex() == capture['event_hash_hex'], name

        signature = bytes.fromhex(capture['station_signature_hex'])
        r, s = int.from_bytes(signature[:32], 'big'), int.from_bytes(signature[32:], 'big')
        assert 1 <= r < P256_ORDER and 1 <= s <= P256_ORDER // 2, f'{name}: not low-S'
        der = encode_dss_signature(r, s)
        public_key.verify(der, envelope, ec.ECDSA(hashes.SHA256()))
        tampered = bytearray(envelope)
        tampered[-1] ^= 1
        try:
            public_key.verify(der, bytes(tampered), ec.ECDSA(hashes.SHA256()))
        except InvalidSignature:
            pass
        else:
            raise AssertionError(f'{name}: signature verifies an altered envelope')

        ready = bytes.fromhex(capture['event_ready_hex'])
        command = bytes.fromhex(capture['command_hex'])
        ack = bytes.fromhex(capture['ack_hex'])
        assert (len(command), len(ready), len(ack)) == (202, 341, 48), name
        assert command[:16] == ready[:16] == ack[:16], name
        assert ready[16:236] == envelope and ready[236:244].hex() == capture['observed_rfid_hex']
        assert ready[244:277] == pubkey and ready[277:341] == signature
        assert ack[16:48] == event_hash
        previous = event_hash

    bind = data['captures']['bind']
    for filename, field in (
        ('v2-serial-command.bin', 'command_hex'),
        ('v2-serial-event-ready.bin', 'event_ready_hex'),
        ('v2-serial-ack.bin', 'ack_hex'),
    ):
        assert (ROOT / 'test-vectors' / filename).read_bytes().hex() == bind[field], filename
    assert len((ROOT / 'test-vectors' / 'serial-error.bin').read_bytes()) == 20
    print('vectors: v2 RFID/StationId hashes, low-S P-256 envelopes, hash chain, payloads and serial fixtures OK')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

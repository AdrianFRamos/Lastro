#!/usr/bin/env python3
"""Verify frozen interoperability vectors without replacing Rust, C, or TypeScript tests."""
from __future__ import annotations
import argparse, hashlib, json, struct

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives.asymmetric.utils import encode_dss_signature
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
V = ROOT / 'test-vectors' / 'vectors.json'

def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('--verify-only', action='store_true')
    parser.parse_args()
    data = json.loads(V.read_text())
    domain = b'LASTRO_RFID\0'
    for label in ('a','b'):
        canonical = bytes.fromhex(data['rfid'][f'{label}_canonical_hex'])
        assert len(canonical) == 8, (label, len(canonical))
        assert hashlib.sha256(domain + canonical).hexdigest() == data['rfid'][f'{label}_hash_hex']

    # P-256 cross-language fixture identity. SEC1 compressed point is decoded by an
    # independent crypto implementation; StationID must bind exactly to that encoding.
    station_pubkey = bytes.fromhex(data['station']['pubkey_compressed_hex'])
    assert len(station_pubkey) == 33 and station_pubkey[0] in (2, 3)
    assert hashlib.sha256(b'LASTRO_STATION\0' + station_pubkey).hexdigest() == data['station']['station_id_hex']
    public_key = ec.EllipticCurvePublicKey.from_encoded_point(ec.SECP256R1(), station_pubkey)
    p256_order = int('FFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551', 16)
    half_order = p256_order // 2

    events = {}
    fixture_files = {
        'origin': 'origin.bin',
        'transfer': 'transfer.bin',
        'reidentify': 'reidentify.bin',
        'transfer_b_to_c': 'transfer-b-to-c.bin',
    }
    for name, filename in fixture_files.items():
        path = ROOT / 'test-vectors' / filename
        raw = path.read_bytes()
        assert len(raw) == 276, (name, len(raw))
        assert raw[:4] == b'LSTR' and raw[4] == 1 and raw[6:8] == b'\0\0'
        assert hashlib.sha256(raw).hexdigest() == data['events'][name]['event_hash_hex']
        assert raw.hex() == data['events'][name]['event_bytes_hex']

        signature = bytes.fromhex(data['events'][name]['station_signature_hex'])
        assert len(signature) == 64, (name, len(signature))
        r = int.from_bytes(signature[:32], 'big')
        sig_s = int.from_bytes(signature[32:], 'big')
        assert 1 <= r < p256_order, (name, 'r out of range')
        assert 1 <= sig_s <= half_order, (name, 'signature is not low-S')
        der = encode_dss_signature(r, sig_s)
        public_key.verify(der, raw, ec.ECDSA(hashes.SHA256()))

        # A one-byte change must invalidate the same compact signature. This catches
        # accidental verification of event_hash bytes or another message representation.
        tampered = bytearray(raw)
        tampered[-1] ^= 0x01
        try:
            public_key.verify(der, bytes(tampered), ec.ECDSA(hashes.SHA256()))
        except InvalidSignature:
            pass
        else:
            raise AssertionError(f'{name}: signature unexpectedly verifies altered StationEvent')

        events[name] = raw

    assert events['origin'][5] == 1 and struct.unpack('<Q', events['origin'][104:112])[0] == 1
    assert events['transfer'][5] == 2 and events['transfer'][116:148] == hashlib.sha256(events['origin']).digest()
    assert events['reidentify'][5] == 3 and events['reidentify'][116:148] == hashlib.sha256(events['transfer']).digest()
    assert events['transfer_b_to_c'][5] == 2
    assert struct.unpack('<Q', events['transfer_b_to_c'][104:112])[0] == 4
    assert struct.unpack('<I', events['transfer_b_to_c'][112:116])[0] == 2
    assert events['transfer_b_to_c'][116:148] == hashlib.sha256(events['reidentify']).digest()
    assert events['transfer_b_to_c'][148:180] == events['transfer_b_to_c'][180:212] == bytes.fromhex(data['rfid']['b_hash_hex'])

    serial = data['serial']
    expected_lengths = {'serial-command.bin': 224, 'serial-event-ready.bin': 397, 'serial-ack.bin': 48, 'serial-error.bin': 20}
    for filename, expected_len in expected_lengths.items():
        raw = (ROOT / 'test-vectors' / filename).read_bytes()
        assert len(raw) == expected_len, (filename, len(raw))
        key = {'serial-command.bin':'command','serial-event-ready.bin':'event_ready','serial-ack.bin':'ack','serial-error.bin':'error_rfid_read_failed'}[filename]
        assert raw.hex() == serial[key]['payload_hex']
    capture = bytes.fromhex(serial['uuid_wire_hex'])
    command = (ROOT / 'test-vectors' / 'serial-command.bin').read_bytes()
    ready = (ROOT / 'test-vectors' / 'serial-event-ready.bin').read_bytes()
    ack = (ROOT / 'test-vectors' / 'serial-ack.bin').read_bytes()
    assert command[:16] == ready[:16] == ack[:16] == capture
    assert command[16] == 1 and command[17:20] == b'\0\0\0'
    assert struct.unpack('<Q', command[84:92])[0] == 1
    assert struct.unpack('<I', command[92:96])[0] == 1
    assert ready[16:292] == events['origin']
    assert ready[292:300] == bytes.fromhex(data['rfid']['a_canonical_hex'])
    assert ack[16:48].hex() == data['events']['origin']['event_hash_hex']
    print('vectors: protocol, RFID, P-256 low-S signatures, predecessor chain and serial payload fixtures OK')
    return 0

if __name__ == '__main__': raise SystemExit(main())

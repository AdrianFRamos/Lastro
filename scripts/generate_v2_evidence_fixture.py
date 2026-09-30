#!/usr/bin/env python3
"""Build test-vectors/v2-evidence-package.valid.json from the signed v2 capture vectors.

The package is the bind -> replace -> observe history of test-vectors/v2-capture.json in the
exact `lastro.evidence-package.v2` shape the API exports. Transaction signatures are fixed
placeholders: only the chain verifier (with mocked RPC) interprets them.
"""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
PROGRAM_ID = 'Vote111111111111111111111111111111111111111'
CUSTODIAN = 'c1' * 32


def base58(data: bytes) -> str:
    number = int.from_bytes(data, 'big')
    out = ''
    while number:
        number, rest = divmod(number, 58)
        out = ALPHABET[rest] + out
    return '1' * (len(data) - len(data.lstrip(b'\0'))) + out


def event(capture: dict, pubkey: str, index: int) -> dict:
    envelope = bytes.fromhex(capture['envelope_hex'])
    le = lambda start, end: int.from_bytes(envelope[start:end], 'little')
    return {
        'eventId': envelope[68:100].hex(),
        'eventHash': capture['event_hash_hex'],
        'deploymentId': envelope[4:36].hex(),
        'subjectId': envelope[36:68].hex(),
        'sourceId': envelope[172:204].hex(),
        'eventType': le(2, 4),
        'stateVersion': le(100, 108),
        'observedAt': le(204, 212),
        'expiresAt': le(212, 220),
        'expectedPreviousHash': envelope[108:140].hex(),
        'payloadHash': envelope[140:172].hex(),
        'envelopeBytesBase64': __import__('base64').b64encode(envelope).decode(),
        'observedRfidHex': capture['observed_rfid_hex'],
        'stationPubkeyHex': pubkey,
        'stationSignatureHex': capture['station_signature_hex'],
        'status': 'FINALIZED',
        'txSignature': base58(bytes([index + 1]) * 64),
    }


def main() -> None:
    vectors = json.loads((ROOT / 'test-vectors/v2-capture.json').read_text())
    pubkey = vectors['station']['pubkey33_hex']
    history = [vectors['captures'][name] for name in ('bind', 'replace', 'observe')]
    events = [event(capture, pubkey, index) for index, capture in enumerate(history)]
    package = {
        'schema': 'lastro.evidence-package.v2',
        'deploymentId': vectors['deployment_id_hex'],
        'lastroProgramId': PROGRAM_ID,
        'asset': {
            'assetId': vectors['asset_id_hex'],
            'assetType': 1,
            'status': 1,
            'custodian': CUSTODIAN,
            'stateVersion': events[-1]['stateVersion'],
            'eventSequence': len(events),
            'lastEventHash': events[-1]['eventHash'],
            'currentRfidHash': vectors['tags']['b']['hash_hex'],
            'availableWeightGrams': 450000,
        },
        'events': events,
        'custodyTransfers': [],
    }
    out = ROOT / 'test-vectors/v2-evidence-package.valid.json'
    out.write_text(json.dumps(package, indent=2) + '\n')
    print(f'wrote {out.relative_to(ROOT)}')


if __name__ == '__main__':
    main()

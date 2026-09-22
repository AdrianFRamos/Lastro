#!/usr/bin/env python3
"""Fail-closed preflight for a release/demo candidate.

This command proves only repository/deployment prerequisites that can be checked
locally. It intentionally does NOT claim that physical hardware, eFuse or Devnet
flows passed; those require the dedicated gates documented in docs/TESTING.md.
"""
from __future__ import annotations

import os
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parents[1]


def run_check(name: str, argv: list[str]) -> bool:
    completed = subprocess.run(argv, cwd=ROOT, text=True, capture_output=True)
    ok = completed.returncode == 0
    print(f"{'OK' if ok else 'FAIL'} {name}")
    if not ok:
        if completed.stdout:
            print(completed.stdout.rstrip())
        if completed.stderr:
            print(completed.stderr.rstrip(), file=sys.stderr)
    return ok


def file_check(name: str, rel: str) -> bool:
    ok = (ROOT / rel).is_file()
    print(f"{'OK' if ok else 'FAIL'} {name}: {rel}")
    return ok


def env_value(name: str) -> str:
    return os.getenv(name, '').strip()


def env_check(name: str, validator=None, expectation: str | None = None) -> bool:
    value = env_value(name)
    ok = bool(value) and (validator(value) if validator else True)
    print(f"{'OK' if ok else 'FAIL'} env {name}")
    if not ok and expectation:
        print(f'  expected: {expectation}')
    return ok


def valid_hex(value: str, byte_len: int) -> bool:
    return len(value) == byte_len * 2 and bool(re.fullmatch(r'[0-9a-fA-F]+', value))


def valid_rpc_url(value: str) -> bool:
    parsed = urlparse(value)
    return parsed.scheme in {'http', 'https'} and bool(parsed.netloc)



def declared_program_id() -> str | None:
    lib = (ROOT / 'chain/programs/lastro/src/lib.rs').read_text(encoding='utf-8')
    if 'LASTRO_PROGRAM_ID_BOOTSTRAP_MARKER' in lib:
        return None
    match = re.search(r'(?m)^\s*declare_id!\("([1-9A-HJ-NP-Za-km-z]{32,44})"\);', lib)
    return match.group(1) if match else None


def program_id_is_bootstrapped() -> bool:
    program_id = declared_program_id()
    ok = program_id is not None
    print(f"{'OK' if ok else 'FAIL'} chain program identity bootstrapped")
    return ok


def program_id_env_matches_source() -> bool:
    declared = declared_program_id()
    configured = env_value('LASTRO_PROGRAM_ID')
    ok = declared is not None and configured == declared
    print(f"{'OK' if ok else 'FAIL'} LASTRO_PROGRAM_ID matches declare_id!")
    return ok


def lockfiles_present() -> bool:
    required = [ROOT / 'Cargo.lock', ROOT / 'package-lock.json', ROOT / 'chain/Cargo.lock']
    missing = [p.name if p.parent == ROOT else str(p.relative_to(ROOT)) for p in required if not p.is_file()]
    ok = not missing
    print(f"{'OK' if ok else 'FAIL'} resolver-owned lockfiles")
    if missing:
        print('  missing: ' + ', '.join(missing))
        print('  generate them with scripts/bootstrap_dependencies.sh using the pinned toolchain')
    return ok


def evidence_files_present() -> bool:
    required = [
        'test-vectors/vectors.json',
        'test-vectors/evidence-package.valid.json',
        'test-vectors/evidence-package.tampered.json',
        'apps/web/src/pages/DemoPage.vue',
        'apps/web/src/pages/VerifyPage.vue',
    ]
    return all(file_check('required artifact', rel) for rel in required)


checks = [
    run_check('repository spec', [sys.executable, 'scripts/spec_check.py']),
    run_check('cryptographic/cross-language fixtures', [sys.executable, 'scripts/check_vectors.py', '--verify-only']),
    evidence_files_present(),
    program_id_is_bootstrapped(),
    lockfiles_present(),
]

checks.extend([
    env_check('LASTRO_PROGRAM_ID'),
    program_id_env_matches_source(),
    env_check('LASTRO_SOLANA_RPC_URL', valid_rpc_url, 'absolute http(s) RPC URL'),
    env_check('LASTRO_DEPLOYMENT_ID_HEX', lambda v: valid_hex(v, 32), '64 hex characters / 32 bytes'),
    env_check(
        'LASTRO_STATION_PUBKEY_HEX',
        lambda v: valid_hex(v, 33) and v[:2].lower() in {'02', '03'},
        '66 hex characters / compressed P-256 SEC1 point beginning 02 or 03',
    ),
    env_check('LASTRO_AGENT_TOKEN', lambda v: len(v) >= 32, 'at least 32 characters'),
])

print('\nNOT CHECKED by this command: physical RFID read, ESP32-C5 P-256 execution, eFuse, wallet signature, local validator transition, Devnet confirmation, or three-run demo stability.')
print('Use G1/H1/G2/G3/G4/G5 gates from docs/TESTING.md for those proofs.')

sys.exit(0 if all(checks) else 1)

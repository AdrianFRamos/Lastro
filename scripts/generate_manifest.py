#!/usr/bin/env python3
"""Generate/check the repository SHA-256 manifest for tracked repository files.

The manifest intentionally excludes itself. Inputs come from the Git index rather than the
working tree so ignored local secrets, caches, and other untracked files cannot make the
release-integrity manifest host-dependent.
"""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / 'MANIFEST.sha256'


def versionable_files() -> list[Path]:
    result = subprocess.run(
        ['git', 'ls-files', '-z'],
        cwd=ROOT,
        check=True,
        capture_output=True,
    )
    out: list[Path] = []
    for raw in result.stdout.split(b'\0'):
        if not raw:
            continue
        rel = Path(raw.decode('utf-8'))
        path = ROOT / rel
        if path == MANIFEST:
            continue
        if not path.is_file():
            raise RuntimeError(f'tracked manifest input is missing or not a file: {rel.as_posix()}')
        out.append(path)
    return sorted(out, key=lambda p: p.relative_to(ROOT).as_posix())


def render() -> str:
    rows: list[str] = []
    for path in versionable_files():
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        rows.append(f'{digest}  {path.relative_to(ROOT).as_posix()}')
    return '\n'.join(rows) + '\n'


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    expected = render()
    if args.check:
        if not MANIFEST.exists():
            raise SystemExit('MANIFEST.sha256 missing; run scripts/generate_manifest.py')
        if MANIFEST.read_text() != expected:
            raise SystemExit('MANIFEST.sha256 is stale; run scripts/generate_manifest.py')
        print(f'manifest: current ({len(versionable_files())} tracked files)')
        return 0
    MANIFEST.write_text(expected)
    print(f'manifest: wrote {MANIFEST.relative_to(ROOT)} ({len(versionable_files())} tracked files)')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

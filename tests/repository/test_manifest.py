from __future__ import annotations

import hashlib
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / 'MANIFEST.sha256'


def tracked_versionable_files() -> list[str]:
    result = subprocess.run(
        ['git', 'ls-files', '-z'],
        cwd=ROOT,
        check=True,
        capture_output=True,
    )
    files: list[str] = []
    for raw in result.stdout.split(b'\0'):
        if not raw:
            continue
        rel = Path(raw.decode('utf-8'))
        if rel == Path('MANIFEST.sha256'):
            continue
        path = ROOT / rel
        assert path.is_file(), f'tracked manifest input is missing or not a file: {rel.as_posix()}'
        files.append(rel.as_posix())
    return sorted(files)


def test_manifest_covers_every_tracked_file_once_and_hashes_match():
    rows = MANIFEST.read_text().splitlines()
    entries = {}
    for row in rows:
        digest, rel = row.split('  ', 1)
        assert rel not in entries
        entries[rel] = digest

    files = tracked_versionable_files()
    assert set(entries) == set(files)
    for rel, expected in entries.items():
        actual = hashlib.sha256((ROOT / rel).read_bytes()).hexdigest()
        assert actual == expected, rel


def test_manifest_generator_ignores_untracked_local_files():
    sentinel = ROOT / '.manifest-untracked-sentinel'
    assert not sentinel.exists()
    sentinel.write_text('untracked local state must not enter MANIFEST.sha256\n', encoding='utf-8')
    try:
        result = subprocess.run(
            [sys.executable, 'scripts/generate_manifest.py', '--check'],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0, result.stderr or result.stdout
    finally:
        sentinel.unlink(missing_ok=True)

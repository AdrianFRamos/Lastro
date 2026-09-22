#!/usr/bin/env python3
"""Static repository guardrails for the Lastro implementation.

This checker validates structure only. It must never claim that hardware, Solana,
P-256 or an environment-gated contract has passed.
"""
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
GENERATED_DIRS = {'.git', '.pytest_cache', '__pycache__', 'node_modules', 'target', 'build', 'dist', 'coverage', 'playwright-report', 'test-results'}

def is_generated(path: Path) -> bool:
    return any(part in GENERATED_DIRS for part in path.relative_to(ROOT).parts)

REQUIRED = [
    'Cargo.toml', 'package.json', 'requirements-dev.txt', 'pytest.ini', '.npmrc', '.prettierrc.json',
    'docs/PLAN.md', 'docs/PROTOCOL.md', 'docs/TESTING.md', 'docs/SOURCES.md',
    'schemas/openapi.yaml', 'schemas/evidence-package.schema.json',
    'crates/lastro-protocol/src/event.rs', 'crates/lastro-protocol/src/rfid.rs',
    'firmware/station/main/app_main.c',
    'services/agent/src/main.rs', 'services/api/src/main.rs',
    'chain/programs/lastro/src/lib.rs',
    'apps/web/src/main.ts', 'apps/web/src/protocol/p256.ts',
    'apps/web/src/pages/DemoPage.vue', 'apps/web/src/pages/VerifyPage.vue',
    'infra/compose.yml', '.github/workflows/ci.yml', 'scripts/generate_test_index.py', 'scripts/generate_manifest.py',
    'tests/repository/test_config_syntax.py',
    'test-vectors/origin.bin', 'test-vectors/transfer.bin', 'test-vectors/reidentify.bin',
    'test-vectors/transfer-b-to-c.bin',
    'test-vectors/evidence-package.valid.json', 'test-vectors/evidence-package.tampered.json',
]

missing = [rel for rel in REQUIRED if not (ROOT / rel).is_file()]
if missing:
    raise SystemExit('Missing required files: ' + ', '.join(missing))

empty = [p for p in ROOT.rglob('*') if p.is_file() and not is_generated(p) and p.stat().st_size == 0]
if empty:
    raise SystemExit('Empty files are forbidden: ' + ', '.join(str(p.relative_to(ROOT)) for p in empty))

# Documentation belongs only at repository root or docs/. Source/test subtrees may not
# contain README.md or explanatory Markdown used as a substitute for real stack files.
stray_md = [
    p for p in ROOT.rglob('*.md')
    if not is_generated(p) and p != ROOT / 'README.md' and ROOT / 'docs' not in p.parents
]
if stray_md:
    raise SystemExit('Markdown outside root/docs is forbidden: ' + ', '.join(str(p.relative_to(ROOT)) for p in stray_md))

# Test files must contain a real framework declaration, not prose-only pseudo tests.
test_roots = [
    ROOT / 'crates/lastro-protocol/tests', ROOT / 'chain/programs/lastro/tests',
    ROOT / 'services/agent/tests', ROOT / 'services/api/tests',
    ROOT / 'firmware/station/components/lastro_station/test', ROOT / 'firmware/station/pytest',
    ROOT / 'apps/web/tests', ROOT / 'apps/web/e2e', ROOT / 'tests/repository', ROOT / 'tests/system',
]
patterns = {
    '.rs': re.compile(r'#\[(?:tokio::)?test\]'),
    '.c': re.compile(r'\bTEST_CASE\s*\('),
    '.ts': re.compile(r'\b(?:it\.todo|it\.skip|test\.skip|it|test)\s*\('),
    '.py': re.compile(r'(?m)^def\s+test_[A-Za-z0-9_]+\s*\('),
}
for base in test_roots:
    for path in base.rglob('*'):
        if not path.is_file() or path.name in {'CMakeLists.txt', 'requirements.txt', 'setup.ts', '__init__.py'}:
            continue
        if path.suffix == '.py' and not path.name.startswith('test_'):
            continue
        if path.suffix == '.ts' and not (path.name.endswith('.test.ts') or path.name.endswith('.spec.ts')):
            continue
        if path.name == 'mod.rs' and path.parent.name == 'common':
            continue
        pat = patterns.get(path.suffix)
        if pat and not pat.search(path.read_text(errors='ignore')):
            raise SystemExit(f'Test file has no framework test declaration: {path.relative_to(ROOT)}')

# Prevent the Makefile from forcing incomplete ignored contracts to run.
makefile = (ROOT / 'Makefile').read_text()
if '--include-ignored' in makefile:
    raise SystemExit('Makefile must not force ignored implementation contracts to execute')

# Freeze cross-layer constants that are already protocol decisions.
checks = {
    '276': [
        'docs/PROTOCOL.md', 'crates/lastro-protocol/src/constants.rs',
        'firmware/station/components/lastro_station/include/lastro_station/event.h',
        'apps/web/src/protocol/constants.ts',
    ],
    '8': [
        'docs/PROTOCOL.md', 'crates/lastro-protocol/src/constants.rs',
        'firmware/station/components/lastro_station/include/lastro_station/rfid.h',
        'apps/web/src/protocol/constants.ts',
    ],
}
for needle, files in checks.items():
    for rel in files:
        if needle not in (ROOT / rel).read_text():
            raise SystemExit(f'Expected protocol constant {needle!r} missing from {rel}')


# TEST_INDEX is generated from real declarations and must never drift.
subprocess.run([sys.executable, str(ROOT / 'scripts/generate_test_index.py'), '--check'], check=True, cwd=ROOT)
subprocess.run([sys.executable, str(ROOT / 'scripts/generate_manifest.py'), '--check'], check=True, cwd=ROOT)

print('spec-check: repository structure and implementation guardrails OK')

"""Meta-tests that protect the quality of pending test contracts.

The repository intentionally contains ignored/todo tests before implementation. Their value comes
from precise executable intent, so every domain test must say what it proves and what failure means.
"""
from __future__ import annotations

import ast
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]


def test_rust_contracts_define_purpose_assertion_and_failure_semantics():
    """Every Rust domain/chain test contract carries reviewable intent, not only a test name."""
    roots = [
        ROOT / 'crates/lastro-protocol/tests',
        ROOT / 'services/agent/tests',
        ROOT / 'services/api/tests',
        ROOT / 'chain/programs/lastro/tests',
    ]
    missing: list[str] = []
    declaration = re.compile(r'#\[(?:tokio::)?test\]')
    for base in roots:
        for path in sorted(base.glob('*.rs')):
            text = path.read_text(encoding='utf-8')
            starts = list(declaration.finditer(text))
            for index, start in enumerate(starts):
                end = starts[index + 1].start() if index + 1 < len(starts) else len(text)
                block = text[start.start():end]
                name_match = re.search(r'fn\s+([A-Za-z0-9_]+)', block)
                name = name_match.group(1) if name_match else '<unknown>'
                upper = block.upper()
                for key in ('PURPOSE', 'ASSERT', 'FAILURE'):
                    if key not in upper:
                        missing.append(f'{path.relative_to(ROOT)}::{name} missing {key}')
    assert not missing, '\n'.join(missing)


def test_unity_contracts_define_purpose_assertion_and_failure_semantics():
    """Every ESP-IDF Unity case documents the hardware/protocol invariant it protects."""
    base = ROOT / 'firmware/station/components/lastro_station/test'
    missing: list[str] = []
    declaration = re.compile(r'TEST_CASE\s*\(\s*"([^"]+)"')
    for path in sorted(base.glob('*.c')):
        text = path.read_text(encoding='utf-8')
        starts = list(declaration.finditer(text))
        for index, start in enumerate(starts):
            end = starts[index + 1].start() if index + 1 < len(starts) else len(text)
            block = text[start.start():end].upper()
            for key in ('PURPOSE', 'ASSERT', 'FAILURE'):
                if key not in block:
                    missing.append(f'{path.relative_to(ROOT)}::{start.group(1)} missing {key}')
    assert not missing, '\n'.join(missing)


def test_browser_contracts_define_arrange_action_assert_and_failure_semantics():
    """Every Vitest/Playwright contract is specific enough to implement without guessing."""
    files = sorted((ROOT / 'apps/web/tests').rglob('*.test.ts')) + sorted((ROOT / 'apps/web/e2e').rglob('*.spec.ts'))
    declaration = re.compile(r'\b(?:it\.todo|it\.skip|test\.skip|it|test)\s*\(\s*[\'\"]([^\'\"]+)')
    missing: list[str] = []
    for path in files:
        text = path.read_text(encoding='utf-8')
        starts = list(declaration.finditer(text))
        for index, start in enumerate(starts):
            previous = starts[index - 1].start() if index else 0
            doc_start = text.rfind('/**', previous, start.start())
            segment_start = doc_start if doc_start != -1 else start.start()
            end = starts[index + 1].start() if index + 1 < len(starts) else len(text)
            block = text[segment_start:end].upper()
            for key in ('ARRANGE', 'ACTION', 'ASSERT', 'FAILURE MEANS'):
                if key not in block:
                    missing.append(f'{path.relative_to(ROOT)}::{start.group(1)} missing {key}')
    assert not missing, '\n'.join(missing)


def test_hardware_and_system_pytest_contracts_define_full_execution_semantics():
    """External gates must state the environment, action, expected evidence and failure meaning."""
    files = sorted((ROOT / 'tests/system').glob('test_*.py')) + sorted((ROOT / 'firmware/station/pytest').glob('test_*.py'))
    missing: list[str] = []
    for path in files:
        tree = ast.parse(path.read_text(encoding='utf-8'))
        for node in tree.body:
            if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) or not node.name.startswith('test_'):
                continue
            doc = (ast.get_docstring(node) or '').upper()
            for key in ('PURPOSE', 'ARRANGE', 'ACTION', 'ASSERT', 'FAILURE MEANS'):
                if key not in doc:
                    missing.append(f'{path.relative_to(ROOT)}::{node.name} missing {key}')
    assert not missing, '\n'.join(missing)


def test_generated_test_index_has_no_trailing_whitespace():
    """Generated documentation must remain clean under git diff --check."""
    import importlib.util
    import sys

    script = ROOT / 'scripts/generate_test_index.py'
    spec = importlib.util.spec_from_file_location('generate_test_index', script)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    try:
        spec.loader.exec_module(module)
    finally:
        sys.modules.pop(spec.name, None)
    rendered = module.render(module.collect())
    offenders = [
        (index, line)
        for index, line in enumerate(rendered.splitlines(), start=1)
        if line != line.rstrip()
    ]
    assert not offenders, offenders

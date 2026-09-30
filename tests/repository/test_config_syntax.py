"""Repository-wide syntax checks for non-compiled configuration/source helpers.

These tests do not replace Rust/TypeScript/C compilers. They guarantee that every
machine-readable config committed in the repository can at least be parsed by its
native parser and that shell/Python helper scripts are syntactically valid.
"""
from __future__ import annotations

import json
from pathlib import Path
import py_compile
import re
import subprocess
import sys
import tomllib

import yaml

ROOT = Path(__file__).resolve().parents[2]
GENERATED = {'.git', '.pytest_cache', '__pycache__', 'node_modules', 'target', 'build', 'dist', 'coverage', 'playwright-report', 'test-results'}


def versionable_files(suffixes: set[str]):
    for path in ROOT.rglob('*'):
        if not path.is_file() or path.suffix not in suffixes:
            continue
        rel = path.relative_to(ROOT)
        if any(part in GENERATED for part in rel.parts):
            continue
        yield path


def test_every_json_file_parses_with_standard_json_parser():
    for path in versionable_files({'.json'}):
        try:
            json.loads(path.read_text(encoding='utf-8'))
        except Exception as exc:  # pragma: no cover - assertion gives filename
            raise AssertionError(f'invalid JSON: {path.relative_to(ROOT)}: {exc}') from exc


def test_every_toml_file_parses_with_python_tomllib():
    for path in versionable_files({'.toml'}):
        try:
            tomllib.loads(path.read_text(encoding='utf-8'))
        except Exception as exc:  # pragma: no cover
            raise AssertionError(f'invalid TOML: {path.relative_to(ROOT)}: {exc}') from exc


def test_every_yaml_file_parses_with_pyyaml():
    for path in versionable_files({'.yaml', '.yml'}):
        try:
            yaml.safe_load(path.read_text(encoding='utf-8'))
        except Exception as exc:  # pragma: no cover
            raise AssertionError(f'invalid YAML: {path.relative_to(ROOT)}: {exc}') from exc


def test_every_shell_script_passes_bash_syntax_check():
    for path in versionable_files({'.sh'}):
        rel = path.relative_to(ROOT).as_posix()
        result = subprocess.run(['bash', '-n', rel], cwd=ROOT, capture_output=True, text=True)
        assert result.returncode == 0, f'{path.relative_to(ROOT)}: {result.stderr}'


def test_every_python_file_compiles_without_writing_bytecode_into_repo(tmp_path):
    for path in versionable_files({'.py'}):
        target = tmp_path / (str(path.relative_to(ROOT)).replace('/', '__') + '.pyc')
        py_compile.compile(str(path), cfile=str(target), doraise=True)


def test_github_workflow_python_heredocs_compile():
    heredoc = re.compile(
        r"(?ms)^python(?:3)?\s+-\s+<<'(?P<tag>[A-Z][A-Z0-9_]*)'\s*\n(?P<body>.*?)^(?P=tag)\s*$"
    )
    checked = 0
    for path in sorted((ROOT / '.github' / 'workflows').glob('*.yml')):
        workflow = yaml.safe_load(path.read_text(encoding='utf-8'))
        for job_name, job in (workflow.get('jobs') or {}).items():
            for step in job.get('steps', []):
                run = step.get('run')
                if not isinstance(run, str):
                    continue
                for match in heredoc.finditer(run):
                    checked += 1
                    try:
                        compile(match.group('body'), f'{path.relative_to(ROOT)}::{job_name}', 'exec')
                    except SyntaxError as exc:
                        raise AssertionError(
                            f'invalid embedded Python in {path.relative_to(ROOT)}::{job_name}: {exc}'
                        ) from exc
    assert checked >= 2, 'expected embedded Python heredocs in CI workflows'


def test_initialize_protocol_config_cli_loads_from_repository_root():
    result = subprocess.run(
        [sys.executable, 'scripts/initialize_protocol_config.py', '--help'],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr

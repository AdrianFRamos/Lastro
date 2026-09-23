#!/usr/bin/env python3
"""Generate docs/TEST_INDEX.md from real test declarations in the repository.

The index is derived; test files remain the source of truth. This script never marks a
contract as passing. It only inventories names and locations so review can detect missing
coverage or orphan test files.
"""
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import argparse
import re

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "docs" / "TEST_INDEX.md"

TEST_DIR_NAMES = {"tests", "test", "e2e", "pytest"}
SKIP_FILES = {"setup.ts", "CMakeLists.txt", "requirements.txt"}


@dataclass(frozen=True)
class Case:
    path: Path
    line: int
    name: str


def is_test_path(path: Path) -> bool:
    if any(part in {"node_modules", ".git", "target", "build", "dist", ".venv", "venv"} for part in path.parts):
        return False
    return any(part in TEST_DIR_NAMES for part in path.parts) and path.name not in SKIP_FILES


def rust_cases(path: Path, text: str) -> list[Case]:
    lines = text.splitlines()
    out: list[Case] = []
    for i, line in enumerate(lines):
        match = re.search(r"\b(?:async\s+)?fn\s+([A-Za-z0-9_]+)\s*\(", line)
        if not match:
            continue
        attrs = lines[max(0, i - 5):i]
        if any("#[test]" in attr or "#[tokio::test]" in attr for attr in attrs):
            out.append(Case(path, i + 1, match.group(1)))
    return out


def c_cases(path: Path, text: str) -> list[Case]:
    out = []
    for m in re.finditer(r'TEST_CASE\(\s*"([^"]+)"', text):
        out.append(Case(path, text.count("\n", 0, m.start()) + 1, m.group(1)))
    return out


def ts_cases(path: Path, text: str) -> list[Case]:
    pattern = re.compile(r"\b(?:it\.todo|it\.skip|test\.skip|it|test)\(\s*['\"]([^'\"]+)['\"]")
    return [Case(path, text.count("\n", 0, m.start()) + 1, m.group(1)) for m in pattern.finditer(text)]


def py_cases(path: Path, text: str) -> list[Case]:
    pattern = re.compile(r"(?m)^def\s+(test_[A-Za-z0-9_]+)\s*\(")
    return [Case(path, text.count("\n", 0, m.start()) + 1, m.group(1)) for m in pattern.finditer(text)]


def collect() -> list[Case]:
    out: list[Case] = []
    for path in sorted(ROOT.rglob("*")):
        if not path.is_file() or not is_test_path(path.relative_to(ROOT)):
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        if path.suffix == ".rs":
            out.extend(rust_cases(path, text))
        elif path.suffix == ".c":
            out.extend(c_cases(path, text))
        elif path.suffix == ".ts":
            out.extend(ts_cases(path, text))
        elif path.suffix == ".py":
            out.extend(py_cases(path, text))
    return out


def render(cases: list[Case]) -> str:
    by_file: dict[Path, list[Case]] = {}
    for case in cases:
        by_file.setdefault(case.path, []).append(case)

    lines = [
        "# Complete Test Index",
        "",
        "This file is generated from the repository's real test declarations. Test files are the source of truth; this index supports coverage review and navigation.",
        "",
        f"**Declared cases:** {len(cases)}",
        f"**Files containing test cases:** {len(by_file)}",
        "",
        "Ignored or environment-gated cases are not proven by declaration alone; only an executed non-skipped run counts as validation evidence.",
        "",
    ]
    for path, file_cases in sorted(by_file.items(), key=lambda item: str(item[0])):
        rel = path.relative_to(ROOT).as_posix()
        lines.append(f"## `{rel}`")
        lines.append("")
        for case in file_cases:
            lines.append(f"- L{case.line}: `{case.name}`")
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail if docs/TEST_INDEX.md is stale")
    args = parser.parse_args()
    rendered = render(collect())
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_text(encoding="utf-8") != rendered:
            raise SystemExit("docs/TEST_INDEX.md is stale; run python scripts/generate_test_index.py")
        print("test-index: current")
        return
    OUTPUT.write_text(rendered, encoding="utf-8")
    print(f"test-index: wrote {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()

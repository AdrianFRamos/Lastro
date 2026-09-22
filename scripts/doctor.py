#!/usr/bin/env python3
"""Read-only, fail-closed toolchain diagnostic for the Lastro hackathon repository.

Presence is not enough: build/test results are comparable only when the frozen toolchain
matches. Rust, Node, npm, Anchor, Solana and ESP-IDF are validated against repository decisions.
Docker is reported because its exact patch is not protocol-significant.
"""
from __future__ import annotations

import re
import shutil
import subprocess
import sys
from dataclasses import dataclass


@dataclass(frozen=True)
class Tool:
    command: tuple[str, ...]
    expected: re.Pattern[str] | None = None
    expectation: str | None = None


TOOLS = {
    'rustc': Tool(('rustc', '--version'), re.compile(r'^rustc 1\.98\.1\b'), 'rustc 1.98.1'),
    'cargo': Tool(('cargo', '--version')),  # Cargo is coupled to the pinned Rust toolchain.
    'node': Tool(('node', '--version'), re.compile(r'^v24\.21\.0$'), 'Node v24.21.0'),
    'npm': Tool(('npm', '--version'), re.compile(r'^11\.19\.0$'), 'npm 11.19.0'),
    'anchor': Tool(('anchor', '--version'), re.compile(r'\b1\.2\.0\b'), 'Anchor CLI 1.2.0'),
    'solana': Tool(('solana', '--version'), re.compile(r'\b4\.1\.2\b'), 'Solana CLI 4.1.2'),
    'idf.py': Tool(('idf.py', '--version'), re.compile(r'\bv?6\.1(?:\.|\b)'), 'ESP-IDF 6.1.x'),
    'docker': Tool(('docker', '--version')),
}


def run(tool: Tool) -> tuple[int, str]:
    proc = subprocess.run(tool.command, capture_output=True, text=True, check=False)
    output = (proc.stdout or proc.stderr).strip()
    return proc.returncode, output


def main() -> int:
    failed = False
    if sys.version_info[:2] != (3, 13):
        print(f'MISMATCH python: {sys.version.split()[0]} (expected Python 3.13.x)')
        failed = True
    else:
        print(f'python: {sys.version.split()[0]}')

    for name, tool in TOOLS.items():
        executable = tool.command[0]
        if not shutil.which(executable):
            print(f'MISSING {name}')
            failed = True
            continue
        returncode, output = run(tool)
        if returncode != 0:
            print(f'ERROR {name}: {output}')
            failed = True
            continue
        if tool.expected and not tool.expected.search(output):
            print(f'MISMATCH {name}: {output} (expected {tool.expectation})')
            failed = True
            continue
        print(f'{name}: {output}')

    return 1 if failed else 0


if __name__ == '__main__':
    raise SystemExit(main())

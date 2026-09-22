"""Deterministic high-confidence secret-hygiene checks for the tracked repository tree."""
from __future__ import annotations

from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]

FORBIDDEN_TRACKED_NAMES = {
    "id_rsa",
    "id_ecdsa",
    "id_ed25519",
}
FORBIDDEN_TRACKED_SUFFIXES = (
    ".pem",
    ".p12",
    ".pfx",
)
SECRET_PATTERNS = {
    "private-key PEM": re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
    "GitHub token": re.compile(r"\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{50,})\b"),
    "AWS access key": re.compile(r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b"),
    "Slack token": re.compile(r"\bxox[baprs]-[A-Za-z0-9-]{20,}\b"),
    "Stripe live secret": re.compile(r"\bsk_live_[A-Za-z0-9]{16,}\b"),
    "npm token": re.compile(r"\bnpm_[A-Za-z0-9]{30,}\b"),
    "Google API key": re.compile(r"\bAIza[0-9A-Za-z_-]{35}\b"),
    "OpenAI-style secret": re.compile(r"\bsk-[A-Za-z0-9_-]{20,}\b"),
}


def tracked_files() -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=ROOT,
        check=True,
        capture_output=True,
    )
    return [ROOT / item.decode() for item in result.stdout.split(b"\0") if item]


def test_no_high_confidence_secret_material_is_tracked():
    """Current tracked source must contain no recognized credential or private-key material."""
    findings: list[str] = []
    for path in tracked_files():
        relative = path.relative_to(ROOT).as_posix()
        lowered_name = path.name.lower()
        if lowered_name in FORBIDDEN_TRACKED_NAMES or lowered_name.endswith(FORBIDDEN_TRACKED_SUFFIXES):
            findings.append(f"{relative}: secret-bearing filename is not allowed")
            continue
        if lowered_name.endswith("-keypair.json"):
            findings.append(f"{relative}: Solana keypair files must never be tracked")
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        for label, pattern in SECRET_PATTERNS.items():
            if pattern.search(text):
                findings.append(f"{relative}: matched {label}")
    assert not findings, "\n".join(findings)


def test_public_vite_configuration_names_cannot_be_secret_bearing():
    """VITE-prefixed browser variables are public build inputs and may not be named as credentials."""
    findings: list[str] = []
    public_name = re.compile(r"\bVITE_[A-Z0-9_]+\b")
    for path in tracked_files():
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        for name in public_name.findall(text):
            if any(marker in name for marker in ("SECRET", "PRIVATE", "TOKEN", "MNEMONIC", "SEED")):
                findings.append(f"{path.relative_to(ROOT).as_posix()}: unsafe public variable {name}")
    assert not findings, "\n".join(findings)


def test_private_runtime_material_is_gitignored():
    """Common local secret/key artifacts must remain excluded from accidental commits."""
    gitignore = (ROOT / ".gitignore").read_text(encoding="utf-8")
    required = (
        ".env",
        ".env.*",
        "**/*-keypair.json",
    )
    for pattern in required:
        assert pattern in gitignore, f".gitignore must keep {pattern} out of source control"

def test_no_high_confidence_secret_material_exists_in_reachable_history():
    """Reachable Git history must not retain recognized credential or private-key material."""
    shallow = subprocess.run(
        ["git", "rev-parse", "--is-shallow-repository"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    assert shallow == "false", "history secret audit requires a complete non-shallow clone"

    names = subprocess.run(
        ["git", "log", "--all", "--format=", "--name-only"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
        errors="replace",
    ).stdout.splitlines()
    forbidden_names = [
        name
        for name in names
        if name
        and (
            Path(name).name.lower() in FORBIDDEN_TRACKED_NAMES
            or Path(name).name.lower().endswith(FORBIDDEN_TRACKED_SUFFIXES)
            or Path(name).name.lower().endswith("-keypair.json")
        )
    ]

    history = subprocess.run(
        ["git", "log", "--all", "--format=", "--patch", "--no-ext-diff"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
        errors="replace",
    ).stdout
    findings = [
        f"reachable Git history matched {label}"
        for label, pattern in SECRET_PATTERNS.items()
        if pattern.search(history)
    ]
    findings.extend(f"reachable Git history contains secret-bearing path {name}" for name in forbidden_names)
    assert not findings, "\n".join(findings)


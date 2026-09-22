#!/usr/bin/env python3
"""Isolated local-development orchestration for Lastro.

This tool keeps local-only identities, validator state, logs, and generated environment files
under .lastro-local/. Production source remains environment-agnostic: the API still consumes a
configured Solana RPC URL, the Agent still consumes its Station transport, and the web app still
consumes Wallet Standard wallets.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.request
from typing import Iterator, Sequence

ROOT = Path(__file__).resolve().parents[1]
STATE_DIR = ROOT / ".lastro-local"
WALLETS_DIR = STATE_DIR / "wallets"
LEDGER_DIR = STATE_DIR / "ledger"
LOG_DIR = STATE_DIR / "logs"
ARTIFACT_DIR = STATE_DIR / "artifacts"
ENV_FILE = STATE_DIR / "local.env"
SECRETS_FILE = STATE_DIR / "secrets.json"
PROGRAM_KEYPAIR = STATE_DIR / "program-keypair.json"
VALIDATOR_PID_FILE = STATE_DIR / "validator.pid"
VALIDATOR_LOG = LOG_DIR / "solana-test-validator.log"
CHAIN_LIB = ROOT / "chain/programs/lastro/src/lib.rs"
ANCHOR_TOML = ROOT / "chain/Anchor.toml"
CHAIN_TARGET_KEYPAIR = ROOT / "chain/target/deploy/lastro-keypair.json"
PROGRAM_SO = ROOT / "chain/target/deploy/lastro.so"
COMPOSE_FILE = ROOT / "infra/compose.yml"
COMPOSE_OVERRIDE = ROOT / "dev-environment/localnet/compose.override.yml"

HOST_RPC_URL = "http://127.0.0.1:8899"
CONTAINER_RPC_URL = "http://host.docker.internal:8899"
API_URL = "http://127.0.0.1:8080"
WEB_URL = "http://127.0.0.1:8088"
SIMULATOR_URL = "http://127.0.0.1:8090"
DEPLOYMENT_ID_HEX = "d0" * 32
STATION_PRIVATE_SCALAR_HEX = "0" * 63 + "1"
STATION_PUBKEY_HEX = "036b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
PROGRAM_MARKER = "// LASTRO_PROGRAM_ID_BOOTSTRAP_MARKER"
PROGRAM_DECLARE = re.compile(r'(?m)^[ \t]*declare_id!\("[1-9A-HJ-NP-Za-km-z]{32,44}"\);[ \t]*$')
PROJECT_NAME = "lastro-local"

WALLET_PATHS = {
    "A": WALLETS_DIR / "wallet-a-keypair.json",
    "B": WALLETS_DIR / "wallet-b-keypair.json",
    "C": WALLETS_DIR / "wallet-c-keypair.json",
}


class LocalDevError(RuntimeError):
    """Actionable local-development failure."""


def _display_command(command: Sequence[str]) -> str:
    return " ".join(command)


def run(
    command: Sequence[str],
    *,
    cwd: Path = ROOT,
    env: dict[str, str] | None = None,
    capture: bool = False,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    process = subprocess.run(
        list(command),
        cwd=cwd,
        env=env,
        text=True,
        capture_output=capture,
        check=False,
    )
    if check and process.returncode != 0:
        detail = (process.stderr or process.stdout or "").strip()
        suffix = f"\n{detail}" if detail else ""
        raise LocalDevError(f"command failed ({process.returncode}): {_display_command(command)}{suffix}")
    return process


def capture(command: Sequence[str], *, cwd: Path = ROOT) -> str:
    return run(command, cwd=cwd, capture=True).stdout.strip()


def require_command(name: str) -> None:
    if shutil.which(name) is None:
        raise LocalDevError(f"required command is missing: {name}")


def require_python_modules(*, for_tests: bool) -> None:
    modules = ["cryptography"] + (["pytest"] if for_tests else [])
    for module in modules:
        try:
            __import__(module)
        except ModuleNotFoundError as error:
            raise LocalDevError(
                f"missing Python module {module}; run: python3 -m pip install -r requirements-dev.txt"
            ) from error


def _require_version(command: Sequence[str], pattern: str, expected: str) -> str:
    output = capture(command)
    if re.search(pattern, output) is None:
        raise LocalDevError(f"toolchain mismatch for {command[0]}: {output!r}; expected {expected}")
    return output


def doctor(*, for_tests: bool = False) -> None:
    if os.name != "posix":
        raise LocalDevError("the local Station/Agent test path requires a POSIX environment; use Linux, macOS, or WSL2")
    if sys.version_info[:2] != (3, 13):
        raise LocalDevError(f"Python {sys.version.split()[0]} is active; Lastro local development requires Python 3.13.x")

    for tool in ("solana", "solana-keygen", "solana-test-validator", "anchor", "cargo", "rustc", "docker"):
        require_command(tool)
    _require_version(("solana", "--version"), r"\b4\.1\.2\b", "Solana CLI 4.1.2")
    _require_version(("solana-test-validator", "--version"), r"\b4\.1\.2\b", "solana-test-validator 4.1.2")
    _require_version(("anchor", "--version"), r"\b1\.2\.0\b", "Anchor CLI 1.2.0")
    _require_version(("rustc", "--version"), r"^rustc 1\.98\.1\b", "rustc 1.98.1")
    run(("docker", "compose", "version"), capture=True)
    require_python_modules(for_tests=for_tests)

    if for_tests:
        require_command("node")
        require_command("npm")
        _require_version(("node", "--version"), r"^v24\.21\.0$", "Node v24.21.0")
        _require_version(("npm", "--version"), r"^11\.19\.0$", "npm 11.19.0")
        if not (ROOT / "node_modules").is_dir():
            raise LocalDevError("node_modules is missing; run: npm ci")

    print("local development toolchain: OK")


def ensure_state_dirs() -> None:
    for path in (STATE_DIR, WALLETS_DIR, LOG_DIR, ARTIFACT_DIR):
        path.mkdir(parents=True, exist_ok=True)


def _generate_keypair(path: Path) -> None:
    if path.exists():
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    process = run(
        ("solana-keygen", "new", "--outfile", str(path), "--no-bip39-passphrase", "--force"),
        capture=True,
    )
    if process.returncode != 0:
        raise LocalDevError(f"failed to generate local test keypair: {path}")
    path.chmod(0o600)


def keypair_pubkey(path: Path) -> str:
    if not path.is_file():
        raise LocalDevError(f"local test keypair is missing: {path}; run local-demo-init")
    return capture(("solana-keygen", "pubkey", str(path)))


def _load_or_create_agent_token() -> str:
    if SECRETS_FILE.exists():
        try:
            data = json.loads(SECRETS_FILE.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise LocalDevError(f"cannot parse {SECRETS_FILE}") from error
        token = data.get("agentToken") if isinstance(data, dict) else None
        if not isinstance(token, str) or len(token) < 32:
            raise LocalDevError(f"{SECRETS_FILE} does not contain a valid local Agent token")
        return token
    token = secrets.token_urlsafe(32)
    SECRETS_FILE.write_text(json.dumps({"agentToken": token}, separators=(",", ":")) + "\n", encoding="utf-8")
    SECRETS_FILE.chmod(0o600)
    return token


def _env_value(value: str) -> str:
    # Compose env files and POSIX shells both accept single-quoted literal values.
    return "'" + value.replace("'", "'\"'\"'") + "'"


def local_env_values(*, program_id: str, agent_token: str) -> dict[str, str]:
    return {
        "LASTRO_SOLANA_RPC_URL": CONTAINER_RPC_URL,
        "LASTRO_PROGRAM_ID": program_id,
        "LASTRO_DEPLOYMENT_ID_HEX": DEPLOYMENT_ID_HEX,
        "LASTRO_STATION_PUBKEY_HEX": STATION_PUBKEY_HEX,
        "LASTRO_AGENT_TOKEN": agent_token,
        "VITE_API_BASE_URL": API_URL,
        "VITE_SOLANA_RPC_URL": HOST_RPC_URL,
        "VITE_SOLANA_CHAIN": "solana:localnet",
        "VITE_LASTRO_PROGRAM_ID": program_id,
        "LASTRO_SYSTEM_API_URL": API_URL,
        "LASTRO_SYSTEM_SOLANA_RPC_URL": HOST_RPC_URL,
        "LASTRO_SYSTEM_AGENT_BIN": str((ROOT / "target/debug/lastro-agent").resolve()),
        "LASTRO_SYSTEM_STATION_PRIVATE_SCALAR_HEX": STATION_PRIVATE_SCALAR_HEX,
        "LASTRO_SYSTEM_WALLET_A_KEYPAIR": str(WALLET_PATHS["A"].resolve()),
        "LASTRO_SYSTEM_WALLET_B_KEYPAIR": str(WALLET_PATHS["B"].resolve()),
        "LASTRO_SYSTEM_WALLET_C_KEYPAIR": str(WALLET_PATHS["C"].resolve()),
        "LASTRO_SYSTEM_TEST": "1",
        "LASTRO_E2E_SYSTEM": "1",
        "LASTRO_E2E_SOLANA_CHAIN": "solana:localnet",
        "RUST_LOG": "info,lastro_api=debug,lastro_agent=debug",
    }


def render_local_env(*, program_id: str, agent_token: str) -> str:
    values = local_env_values(program_id=program_id, agent_token=agent_token)
    header = [
        "# Generated by scripts/local_dev.py. Test-only; do not commit.",
        "# LASTRO_SOLANA_RPC_URL is container-facing; browser/system tests use 127.0.0.1:8899.",
    ]
    return "\n".join(header + [f"{key}={_env_value(value)}" for key, value in values.items()]) + "\n"


def compose_process_env(base_env: dict[str, str] | None = None) -> dict[str, str]:
    """Return a deterministic Compose environment that cannot inherit deployment RPC/settings.

    Docker Compose gives the invoking shell higher interpolation precedence than --env-file.
    Local orchestration therefore strips Lastro/Vite/Compose overrides from the parent process and
    injects the generated local values explicitly. This prevents an exported Devnet/Mainnet RPC
    or program ID from escaping into a supposedly local run.
    """
    require_initialized()
    source = dict(os.environ if base_env is None else base_env)
    for key in list(source):
        if key.startswith(("LASTRO_", "VITE_", "COMPOSE_")):
            source.pop(key)
    token = json.loads(SECRETS_FILE.read_text(encoding="utf-8"))["agentToken"]
    source.update(local_env_values(program_id=keypair_pubkey(PROGRAM_KEYPAIR), agent_token=token))
    return source


def init_environment() -> None:
    require_command("solana-keygen")
    ensure_state_dirs()
    _generate_keypair(PROGRAM_KEYPAIR)
    for path in WALLET_PATHS.values():
        _generate_keypair(path)
    program_id = keypair_pubkey(PROGRAM_KEYPAIR)
    token = _load_or_create_agent_token()
    ENV_FILE.write_text(render_local_env(program_id=program_id, agent_token=token), encoding="utf-8")
    ENV_FILE.chmod(0o600)

    print(f"local state: {STATE_DIR.relative_to(ROOT)}")
    print(f"program ID: {program_id}")
    for label, path in WALLET_PATHS.items():
        print(f"Wallet {label}: {keypair_pubkey(path)}")
    print(f"environment: {ENV_FILE.relative_to(ROOT)}")


def require_initialized() -> None:
    required = [PROGRAM_KEYPAIR, ENV_FILE, SECRETS_FILE, *WALLET_PATHS.values()]
    missing = [path.relative_to(ROOT).as_posix() for path in required if not path.is_file()]
    if missing:
        raise LocalDevError(f"local environment is not initialized ({', '.join(missing)}); run: make local-demo-init")


@contextmanager
def temporary_program_identity(source: Path, program_id: str) -> Iterator[None]:
    original = source.read_text(encoding="utf-8")
    replacement = f'declare_id!("{program_id}");'
    if PROGRAM_MARKER in original:
        patched = original.replace(PROGRAM_MARKER, replacement, 1)
    else:
        matches = list(PROGRAM_DECLARE.finditer(original))
        if len(matches) != 1:
            raise LocalDevError("program source must contain exactly one bootstrap marker or declare_id! for a local build")
        patched = PROGRAM_DECLARE.sub(replacement, original, count=1)
    source.write_text(patched, encoding="utf-8")
    try:
        yield
    finally:
        source.write_text(original, encoding="utf-8")


def build_local_program() -> str:
    require_initialized()
    program_id = keypair_pubkey(PROGRAM_KEYPAIR)
    if CHAIN_TARGET_KEYPAIR.exists():
        raise LocalDevError(
            f"refusing to overwrite existing Anchor deploy keypair {CHAIN_TARGET_KEYPAIR.relative_to(ROOT)}; "
            "move/back up that identity before building the disposable local program"
        )

    anchor_original = ANCHOR_TOML.read_bytes()
    CHAIN_TARGET_KEYPAIR.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(PROGRAM_KEYPAIR, CHAIN_TARGET_KEYPAIR)
    CHAIN_TARGET_KEYPAIR.chmod(0o600)
    try:
        with temporary_program_identity(CHAIN_LIB, program_id):
            run(("anchor", "keys", "sync"), cwd=ROOT / "chain")
            run(("anchor", "build", "--ignore-keys"), cwd=ROOT / "chain")
    finally:
        # anchor keys sync is permitted to rewrite Anchor.toml for the temporary local identity.
        ANCHOR_TOML.write_bytes(anchor_original)
        CHAIN_TARGET_KEYPAIR.unlink(missing_ok=True)

    if not PROGRAM_SO.is_file():
        raise LocalDevError(f"Anchor build did not produce {PROGRAM_SO.relative_to(ROOT)}")
    return program_id


def _pid_alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def _read_validator_pid() -> int | None:
    if not VALIDATOR_PID_FILE.exists():
        return None
    try:
        return int(VALIDATOR_PID_FILE.read_text(encoding="utf-8").strip())
    except (OSError, ValueError):
        return None


def rpc_ready() -> bool:
    try:
        process = run(("solana", "cluster-version", "--url", HOST_RPC_URL), capture=True, check=False)
    except OSError:
        return False
    return process.returncode == 0


def validator_command() -> tuple[str, ...]:
    return (
        "solana-test-validator",
        "--ledger",
        str(LEDGER_DIR),
        "--rpc-port",
        "8899",
        "--bind-address",
        "0.0.0.0",
    )


def start_validator() -> None:
    ensure_state_dirs()
    pid = _read_validator_pid()
    if pid is not None and _pid_alive(pid) and rpc_ready():
        print(f"local validator already running (pid {pid})")
        return
    if rpc_ready():
        raise LocalDevError(
            f"{HOST_RPC_URL} already responds but is not owned by {VALIDATOR_PID_FILE.relative_to(ROOT)}; "
            "stop the external validator before starting Lastro local development"
        )
    VALIDATOR_PID_FILE.unlink(missing_ok=True)
    LEDGER_DIR.mkdir(parents=True, exist_ok=True)
    VALIDATOR_LOG.parent.mkdir(parents=True, exist_ok=True)
    log_handle = VALIDATOR_LOG.open("ab")
    try:
        process = subprocess.Popen(
            validator_command(),
            cwd=ROOT,
            stdout=log_handle,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
    finally:
        log_handle.close()
    VALIDATOR_PID_FILE.write_text(f"{process.pid}\n", encoding="utf-8")

    deadline = time.monotonic() + 45
    while time.monotonic() < deadline:
        if process.poll() is not None:
            tail = VALIDATOR_LOG.read_text(encoding="utf-8", errors="replace")[-5000:]
            raise LocalDevError(f"solana-test-validator exited with status {process.returncode}:\n{tail}")
        if rpc_ready():
            print(f"local validator ready: {HOST_RPC_URL} (pid {process.pid})")
            return
        time.sleep(0.5)
    raise LocalDevError(f"local validator did not become ready; inspect {VALIDATOR_LOG.relative_to(ROOT)}")


def stop_validator() -> None:
    pid = _read_validator_pid()
    if pid is None:
        VALIDATOR_PID_FILE.unlink(missing_ok=True)
        return
    if _pid_alive(pid):
        try:
            os.killpg(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline and _pid_alive(pid):
            time.sleep(0.1)
        if _pid_alive(pid):
            try:
                os.killpg(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
    VALIDATOR_PID_FILE.unlink(missing_ok=True)
    print("local validator stopped")


def fund_wallets() -> None:
    for path in WALLET_PATHS.values():
        address = keypair_pubkey(path)
        run(("solana", "airdrop", "20", address, "--url", HOST_RPC_URL))


def prepare_chain() -> str:
    require_initialized()
    start_validator()
    program_id = build_local_program()
    fund_wallets()
    run(
        (
            "solana",
            "program",
            "deploy",
            str(PROGRAM_SO),
            "--program-id",
            str(PROGRAM_KEYPAIR),
            "--keypair",
            str(WALLET_PATHS["A"]),
            "--url",
            HOST_RPC_URL,
        )
    )
    run(
        (
            sys.executable,
            str(ROOT / "scripts/initialize_protocol_config.py"),
            "--rpc-url",
            HOST_RPC_URL,
            "--program-id",
            program_id,
            "--deployment-id-hex",
            DEPLOYMENT_ID_HEX,
            "--station-pubkey-hex",
            STATION_PUBKEY_HEX,
            "--authority-keypair",
            str(WALLET_PATHS["A"]),
        )
    )
    print(f"local Lastro program deployed: {program_id}")
    return program_id


def compose_command() -> list[str]:
    if not ENV_FILE.is_file():
        raise LocalDevError(f"local Compose environment is missing: {ENV_FILE}; run local-demo-init")
    return [
        "docker",
        "compose",
        "--env-file",
        str(ENV_FILE),
        "--project-name",
        PROJECT_NAME,
        "-f",
        str(COMPOSE_FILE),
        "-f",
        str(COMPOSE_OVERRIDE),
    ]


def validate_compose() -> None:
    run(
        (*compose_command(), "--profile", "app", "--profile", "hardware-sim", "config"),
        env=compose_process_env(),
        capture=True,
    )


def wait_http(url: str, *, timeout: float = 90.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=1.5) as response:
                if 200 <= response.status < 300:
                    return
        except (OSError, urllib.error.HTTPError):
            pass
        time.sleep(0.5)
    raise LocalDevError(f"service did not become healthy: {url}")


def compose_up_interactive() -> None:
    validate_compose()
    run(
        (*compose_command(), "--profile", "app", "--profile", "hardware-sim", "up", "-d", "--build"),
        env=compose_process_env(),
    )
    wait_http(f"{API_URL}/api/health")
    wait_http(f"{SIMULATOR_URL}/healthz")
    print(f"Lastro web: {WEB_URL}")
    print(f"Hardware Simulator: {SIMULATOR_URL}")
    print(f"Solana RPC: {HOST_RPC_URL}")


def compose_up_test_stack() -> None:
    validate_compose()
    # A previously running simulator Agent would compete with the deterministic G3 Agent harness.
    run(
        (*compose_command(), "stop", "hardware-simulator-agent", "hardware-simulator"),
        env=compose_process_env(),
        check=False,
        capture=True,
    )
    run(
        (*compose_command(), "--profile", "app", "up", "-d", "--build", "postgres", "api"),
        env=compose_process_env(),
    )
    wait_http(f"{API_URL}/api/health")


def capture_compose_logs(name: str) -> None:
    if shutil.which("docker") is None or not ENV_FILE.exists():
        return
    LOG_DIR.mkdir(parents=True, exist_ok=True)
    process = run(
        (*compose_command(), "logs", "--no-color"),
        env=compose_process_env(),
        capture=True,
        check=False,
    )
    (LOG_DIR / name).write_text((process.stdout or "") + (process.stderr or ""), encoding="utf-8")


def compose_down(*, volumes: bool = False) -> None:
    if shutil.which("docker") is None or not ENV_FILE.exists():
        return
    command = [
        *compose_command(),
        "--profile",
        "app",
        "--profile",
        "hardware-sim",
        "down",
        "--remove-orphans",
    ]
    if volumes:
        command.append("--volumes")
    run(command, env=compose_process_env(), check=False)


def up() -> None:
    doctor(for_tests=False)
    if not ENV_FILE.exists():
        init_environment()
    require_initialized()
    start_validator()
    prepare_chain()
    compose_up_interactive()


def down() -> None:
    compose_down(volumes=False)
    stop_validator()


def reset(*, identities: bool = False) -> None:
    compose_down(volumes=True)
    stop_validator()
    for path in (LEDGER_DIR, LOG_DIR, ARTIFACT_DIR):
        shutil.rmtree(path, ignore_errors=True)
    if identities:
        for path in (WALLETS_DIR, PROGRAM_KEYPAIR, ENV_FILE, SECRETS_FILE):
            if path.is_dir():
                shutil.rmtree(path, ignore_errors=True)
            else:
                path.unlink(missing_ok=True)
    ensure_state_dirs()
    print("local runtime reset" + ("; test identities removed" if identities else "; test identities preserved"))


def test_environment(*, headed: bool = False) -> None:
    doctor(for_tests=True)
    if not ENV_FILE.exists():
        init_environment()
    require_initialized()

    # Full-local tests are intentionally fresh and hermetic while preserving disposable identities.
    reset(identities=False)
    try:
        start_validator()
        prepare_chain()
        compose_up_test_stack()
        run(("cargo", "build", "--locked", "-p", "lastro-agent"))

        test_env = os.environ.copy()
        test_env.update(
            {
                "LASTRO_SYSTEM_API_URL": API_URL,
                "LASTRO_SYSTEM_SOLANA_RPC_URL": HOST_RPC_URL,
                "LASTRO_PROGRAM_ID": keypair_pubkey(PROGRAM_KEYPAIR),
                "LASTRO_DEPLOYMENT_ID_HEX": DEPLOYMENT_ID_HEX,
                "LASTRO_SYSTEM_AGENT_BIN": str((ROOT / "target/debug/lastro-agent").resolve()),
                "LASTRO_AGENT_TOKEN": json.loads(SECRETS_FILE.read_text(encoding="utf-8"))["agentToken"],
                "LASTRO_STATION_PUBKEY_HEX": STATION_PUBKEY_HEX,
                "LASTRO_SYSTEM_STATION_PRIVATE_SCALAR_HEX": STATION_PRIVATE_SCALAR_HEX,
                "LASTRO_SYSTEM_WALLET_A_KEYPAIR": str(WALLET_PATHS["A"].resolve()),
                "LASTRO_SYSTEM_WALLET_B_KEYPAIR": str(WALLET_PATHS["B"].resolve()),
                "LASTRO_SYSTEM_WALLET_C_KEYPAIR": str(WALLET_PATHS["C"].resolve()),
                "LASTRO_SYSTEM_TEST": "1",
                "LASTRO_E2E_SYSTEM": "1",
                "LASTRO_E2E_SOLANA_CHAIN": "solana:localnet",
                "VITE_API_BASE_URL": API_URL,
                "VITE_SOLANA_RPC_URL": HOST_RPC_URL,
                "VITE_SOLANA_CHAIN": "solana:localnet",
                "VITE_LASTRO_PROGRAM_ID": keypair_pubkey(PROGRAM_KEYPAIR),
            }
        )
        run(
            (
                sys.executable,
                "-m",
                "pytest",
                "-q",
                "tests/system/test_full_local.py",
                "tests/system/test_recovery.py",
                "tests/system/test_tamper.py",
            ),
            env=test_env,
        )

        stability_env = test_env.copy()
        stability_env["LASTRO_DEMO_STABILITY_TEST"] = "1"
        stability_env["LASTRO_DEMO_ARTIFACT_DIR"] = str((ARTIFACT_DIR / "stability").resolve())
        run(
            (sys.executable, "-m", "pytest", "-q", "tests/system/test_demo_stability.py"),
            env=stability_env,
        )

        browser_command = ["npm", "--workspace", "@lastro/web", "run", "test:e2e"]
        if headed:
            browser_command.extend(("--", "--headed"))
        run(browser_command, env=test_env)
        print("local full-system validation: PASS")
    finally:
        capture_compose_logs("compose-test.log")
        compose_down(volumes=True)
        stop_validator()


def status() -> None:
    initialized = all(path.is_file() for path in (ENV_FILE, PROGRAM_KEYPAIR, *WALLET_PATHS.values()))
    print(f"initialized: {'yes' if initialized else 'no'}")
    if initialized:
        print(f"program ID: {keypair_pubkey(PROGRAM_KEYPAIR)}")
        for label, path in WALLET_PATHS.items():
            print(f"Wallet {label}: {keypair_pubkey(path)}")
    pid = _read_validator_pid()
    print(f"validator: {'ready' if rpc_ready() else 'stopped'}" + (f" (pid {pid})" if pid else ""))
    if shutil.which("docker") and ENV_FILE.exists():
        process = run(
            (*compose_command(), "ps", "--status", "running"),
            env=compose_process_env(),
            capture=True,
            check=False,
        )
        output = process.stdout.strip()
        print("compose services:\n" + (output if output else "  none"))
    print(f"web: {WEB_URL}")
    print(f"hardware simulator: {SIMULATOR_URL}")
    print(f"local Solana RPC: {HOST_RPC_URL}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Lastro isolated local-development environment")
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("doctor", help="validate the pinned local software toolchain")
    sub.add_parser("init", help="create disposable local program/wallet identities and configuration")
    sub.add_parser("up", help="start validator, deploy Lastro, initialize config, and start app + hardware simulator")
    sub.add_parser("status", help="show local identities and service status")
    test_parser = sub.add_parser("test", help="run fresh G3/G5/browser full-system validation")
    test_parser.add_argument("--headed", action="store_true", help="show the Playwright Chromium run")
    sub.add_parser("down", help="stop Compose services and the owned local validator")
    reset_parser = sub.add_parser("reset", help="remove local runtime state while preserving test identities")
    reset_parser.add_argument("--identities", action="store_true", help="also remove disposable program/wallet identities")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.command == "doctor":
            doctor(for_tests=False)
        elif args.command == "init":
            init_environment()
        elif args.command == "up":
            up()
        elif args.command == "status":
            status()
        elif args.command == "test":
            test_environment(headed=args.headed)
        elif args.command == "down":
            down()
        elif args.command == "reset":
            reset(identities=args.identities)
        else:  # pragma: no cover - argparse prevents this.
            raise AssertionError(args.command)
    except LocalDevError as error:
        print(f"local-dev error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

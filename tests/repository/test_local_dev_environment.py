from __future__ import annotations

import importlib.util
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/local_dev.py"


def load_module():
    spec = importlib.util.spec_from_file_location("lastro_local_dev", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_local_env_separates_container_rpc_from_browser_and_system_rpc():
    local = load_module()
    rendered = local.render_local_env(program_id="11111111111111111111111111111111", agent_token="x" * 40)
    assert "LASTRO_SOLANA_RPC_URL='http://host.docker.internal:8899'" in rendered
    assert "LASTRO_SYSTEM_SOLANA_RPC_URL='http://127.0.0.1:8899'" in rendered
    assert "VITE_SOLANA_RPC_URL='http://127.0.0.1:8899'" in rendered
    assert "VITE_SOLANA_CHAIN='solana:localnet'" in rendered
    assert "LASTRO_E2E_SOLANA_CHAIN='solana:localnet'" in rendered
    assert "LASTRO_SYSTEM_STATION_PRIVATE_SCALAR_HEX='" + ("0" * 63 + "1") + "'" in rendered


def test_temporary_program_identity_restores_tracked_source_on_success_and_failure(tmp_path):
    local = load_module()
    source = tmp_path / "lib.rs"
    original = 'use anchor_lang::prelude::*;\n\n// LASTRO_PROGRAM_ID_BOOTSTRAP_MARKER\n\n#[program]\npub mod lastro {}\n'
    source.write_text(original, encoding="utf-8")

    with local.temporary_program_identity(source, "11111111111111111111111111111111"):
        assert '// LASTRO_PROGRAM_ID_BOOTSTRAP_MARKER' not in source.read_text(encoding="utf-8")
        assert 'declare_id!("11111111111111111111111111111111");' in source.read_text(encoding="utf-8")
    assert source.read_text(encoding="utf-8") == original

    try:
        with local.temporary_program_identity(source, "22222222222222222222222222222222"):
            raise RuntimeError("boom")
    except RuntimeError:
        pass
    else:
        raise AssertionError("fixture exception must propagate")
    assert source.read_text(encoding="utf-8") == original


def test_validator_command_uses_real_test_validator_with_explicit_local_dev_bind():
    local = load_module()
    command = local.validator_command()
    assert command[0] == "solana-test-validator"
    assert "--ledger" in command
    assert "--rpc-port" in command
    assert command[command.index("--rpc-port") + 1] == "8899"
    assert "--bind-address" in command
    assert command[command.index("--bind-address") + 1] == "0.0.0.0"
    assert "--reset" not in command


def test_local_compose_override_only_bridges_api_to_the_host_validator():
    override = (ROOT / "dev-environment/localnet/compose.override.yml").read_text(encoding="utf-8")
    assert "host.docker.internal:host-gateway" in override
    assert "solana-test-validator" not in override
    assert "hardware-simulator" not in override


def test_local_runtime_state_is_gitignored_and_core_source_has_no_local_dev_dependency():
    gitignore = (ROOT / ".gitignore").read_text(encoding="utf-8")
    assert "/.lastro-local/" in gitignore

    findings: list[str] = []
    for base in ("apps", "services", "crates", "chain", "firmware"):
        root = ROOT / base
        if not root.exists():
            continue
        for path in root.rglob("*"):
            if not path.is_file() or path.suffix not in {".rs", ".ts", ".vue", ".c", ".h", ".py", ".toml"}:
                continue
            text = path.read_text(encoding="utf-8", errors="ignore")
            if ".lastro-local" in text or "scripts/local_dev.py" in text:
                findings.append(path.relative_to(ROOT).as_posix())
    assert findings == [], "production core must not depend on local orchestration: " + ", ".join(findings)


def test_compose_environment_overrides_hostile_shell_deployment_values(tmp_path, monkeypatch):
    local = load_module()
    monkeypatch.setattr(local, "STATE_DIR", tmp_path)
    monkeypatch.setattr(local, "WALLETS_DIR", tmp_path / "wallets")
    monkeypatch.setattr(local, "ENV_FILE", tmp_path / "local.env")
    monkeypatch.setattr(local, "SECRETS_FILE", tmp_path / "secrets.json")
    monkeypatch.setattr(local, "PROGRAM_KEYPAIR", tmp_path / "program-keypair.json")
    monkeypatch.setattr(local, "WALLET_PATHS", {
        "A": tmp_path / "wallets/a.json",
        "B": tmp_path / "wallets/b.json",
        "C": tmp_path / "wallets/c.json",
    })
    for path in [local.PROGRAM_KEYPAIR, local.ENV_FILE, *local.WALLET_PATHS.values()]:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("fixture", encoding="utf-8")
    local.SECRETS_FILE.write_text('{"agentToken":"' + ('x' * 40) + '"}\n', encoding="utf-8")
    monkeypatch.setattr(local, "keypair_pubkey", lambda _path: "11111111111111111111111111111111")

    env = local.compose_process_env({
        "PATH": "/usr/bin",
        "HOME": "/tmp/home",
        "LASTRO_SOLANA_RPC_URL": "https://api.mainnet-beta.solana.com",
        "LASTRO_PROGRAM_ID": "HostileProgramId",
        "VITE_SOLANA_RPC_URL": "https://api.devnet.solana.com",
        "COMPOSE_PROJECT_NAME": "wrong-project",
    })
    assert env["PATH"] == "/usr/bin"
    assert env["HOME"] == "/tmp/home"
    assert env["LASTRO_SOLANA_RPC_URL"] == "http://host.docker.internal:8899"
    assert env["LASTRO_PROGRAM_ID"] == "11111111111111111111111111111111"
    assert env["VITE_SOLANA_RPC_URL"] == "http://127.0.0.1:8899"
    assert "COMPOSE_PROJECT_NAME" not in env

def test_local_program_build_restores_anchor_files_and_removes_temporary_deploy_key(tmp_path, monkeypatch):
    local = load_module()
    local_program = tmp_path / "program-keypair.json"
    target_program = tmp_path / "target/deploy/lastro-keypair.json"
    anchor_toml = tmp_path / "Anchor.toml"
    chain_lib = tmp_path / "lib.rs"
    program_so = tmp_path / "target/deploy/lastro.so"
    local_program.write_text("local-program-key", encoding="utf-8")
    anchor_original = b"[provider]\ncluster = \"Localnet\"\n"
    anchor_toml.write_bytes(anchor_original)
    source_original = "use anchor_lang::prelude::*;\n// LASTRO_PROGRAM_ID_BOOTSTRAP_MARKER\n"
    chain_lib.write_text(source_original, encoding="utf-8")

    monkeypatch.setattr(local, "PROGRAM_KEYPAIR", local_program)
    monkeypatch.setattr(local, "CHAIN_TARGET_KEYPAIR", target_program)
    monkeypatch.setattr(local, "ANCHOR_TOML", anchor_toml)
    monkeypatch.setattr(local, "CHAIN_LIB", chain_lib)
    monkeypatch.setattr(local, "PROGRAM_SO", program_so)
    monkeypatch.setattr(local, "require_initialized", lambda: None)
    monkeypatch.setattr(local, "keypair_pubkey", lambda _path: "11111111111111111111111111111111")

    def fake_run(command, **_kwargs):
        assert 'declare_id!("11111111111111111111111111111111");' in chain_lib.read_text(encoding="utf-8")
        assert target_program.read_text(encoding="utf-8") == "local-program-key"
        if tuple(command[:3]) == ("anchor", "keys", "sync"):
            anchor_toml.write_text("mutated-by-anchor\n", encoding="utf-8")
        if tuple(command[:2]) == ("anchor", "build"):
            program_so.parent.mkdir(parents=True, exist_ok=True)
            program_so.write_bytes(b"fixture-sbf")
        return None

    monkeypatch.setattr(local, "run", fake_run)
    assert local.build_local_program() == "11111111111111111111111111111111"
    assert chain_lib.read_text(encoding="utf-8") == source_original
    assert anchor_toml.read_bytes() == anchor_original
    assert not target_program.exists()
    assert program_so.read_bytes() == b"fixture-sbf"


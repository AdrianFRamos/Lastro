from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parents[2]


def test_no_empty_files():
    generated = {'.git', '.pytest_cache', '__pycache__', 'node_modules', 'target', 'build', 'dist', 'coverage', 'playwright-report', 'test-results'}
    assert not [
        p for p in ROOT.rglob('*')
        if p.is_file() and p.stat().st_size == 0 and not any(part in generated for part in p.relative_to(ROOT).parts)
    ]


def test_no_nested_readmes_or_source_markdown():
    generated = {'.git', '.pytest_cache', '__pycache__', 'node_modules', 'target', 'build', 'dist', 'coverage', 'playwright-report', 'test-results'}
    readmes = [p for p in ROOT.rglob('README.md') if not any(part in generated for part in p.relative_to(ROOT).parts)]
    assert readmes == [ROOT / 'README.md']
    stray = [
        p for p in ROOT.rglob('*.md')
        if not any(part in generated for part in p.relative_to(ROOT).parts)
        and p != ROOT / 'README.md'
        and ROOT / 'docs' not in p.parents
    ]
    assert not stray, stray


def test_all_production_layers_exist():
    for rel in [
        'crates/lastro-protocol', 'firmware/station', 'services/agent',
        'services/api', 'chain/programs/lastro', 'apps/web'
    ]:
        assert (ROOT / rel).is_dir(), rel


def test_root_cargo_workspace_does_not_embed_anchor_nested_workspace():
    """A Cargo package may not belong to two workspaces; Anchor owns chain/Cargo.toml."""
    root_cargo = tomllib.loads((ROOT / 'Cargo.toml').read_text())
    members = set(root_cargo['workspace']['members'])
    assert 'chain/programs/lastro' not in members
    assert (ROOT / 'chain/Cargo.toml').is_file()


def test_static_vite_container_requires_all_public_build_arguments():
    dockerfile = (ROOT / 'infra/Dockerfile.web').read_text()
    compose = (ROOT / 'infra/compose.yml').read_text()
    required = [
        'VITE_API_BASE_URL', 'VITE_SOLANA_RPC_URL',
        'VITE_SOLANA_CHAIN', 'VITE_LASTRO_PROGRAM_ID',
    ]
    for name in required:
        assert f'ARG {name}' in dockerfile
        assert name in compose
    # Secrets/server settings must never be ARG/ENV in the public web image.
    for forbidden in ['LASTRO_AGENT_TOKEN', 'LASTRO_DATABASE_URL', 'STATION_PRIVATE_KEY']:
        assert forbidden not in dockerfile


def test_chain_toolchain_is_isolated_and_pinned():
    anchor = tomllib.loads((ROOT / 'chain/Anchor.toml').read_text())
    assert anchor['toolchain']['anchor_version'] == '1.2.0'
    assert anchor['toolchain']['solana_version'] == '=4.1.2'
    assert anchor['toolchain']['package_manager'] == 'npm'


def test_litesvm_chain_tests_enable_native_precompiles():
    cargo = (ROOT / 'chain/programs/lastro/Cargo.toml').read_text()
    assert 'litesvm = { version = "=0.15.2", features = ["precompiles"] }' in cargo
    secp_test = (ROOT / 'chain/programs/lastro/tests/secp_binding.rs').read_text()
    assert 'with_precompiles' in secp_test


def test_anchor_litesvm_workspace_declares_rust_test_script_and_skips_validator():
    """Anchor test must execute the Rust/LiteSVM suite without starting a network validator."""
    import tomllib
    data = tomllib.loads((ROOT / 'chain/Anchor.toml').read_text(encoding='utf-8'))
    assert data['test']['skip_local_validator'] is True
    assert data['scripts']['test'] == 'cargo test --locked --workspace'

def test_program_identity_bootstrap_keeps_real_and_ci_modes_separate():
    script = (ROOT / 'scripts/bootstrap_program_id.sh').read_text(encoding='utf-8')
    e2e = (ROOT / '.github/workflows/e2e.yml').read_text(encoding='utf-8')
    chain = (ROOT / '.github/workflows/chain.yml').read_text(encoding='utf-8')
    assert '--ephemeral-ci' in script
    assert 'requires CI=true' in script
    assert 'refusing to generate a second program identity' in script
    assert 'scripts/bootstrap_program_id.sh --ephemeral-ci' in e2e
    assert 'scripts/bootstrap_program_id.sh --ephemeral-ci' in chain

def test_full_stack_playwright_is_serial_and_has_no_retry_masking():
    config = (ROOT / 'apps/web/playwright.config.ts').read_text(encoding='utf-8')
    assert 'fullyParallel: false' in config
    assert 'workers: 1' in config
    assert 'retries: 0' in config

def test_github_actions_use_fixed_ubuntu_runner_label():
    workflows = sorted((ROOT / '.github/workflows').glob('*.yml'))
    assert workflows
    for workflow in workflows:
        text = workflow.read_text(encoding='utf-8')
        assert 'runs-on: ubuntu-latest' not in text, workflow
        assert 'runs-on: ubuntu-24.04' in text, workflow

def test_ci_uses_committed_resolver_lockfiles_without_fallback_resolution():
    ci = (ROOT / '.github/workflows/ci.yml').read_text(encoding='utf-8')
    chain = (ROOT / '.github/workflows/chain.yml').read_text(encoding='utf-8')
    e2e = (ROOT / '.github/workflows/e2e.yml').read_text(encoding='utf-8')
    assert 'test -f package-lock.json' in ci
    assert 'npm ci' in ci
    assert 'npm install' not in ci
    assert 'test -f chain/Cargo.lock' in chain
    assert 'cargo generate-lockfile' not in chain
    assert 'test -f package-lock.json' in e2e
    assert 'test -f chain/Cargo.lock' in e2e
    assert 'npm ci' in e2e
    assert 'npm install' not in e2e
    assert 'cargo generate-lockfile' not in e2e

def test_dependency_bootstrap_refreshes_manifest_after_resolver_outputs():
    script = (ROOT / 'scripts/bootstrap_dependencies.sh').read_text(encoding='utf-8')
    assert 'cargo metadata --locked --format-version 1 >/dev/null' in script
    assert '\ncargo generate-lockfile\n' not in script
    assert 'cargo generate-lockfile --manifest-path chain/Cargo.toml' in script
    assert 'npm install --package-lock-only --ignore-scripts' in script
    assert 'python3 scripts/generate_manifest.py' in script

def test_github_actions_are_pinned_to_immutable_commits():
    workflows = sorted((ROOT / '.github/workflows').glob('*.yml'))
    refs = []
    for workflow in workflows:
        for line in workflow.read_text(encoding='utf-8').splitlines():
            stripped = line.strip()
            if stripped.startswith('- uses: '):
                refs.append(stripped.split('#', 1)[0].split('@', 1)[1].strip())
    assert refs
    assert all(len(ref) == 40 and all(char in '0123456789abcdef' for char in ref) for ref in refs)

def test_container_builds_use_committed_dependency_locks():
    api = (ROOT / 'infra/Dockerfile.api').read_text(encoding='utf-8')
    web = (ROOT / 'infra/Dockerfile.web').read_text(encoding='utf-8')
    assert 'COPY Cargo.toml Cargo.lock rust-toolchain.toml ./' in api
    assert 'cargo build --locked --release -p lastro-api' in api
    assert 'COPY package.json package-lock.json .npmrc ./' in web
    assert 'RUN npm ci' in web
    assert 'npm install' not in web

def test_devnet_smoke_requires_committed_program_identity():
    script = (ROOT / 'scripts/devnet_smoke.sh').read_text(encoding='utf-8')
    assert 'LASTRO_PROGRAM_ID_BOOTSTRAP_MARKER' in script
    assert 'Expected exactly one committed Lastro declare_id!' in script
    assert 'does not match the committed declare_id!' in script

def test_pinned_rust_action_selects_exact_toolchain_explicitly():
    workflows = ['ci.yml', 'chain.yml', 'e2e.yml', 'devnet.yml']
    for name in workflows:
        text = (ROOT / '.github' / 'workflows' / name).read_text(encoding='utf-8')
        assert 'dtolnay/rust-toolchain@ce678459e9fc7500d337468f904b95f1b5c10b5e' in text
        assert "toolchain: '1.98.1'" in text

def test_python_ci_uses_exact_patch_without_mutable_pip_upgrade():
    workflows = ['ci.yml', 'e2e.yml', 'devnet.yml']
    for name in workflows:
        text = (ROOT / '.github' / 'workflows' / name).read_text(encoding='utf-8')
        assert "python-version: '3.13.15'" in text
        assert 'pip install --upgrade pip' not in text

def test_node_and_npm_resolvers_are_exactly_pinned():
    import json
    root = json.loads((ROOT / 'package.json').read_text(encoding='utf-8'))
    web = json.loads((ROOT / 'apps/web/package.json').read_text(encoding='utf-8'))
    assert root['engines'] == {'node': '24.21.0', 'npm': '11.19.0'}
    assert root['packageManager'] == 'npm@11.19.0'
    assert web['engines'] == {'node': '24.21.0', 'npm': '11.19.0'}
    doctor = (ROOT / 'scripts/doctor.py').read_text(encoding='utf-8')
    assert "re.compile(r'^11\\.19\\.0$')" in doctor
    assert doctor.count('def main() -> int:') == 1
    compile(doctor, 'scripts/doctor.py', 'exec')

def test_devnet_artifacts_do_not_persist_secret_rpc_or_key_material():
    source = (ROOT / 'tests/system/test_devnet.py').read_text(encoding='utf-8')
    workflow = (ROOT / '.github/workflows/devnet.yml').read_text(encoding='utf-8')
    assert '"rpcUrl": env.rpc_url' not in source
    assert '"cluster": "devnet"' in source
    for secret_name in [
        'LASTRO_DEVNET_RPC_URL',
        'LASTRO_AGENT_TOKEN',
        'LASTRO_DEVNET_STATION_PRIVATE_SCALAR_HEX',
        'LASTRO_DEVNET_WALLET_A_KEYPAIR_JSON',
        'LASTRO_DEVNET_WALLET_B_KEYPAIR_JSON',
        'LASTRO_DEVNET_WALLET_C_KEYPAIR_JSON',
    ]:
        assert secret_name not in source
    assert '${{ secrets.LASTRO_DEVNET_RPC_URL }}' in workflow
    assert 'lastro-devnet-wallet-' in workflow
    assert '${{ runner.temp }}/lastro-devnet-artifacts' in workflow

def test_local_rust_commands_use_locked_dependency_graphs():
    makefile = (ROOT / 'Makefile').read_text(encoding='utf-8')
    for command in [
        'cargo clippy --locked --workspace',
        'cargo test --locked -p lastro-protocol',
        'cargo test --locked -p lastro-agent',
        'cargo test --locked -p lastro-api',
        'cargo test --locked --workspace',
    ]:
        assert command in makefile
    assert 'idf.py -C firmware/station set-target esp32c5' in makefile
    chain = (ROOT / '.github/workflows/chain.yml').read_text(encoding='utf-8')
    assert 'git diff --exit-code -- Cargo.lock' in chain

def test_web_ci_uses_only_locked_local_npm_executables():
    import json
    package = json.loads((ROOT / 'apps/web/package.json').read_text(encoding='utf-8'))
    ci = (ROOT / '.github/workflows/ci.yml').read_text(encoding='utf-8')
    e2e = (ROOT / '.github/workflows/e2e.yml').read_text(encoding='utf-8')
    assert package['scripts']['format:check'] == 'prettier --check .'
    assert package['scripts']['playwright:install'] == 'playwright install --with-deps chromium'
    assert 'npx ' not in json.dumps(package['scripts'])
    assert 'npx ' not in ci
    assert 'npx ' not in e2e
    for workflow in (ci, e2e):
        assert "node --version | grep -Fx 'v24.21.0'" in workflow
        assert "npm --version | grep -Fx '11.19.0'" in workflow

def test_production_container_bases_are_immutable_and_runtime_has_no_package_resolution():
    api = (ROOT / 'infra/Dockerfile.api').read_text(encoding='utf-8')
    web = (ROOT / 'infra/Dockerfile.web').read_text(encoding='utf-8')
    expected = [
        'rust:1.98.1-bookworm@sha256:9a73a5088750b4c95158ab26629c854c3d6fc4b173cb7bc8079ad252d8ed7bfa',
        'debian:bookworm-20260918-slim@sha256:3783cc01769c7b2b1b83a5c5ad96c815348e28ed7da68e2e3687004faa906251',
        'node:24.21.0-bookworm-slim@sha256:2fe369e969550cde8e867afc3fe370b260140cab4a23d467074295b42163d553',
        'caddy:2.10.2-alpine@sha256:4c6e91c6ed0e2fa03efd5b44747b625fec79bc9cd06ac5235a779726618e530d',
    ]
    combined = api + web
    for image in expected:
        assert image in combined
    assert 'apt-get update' not in api
    assert 'apt-get install' not in api
    assert 'COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt' in api

def test_ci_runs_pure_system_harness_tests_without_external_gate_flags():
    ci = (ROOT / '.github/workflows/ci.yml').read_text(encoding='utf-8')
    assert 'tests/system/test_support.py' in ci
    assert 'tests/system/test_e2e_controller.py' in ci
    assert 'LASTRO_SYSTEM_TEST' not in ci
    assert 'LASTRO_DEVNET_TEST' not in ci
    assert 'LASTRO_HARDWARE_TEST' not in ci

def test_solana_cli_checks_require_exact_4_1_2_patch():
    expected = "solana --version | grep -E '^solana-cli 4\\.1\\.2([[:space:]]|$)'"
    for name in ['chain.yml', 'e2e.yml', 'devnet.yml']:
        text = (ROOT / '.github' / 'workflows' / name).read_text(encoding='utf-8')
        assert expected in text
        assert "solana --version | grep -F '4.1.2'" not in text

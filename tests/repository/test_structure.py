from pathlib import Path
import re
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
        'VITE_LASTRO_DEPLOYMENT_ID_HEX',
        'VITE_LASTRO_AUTHORITY',
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
    assert anchor['toolchain']['solana_version'] == '4.2.0'
    assert anchor['toolchain']['package_manager'] == 'npm'


def test_litesvm_chain_tests_enable_native_precompiles():
    cargo = (ROOT / 'chain/programs/lastro/Cargo.toml').read_text()
    assert 'litesvm = { version = "=0.16.0", features = ["precompiles"] }' in cargo
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

def test_github_actions_use_ubuntu_latest_linux_runner():
    workflows = sorted((ROOT / '.github/workflows').glob('*.yml'))
    assert workflows
    for workflow in workflows:
        text = workflow.read_text(encoding='utf-8')
        assert 'runs-on: [self-hosted, linux]' not in text, workflow
        assert 'runs-on: ubuntu-latest' in text, workflow


def test_github_actions_cancel_superseded_runs_per_workflow_and_ref():
    workflows = sorted((ROOT / '.github/workflows').glob('*.yml'))
    assert workflows
    for workflow in workflows:
        text = workflow.read_text(encoding='utf-8')
        assert 'concurrency:' in text, workflow
        assert 'group: ${{ github.workflow }}-${{ github.ref }}' in text, workflow
        assert 'cancel-in-progress: true' in text, workflow

def test_self_hosted_jobs_clean_workspace_before_checkout():
    workflows = sorted((ROOT / '.github/workflows').glob('*.yml'))
    assert workflows
    for workflow in workflows:
        text = workflow.read_text(encoding='utf-8')
        checkout = 'uses: actions/checkout@'
        for job_block in re.split(r'(?m)^  [A-Za-z0-9_-]+:\s*$', text)[1:]:
            if checkout not in job_block:
                continue
            assert 'name: Reset self-hosted workspace' in job_block, workflow
            assert 'find "$GITHUB_WORKSPACE" -mindepth 1 -maxdepth 1 -exec rm -rf -- {} +' in job_block, workflow
            assert job_block.index('name: Reset self-hosted workspace') < job_block.index(checkout), workflow


def test_firmware_build_does_not_run_the_checkout_inside_a_root_job_container():
    workflow = (ROOT / '.github/workflows/firmware.yml').read_text(encoding='utf-8')
    assert '\n    container:' not in workflow
    assert 'docker run --rm' in workflow
    assert ':/source:ro' in workflow
    assert 'espressif/idf:v6.1@sha256:81893c71bb5e570088901f21def8684c25cd2a9020281bd01b843a7655edb18c' in workflow


def test_self_hosted_solana_jobs_use_checksum_pinned_anchor_and_solana_binaries():
    for name in ['chain.yml', 'e2e.yml', 'devnet.yml']:
        text = (ROOT / '.github' / 'workflows' / name).read_text(encoding='utf-8')
        assert 'scripts/install_pinned_anchor_cli.sh' in text
        assert 'scripts/install_pinned_solana_cli.sh' in text
        assert 'avm solana install' not in text
        assert "anchor --version | grep -Fx 'anchor-cli 1.2.0'" in text
        assert "solana --version | grep -E '^solana-cli 4\\.2\\.0([[:space:]]|$)'" in text


def test_firmware_workflow_activates_esp_idf_environment_before_using_idf_py():
    text = (ROOT / '.github/workflows/firmware.yml').read_text(encoding='utf-8')
    assert '. "$IDF_PATH/export.sh"' in text
    assert 'idf.py --version' in text
    assert 'idf.py -C /tmp/station set-target esp32c5' in text
    assert 'idf.py -C /tmp/station -DSDKCONFIG_DEFAULTS="sdkconfig.defaults;sdkconfig.efuse.defaults" set-target esp32c5' in text
    assert 'idf.py -C /tmp/station -DSDKCONFIG_DEFAULTS="sdkconfig.defaults;sdkconfig.efuse.defaults" build' in text


def test_maintenance_sync_stages_only_existing_repository_paths():
    workflow = (ROOT / '.github/workflows/maintenance-sync.yml').read_text(encoding='utf-8')
    command = (
        'git add Cargo.lock chain/Cargo.lock package-lock.json crates services '
        'chain/programs apps/web docs/TEST_INDEX.md MANIFEST.sha256'
    )
    assert command in workflow
    assert ' services agents ' not in workflow

def test_maintenance_sync_covers_every_push_for_manifest_consistency():
    workflow = (ROOT / '.github/workflows/maintenance-sync.yml').read_text(encoding='utf-8')
    push_block = workflow.split('  push:', 1)[1].split('\n\nconcurrency:', 1)[0]
    assert 'paths:' not in push_block
    assert 'cancel-in-progress: true' in workflow

def test_maintenance_sync_executes_repository_generators_on_every_push():
    workflow = (ROOT / '.github/workflows/maintenance-sync.yml').read_text(encoding='utf-8')
    assert 'python scripts/generate_test_index.py' in workflow
    assert 'python scripts/generate_manifest.py' in workflow
    assert 'cancel-in-progress: true' in workflow

def test_chain_resolver_lockfile_is_committed():
    assert (ROOT / 'chain/Cargo.lock').is_file()


def test_postgres_18_compose_persists_the_real_data_root_and_binds_dev_port_to_loopback():
    compose = (ROOT / 'infra/compose.yml').read_text(encoding='utf-8')
    assert '127.0.0.1:5432:5432' in compose
    assert 'lastro-postgres:/var/lib/postgresql' in compose
    assert 'lastro-postgres:/var/lib/postgresql/data' not in compose


def test_browser_deployment_id_is_propagated_across_build_and_local_environments():
    variable = 'VITE_LASTRO_DEPLOYMENT_ID_HEX'
    assert variable in (ROOT / '.env.example').read_text(encoding='utf-8')
    assert variable in (ROOT / 'scripts/local_dev.py').read_text(encoding='utf-8')
    for name in ['ci.yml', 'e2e.yml']:
        assert variable in (ROOT / '.github' / 'workflows' / name).read_text(encoding='utf-8')


def test_local_development_requires_the_same_solana_patch_as_anchor_ci():
    source = (ROOT / 'scripts/local_dev.py').read_text(encoding='utf-8')
    assert r'\b4\.2\.0\b' in source
    assert 'Solana CLI 4.2.0' in source
    assert 'solana-test-validator 4.2.0' in source


def test_agent_main_reconnects_after_serial_transport_failures():
    source = (ROOT / 'services/agent/src/main.rs').read_text(encoding='utf-8')
    assert 'AgentError::Serial' in source
    assert 'sleep(config.poll_interval).await' in source
    assert 'config.station_response_timeout' in source
    assert source.count('SerialStationTransport::open') >= 1
    assert 'loop {' in source

def test_web_ci_program_id_matches_transaction_validation_fixture():
    ci = (ROOT / '.github/workflows/ci.yml').read_text(encoding='utf-8')
    transaction_test = (ROOT / 'apps/web/tests/solana/transaction.test.ts').read_text(encoding='utf-8')
    test_program_id = 'Vote111111111111111111111111111111111111111'
    assert f"VITE_LASTRO_PROGRAM_ID: '{test_program_id}'" in ci
    assert f"const PROGRAM_ID = '{test_program_id}'" in transaction_test

def test_browser_records_signed_solana_identity_before_rpc_broadcast():
    source = (ROOT / 'apps/web/src/solana/transaction.ts').read_text(encoding='utf-8')
    signature = 'const signature = String(getSignatureFromTransaction(signedTransaction))'
    persisted = 'onSigned({'
    broadcast = '.sendTransaction(wire,'
    assert signature in source
    assert persisted in source
    assert broadcast in source
    assert source.index(signature) < source.index(persisted) < source.index(broadcast)

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

def test_protocol_initialize_transaction_requests_explicit_compute_budget():
    import re
    import tomllib

    harness = (ROOT / 'chain/programs/lastro/tests/common/mod.rs').read_text(encoding='utf-8')
    script = (ROOT / 'scripts/initialize_protocol_config.py').read_text(encoding='utf-8')
    cargo = tomllib.loads((ROOT / 'chain/programs/lastro/Cargo.toml').read_text(encoding='utf-8'))

    assert cargo['dev-dependencies']['solana-compute-budget-interface'] == '=3.0.0'
    rust_limit = re.search(r'pub const INITIALIZE_COMPUTE_UNIT_LIMIT: u32 = ([\d_]+);', harness)
    python_limit = re.search(r'INITIALIZE_COMPUTE_UNIT_LIMIT = (\d+)', script)
    assert rust_limit is not None
    assert python_limit is not None
    assert rust_limit.group(1).replace('_', '') == python_limit.group(1)
    assert 'ComputeBudgetInstruction::set_compute_unit_limit(INITIALIZE_COMPUTE_UNIT_LIMIT)' in harness
    assert 'COMPUTE_BUDGET_PROGRAM_ID = "ComputeBudget111111111111111111111111111111"' in script
    assert 'bytes([2]) + INITIALIZE_COMPUTE_UNIT_LIMIT.to_bytes(4, "little")' in script
    assert 'compile_legacy_message(authority.public_key, rpc.latest_blockhash(), [compute_budget, instruction])' in script


def test_litesvm_test_harness_boxes_large_failure_metadata():
    import re

    source = (ROOT / 'chain/programs/lastro/tests/common/mod.rs').read_text(encoding='utf-8')
    assert 'pub type TestTransactionResult = Result<TransactionMetadata, Box<FailedTransactionMetadata>>;' in source
    assert re.search(r'\bTransactionResult\b', source) is None
    assert 'svm.send_transaction(tx).map_err(Box::new)' in source


def test_chain_tests_use_split_solana_4_2_ids_and_traits_explicitly():
    common = (ROOT / 'chain/programs/lastro/tests/common/mod.rs').read_text(encoding='utf-8')
    event = (ROOT / 'chain/programs/lastro/src/verify/event.rs').read_text(encoding='utf-8')

    assert 'solana_sdk_ids::sysvar::instructions::ID' in common
    assert 'anchor_lang::solana_program::sysvar::instructions::ID' not in common
    for test_file in (ROOT / 'chain/programs/lastro/tests').glob('*.rs'):
        source = test_file.read_text(encoding='utf-8')
        if '.pubkey()' in source:
            assert 'use solana_signer::Signer;' in source, test_file
    assert 'let expected_hash: [u8; 32] = Sha256::digest(ORIGIN).into();' in event
    assert 'assert_eq!(parsed.event_hash(), expected_hash);' in event


def test_secp256r1_verifier_has_no_obsolete_descriptor_boundary_constant():
    source = (ROOT / 'chain/programs/lastro/src/verify/secp256r1.rs').read_text(encoding='utf-8')
    assert 'DESCRIPTOR_END' not in source
    for required in [
        'SIGNATURE_OFFSET',
        'SIGNATURE_END',
        'PUBLIC_KEY_OFFSET',
        'PUBLIC_KEY_END',
    ]:
        assert required in source


def test_anchor_program_reexports_nested_generated_account_helpers_at_crate_root():
    source = (ROOT / 'chain/programs/lastro/src/lib.rs').read_text(encoding='utf-8')
    for module in ['initialize', 'origin', 'reidentify', 'transfer']:
        for prefix in ['__client_accounts_', '__cpi_client_accounts_']:
            expected = (
                f'pub(crate) use instructions::{module}::{prefix}{module};'
            )
            assert expected in source


def test_chain_sources_follow_anchor_1_2_program_api_contract():
    import tomllib

    cargo_path = ROOT / 'chain/programs/lastro/Cargo.toml'
    cargo = cargo_path.read_text(encoding='utf-8')
    manifest = tomllib.loads(cargo)
    dependencies = manifest['dependencies']
    assert dependencies['solana-sdk-ids'] == '=3.1.0'
    assert 'solana-secp256r1-program' not in dependencies
    assert 'anchor-debug = []' in cargo
    assert 'custom-heap = []' in cargo
    assert 'custom-panic = []' in cargo
    assert 'solana-instructions-sysvar = "=3.0.1"' in cargo
    assert 'litesvm = { version = "=0.16.0", features = ["precompiles"] }' in cargo

    instructions = ROOT / 'chain/programs/lastro/src/instructions'
    for name in ['initialize.rs', 'origin.rs', 'reidentify.rs', 'transfer.rs']:
        source = (instructions / name).read_text(encoding='utf-8')
        assert "Context<'_, '_, '_, '_," not in source, name

    module = (instructions / 'mod.rs').read_text(encoding='utf-8')
    assert 'pub use initialize::*;' not in module
    assert 'pub use origin::*;' not in module
    assert 'pub use reidentify::*;' not in module
    assert 'pub use transfer::*;' not in module
    for exported in [
        'pub use initialize::Initialize;',
        'pub use origin::Origin;',
        'pub use reidentify::Reidentify;',
        'pub use transfer::Transfer;',
    ]:
        assert exported in module

    entrypoint = (ROOT / 'chain/programs/lastro/src/lib.rs').read_text(encoding='utf-8')
    assert 'Context<instructions::' not in entrypoint
    for account_type in ['Initialize', 'Origin', 'Transfer', 'Reidentify']:
        assert f'Context<{account_type}>' in entrypoint
    assert 'use crate::instructions::{Initialize, Origin, Reidentify, Transfer};' in entrypoint

    verifier = (ROOT / 'chain/programs/lastro/src/verify/secp256r1.rs').read_text(encoding='utf-8')
    assert 'use solana_instructions_sysvar::{' in verifier
    assert 'load_current_index_checked' in verifier
    assert 'load_instruction_at_checked' in verifier
    assert 'solana_program::sysvar::instructions' not in verifier
    assert 'solana_sdk_ids::secp256r1_program::ID' in verifier
    assert 'solana_secp256r1_program::ID' not in verifier

    harness = (ROOT / 'chain/programs/lastro/tests/common/mod.rs').read_text(encoding='utf-8')
    assert 'solana_sdk_ids::secp256r1_program::ID' in harness
    assert 'solana_secp256r1_program::ID' not in harness


def test_maintenance_sync_regenerates_both_rust_lockfiles():
    workflow = (ROOT / '.github/workflows/maintenance-sync.yml').read_text(encoding='utf-8')
    assert 'cargo generate-lockfile' in workflow
    assert 'cargo generate-lockfile --manifest-path chain/Cargo.toml' in workflow

def test_chain_rustsec_exceptions_are_narrow_and_documented():
    ci = (ROOT / '.github/workflows/ci.yml').read_text(encoding='utf-8')
    security = (ROOT / 'docs/SECURITY.md').read_text(encoding='utf-8')
    root_audit = 'cargo audit --file Cargo.lock'
    chain_audit = (
        'cargo audit --file chain/Cargo.lock '
        '--ignore RUSTSEC-2022-0093 --ignore RUSTSEC-2024-0344'
    )
    assert root_audit in ci
    assert root_audit + ' --ignore' not in ci
    assert chain_audit in ci
    assert ci.count('RUSTSEC-2022-0093') == 1
    assert ci.count('RUSTSEC-2024-0344') == 1
    assert 'RUSTSEC-2022-0093' in security
    assert 'RUSTSEC-2024-0344' in security
    assert 'litesvm' in security.lower()
    assert '[dev-dependencies]' in security.lower()

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

def test_root_node_toolchain_pins_typescript_used_by_hoisted_vue_tsc():
    import json
    root_package = json.loads((ROOT / 'package.json').read_text(encoding='utf-8'))
    assert root_package['devDependencies']['typescript'] == '6.0.2'

    lock = json.loads((ROOT / 'package-lock.json').read_text(encoding='utf-8'))
    assert lock['packages']['node_modules/typescript']['version'] == '6.0.2'


def test_web_typecheck_keeps_strict_source_checks_but_skips_third_party_declarations():
    import json
    for name in ['tsconfig.app.json', 'tsconfig.node.json']:
        config = json.loads((ROOT / 'apps/web' / name).read_text(encoding='utf-8'))
        compiler = config['compilerOptions']
        assert compiler['skipLibCheck'] is True
    app = json.loads((ROOT / 'apps/web/tsconfig.app.json').read_text(encoding='utf-8'))
    compiler = app['compilerOptions']
    assert compiler['strict'] is True
    assert compiler['noUncheckedIndexedAccess'] is True
    assert compiler['exactOptionalPropertyTypes'] is True


def test_vite_config_includes_vitest_types_for_inline_test_configuration():
    source = (ROOT / 'apps/web/vite.config.ts').read_text(encoding='utf-8')
    assert source.startswith('/// <reference types="vitest/config" />\n')
    assert "import { defineConfig } from 'vite'" in source
    assert 'test: {' in source


def test_vue_tsc_uses_typescript_version_known_to_be_supported():
    import json
    web = json.loads((ROOT / 'apps/web/package.json').read_text(encoding='utf-8'))
    dev = web['devDependencies']
    assert dev['vue-tsc'] == '3.3.11'
    assert dev['typescript'] == '6.0.2'


def test_maintenance_sync_regenerates_node_lock_before_locked_install():
    workflow = (ROOT / '.github/workflows/maintenance-sync.yml').read_text(encoding='utf-8')
    assert 'npm install --package-lock-only --ignore-scripts' in workflow
    assert workflow.index('npm install --package-lock-only --ignore-scripts') < workflow.index('npm ci')

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

def test_solana_cli_checks_require_exact_4_2_0_patch():
    expected = "solana --version | grep -E '^solana-cli 4\\.2\\.0([[:space:]]|$)'"
    for name in ['chain.yml', 'e2e.yml', 'devnet.yml']:
        text = (ROOT / '.github' / 'workflows' / name).read_text(encoding='utf-8')
        assert expected in text
        assert "solana --version | grep -F '4.2.0'" not in text

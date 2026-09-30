from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def test_demo_preflight_is_fail_closed_and_does_not_claim_external_gates():
    """Repository guardrail for the release preflight itself.

    PURPOSE: keep demo_preflight from degrading into a file-existence smoke test.
    ASSERT: it runs spec/vector checks, requires resolver lockfiles and real program/deployment
    configuration, and explicitly excludes physical/eFuse/Devnet/stability claims.
    FAILURE MEANS: a future edit could report a release candidate as ready without the local
    prerequisites required by the implementation plan.
    """
    text = (ROOT / 'scripts/demo_preflight.py').read_text(encoding='utf-8')
    required_fragments = [
        'scripts/spec_check.py',
        'scripts/check_vectors.py',
        'Cargo.lock',
        'package-lock.json',
        'chain/Cargo.lock',
        'LASTRO_PROGRAM_ID_BOOTSTRAP_MARKER',
        'LASTRO_PROGRAM_ID',
        'LASTRO_SOLANA_RPC_URL',
        'LASTRO_DEPLOYMENT_ID_HEX',
        'LASTRO_STATION_PUBKEY_HEX',
        'NOT CHECKED by this command',
        'eFuse',
        'Devnet',
    ]
    for fragment in required_fragments:
        assert fragment in text, f'demo_preflight.py lost required fail-closed contract: {fragment}'

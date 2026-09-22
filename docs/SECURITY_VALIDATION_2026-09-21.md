# Security validation and hardening report — 2026-09-21

## Scope

This report records the software security validation work performed for the Lastro hackathon core. It distinguishes source/test evidence from execution evidence and does not claim that hardware-gated properties were validated by mocks.

The review covered the current `main` baseline, recent commits, all GitHub Actions workflows, workflow-run/job metadata, `docs/SECURITY.md`, protocol/Agent/API/Solana/verifier/firmware tests, browser rendering paths, persistence constraints, secret handling, and dependency-gate design.

## Security gaps actually found and fixes applied

### HTTP resource bounds

The API relied on Axum's generic JSON body limit rather than a protocol-derived application bound. The largest mutation request is `AgentEvidenceRequest`: the fixed 276-byte StationEvent encodes to exactly 368 Base64 characters, plus a UUID and fixed 8/33/64-byte evidence fields.

Fix: all JSON extractors now use a 1024-byte `DefaultBodyLimit`. Evidence Base64 length and fixed hex fields are rejected before decode/crypto/database/RPC work. Solana transaction signatures are bounded before Base58 decoding.

Tests cover oversized bodies, oversized Base64, long identifiers, malformed JSON, wrong content type, undeclared fields, invalid hex/Base64, and repeated oversized requests.

### Unbounded Solana RPC waits

The async reqwest client had no total request deadline. A stalled RPC could therefore hold API work indefinitely.

Fix: backend Solana RPC requests now use a 10-second total timeout. Browser-side independent Solana verification also has a 10-second deadline and preserves the semantic distinction required by the verifier: network/RPC failure is `NOT_CHECKED`, not `VALID` and not contradictory `INVALID`.

### Agent bearer-token comparison

The previous comparison returned immediately when token lengths differed. That exposed token length through the comparison path.

Fix: both presented and expected tokens are hashed to fixed-size SHA-256 digests and compared with `subtle::ConstantTimeEq`. Authorization tests exercise wrong tokens with differences at different positions and lengths.

### Evidence and durable-browser input bounds

Evidence transport parsing accepted several attacker-controlled strings before tight encoded-size checks, and durable browser recovery state accepted generic non-empty identifiers.

Fix: EvidencePackage parsing now enforces exact StationEvent Base64 length, fixed hex lengths, bounded Solana-signature text, strict UUID capture identifiers, strict 32-byte hex identifiers, and bounded transaction-signature text before later cryptographic/RPC verification. Local recovery state fails closed when malformed.

No arbitrary event-history count cap was retained: canonical event history can legitimately grow. Fixed event fields remain bounded, and deployment-scale pagination must preserve complete verification semantics rather than truncate history.

### Agent transport error secret leakage

Reqwest transport errors can retain request URLs. If an API URL contains credentials or sensitive query data, persisting the error in retry state can leak it.

Fix: Agent transport errors call `reqwest::Error::without_url()` before crossing into durable diagnostics. Regression coverage checks that URL credentials do not enter retry state.

### Firmware configuration coverage

CI previously compiled only the development signer configuration.

Fix: firmware CI now compiles both the default signer and the eFuse-backed signer configuration while explicitly avoiding any eFuse provisioning/burn command. This is compile-time evidence only; it does not prove physical eFuse protection.

### Secret hygiene

The repository now has deterministic checks for high-confidence private-key/token patterns, secret-bearing filenames, unsafe `VITE_*` secret names, required gitignore patterns, and reachable Git history. The CI checkout for this test uses complete history.

No native GitHub secret-scanning result is claimed. GitHub documentation limits secret scanning for private repositories by ownership/plan and Secret Protection availability, and the repository metadata available in this environment did not expose an enabled secret-scanning state.

## Existing security properties confirmed in executable tests

The existing suite already declares direct adversarial coverage for the core chain invariants: missing Secp256r1 precompile, wrong Station key, different signed StationEvent bytes, descriptor offset/index/message-length errors, high-S signatures, forbidden instruction order/additions, PDA constraints, duplicate ORIGIN, stale/old custodian, replay, sequence gap, predecessor mismatch, revision errors, active RFID collision, retired RFID reuse, and full ORIGIN -> TRANSFER -> REIDENTIFY -> TRANSFER state-machine behavior.

Persistence tests cover identical retry idempotency, divergent duplicate conflicts, concurrent evidence admission, append-only/immutable evidence, monotonic lifecycle, stable transaction signatures, and projection advancement only after canonical Solana confirmation.

Verifier tests cover evidence-byte/signature tampering, omitted/forked/reordered history, canonical owner/layout mismatches, missing/mixed transaction references, non-finalized/failed Solana transactions, transaction-signature mismatch, wrong program/accounts/instruction bytes, and RPC failure mapping to `NOT_CHECKED`.

Browser review of the primary application/verifier components found no `v-html`, `innerHTML`, `outerHTML`, `insertAdjacentHTML`, `eval`, `Function`, `javascript:`, untrusted dynamic style binding, or session-storage sink. Untrusted evidence details continue to render through Vue text interpolation. Local storage is limited to bounded recovery metadata and is parsed fail-closed; wallet secrets are not stored there.

## Property/fuzz-style tests

Deterministic arbitrary-byte property tests were added for StationEvent and Agent serial framing. The fixed-size decoders already reject truncation, trailing bytes, invalid actions, non-zero reserved bytes, invalid CRC/length fields, and malformed fixed payloads. Round-trip/vector tests cover deterministic encoding and hashing.

`cargo-fuzz` was researched but not added in this round. The Rust Fuzz Book recommends it for Rust fuzzing, but it requires a nightly fuzzing toolchain and a separate execution path. With GitHub-hosted runners currently failing before step execution, adding an unexecutable nightly workflow would not provide stronger evidence than the deterministic normal-CI properties and would violate the goal of avoiding security theater. A future fuzz workflow is appropriate once runners execute reliably; any discovered crash input should be preserved as a deterministic regression test.

## Dependency security

The existing `ci` workflow now has a dedicated `dependency-audit` job using only already-reviewed, full-SHA-pinned setup actions.

Rust:
- installs RustSec `cargo-audit 0.22.2` with `cargo install --locked`;
- audits the committed root `Cargo.lock`;
- audits the committed `chain/Cargo.lock`.

Node:
- verifies Node `24.21.0` and npm `11.19.0`;
- audits runtime dependencies from the committed `package-lock.json` with `--omit=dev`;
- audits the complete runtime + development dependency graph from the same committed lockfile;
- uses `--package-lock-only` and does not run `npm audit fix` or `--force`.

Advisory disposition is **not verified in this report** because the GitHub-hosted runner did not start the job, and this execution environment cannot access the private repository through a local network checkout. No advisory was ignored or suppressed.

## GitHub Actions hardening and execution status

All reviewed workflows retain top-level:

```yaml
permissions:
  contents: read
```

Third-party Actions are pinned to full 40-character commit SHAs. The workflows use GitHub-hosted `ubuntu-24.04` runners and do not use `pull_request_target`. Test credentials in deterministic E2E are explicitly synthetic.

The deterministic workflow commands are **not green as execution evidence** at the time of this report because GitHub is failing before runner assignment. For the latest `ci` run after adding the dependency audit, all three jobs (`rust-and-spec`, `web`, and `dependency-audit`) completed as failures in about three seconds with:
- `runner_id: 0`;
- empty `runner_name`;
- zero recorded steps.

Earlier `ci`, `chain`, `e2e`, and `firmware` runs show the same no-step/no-runner pattern. Rerunning failed jobs reproduced it. This is not counted as a passed software gate, and no test was disabled or downgraded to hide it.

GitHub documents `ubuntu-24.04` as a supported standard GitHub-hosted runner label for private repositories. GitHub also documents that private-repository hosted-runner use depends on Actions availability/quota/billing. Account-level runner/billing state was not available through the repository connection used for this review.

## Secret scanning and CodeQL availability

The repository is private and user-owned. The available repository metadata did not expose an enabled `security_and_analysis` configuration, and the connector did not permit querying the secret-scanning/code-scanning alert endpoints. Therefore neither native secret scanning nor CodeQL is claimed as enabled.

GitHub documentation states that CodeQL/code scanning for private repositories requires GitHub Code Security, and native secret scanning for private repositories depends on Secret Protection and eligible ownership/plan. The deterministic repository/history secret-hygiene test remains the enforceable fallback in this repository.

## Rate limiting

No application-level rate limiter was added. Mutation request bodies are fixed and bounded, database concurrency is constrained by the existing pool/constraints, and Solana RPC calls now have deadlines. Request-frequency controls depend on public ingress topology and client identity, so they belong at the deployment/reverse-proxy boundary for a public deployment. The hackathon compose configuration does not prove such an ingress rate limit and this remains a deployment gate.

## Physical/hardware gates still external

The following are not marked PASS by this software review:
- real ESP32-C5 execution;
- real RFID reader protocol and captured real reader frames;
- real tags and electrical/UART characteristics;
- physical reboot/power-loss behavior;
- actual eFuse key provisioning, key purpose, read protection, and post-reboot persistence;
- real browser-wallet extension interoperability.

Runtime firmware still must not provision or mutate eFuse state automatically. The CI eFuse configuration gate is compile-only.

## Anything not verified

Because the GitHub-hosted runner never started:
- root and chain `cargo-audit` results are not available;
- npm runtime/full audit results are not available;
- Rust/Python/web test commands on this hardening head were not executed by GitHub Actions;
- chain LiteSVM/Anchor tests were not executed on this hardening head;
- firmware builds were not executed on this hardening head;
- deterministic full-stack E2E and three-run stability were not executed on this hardening head.

No statement in this report treats those items as PASS.

## Research sources used for security-sensitive decisions

- Axum 0.8 request-body limits: https://docs.rs/axum/0.8.9/axum/extract/struct.DefaultBodyLimit.html
- reqwest request timeout behavior: https://docs.rs/reqwest/0.12/reqwest/struct.ClientBuilder.html
- RustSec cargo-audit documentation: https://github.com/RustSec/rustsec/tree/main/cargo-audit
- RustSec cargo-audit 0.22.2 release: https://github.com/RustSec/rustsec/releases/tag/cargo-audit%2Fv0.22.2
- npm 11 audit documentation: https://docs.npmjs.com/cli/v11/commands/npm-audit/
- Rust Fuzz Book / cargo-fuzz: https://rust-fuzz.github.io/book/cargo-fuzz.html
- GitHub Actions secure-use guidance: https://docs.github.com/en/actions/reference/security/secure-use
- GitHub-hosted runners: https://docs.github.com/en/actions/reference/runners/github-hosted-runners
- GitHub Actions billing/usage: https://docs.github.com/en/actions/concepts/billing-and-usage
- GitHub CodeQL private-repository availability: https://docs.github.com/en/code-security/reference/code-scanning/troubleshoot-analysis-errors/private-repository-enablement
- GitHub secret scanning availability: https://docs.github.com/en/code-security/concepts/secret-security/secret-scanning

## Completion statement

This phase produced stronger deterministic source-level security controls and executable regression coverage without weakening Lastro's protocol or canonical-state invariants. The remaining blocker to the requested all-green completion criterion is execution infrastructure plus explicitly external hardware/wallet gates. The repository must not be described as universally secure or production-ready from this evidence alone.

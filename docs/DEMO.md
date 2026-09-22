# Demo

Maximum presentation target: 3 minutes.

## Operator flow

1. Show RFID and `visual_recovery_id` as separate physical identifiers.
2. Create one AnimalID using the visual recovery identifier.
3. Connect Wallet A.
4. ORIGIN with physical RFID A.
5. TRANSFER A → B after a fresh physical read of RFID A.
6. Reconnect stale Wallet A and show the explicit rejection without consuming a capture/event.
7. Treat RFID A as unavailable.
8. Recover the same AnimalID through `visual_recovery_id`.
9. Connect Wallet B, physically read RFID B, and REIDENTIFY.
10. TRANSFER B → C after a fresh physical read of RFID B.
11. Open the independent verifier and show all five layers `VALID`.
12. Change one byte in the exported `EvidencePackage` and show `INVALID`.

At the end, canonical expectations are:

```text
same AnimalID
sequence = 4
identity_revision = 2
current RFID = RFID B hash
current custodian = Wallet C
RFID A binding = RETIRED
RFID B binding = ACTIVE
```

## Pre-demo validation

Run G3, the Playwright browser flow, and G5 in the actual demo environment before presenting. Do not repair the demonstration by manually editing PostgreSQL, Agent SQLite, event bytes, transaction data, or Solana accounts.

For the disposable zero-cost local software environment, the preferred command runs all three gates against a fresh local validator/program/database runtime:

```bash
make local-demo-test
```

The equivalent low-level commands remain:

```bash
LASTRO_SYSTEM_TEST=1 python3 -m pytest -q tests/system/test_full_local.py tests/system/test_recovery.py tests/system/test_tamper.py
LASTRO_E2E_SYSTEM=1 npm --workspace @lastro/web run test:e2e
LASTRO_DEMO_STABILITY_TEST=1 python3 -m pytest -q tests/system/test_demo_stability.py
```

Physical hardware, Devnet, and real browser-wallet-extension interoperability are separate gates. Playwright uses deterministic Wallet Standard test wallets; before presenting, repeat the authorization/reload/rejection path with the chosen installed wallet extension. If any external gate has not actually run successfully, label it `NOT VERIFIED IN THIS ENVIRONMENT` rather than implying that the local harness proves it.

Final message:

> Lastro turns signed physical evidence into verifiable identity and custody infrastructure for physical assets.

Do not claim biological identity, EUDR compliance, or hardware-backed key storage unless the corresponding evidence gate has actually passed.

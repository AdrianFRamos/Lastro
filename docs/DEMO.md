# Demo

Maximum presentation target: 3 minutes. Operator console: `/demo`.

## Operator flow

1. Connect the deployment authority wallet (Wallet A) and **Register animal** with Wallet B as
   the initial custodian. The AssetID now exists on Solana with no RFID.
2. Connect Wallet B and **Bind RFID**: the Station reads tag A and signs the envelope; Wallet B
   signs the transaction.
3. Still as Wallet B, **Record presence**: the Station reads tag A again (presence proof).
4. Treat tag A as lost and **Replace RFID** with tag B.
5. Look up tag A's hash: it is `RETIRED` and still resolves to the same AssetID.
6. **Custody transfer** B → C: Wallet B proposes, Wallet C accepts on-chain.
7. Reconnect Wallet B and show the explicit stale-custodian rejection.
8. Open the independent verifier and show all five layers `VALID`.
9. Change one byte of an envelope in the exported `EvidencePackage` and show `INVALID`.

At the end, canonical expectations are:

```text
same AssetID
current RFID     = tag B hash
tag A binding    = RETIRED (resolves to the same AssetID)
tag B binding    = ACTIVE
custodian        = Wallet C
events           = IDENTIFIER_BOUND, OBSERVATION_RECORDED, IDENTIFIER_REPLACED
```

## Pre-demo validation

Run the gates in the actual demo environment before presenting. Do not repair the demonstration
by manually editing PostgreSQL, Agent SQLite, envelope bytes, transaction data, or Solana accounts.

```bash
make local-demo-test
```

This deploys the v2 program to a fresh local validator, initializes `ProtocolConfigV2` and the
Station, and runs G3 (the flow above plus stale-custodian, tamper and independent verification
checks), G5 (three consecutive runs) and the browser checks. See `docs/TESTING.md`.

Physical hardware, Devnet, and real browser-wallet-extension interoperability are separate gates.
If any external gate has not actually run successfully, label it
`NOT VERIFIED IN THIS ENVIRONMENT` rather than implying that the local harness proves it.

Final message:

> Lastro turns signed physical evidence into verifiable identity and custody infrastructure for physical assets.

Do not claim biological identity, EUDR compliance, or hardware-backed key storage unless the
corresponding evidence gate has actually passed.

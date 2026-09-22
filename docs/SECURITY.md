# Security model

## Protected properties

1. Station signature is bound to the exact bytes used by the program.
2. Current custodian is verified on-chain.
3. Sequence + predecessor make ordering explicit.
4. Revision advances exactly once on REIDENTIFY.
5. Current RFID is globally unique through `RfidBinding`.
6. A retired RFID cannot silently create a second history.
7. PostgreSQL cannot rewrite canonical custody.

## Attacks that require tests

- missing precompile;
- wrong Station;
- valid signature over a different message;
- wrong offsets/indexes;
- old custodian;
- replay;
- sequence gap;
- wrong predecessor;
- stale revision;
- old RFID after REIDENTIFY;
- RFID already active for another AnimalID;
- altered/omitted/forked evidence package;
- backend projection different from RPC.

## What we do not prove

- biological identity;
- impossibility of physically removing a tag;
- legal ownership;
- regulatory compliance;
- absolute sensor truth;
- bilateral commercial acceptance.

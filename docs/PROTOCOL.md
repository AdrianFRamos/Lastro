# Protocol (v2)

Lastro has a single protocol. The Station signs a fixed **220-byte** domain envelope; the
Lastro v2 Solana program anchors it after verifying the Station signature through the
Secp256r1 precompile. Reference implementation: `crates/lastro-protocol/src/v2`.

## Domain envelope

Fixed format, little-endian, **220 bytes**.

| Field | Offset | Size |
|---|---:|---:|
| schema_version = 1 | 0 | 2 |
| event_type | 2 | 2 |
| deployment_id | 4 | 32 |
| subject_id (AssetID) | 36 | 32 |
| event_id | 68 | 32 |
| state_version | 100 | 8 |
| expected_previous_hash | 108 | 32 |
| payload_hash | 140 | 32 |
| source_id (StationId) | 172 | 32 |
| observed_at (i64) | 204 | 8 |
| expires_at (i64) | 212 | 8 |

`deployment_id`, `subject_id`, `event_id` and `source_id` are never zero. The first event of an
asset has an all-zero `expected_previous_hash`. `expires_at - observed_at` is at most the
deployment's `max_event_age_seconds` (86,400 by default).

Physical capture event types:

```text
 2 OBSERVATION_RECORDED   the active tag was read (presence proof)
18 IDENTIFIER_BOUND       first RFID of an asset
19 IDENTIFIER_REPLACED    lost/damaged tag replaced; the old tag is retired, never reused
```

## Hashes

```text
rfid_hash    = SHA-256("LASTRO_RFID\0"       || canonical_rfid[8])
station_id   = SHA-256("LASTRO_STATION\0"    || compressed_p256_pubkey[33])
event_hash   = SHA-256("LASTRO_V2_EVENT\0"   || envelope[220])
payload_hash = SHA-256("LASTRO_V2_PAYLOAD\0" || payload)
event_id     = SHA-256("LASTRO_V2_CAPTURE_EVENT\0" || capture_id[16])   (capture events)
```

Identity and presence events commit to `payload = old_rfid_hash || new_rfid_hash` (64 bytes):

| event_type | Station rule (`h` = hash of the tag read) | payload |
|---|---|---|
| IDENTIFIER_BOUND | expected RFID is zero | `0 ‖ h` |
| IDENTIFIER_REPLACED | `h ≠ expected ≠ 0` | `expected ‖ h` |
| OBSERVATION_RECORDED | `h = expected` | `h ‖ h` |

### Canonical RFID representation

The FDX-B telegram carries a **logical 64-bit identification code**. The reader interface may
expose it as decimal, hexadecimal, manufacturer-specific byte order, or inside a larger frame, so
Lastro never hashes the raw reader frame:

```text
reader-specific frame
→ validate framing/checksum according to the selected reader manual
→ extract the logical 64-bit identification value
→ interpret it as an unsigned integer
→ serialize as exactly 8 big-endian bytes
→ canonical_rfid[8]
```

The 8 big-endian bytes are a Lastro convention, not the RF transmission order (FDX-B transmits
LSB-first). `docs/HARDWARE.md` must record the reader model, manual and repeated frames from at
least two tags before the adapter is frozen (gate G1).

## P-256

```text
message    = envelope[220]            (raw bytes, never event_hash)
public key = SEC1 compressed P-256, 33 bytes, prefix 0x02/0x03
signature  = compact IEEE-P1363 r||s, 64 bytes
s          = 1..floor(n/2)            (low-S required)
```

The Secp256r1 precompile hashes the referenced message with SHA-256; passing `event_hash` would
add a second hash and break interoperability. The browser verifier uses `@noble/curves` with the
compressed key and compact signature and checks low-S explicitly. No layer uses DER as the
canonical format. `test-vectors/v2-capture.json` must pass in Rust, firmware C, the simulator,
the browser and `scripts/check_vectors.py`.

## On-chain envelope

Station events use exactly two protocol instructions, optionally followed by up to two
account-less ComputeBudget instructions (priority fee):

```text
instruction[0]     = Secp256r1SigVerify1111111111111111111111111
instruction[1]     = Lastro v2 bind_identifier | replace_identifier | record_observation
instruction[2..=3] = ComputeBudget111111111111111111111111111111 (optional)
```

Instruction 1 data:

```text
discriminator(8) | subject_id(32) | event_id(32) | station_id(32) | envelope(220) | rfid args
rfid args: bind = new_rfid_hash(32); replace = old(32) || new(32); observation = none
```

Accounts: signer, ProtocolConfigV2, station registry, StationRecord, AssetState (w), EventAnchor
(w), one RfidBinding per RFID argument (w), instructions sysvar, system program. The signer is the
asset's current custodian; observations may also be signed by the deployment authority (for
capture-less relays), so day-to-day operation never needs the authority key.

Secp256r1 descriptor (instruction 0, all `u16` little-endian):

```text
offset  size  field
0       1     count = 1
1       1     padding = 0
2       2     signature_offset = 16
4       2     signature_instruction_index = 0
6       2     public_key_offset = 80
8       2     public_key_instruction_index = 0
10      2     message_data_offset = 104
12      2     message_data_size = 220
14      2     message_instruction_index = 1
16      64    signature r||s
80      33    Station compressed P-256 pubkey
113           end
```

The program reads `sysvar::instructions`, checks program, indexes, offsets, size and public key
against the envelope of the instruction being processed, and rejects any instruction after
index 1 other than ComputeBudget. The runtime reads ComputeBudget from any position, so indices
0 and 1 never move. The browser refuses a priority fee above 200,000 lamports.

Wallet-only instructions (`register_asset`, custody `create_intent` / `accept_custody_transfer`)
are single Lastro instructions, optionally followed by the same ComputeBudget pair.

## Canonical state

- `AssetState.current_rfid_hash` is the active tag (zero before the first bind).
- `RfidBinding` (PDA `["rfid", deployment, rfid_hash]`) is **ACTIVE** or **RETIRED** and is never
  closed: a retired tag still resolves to the asset it identified and cannot be bound again.
- `EventAnchor` (PDA `["event", deployment, event_id]`) stores the event hash, payload hash and
  source of every anchored event.
- Custody changes in two phases (custodian proposes, recipient accepts) and bumps
  `state_version` without an event; a capture is therefore bound to `state_version ≥ 1` and to
  the asset's `last_event_hash`, not to a fixed version sequence.

## Serial

The serial protocol is binary and ABI-independent. Firmware C and Agent Rust encode fields at
explicit offsets; `memcpy` of a C struct or serde/bincode serialization is not the wire format.

### Frame

```text
offset  size  field
0       4     magic = ASCII "LSTR"
4       1     version = 1
5       1     message_type
6       2     reserved = 0, little-endian
8       4     payload_len u32_le
12      N     payload
12+N    4     crc32c u32_le
```

CRC32C (Castagnoli) covers exactly `version || message_type || reserved || payload_len || payload`.
The standard vector `"123456789" → 0xe3069283` is tested in C and Rust. `payload_len` never
exceeds 1024 and is rejected before allocation.

Types: `1 COMMAND`, `2 EVENT_READY`, `3 ACK`, `4 ERROR`, `5 DOMAIN_EVENT_READY`, `6 DOMAIN_ACK`.

### COMMAND — 202 bytes

```text
offset  size  field
0       16    capture_id (canonical UUID bytes)
16      2     event_type u16_le (2, 18 or 19)
18      32    deployment_id
50      32    asset_id
82      32    event_id
114     8     state_version u64_le (asset version + 1)
122     32    previous_event_hash (zero before the first event)
154     32    expected_rfid_hash (zero for BOUND)
186     8     observed_at i64_le (host clock)
194     8     expires_at i64_le
202           end
```

`COMMAND` never contains the observed RFID, a new RFID hash, or a signature: the Station reads the
tag itself and derives the payload.

### EVENT_READY — 341 bytes

```text
offset  size  field
0       16    capture_id
16      220   signed envelope
236     8     canonical observed_rfid
244     33    compressed SEC1 station_pubkey
277     64    compact r||s station_signature
341           end
```

The Agent and API rebuild the envelope from their own capture context plus `observed_rfid` and
require byte equality before accepting it. The Agent never reserializes the envelope.

### ACK — 48 bytes

```text
0   16  capture_id
16  32  event_hash
48      end
```

The Agent sends ACK only after EVENT_READY is persisted in SQLite with state `LOCAL`. The Station
has no durable journal, so power loss before this boundary requires a new physical capture.

### ERROR — 20 bytes

```text
0   16  capture_id (zero UUID allowed before accepting a capture)
16  2   error_code u16_le
18  2   reserved = zero
20      end
```

Codes: `1 INVALID_COMMAND`, `2 RFID_READ_FAILED`, `3 INVALID_EVENT_CONTEXT`, `4 SIGNING_FAILED`,
`5 BUSY`. No variable text enters the wire protocol.

### Shared fixtures

`test-vectors/v2-serial-command.bin`, `v2-serial-event-ready.bin`, `v2-serial-ack.bin` and
`serial-error.bin` freeze the payloads above (regenerate with
`cargo run -p lastro-protocol --example generate_v2_vectors`). Rust and firmware C compare
complete bytes, not only decoded fields.

**Known limit:** `observed_at` comes from the host and is set 300 s before the capture, because the program requires `observed_at <= Clock` and the cluster clock (especially at finalized commitment) runs behind wall time; it is a lower bound of the capture window, not the read instant. The Station attests the physical read and the
context, not the time; a signed clock (GNSS/RTC) is a pilot item.

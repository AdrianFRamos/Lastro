# Protocol

## StationEvent

Fixed format, little-endian, **276 bytes**.

| Field | Offset | Size |
|---|---:|---:|
| magic `"LSTR"` | 0 | 4 |
| version = 1 | 4 | 1 |
| action | 5 | 1 |
| reserved = 0 | 6 | 2 |
| deployment_id | 8 | 32 |
| animal_id | 40 | 32 |
| station_id | 72 | 32 |
| event_sequence | 104 | 8 |
| identity_revision | 112 | 4 |
| previous_event_hash | 116 | 32 |
| old_rfid_hash | 148 | 32 |
| new_rfid_hash | 180 | 32 |
| from_custodian | 212 | 32 |
| to_custodian | 244 | 32 |

Actions:

```text
1 ORIGIN
2 TRANSFER
3 REIDENTIFY
```

## Hashes

```text
rfid_hash =
SHA-256("LASTRO_RFID\0" || canonical_rfid)

station_id =
SHA-256("LASTRO_STATION\0" || compressed_p256_pubkey)

event_hash =
SHA-256(StationEvent[276])
```

### Canonical RFID representation

The FDX-B telegram carries a **logical 64-bit identification code**. The reader interface may expose this value as decimal, hexadecimal, manufacturer-specific byte order, or inside a larger frame. Therefore, the Lastro protocol does not hash the raw frame received from the reader.

Contract:

```text
reader-specific frame
→ validate framing/checksum according to the selected reader manual
→ extract the logical 64-bit identification value
→ interpret it as an unsigned integer
→ serialize as exactly 8 big-endian bytes
→ canonical_rfid[8]
```

The use of **8 big-endian bytes** is a Lastro canonical convention. It does not describe RF transmission order. The FDX-B transponder documentation used as reference states that the telegram transmits bits LSB-first; precisely for that reason, the reader adapter must separate the physical/serial format from the protocol canonical representation.

Before freezing the hardware adapter, `docs/HARDWARE.md` must record the reader manufacturer, model, revision, and manual, as well as repeated frames from at least two tags. If the reader cannot deterministically reconstruct the 64 bits of the logical identification code, it does not satisfy gate G1.

## P-256

The Station signs the **raw 276 bytes** of `StationEvent` with ECDSA P-256/SHA-256.

```text
message = StationEvent[276]
digest  = SHA-256(message)
signature = ECDSA-P256(digest) encoded as r||s (64 bytes, low-S)
```

`event_hash` is also `SHA-256(StationEvent[276])`, so it has the same 32 bytes as the event cryptographic digest. This **does not** mean the precompile receives `event_hash` as its message.

The Solana Secp256r1 precompile must point to the **raw 276 bytes** located inside the Lastro instruction. The precompile applies ECDSA/SHA-256 semantics to the referenced message. Passing the 32 bytes of `event_hash` would introduce a second SHA-256 and break interoperability with the Station.

Format:

```text
public key = SEC1 compressed P-256, 33 bytes, prefix 0x02/0x03
signature  = compact IEEE-P1363 r||s, 64 bytes
s          = 1..floor(n/2)  (low-S required)
message    = StationEvent[276]
```

The web verifier uses `@noble/curves` P-256 with the protocol compressed key and compact signature. The library receives the non-prehashed message and performs SHA-256 for P-256; Lastro also performs an explicit low-S check before verification. This choice avoids depending on WebCrypto import support for compressed EC points, which is not uniform across runtimes.

Interoperability vectors in `test-vectors/vectors.json` must pass in Rust, firmware C, browser, and the Solana builder. No layer may convert the signature to DER as the canonical protocol format.

## On-chain Secp256r1 envelope

The hackathon transaction contains exactly two protocol instructions:

```text
instruction[0] = Secp256r1SigVerify1111111111111111111111111
instruction[1] = Lastro origin/transfer/reidentify(event: [u8;276])
```

For one signature, the official precompile uses `u8 count`, `u8 padding`, and seven `u16` fields in `Secp256r1SignatureOffsets`. The Lastro builder freezes the following layout for instruction[0]:

```text
offset  size  field
0       1     count = 1
1       1     padding = 0
2       2     signature_offset = 16
4       2     signature_instruction_index = 0
6       2     public_key_offset = 80
8       2     public_key_instruction_index = 0
10      2     message_data_offset = DERIVED from serialized instruction[1]
12      2     message_data_size = 276
14      2     message_instruction_index = 1
16      64    signature r||s
80      33    Station compressed P-256 pubkey
113            end
```

All `u16` values are little-endian. `message_data_offset` is never a constant based on an assumption about the Anchor discriminator: the transaction builder serializes the real Lastro instruction, finds exactly one occurrence of the 276 `event_bytes`, and uses that offset. If the subsequence is not present exactly once or does not fit in `u16`, the builder fails.

The program reads `sysvar::instructions` and checks program/indexes/offsets/size/public key and the 276 bytes of the current instruction. The precompile already performs ECDSA P-256 verification and requires low-S; Lastro proves that the successful signature corresponds **to the event being processed**.

## Semantics

### ORIGIN
sequence=1, revision=1, predecessor=zero, old RFID=zero, new RFID=observed, from=zero, to=initial custodian.

### TRANSFER
sequence=current+1, revision=current, predecessor=current.last_event_hash, old/new RFID=current, from=current custodian, to=destination.

### REIDENTIFY
sequence=current+1, revision=current+1, predecessor=current.last_event_hash, old RFID=current, new RFID=observed, from=to=current custodian.

The old RFID does not need to be physically present during `REIDENTIFY`.

## Serial

The serial protocol is binary and ABI-independent. Firmware C and Agent Rust encode fields at explicit offsets; `memcpy` of a C struct or serde/bincode serialization is not part of the wire format.

### Envelope

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

CRC32C uses the Castagnoli polynomial and covers **exactly**:

```text
version || message_type || reserved || payload_len || payload
```

`magic` and the CRC itself are outside the coverage. The standard CRC32C vector `"123456789" → 0xe3069283` must appear in both C and Rust tests before testing Lastro frames.

Types:

```text
1 COMMAND
2 EVENT_READY
3 ACK
4 ERROR
```

`payload_len` may never exceed 1024 bytes. The decoder rejects the declared size before allocating/copying the payload.

### COMMAND — 224 bytes

`capture_id` uses the 16 canonical UUID bytes (`Uuid::as_bytes()` in the Agent), without strings, hyphens, or numeric conversion. All multi-byte integers use little-endian.

```text
offset  size  field
0       16    capture_id
16      1     action (1 ORIGIN, 2 TRANSFER, 3 REIDENTIFY)
17      3     reserved = zero
20      32    deployment_id
52      32    animal_id
84      8     event_sequence u64_le
92      4     identity_revision u32_le
96      32    previous_event_hash
128     32    expected_old_rfid_hash
160     32    from_custodian
192     32    to_custodian
224           end
```

`COMMAND` **never** contains `new_rfid_hash`, observed RFID, or a signature. Backend/Agent provide only expected context; the Station derives the new hash from the physical reading.

### EVENT_READY — 397 bytes

```text
offset  size  field
0       16    capture_id
16      276   signed StationEvent bytes
292     8     canonical observed_rfid
300     33    compressed SEC1 station_pubkey
333     64    compact r||s station_signature
397           end
```

The API must be able to recompute `new_rfid_hash` from the 8 bytes of `observed_rfid`, decode the same 276 bytes, and validate the same signature. The Agent does not reserialize the event.

### ACK — 48 bytes

```text
offset  size  field
0       16    capture_id
16      32    event_hash = SHA-256(StationEvent[276])
48            end
```

The Agent sends ACK only after the corresponding EVENT_READY has been persisted in SQLite with state `LOCAL`. Because the Station has no durable journal in the hackathon, power loss before this boundary requires a new physical capture; ACK must never precede the local commit.

### ERROR — 20 bytes

```text
offset  size  field
0       16    capture_id (zero UUID allowed before accepting a capture)
16      2     error_code u16_le
18      2     reserved = zero
20            end
```

Codes:

```text
1 INVALID_COMMAND
2 RFID_READ_FAILED
3 INVALID_EVENT_CONTEXT
4 SIGNING_FAILED
5 BUSY
```

No variable text enters the wire protocol. The Agent maps the code to local logs/telemetry; error messages do not alter canonical state.

### Shared fixtures

`test-vectors/serial-command.bin`, `serial-event-ready.bin`, `serial-ack.bin`, and `serial-error.bin` freeze the binary payloads above. Rust and firmware C must compare the complete bytes, not only decoded fields.

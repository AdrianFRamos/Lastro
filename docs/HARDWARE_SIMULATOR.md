# Hardware Simulator

The hardware simulator is a development-only implementation of the Lastro **physical Station boundary**. It is intentionally separate from the production web application, API, Solana program, and protocol crate.

Its job is narrow: emulate the evidence-producing hardware path up to the exact byte stream the existing Rust Agent already consumes.

```text
SIMULATED PATH                              REAL PATH

Virtual RFID tag                           Physical FDX-B tag
      |                                           |
Generic reader-adapter boundary             Documented RFID reader
      | canonical RFID[8]                         | canonical RFID[8]
      v                                           v
Simulated ESP32-C5 Station                  ESP32-C5 Station firmware
      | exact LSTR v1 frames                      | native USB Serial/JTAG CDC
      v                                           v
Existing lastro-agent  <-------------------- same Agent contract
      |
      v
Real Lastro API -> wallet authorization -> Solana
```

The simulator does **not** post evidence directly to the API and does not reproduce backend or Solana rules. Deleting `hardware-simulator/` and the two optional Compose services must leave production runtime source unchanged.

## What is factual today

The simulator mirrors contracts already frozen in this repository:

- canonical RFID input is exactly 8 bytes representing Lastro's big-endian serialization of the logical 64-bit RFID value;
- the Station consumes RFID only while a valid capture is in `WAIT_RFID`;
- Station events are exactly 276 bytes;
- Station signatures are ECDSA P-256 / SHA-256, compact `r || s`, normalized to low-S;
- the Agent wire is `LSTR` version 1 with `COMMAND`, `EVENT_READY`, `ACK`, and `ERROR` frames and CRC-32C/Castagnoli;
- identical commands are replay-safe while a capture is active, matching the firmware state machine;
- the default simulator Station identity is the repository's frozen test-vector scalar-1 key and is never deployment key material.

The hardware facts used by the simulator are deliberately limited:

- ESP32-C5 provides a fixed-function USB Serial/JTAG controller that exposes a CDC serial channel and JTAG to the host. The production firmware uses that native interface for Agent traffic.
- ISO 11784:2024 defines the animal-identification code structure; ISO 11785:2026 defines how a transponder is activated and how stored information is transferred to a transceiver. The 134.2 kHz activation frequency used by ISO 11784/11785 animal RFID is documented in ISO/ICAR technical material.
- ICAR maintains conformance/certification material for ISO 11784/11785 animal-identification devices.

Primary references:

- Espressif, ESP32-C5 USB Serial/JTAG Console: https://docs.espressif.com/projects/esp-idf/en/stable/esp32c5/api-guides/usb-serial-jtag-console.html
- Espressif, ESP32-C5 serial connection: https://docs.espressif.com/projects/esp-idf/en/stable/esp32c5/get-started/establish-serial-connection.html
- ISO 11784:2024: https://www.iso.org/standard/83944.html
- ISO 11785:2026: https://www.iso.org/standard/91012.html
- ICAR RFID transceivers/conformance: https://www.icar.org/icar-registry-of-rfid-devices-in-conformance-with-iso-11784-11785/

## What is intentionally not simulated

No reader manufacturer/model has been selected and physically characterized in the repository. Therefore the simulator does not invent:

- GPIO assignments;
- supply voltage/current;
- UART, RS-485, TTL, or other reader bus selection;
- reader baud rate;
- manufacturer-specific raw telegram/framing/checksum;
- antenna geometry;
- read distance or field strength in physical units;
- collision/multi-tag behavior not established by the selected reader documentation and captures.

The UI therefore labels its reader as a **generic, uncalibrated adapter boundary**. Dragging a tag into the visual antenna means only: “a reader adapter has produced one validated canonical RFID value.”

When a reader is selected, its adapter should be implemented and validated behind the existing firmware `lastro_rfid_poll()` / `lastro_rfid_feed()` / `lastro_rfid_take()` boundary. Real reader frames and the manufacturer manual then become the evidence for any reader-specific simulator profile.

## Run the simulator only

The UI can be launched without the application stack:

```bash
docker compose -f infra/compose.yml up --build hardware-simulator
```

Open:

```text
http://127.0.0.1:8090
```

The simulator's LSTR wire port is exposed only to the Compose network, not published on the host.

## Run it against the real Agent/API path

The integrated profile requires the same deployment inputs as the normal application. For the default simulator key, set `LASTRO_STATION_PUBKEY_HEX` to the repository's public test-vector key:

```text
036b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296
```

Then enable both profiles:

```bash
docker compose -f infra/compose.yml --profile app --profile hardware-sim up --build
```

Relevant endpoints are then:

```text
Lastro web              http://127.0.0.1:8088
Lastro API              http://127.0.0.1:8080
Hardware Simulator      http://127.0.0.1:8090
```

The `hardware-simulator-agent` container runs the **existing** `lastro-agent`. A simulator-owned `socat` bridge creates a local PTY and forwards its bytes to the simulator TCP wire. The Agent therefore continues to open `LASTRO_AGENT_SERIAL_PORT` through `SerialStationTransport`; production Agent source does not contain a simulator transport.

`LASTRO_AGENT_SERIAL_BAUD=115200` remains configured because it is a required Agent serial setting. The PTY/TCP bridge does not model physical UART timing, and the real ESP32-C5 native USB CDC transport is not an RFID-reader baud-rate claim.

The Agent bridge persists its SQLite outbox in the `lastro-agent-sim` Compose volume. `docker compose down -v` removes this development state.

## Field-oriented simulator workflow

1. Start the real Lastro application plus `hardware-sim` profile.
2. In the Lastro Demo, create/recover the AnimalID and start `ORIGIN`, `TRANSFER`, or `REIDENTIFY` normally.
3. The real API dispatches a `COMMAND` to the real Agent.
4. The Agent writes that command to its serial transport. The PTY bridge forwards the exact LSTR bytes to the simulator.
5. The simulator UI moves to `WAIT_RFID` and shows the immutable command context.
6. Drag an RFID ear tag into the reader field or press **Read**.
7. The simulator consumes the canonical RFID only while `WAIT_RFID`, builds the exact StationEvent, signs it, and emits `EVENT_READY` to the Agent.
8. The real Agent verifies/persists the evidence and sends the matching `ACK`.
9. The simulator returns to `IDLE`. Everything after the Agent boundary remains the real Lastro stack.

Two fixture tags are preloaded from `test-vectors/vectors.json`:

```text
Tag A  8000130000000001
Tag B  8000130000000002
```

Custom simulator tags must be exactly 16 hexadecimal characters (8 bytes). Their values are protocol-level canonical IDs, not claims about a physical reader's text representation.

## Fault injection

Fault controls are one-shot unless explicitly cleared. They exercise existing fail-closed contracts:

- **Reader / fail next observation** -> Station `RFID_READ_FAILED`;
- **Station / busy next command** -> Station `BUSY`;
- **Signer / fail next signature** -> Station `SIGNING_FAILED`;
- **Transport / corrupt next CRC32C** -> malformed LSTR output that the Agent framing layer must reject;
- **Disconnect Agent wire** -> closes the simulator side of the serial bridge;
- **Reset simulator** -> clears the current simulated capture and all armed one-shot faults.

Fault injection never generates an optimistic success result.

## Tests

Simulator-only contracts:

```bash
PYTHONPATH=hardware-simulator/src \
  python -m pytest -q hardware-simulator/tests
```

The suite checks the frozen CRC vector, fragmentation/recovery, exact StationEvent fixture bytes, low-S P-256 verification, firmware-equivalent command replay, fault behavior, architectural isolation, and one live HTTP/TCP capture through `COMMAND -> RFID -> EVENT_READY -> ACK -> IDLE`.

Repository validation remains authoritative for shared contracts:

```bash
python scripts/spec_check.py
python -m pytest -q tests/repository
cargo test --locked --workspace
npm --workspace @lastro/web run test
```

Physical hardware gates remain separate. A simulator pass never counts as evidence that a physical reader, antenna, ESP32-C5 board, USB path, or eFuse-backed key has passed a hardware gate.

## Removal boundary

Production source under `apps/`, `services/`, `crates/`, `chain/`, and `firmware/` must not import or reference simulator implementation modules. The simulator-specific integration surface is limited to:

```text
hardware-simulator/
infra/compose.yml              # optional profile only
docs/HARDWARE_SIMULATOR.md     # this document
generated repository indexes/manifests
```

That boundary is itself covered by a repository test in `hardware-simulator/tests/test_repository_boundary.py`.

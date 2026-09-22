# Hardware

## Required

- ESP32-C5-DevKitC-1
- documented FDX-B reader
- compatible antenna
- two or more FDX-B tags
- visual identifier physically separate from RFID
- reliable data USB cable
- correct reader power supply
- common ground and verified logic levels

## Agent link

The ESP32-C5 Station uses the chip's fixed-function native USB Serial/JTAG CDC port for the binary Agent protocol. The firmware installs the interrupt-driven `usb_serial_jtag` driver directly and keeps the normal ESP-IDF log console on the default UART, with the secondary USB console disabled, so logs do not share the `LSTR` frame stream.

The Agent opens the resulting host serial device through `LASTRO_AGENT_SERIAL_PORT`. `LASTRO_AGENT_SERIAL_BAUD` remains part of the host serial API configuration; the native USB CDC transport itself does not depend on an RFID-reader baud rate or GPIO assignment.

The board loop accepts Agent COMMAND/ACK frames, retries Station output through the existing runtime, and consumes a canonical RFID only while the Station is waiting for physical evidence. Reader-specific raw I/O, bus polling, and framing stay behind `lastro_rfid_poll()` / `lastro_rfid_feed()` / `lastro_rfid_take()`; integrating the selected reader does not require changing the board loop. The real reader adapter must discard stale observations acquired outside the active capture and must not emit a canonical RFID until one complete frame passes the selected reader's documented integrity checks.

## Reader gate

No pinout, parser, or canonicalization is final before recording from the real hardware:

1. manufacturer;
2. model/revision;
3. manual/datasheet;
4. supply voltage and current;
5. logic level;
6. interface;
7. baud/bus;
8. a real frame from at least two tags;
9. checksum/framing;
10. timeout/error behavior.

The repository does not invent GPIO assignments, reader bus selection, buffer sizing, or reader framing. Reader-specific Kconfig options are added only after the real reader evidence establishes those facts. The adapter output, however, is already frozen by the protocol: exactly the **8 bytes of the 64-bit ISO 11784 identification code, bit 1/MSB first**. The manual and real frames are used to prove how the selected reader is converted into these 8 bytes without ambiguity.

## P-256 key

First prove the flow with a controlled development key.

Then, optionally, run H1 with eFuse and repeat the same vectors. The claim `hardware-backed Station key` may appear only if H1 actually passes.

## Hardware evidence

Each G1 execution must make it possible to save:
- reader frame;
- canonical RFID;
- StationEvent bytes;
- public key;
- signature;
- host verification result;
- firmware build/version;
- board/reader identification.

GNSS is not part of the hackathon core.

## Optional eFuse-backed Station key

The eFuse signer is implemented for ESP32-C5 but remains an **H1 hardware gate** until it is executed on a manually provisioned board. Firmware never calls an eFuse write API and never provisions a key automatically.

Provisioning is an explicit operator action. With ESP-IDF 6.1, the documented CLI form is:

```bash
idf.py efuse-burn-key BLOCK_KEY<N> /path/to/ecdsa_private_key.pem ECDSA_KEY
```

where `<N>` is `0` through `5` on ESP32-C5. Treat this operation as irreversible. Review the selected physical block and the generated key before executing it.

For a firmware build that uses the provisioned key:

1. enable ESP-IDF `MBEDTLS_HARDWARE_ECDSA_SIGN`;
2. enable `LASTRO_STATION_USE_EFUSE_KEY`;
3. set `LASTRO_STATION_EFUSE_KEY_BLOCK_INDEX` to the selected `KEY0` through `KEY5` index;
4. build and flash normally.

At runtime the signer fails closed unless all of these conditions hold:

- the target supports P-256 hardware ECDSA;
- the selected key block is in the ESP32-C5 KEY0..KEY5 range;
- the block purpose is `ECDSA_KEY_P256`;
- the block is read-protected from software;
- ESP-IDF can import the block as an opaque P-256 PSA key reference;
- the public key can be exported from the hardware key reference.

Signing hashes the exact 276-byte `StationEvent` once with SHA-256 and then calls PSA `sign_hash`. The returned compact `r || s` signature is normalized to low-S before it leaves the Station, matching the development signer and Solana Secp256r1 binding. No private key bytes are read by Lastro firmware.

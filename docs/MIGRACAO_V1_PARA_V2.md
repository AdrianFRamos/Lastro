# Migração v1 → v2 (remoção do v1)

**Decisão (30/09/2026):** o produto passa a ter um único protocolo, o v2. O v1 (programa
`lastro`, `StationEvent` de 276 bytes, rotas `/api/animals|captures|events`, demo e verificador
v1) é removido **depois** que o fluxo físico completo existir no v2.

## Fases

| Fase | Conteúdo | Status |
|---|---|---|
| 1 | Contrato v2: `bind_identifier` / `replace_identifier`, `RfidBinding` nunca fechado, `AssetState.current_rfid_hash` | Concluída |
| 2 | Contrato serial v2 (protocolo, Agent, simulador, firmware) | Concluída: vetores `test-vectors/v2-capture.json`; Agent 47 testes, simulador 19, firmware 29 (host/MSVC) |
| 3 | API: registro de ativo, captura v2, transação de identidade, confirmação, pacote de evidência v2 | Concluída: rotas v1 removidas (migração `0017_drop_v1.sql`), OpenAPI v2, `tests/capture_v2.rs` (exige PostgreSQL) |
| 4 | Frontend: fluxo de carteira e verificador independente v2 | Concluída: console (registro, bind, presença, substituição, custódia em duas fases), validação de transação no navegador, verificador `lastro.evidence-package.v2`; 92 testes Vitest, typecheck e build |
| 5 | Remoção do v1 (programa, rotas, telas, testes, vetores, scripts, CI, docs) | Concluída (ver abaixo) |

## O que foi removido na fase 5

- Programa `chain/programs/lastro` e sua entrada em `chain/Cargo.lock`.
- Módulos v1 do crate de protocolo (`StationEvent`, `Action`, `EvidencePackage` v1) e seus testes.
- Vetores v1 (`vectors.json`, `*.bin` de 276 bytes, pacotes de evidência v1) e `schemas/evidence-package.schema.json`.
- Harness de sistema v1 em Python (`tests/system`) e os specs Playwright de pilha completa que dependiam dele.
- Script `bootstrap_program_id.sh` v1 (o de v2 passou a ter esse nome).

Substituições: `scripts/solana_tools.py` (utilitários RPC/carteira/PDA), `scripts/initialize_protocol_config.py`
(inicializa `ProtocolConfigV2` **e** registra a Station), `scripts/check_vectors.py` (verificação
independente dos vetores v2), `scripts/generate_v2_evidence_fixture.py` e os testes de hardware
portados para o comando/envelope v2.

O harness de sistema foi reescrito para o v2 em `tests/system/` (G3 local, G4 Devnet, G5
estabilidade): Station por PTY com o Agent real, carteiras reais, API/PostgreSQL/programa reais e
verificação independente do pacote de evidência.

## Garantias preservadas do v1

| Garantia v1 | Onde fica no v2 |
|---|---|
| AnimalID persistente, RFID substituível | `AssetState` + `replace_identifier` |
| RFID nunca reutilizado | `RfidBinding` PDA `["rfid", deployment, rfid_hash]`, nunca fechado |
| Evento assinado pela Station e verificado on-chain | Envelope de 220 B + Secp256r1 vinculado aos bytes exatos |
| Cadeia sem fork/replay | `state_version` + `expected_previous_hash` por ativo |
| Só o custodiante move a identidade | `bind`/`replace` exigem assinatura de `asset.custodian` |
| Custódia | Duas fases on-chain (proposta do custodiante + aceite do recebedor) — mais forte que o TRANSFER push do v1 |

## Contrato serial v2 (little-endian)

O quadro serial (`LSTR`, versão, tipo, CRC) não muda. Os payloads passam a ser:

```text
COMMAND (202 B)
  0   capture_id            16   UUID (bytes canônicos)
  16  event_type            u16  2 = OBSERVATION_RECORDED, 18 = IDENTIFIER_BOUND, 19 = IDENTIFIER_REPLACED
  18  deployment_id         32
  50  asset_id              32
  82  event_id              32
  114 state_version         u64  versão do ativo + 1
  122 previous_event_hash   32   last_event_hash do ativo (zero no primeiro evento)
  154 expected_rfid_hash    32   zero para BOUND; RFID ativo para REPLACED/OBSERVATION
  186 observed_at           i64  relógio do host (a Station não tem RTC confiável)
  194 expires_at            i64

EVENT_READY (341 B)
  0   capture_id            16
  16  envelope             220   DomainEventEnvelope v2 assinado
  236 observed_rfid          8   RFID canônico lido
  244 station_pubkey33      33
  277 station_signature64   64   ECDSA P-256 (low-S) sobre SHA-256(envelope)

ACK (48 B) = capture_id(16) + event_hash(32);  ERROR (20 B) inalterado.
```

Regras da Station ao ler o RFID (`h = hash(RFID)`):

| event_type | Exigência | `payload_hash` assinado |
|---|---|---|
| IDENTIFIER_BOUND | `expected_rfid_hash == 0` | `identifier_payload_hash(0, h)` |
| IDENTIFIER_REPLACED | `h != expected_rfid_hash != 0` | `identifier_payload_hash(expected, h)` |
| OBSERVATION_RECORDED | `h == expected_rfid_hash` | `identifier_payload_hash(h, h)` |

`event_id = SHA-256("LASTRO_V2_CAPTURE_EVENT\0" || capture_id)`.

**Limite conhecido:** `observed_at` vem do host. A Station atesta a leitura física e o
contexto, não o horário. Um relógio assinado (GNSS/RTC) fica para o piloto.

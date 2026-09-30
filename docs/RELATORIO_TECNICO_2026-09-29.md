# Lastro — Relatório Técnico

**Data:** 29/09/2026
**Perspectiva:** engenharia de software, engenharia de produto, smart contracts e blockchain (Solana/Anchor)
**Base analisada:** árvore de trabalho local em `C:\Users\bahni\Downloads\Lastro` (sem `.git` neste checkout)

---

## 0. Status após a rodada de correções (29/09/2026)

| Achado | Status |
|---|---|
| **V2-01** `AssetState::SPACE` | **Corrigido.** Confirmado por execução: o teste novo falha com o valor antigo (`277 != 309`) e passa com a correção |
| V2-02 centralização | **Corrigido**: transformações assinadas pelo owner da facility; `transfer_config_authority` permite migrar para multisig |
| V2-03 linhagem sem prova | **Corrigido**: prova Merkle on-chain para insumos e produtos, e custódia dos insumos exigida |
| V2-04 sem custódia on-chain | **Corrigido**: custódia em duas fases (`CUSTODY_TRANSFER` + `accept_custody_transfer`) |
| V2-05, 07, 08, 10, 11 | **Corrigidos** (detalhes em `IMPLEMENTACAO_ETAPA_P1_LINHAGEM_TRANSFORMACAO.md`, "Corte P4") |
| Novos bugs achados durante a correção | **Corrigidos**: mesmo insumo consumível duas vezes; reservas acima do manifesto; subprodutos que impediam a finalização; suíte LiteSVM do v2 que não compilava; 2 testes da API que não compilavam; 3 erros de clippy estrito na API |
| API-03 CORS | **Corrigido**: `LASTRO_CORS_ALLOWED_ORIGINS`, obrigatória no compose de produção |
| API-05 limite de body | **Corrigido**: era uma regressão real (64 KiB em todas as rotas); v1/públicas voltaram a 1 KiB, só `/api/v2/*` usa 64 KiB |
| V1-01 / V2-12 squatting de `initialize` | **Corrigido (30/09)**: `initialize`/`initialize_v2` exigem o *upgrade authority* do programa (`ProgramData`); script de inicialização, harness LiteSVM e testes de regressão atualizados |
| V1-03 ComputeBudget | **Corrigido (30/09)**: até 2 instruções ComputeBudget **depois** da instrução Lastro (índices 0/1 e descritor Secp256r1 inalterados); API opcional via `LASTRO_PRIORITY_FEE_MICRO_LAMPORTS`; navegador limita a taxa a 200.000 lamports; confirmação e verificador independente toleram só ComputeBudget sem contas |
| API-01 aceite de custódia por token | **Corrigido (30/09)**: aceite só com a transação `accept_custody_transfer` finalizada, assinada pelo recebedor, e custodiante on-chain conferido; rotas `transaction-data?phase=propose|accept` preparam as transações para as carteiras |
| API-02 pipeline de transformação | **Corrigido (30/09)**: `POST /api/v2/transformations` (raízes/contagens/massa derivadas das folhas + provas Merkle) e `POST .../sync` (espelha a `TransformationAnchor` finalizada; FINALIZED exige a transação do dono da instalação) |
| V1-02 rotação da chave de Station (v1) | **Decisão: não alterar o v1.** Rotação exige saber qual chave assinou cada evento histórico; no v1 isso mudaria o `StationEvent` congelado e o verificador em 4 camadas. O v2 já tem registry com rotação, validade e revogação, e existe a migração v1→v2 (`/api/v2/migrations`) |
| V1-04 TRANSFER push (v1) | **Por design**: o v2 oferece o aceite em duas fases |
| V2-06 status/contas não usados | **Mantido**: `QUALITY_HOLD`/`RECALLED`/`IN_TRANSIT` são alvo de instruções futuras (recall on-chain) |
| Rotas administrativas v2 (party/facility/lot/grant) | **Mantido com token de operador**: espelham instruções que on-chain também são exclusivas da autoridade; o token é credencial administrativa, não prova de custódia |
| FW: driver RFID | **Bloqueado por decisão de hardware**: precisa do modelo do leitor e de frames reais capturados (`docs/HARDWARE.md`); implementar sem isso seria inventar o protocolo físico |
| CI | **Bloqueado fora do repositório**: os runners hospedados do GitHub não iniciam (cota/billing de repositório privado). Resolver com billing, repositório público ou runner self-hosted |

**Validação executada na rodada de 30/09:** programas v1 e v2 (com todas as suítes LiteSVM) compilando com `clippy -D warnings`; testes host de ambos passando; API com 23 testes unitários + 51 de integração sem banco + clippy estrito; contrato OpenAPI (5 testes) e testes Python de repositório (81) passando; arquivos TypeScript alterados sem erro de sintaxe. **Não executado:** LiteSVM (sem `cargo build-sbf`), testes da API com banco (sem credenciais), Vitest/typecheck completo (cache npm incompleto).

**Validação executada na rodada de 29/09:** programa v2 com testes host e clippy estrito; suíte LiteSVM compilando (execução pendente de `cargo build-sbf`); `lastro-protocol` com todos os testes; API com 51 testes sem banco + clippy estrito (os testes com banco exigem `LASTRO_DATABASE_URL`); testes Python de repositório com 81 aprovados (os 5 restantes exigem um checkout `git`).

---

## 1. Sumário executivo

O Lastro é um monorepo que implementa uma **primitiva de identidade persistente e custódia verificável para ativos físicos**, começando por bovinos. Uma Station (ESP32-C5) lê um RFID, assina um evento binário com ECDSA P-256, um Agent em Rust entrega esse evento de forma durável a uma API em Rust, e a carteira do custodiante submete uma transação Solana que valida a assinatura da Station **dentro do programa** (precompile Secp256r1 mais introspecção do sysvar de instruções). Um verificador no navegador reconstrói toda a cadeia de forma independente.

**Veredito geral:** a engenharia fica bem acima da média de hackathon. Os contratos binários são congelados, há vetores de teste compartilhados entre linguagens, a validação criptográfica acontece on-chain, os limites de confiança estão documentados com honestidade e existem cerca de 600 testes declarados. O **programa v1 está sólido** para o escopo proposto. O **programa v2 (linhagem e pós-abate) ainda não pode ir para deploy**: há um bug de alocação de conta que quebra o registro de qualquer ativo, a autorização é totalmente centralizada em uma única chave, e parte do modelo on-chain é decorativa (intents, linhagem, status que nenhuma instrução alcança).

| Dimensão | Nota (0–10) | Comentário |
|---|---|---|
| Arquitetura e separação de responsabilidades | 9 | Camadas e autoridades explícitas; o PostgreSQL é só projeção |
| Protocolo e criptografia | 8,5 | Layout fixo, hashes com separação de domínio, vetores cruzados |
| Smart contract v1 | 8 | Correto e restritivo; falta governança (rotação e revogação de chave) |
| Smart contract v2 | 4 | Bug crítico de `SPACE`; centralização total; invariantes só off-chain |
| Backend (API/Agent) | 8 | Idempotência, outbox durável, confirmação *finalized*, reconciliador |
| Firmware | 5 | Máquina de estados e assinatura prontas; **driver RFID inexistente** |
| Testes | 7 | Boa cobertura declarada, mas **CI nunca executou** e o v2 praticamente não é testado |
| Prontidão para produção | 3 | Hardware, governança de chaves, multi-tenant e v2 em aberto |

---

## 2. Escopo e método

- Li integralmente: os programas Solana v1 (`chain/programs/lastro`, cerca de 900 linhas) e v2 (`chain/programs/lastro-v2`, cerca de 2.600 linhas), os módulos centrais do protocolo (`crates/lastro-protocol`, v1 e v2), as rotas e os pontos críticos da API (autenticação, captura, submit/confirm, RPC, reconciliação, processamento v2) e o signer e o RFID do firmware.
- Revisei em nível estrutural: Agent, frontend Vue, simulador de hardware, migrations, workflows de CI e a documentação (cerca de 14,8 mil linhas em `docs/`, incluindo revisões anteriores: `REVISAO_SENIOR_PROJETO.md`, `ANALISE_PROJETO.md`, `SECURITY_VALIDATION_2026-09-21.md`).
- **Não executei os testes.** Este ambiente não tem acesso à rede (crates.io) nem os CLIs `solana`/`anchor`. A própria documentação registra que os runners do GitHub Actions **nunca iniciaram** (`runner_id: 0`, zero steps) e que `cargo test -p lastro-v2` com LiteSVM ficou "Pendente". Todas as conclusões abaixo vêm de revisão de código e de cálculo manual de layouts.

### Tamanho do código

| Componente | Linhas (aprox.) | Tecnologia |
|---|---|---|
| `chain/programs/lastro` (v1) | 905 | Anchor 1.2, Rust 2024 |
| `chain/programs/lastro-v2` | 2.635 | Anchor 1.2 |
| `crates/lastro-protocol` | 1.374 | Rust (compartilhado on/off-chain) |
| `services/api` | 9.557 + 942 SQL | Axum, Tokio, SQLx, PostgreSQL 18 |
| `services/agent` | 2.033 | Rust, SQLite outbox, serial |
| `firmware/station` | 2.561 | C, ESP-IDF 6.1, PSA/mbedTLS |
| `apps/web` | 7.869 | Vue 3.5, Vite 8, Solana Kit, Wallet Standard |
| `hardware-simulator` | 1.266 | Python |
| Testes declarados | cerca de 290 Rust, 131 TS, 136 Python, 47 C (Unity) | |

---

## 3. Arquitetura

```text
RFID físico → Station ESP32-C5 ─(StationEvent 276 B + ECDSA P-256)→ Agent (outbox SQLite)
   → API Rust (admissão criptográfica, PostgreSQL = projeção)
   → carteira do custodiante (Wallet Standard) assina a tx
   → Solana: [ix0 Secp256r1 precompile] + [ix1 Lastro] → PDAs AnimalState / RfidBinding
   → API confirma em commitment "finalized" → reconciliador
   → verificador independente no navegador (EvidencePackage + RPC)
```

**Decisões corretas que merecem destaque:**

1. **Identidade ≠ identificador físico.** O `AnimalID` é estável e o RFID é substituível (`REIDENTIFY`). É o insight de produto certo.
2. **A assinatura física é verificada on-chain**, não apenas "hash ancorado". O programa confere que a ix0 é o precompile Secp256r1 com offsets exatos, com a chave da Station da configuração e com a mensagem sendo exatamente os bytes do evento presentes na ix1.
3. **Bytes originais nunca são reserializados.** O parser trabalha em offsets fixos sobre os 276 bytes assinados, o que elimina ambiguidades de canonicalização.
4. **O banco é projeção, não fonte da verdade.** A API só avança o estado após verificar a transação exata em `finalized` e comparar as contas canônicas.
5. **A carteira do custodiante é quem assina.** A API nunca toca chaves privadas de usuário, e o desafio de captura é autenticado por `signMessage` Ed25519 (`verify_strict`).
6. **Honestidade de engenharia.** O repositório recusa "mocks de sucesso" e marca como NOT VERIFIED tudo o que depende de hardware, Devnet ou eFuse.

---

## 4. Smart contract v1 (`chain/programs/lastro`)

### 4.1 Modelo

| Conta (PDA) | Seeds | Tamanho | Função |
|---|---|---|---|
| `ProtocolConfig` | `["config", deployment_id]` | 106 B | Chave pública da Station (33 B comprimida), autoridade |
| `AnimalState` | `["animal", deployment_id, animal_id]` | 149 B | RFID atual, custodiante, revisão, sequência, último hash |
| `RfidBinding` | `["rfid", deployment_id, rfid_hash]` | 74 B | ACTIVE/RETIRED; nunca fechado, o que impede reuso de RFID |

Instruções: `initialize`, `origin`, `transfer`, `reidentify`.

### 4.2 Pontos fortes verificados

- Encadeamento completo: `event_sequence = prev + 1` (com `checked_add`), `previous_event_hash == last_event_hash` e a revisão de identidade incrementa só no `REIDENTIFY`. Isso bloqueia replay, fork e reordenação.
- A semântica local de cada ação é validada antes do estado (zeros obrigatórios no ORIGIN, `old == new` no TRANSFER, `old != new` no REIDENTIFY, bytes reservados iguais a zero).
- As PDAs derivam dos próprios bytes do evento (`event[8..40]`, `event[40..72]`, …), então conta e evento não podem divergir.
- A vinculação ao precompile é rígida: exatamente uma assinatura, índices de instrução fixos, `message_size = 276`, offset **único** do evento dentro da ix1 (duplicatas rejeitadas) e proibição de uma terceira instrução.
- `#![forbid(unsafe_code)]` e `overflow-checks = true` no perfil release.
- O `RfidBinding` nunca é fechado, o que garante globalmente que um RFID aposentado não volte a ser usado por outro animal.

### 4.3 Achados no v1

| ID | Sev. | Achado | Local |
|---|---|---|---|
| V1-01 | Média | **Squatting de `initialize`.** Qualquer conta pode inicializar a `ProtocolConfig` de um `deployment_id` ainda não usado e fixar a própria chave de Station. Não existe allowlist de autoridade nem vínculo com o *upgrade authority* do programa. | `instructions/initialize.rs` |
| V1-02 | Média | **Chave de Station única, imutável e sem revogação.** Se a chave for comprometida, todo o deployment fica comprometido e não há como rotacionar. Isso limita o sistema a uma Station por deployment. (Já reconhecido em `HARDENING_STATUS.md`.) | `state/protocol_config.rs` |
| V1-03 | Média | **Envelope de transação congelado em 2 instruções.** `load_instruction_at_checked(2).is_err()` e `current_index == 1` impedem `ComputeBudget` (priority fee e limite de CU). Em mainnet congestionada as transações podem não entrar em bloco. O próximo formato deveria permitir instruções de ComputeBudget antes da ix0. | `verify/secp256r1.rs:108` |
| V1-04 | Média (produto) | **TRANSFER "push", sem aceite do destinatário.** Só o custodiante atual assina, então o destino recebe custódia sem consentimento on-chain. Juridicamente, custódia exige aceite. | `instructions/transfer.rs` |
| V1-05 | Baixa | **Sem timestamp nem nonce no `StationEvent`.** O evento não prova *quando* ocorreu a observação; uma assinatura antiga ainda não submetida continua válida enquanto o predecessor não muda. (Já documentado.) | protocolo v1 |
| V1-06 | Baixa | O campo `authority` da `ProtocolConfig` é gravado mas nunca usado (não há `set_*` nem fechamento). | `state/protocol_config.rs` |
| V1-07 | Baixa | Nenhum `emit!`/evento Anchor. Indexadores precisam decodificar instruções em vez de consumir logs estruturados. | todas as instruções |
| V1-08 | Info | Contas nunca são fechadas, então o rent fica permanentemente travado (ver custos, §9). Nesse caso é uma decisão consciente (histórico de RFID). | — |

---

## 5. Smart contract v2 (`chain/programs/lastro-v2`)

O v2 generaliza para ativos (animal, lote, carcaça, produto), registries (Station, Facility, Party), observações com envelope de 220 B, intents, e transformações com conservação de massa (reserva → consumo → outputs → finalização).

### 5.1 Achado crítico

> **V2-01 — CRÍTICO: `AssetState::SPACE` aloca 32 bytes a menos.**
> `state/assets.rs:27` define `8 + (32 * 7) + 1 + 1 + (8 * 5) + 2 + 1 = 277`, mas a struct tem **oito** campos de 32 bytes (`asset_id`, `deployment_id`, `custodian`, `parent_root`, `lineage_root`, `current_lot_id`, `last_event_hash`, `reserved_by`). O tamanho serializado real é **309 bytes**, e a própria API já espera 309 (`services/api/src/solana/rpc.rs`, `expect_layout(data, "AssetState", 309)`).
> **Efeito:** `register_asset` e `create_transformation_output` falham ao serializar a conta (`AccountDidNotSerialize`), e **nenhum ativo v2 pode ser criado**. Com isso, todo o restante do v2 (observações, reservas, consumo, transformações) fica inalcançável.
> **Por que não foi detectado:** o teste de layout só verifica `SPACE > 8`; o teste LiteSVM que registra ativo nunca rodou (CI sem runner, LiteSVM "Pendente").
> **Correção:** trocar para `32 * 8` (ou usar `#[derive(InitSpace)]` com `8 + AssetState::INIT_SPACE`) e acrescentar um teste que compare `SPACE` com `try_to_vec().len() + 8` para **todas** as contas.

### 5.2 Demais achados no v2

| ID | Sev. | Achado |
|---|---|---|
| V2-02 | Alta | **Centralização total em uma chave.** `register_asset`, `record_observation`, `begin/reserve/consume/output/finalize` exigem `has_one = authority`. Em `begin`/`reserve`, `facility.owner == authority` implica que só instalações da própria autoridade transformam. Não existe `set_authority`, multisig nem delegação por papel (os `PartyRecord`s nunca são consultados). Na prática o v2 é um ledger permissionado de um operador; a blockchain agrega imutabilidade e carimbo temporal, mas não descentralização da autoridade. |
| V2-03 | Alta | **A linhagem não é verificada on-chain.** `input_root`/`output_root` são só compromissos: `consume` aceita qualquer ativo (sem prova Merkle de pertencer ao `input_root`) e `create_transformation_output` aceita qualquer `lineage_root`. A conservação de massa é validada só no manifesto declarado, não contra os ativos realmente consumidos (o manifesto e os consumos só precisam ter a mesma soma). |
| V2-04 | Alta | **Não há transferência de custódia no v2.** Nenhuma instrução muda `AssetState.custodian`. As transferências v2 existem só no PostgreSQL (`/api/v2/custody-transfers`), aceitas via token de operador, **sem assinatura do recebedor**. |
| V2-05 | Média | **Intents decorativos.** `create/consume_intent` não tocam nenhum ativo, não validam `subject_id` nem `expected_state_version` contra o estado real, e nenhuma instrução exige intent consumido. Qualquer um pode criar intents. A proteção de "supersessão e replay" prometida não existe on-chain. |
| V2-06 | Média | **Estados e contas inalcançáveis.** `IN_TRANSIT`, `CLOSED`, `QUALITY_HOLD`, `RECALLED` e `RETIRED` não são atribuídos por nenhuma instrução; `LineageAnchor` e `document_registry` nunca são criados ou usados; os seeds `lineage`, `transformation-chunk` e `recall` não são usados. Recall existe só off-chain. |
| V2-07 | Média | `record_observation` **não checa o status do ativo**: um ativo consumido ou fechado continua aceitando observações. Também não aplica `config.max_event_age_seconds` (usa só o limite fixo de 86.400 s do protocolo). |
| V2-08 | Média | **Reserva sobrescrita.** Com uma reserva expirada, `reserve` sobrescreve `asset.reserved_by` (`reservations.rs:81`) sem fechar a PDA antiga. Essa reserva antiga não pode mais ser liberada (`release` exige `reserved_by == transformation_id`), o que prende contadores e rent da transformação anterior. |
| V2-09 | Média | O timestamp da observação vem do relógio da Station (`observed_at`); o ESP32 não tem RTC confiável nem fonte de tempo assinada. A janela `observed_at..expires_at` valida o intervalo declarado pelo dispositivo, não o tempo real da observação. |
| V2-10 | Baixa | `finalize_transformation` não checa `expires_at`: uma transformação expirada mas ainda com status OPEN/FINALIZING pode ser finalizada. |
| V2-11 | Baixa | `expire_intent` devolve `IntentExpired` quando o intent *ainda não* expirou (código de erro invertido); `asset.status.max(1)` em `consumption.rs:86` não tem efeito. |
| V2-12 | Baixa | `initialize_v2` herda o mesmo squatting do V1-01, e os `RegistryRoot` não têm função além de existir. |
| V2-13 | Info | `unique_event_offset` do v2 exige offset fixo; a varredura O(n) é redundante (inofensiva). |

**Conclusão do v2:** a direção de design está correta (versão de estado, reserva otimista, conservação de massa em u128, hashes com separação de domínio), mas o programa hoje é um **esqueleto não executado**. Antes de qualquer Devnet é preciso corrigir o V2-01, rodar a suíte LiteSVM e decidir o modelo de autorização (V2-02).

---

## 6. Protocolo e criptografia (`crates/lastro-protocol`)

- `StationEvent` v1: 276 B em layout fixo (`LSTR`, versão, ação, reservados, deployment, animal, station, seq u64 LE, revisão u32 LE, prev hash, RFID antigo/novo, from/to). `event_hash = SHA-256(bytes)`; `station_id = SHA-256("LASTRO_STATION\0" ‖ pubkey33)`; hash de RFID com o domínio `LASTRO_RFID\0`.
- Envelope v2: 220 B com `schema_version`, `event_type`, janela temporal e domínios distintos (`LASTRO_V2_EVENT\0` etc.). Merkle de linhagem com domínios de folha e nó distintos, o que evita *second-preimage* entre níveis.
- A conservação de massa usa u128 com tolerância em basis points (máximo de 10%).
- Os vetores (`test-vectors/*.bin`, `vectors.json`) são consumidos por Rust, C, TypeScript e Python. Esse é o melhor ativo de qualidade do projeto.
- Assinaturas P-256 com low-S exigido (coerente com o precompile Secp256r1 da Solana).

**Ressalva:** o crate de protocolo (com `uuid`, `base64`, `serde`, `thiserror`) é dependência do programa on-chain v2. É bom confirmar o tamanho do `.so` SBF e o custo de CU depois do build (não verificado aqui).

---

## 7. Off-chain

### 7.1 Firmware (`firmware/station`)
- A máquina de estados, a construção e assinatura de eventos (PSA) e o framing serial LSTR estão implementados, com 47 testes Unity.
- O **signer eFuse** valida propósito `ECDSA_KEY_P256` e `dis_read` e usa uma chave opaca. É o caminho certo para root of trust.
- **Bloqueador P0:** `rfid.c` é um stub (`lastro_rfid_take` sempre devolve `false`). Não há leitor, pinout nem frames reais. **Sem isso não existe produto físico.**
- O modo de desenvolvimento lê a chave privada de `CONFIG_LASTRO_STATION_DEV_PRIVATE_KEY_HEX` (sdkconfig), e o modo test-vector usa a chave `= 1`. É preciso garantir no build de release que esses modos não compilem (hoje isso depende de Kconfig; vale uma checagem `#error` para builds com secure boot).
- Não há journal persistente na Station: uma queda de energia entre assinar e entregar ao Agent perde o evento.

### 7.2 Agent (`services/agent`)
- Outbox SQLite durável com migrations, retry idempotente, quarentena e erros sem URL (`without_url()`). O desenho é bom.
- Hoje há um único Agent por Station e não existe identidade forte do Agent perante a API (bearer token compartilhado).

### 7.3 API (`services/api`)
- **Pontos fortes:** challenge de captura assinado pela carteira (TTL de 2 min, consumo atômico); comparação de token em tempo constante (SHA-256 + `subtle`); verificação da transação exata em `confirmed` e `finalized`; decodificação estrita de layouts com checagem de seeds e bump; reconciliador com `FOR UPDATE SKIP LOCKED` e quarentena; limites de body e de RPC.
- **Achados:**

| ID | Sev. | Achado |
|---|---|---|
| API-01 | Alta | **Autorização operacional v2 = um token compartilhado.** Todas as rotas `/api/v2/*` de parties, facilities, lots, custody-transfers, processing, recalls e authority-grants usam só `LASTRO_OPERATOR_TOKEN`. "Aceite de custódia" e "authority grants" não são assinados pela parte. O próprio código admite que é um "bootstrap boundary". |
| API-02 | Alta | **O fluxo v2 de transformação não fecha ponta a ponta.** Nenhum código da API insere ou atualiza `v2_transformations` e não há builder para `begin/reserve/consume/output/finalize`. Por isso `finalize_processing` sempre falha com "not finalized on Solana", a menos que alguém escreva direto no banco. As rotas v2 não têm testes em `services/api/tests`. |
| API-03 | Média | `CorsLayer::allow_origin(Any)` em todas as rotas. O risco de CSRF é baixo (bearer, sem cookies), mas em produção a origem deveria ser restrita. |
| API-04 | Média | `POST /api/animals` é anônimo (há orçamento global, mas não por cliente). Não existe rate limit por IP na aplicação; o projeto delega ao proxy, e o Caddyfile não mostra essa proteção. |
| API-05 | Baixa | Deriva de documentação: `SECURITY_VALIDATION` fala em limite de body de 1024 B, mas o código usa 64 KiB (`MAX_JSON_BODY_BYTES`). |

### 7.4 Frontend e verificador (`apps/web`)
- O verificador independente checa assinatura da Station, continuidade de identidade e custódia, transação finalizada e estado canônico, e distingue `NOT_CHECKED` (falha de RPC) de `INVALID`. É o diferencial de produto: **terceiros verificam sem confiar no Lastro**.
- Sem `v-html`/`innerHTML`, com estado local validado de forma fail-closed.
- As páginas de narrativa (Problem, Future, Lineage) são material de pitch e deveriam ficar separadas do console operacional num produto real.

### 7.5 Infra e CI
- Existem compose de dev e produção, Caddy, Dockerfiles e script de backup, com actions fixadas por SHA e `permissions: contents: read`.
- **Nenhum workflow executou de fato** (runner nunca atribuído). Não há evidência de build SBF, de `cargo audit` nem de `npm audit`. É o maior risco de processo: centenas de testes existem, mas não há prova de que passem.

---

## 8. Modelo de confiança (o que a solução prova e o que não prova)

| Afirmação | Prova hoje? |
|---|---|
| O evento foi assinado pela chave registrada da Station | **Sim**, on-chain |
| Só o custodiante atual pôde mover a custódia (v1) | **Sim**, on-chain |
| A história não pode ser reescrita, forkada ou repetida (v1) | **Sim**, on-chain |
| Um RFID nunca serve a dois animais (v1) | **Sim**, on-chain |
| A chave da Station está protegida em hardware | **Não verificado** (eFuse sem provisionamento físico) |
| A Station leu fisicamente aquele RFID naquele animal | **Não** (driver RFID inexistente; RFID clonável é risco inerente) |
| A observação ocorreu no momento declarado | **Não** (v1 sem tempo; v2 com relógio do dispositivo) |
| O destinatário aceitou a custódia | **Não** (v1 push; v2 off-chain com token) |
| Os produtos derivam dos insumos declarados (v2) | **Não** (a raiz Merkle não é verificada; v2 não executa) |
| Custódia = propriedade | **Não**, e o projeto reconhece isso corretamente |

---

## 9. Custos on-chain (estimativa)

Rent-exempt na Solana: aproximadamente `(128 + bytes) × 6.960` lamports.

| Operação | Contas criadas | Rent (SOL) | Taxa (SOL) | Total aprox. (SOL) |
|---|---|---|---|---|
| v1 ORIGIN | AnimalState (149 B) + RfidBinding (74 B) | ≈ 0,00334 | ≈ 0,00001 | **≈ 0,0034** |
| v1 TRANSFER | nenhuma | 0 | ≈ 0,00001 | **≈ 0,00001** |
| v1 REIDENTIFY | RfidBinding | ≈ 0,00141 | ≈ 0,00001 | **≈ 0,0014** |
| v2 ativo | AssetState (309 B, após a correção) | ≈ 0,00304 | — | ≈ 0,0030 |
| v2 observação | EventAnchor (259 B) | ≈ 0,00269 | ≈ 0,00001 | **≈ 0,0027 por observação** |

A taxa considera a assinatura da carteira mais a assinatura verificada pelo precompile, cerca de 5.000 lamports cada. Os valores podem variar com a política de fees da rede.

**Leitura:** o v1 é barato por animal (centavos a poucos dólares, conforme o preço do SOL). No v2, **cada observação trava rent permanentemente**. Com dezenas de observações por animal ao longo da vida, o custo passa a dominar. Recomendo: fechar `EventAnchor` depois de consolidar num compromisso Merkle por ativo/período, ou emitir o evento só via log/compressed accounts e manter on-chain apenas o `last_event_hash`.

---

## 10. Achados priorizados (backlog técnico)

| Prioridade | ID | Ação |
|---|---|---|
| **P0** | V2-01 | Corrigir `AssetState::SPACE` e criar teste `SPACE == serialized + 8` para todas as contas |
| **P0** | CI | Fazer o CI rodar de fato (runner self-hosted ou billing) e registrar evidência de `cargo test`, LiteSVM, build SBF e audits |
| **P0** | FW | Escolher o leitor RFID (ex.: ISO 11784/11785 FDX-B, padrão bovino) e implementar o adaptador com frames reais |
| **P1** | V1-01/V2-12 | Restringir `initialize` ao *upgrade authority* ou a uma chave fixada no programa |
| **P1** | V1-02 | Levar o registry de Stations (já iniciado no v2) para o fluxo v1, com rotação, revogação e validade temporal |
| **P1** | V2-02/API-01 | Modelo de autorização por papel: Party assina com a própria carteira, facility owner ≠ authority global, multisig (Squads) para a autoridade |
| **P1** | V1-04/V2-04 | Custódia em duas fases on-chain (propor → aceitar), assinada pelo recebedor |
| **P1** | V2-03 | Prova Merkle de pertencimento no `consume`/`output`, ou chunks de transformação verificados |
| **P1** | V1-03 | Aceitar instruções ComputeBudget no envelope (priority fees) |
| **P2** | V2-05/06/07/08/10 | Ligar intents ao estado, remover contas e status mortos, checar status em observação, fechar reservas órfãs |
| **P2** | V1-05/V2-09 | Freshness: challenge on-chain (nonce derivado de slot/blockhash) assinado pela Station |
| **P2** | Custo | Estratégia de fechamento ou compressão de `EventAnchor` |
| **P2** | API-02 | Implementar builders e testes do pipeline de transformação v2, ou retirar as rotas até existirem |
| **P3** | API-03/04/05, V1-07 | CORS restrito, rate limit por IP, eventos Anchor (`emit!`), corrigir docs |

---

## 11. Veredito técnico

O **núcleo v1** (identidade, custódia e reidentificação com prova física assinada e verificada on-chain) está pronto para demonstração e é tecnicamente defensável em banca ou due diligence. Os contratos são pequenos, restritivos e corretos para o escopo, e a trilha criptográfica ponta a ponta é a parte mais madura do projeto.

O **v2** é hoje um rascunho promissor com um bug bloqueante e sem execução comprovada. O projeto também ainda não atravessou três fronteiras que definem um produto real: **(1) leitor RFID físico, (2) governança de chaves e identidades, (3) execução comprovada em CI/Devnet.** Recomendo congelar o escopo do v2, corrigir o V2-01, colocar o CI para rodar e investir o próximo ciclo em hardware e autorização por papel, antes de qualquer nova feature de domínio.

## Atualização 30/09/2026 — produto somente v2

Após este relatório, o v1 foi removido e o v2 passou a ser o único protocolo (detalhes em
`docs/MIGRACAO_V1_PARA_V2.md`). Toda garantia do v1 foi portada antes da remoção: AssetID
persistente com RFID substituível (`bind_identifier`/`replace_identifier`), RFID nunca reutilizado
(`RfidBinding` nunca fechado), evento assinado pela Station e vinculado ao Secp256r1, cadeia sem
fork/replay e custódia em duas fases. As referências a "v1", `StationEvent` de 276 bytes e às rotas
`/api/animals` neste relatório descrevem o estado de 29/09 e não se aplicam mais.

Pendências que continuam valendo: leitor RFID físico, execução do CI/LiteSVM com a toolchain
fixada, e a primeira execução dos gates de sistema v2 (`tests/system`) contra um validador real.

# Implementação da Etapa P1 — Linhagem e transformação

**Projeto:** Lastro
**Data:** 25 de setembro de 2026
**Escopo:** protocolo canônico, fixture cross-language e fundação on-chain de transformação

## Resultado

Esta etapa fecha a base determinística para representar o fluxo em que um animal ou lote entra em uma facility, é transformado em carcaça, cortes, produtos e subprodutos. A implementação não tenta fingir que o processo está completo: a finalização que consome inputs, a criação de outputs e a emissão de eventos de custódia continuam como próximo corte.

## Alterações entregues

### Protocolo compartilhado Rust

- novos enums wire-level: `FacilityType`, `FacilityStatus`, `StationStatus`, `TransformationStatus`, `RecallStatus`, `ProvenanceType`, `UnitCode` e `LineageRole`;
- validação de enum desconhecido e ID v2 nulo;
- `LineageLeaf` fixo de 53 bytes;
- Merkle root determinística, ordenada por `position`, com rejeição de posição duplicada;
- domínios separados para hash de folha e nó de Merkle;
- `MassBalance` com aritmética de 128 bits e tolerância máxima de 1.000 basis points;
- `TransformationManifest` fixo de 188 bytes;
- hash de manifesto usando `LASTRO_V2_TRANSFORMATION\\0`;
- erros específicos para massa, roots, roles e manifesto;
- testes unitários e fixture cross-language.

### Frontend TypeScript

Foi adicionado um codec independente em `apps/web/src/protocol/v2/transformation.ts` com:

- encoding little-endian de folhas e manifesto;
- Merkle root com a mesma regra do Rust;
- validação de massa e limites de u32/u64;
- cálculo assíncrono dos hashes SHA-256;
- teste Vitest comparando roots, comprimento de 188 bytes e hash da fixture.

O código está pronto, mas a execução do Vitest no ambiente atual ficou bloqueada porque o checkout Windows não possui `pnpm`, não possui `node_modules` e a tentativa de instalação temporária do npm falhou internamente (`Cannot read properties of null (reading 'edgesOut')`). Isso é uma limitação de ferramenta, não uma falha observada no algoritmo Rust/fixture.

### Programa Anchor `lastro-v2`

- `LineageAnchor` com `asset_id`, `lineage_root`, `parent_root`, `edge_count`, `sequence` e última transformação;
- `TransformationAnchor` com facility, roots, contagens, pesos, tolerância, hash do manifesto, sequência, expiração e status;
- `TransformationReservation` para ligar uma transformação a um input e quantidade reservada;
- `AssetState.reserved_weight_grams` para impedir dupla alocação concorrente;
- `begin_transformation`:
  - exige facility ativa do tipo `PROCESSING_FACILITY`;
  - exige que o signer seja owner da facility e autoridade do deployment;
  - valida janela de expiração contra `max_event_age_seconds`;
  - reconstrói o manifesto canônico on-chain;
  - rejeita `manifest_hash` divergente;
  - limita tolerância ao parâmetro do deployment;
  - cria uma conta de transformação em estado `OPEN`;
- `reserve_transformation_input`:
  - exige transformação aberta e não expirada;
  - exige facility ativa e owner autorizado;
  - exige `state_version` exata;
  - aceita apenas asset ativo ou em trânsito;
  - rejeita peso zero, peso acima do saldo e reservation concorrente;
  - acumula quantidade e peso reservados sem superar o manifesto anunciado;
  - registra a reserva em PDA própria e marca o asset como reservado.

## Garantias preservadas

- programa v1 não foi alterado;
- `StationEvent` v1 não foi alterado;
- dados detalhados do manifesto não entram na transação: a cadeia guarda commitments e números compactos;
- uma transformação não pode anunciar hash de manifesto diferente dos bytes canônicos;
- um mesmo asset não pode ser reservado por duas transformações enquanto a reserva anterior estiver válida;
- `state_version` é usado para rejeitar writers obsoletos;
- estados possuem seeds e `SPACE` explícitos;
- a massa de perda e subproduto não pode ser omitida silenciosamente do balanço.

## Validação executada

Com sucesso no checkout:

```text
cargo fmt --all -- --check
cargo test --locked -p lastro-protocol
cargo check --locked --manifest-path chain/Cargo.toml -p lastro-v2
```

A suíte do protocolo passou com os testes legados e os novos testes de linhagem/transformação. O programa Anchor v2 compilou com as novas contas e instruções.

## Não incluído deliberadamente nesta etapa

Ainda não foi criada uma instrução que declare a transformação como finalizada. Isso é intencional: uma finalização segura precisa, na mesma máquina de estados, consumir ou liberar cada reservation, atualizar os inputs, criar os outputs, ancorar a nova linhagem e verificar balanço de massa. Criar apenas um `finalize_transformation` que mudasse o status deixaria inputs ativos ou bloqueados e seria um desenho inseguro.

O próximo corte deve implementar, nessa ordem:

1. liberação/expiração de reservas;
2. consumo idempotente de cada input;
3. criação de output assets e `LineageAnchor`;
4. finalização atômica da transformação;
5. confirmação de abate e criação de carcaças;
6. chunks para lotes grandes de cortes;
7. projeção API, Agent e verificador da linhagem.


## Corte P2 — ciclo de reserva e transformação

O programa v2 também recebeu o ciclo operacional mínimo da transformação:

- `release_transformation_input` libera uma reserva somente quando ela expirou ou quando a transformação foi abortada/expirada; a PDA da reserva é fechada e devolve lamports à autoridade;
- `abort_transformation` e `expire_transformation` são transições explícitas, sem apagar o anchor histórico;
- `consume_transformation_input` exige a versão exata do asset, consome a massa reservada, incrementa `state_version`, marca o asset como consumido quando o saldo chega a zero e remove a reserva;
- `create_transformation_output` cria um novo `AssetState`, liga `parent_root` ao root de inputs e acumula quantidade/peso de outputs;
- `finalize_transformation` somente aceita a transição quando não há reservas, todos os inputs foram consumidos e todos os outputs/pesos anunciados foram criados.

A máquina de estados agora evita o erro perigoso de declarar transformação finalizada enquanto ainda existem inputs reservados ou enquanto o peso efetivamente produzido não cobre o manifesto. A finalização ainda não cria automaticamente cortes em lote: cada output é uma conta própria nesta primeira implementação. O próximo incremento pode adicionar chunks de outputs, desde que preserve os mesmos contadores e commitments.

### Validação do corte P2

O `cargo check --locked --manifest-path chain/Cargo.toml -p lastro-v2` passou depois da inclusão de consumo, outputs e finalização. O protocolo compartilhado continuou passando em `cargo test --locked -p lastro-protocol`, e `cargo fmt --all -- --check` passou. O alvo de testes de integração do Anchor ficou silencioso no terminal Windows e foi encerrado pelo limite do ambiente; portanto, este corte está confirmado como compilável, mas a execução LiteSVM permanece pendente de um ambiente de teste estável.


## Corte P3 — projeção pública de transformação e linhagem

Foi adicionada à API uma projeção read-only preparada para o pós-abate:

- migration `0009_transformation_lineage.sql` cria `v2_transformations` e `v2_lineage_edges`;
- o manifesto de 188 bytes e seu `manifest_hash` são tratados como evidência imutável;
- a API expõe `GET /api/v2/transformations/{transformationId}` e `GET /api/v2/assets/{assetId}/lineage`;
- a leitura é sempre limitada ao `deployment_id` configurado, com limite público de 256 edges;
- a migration `0010_asset_reservation_projection.sql` acrescenta à projeção os campos de reserva, expiração e flags do `AssetState` v2;
- o frontend/consumidor deve verificar os roots e o hash do manifesto antes de tratar a resposta como prova.

Este corte é deliberadamente somente de consulta: ainda falta ligar a confirmação das transações Solana a um writer que projete `v2_transformations` e `v2_lineage_edges`. Não foi criada uma rota pública de escrita; nenhum dado de pessoa física, comprador ou contrato privado deve ser colocado on-chain ou exposto por esses endpoints.

A validação desta etapa foi concluída com `cargo check --locked -j 1 -p lastro-api` em modo low-memory, `vue-tsc -b --force` e os dois testes Vitest de transformação. O primeiro check Rust falhou por pressão de memória/PDB corrompido no toolchain Windows; a execução low-memory posterior passou sem erro de código.

## Corte P4 — correções de auditoria (29/09/2026)

Origem: `RELATORIO_TECNICO_2026-09-29.md`. **Quebra a interface** de várias instruções do `lastro-v2`; nenhum cliente off-chain (API, web, scripts) as usava, e o programa ainda não teve deploy real.

| Achado | Correção |
|---|---|
| V2-01 `AssetState::SPACE` = 277 B, layout real 309 B (`register_asset`/outputs falhavam) | `32 * 8`; `tests/layout.rs` compara `SPACE` com o tamanho Borsh de **todas** as contas |
| V2-02 tudo exigia a chave global | Transformações são assinadas pelo **owner da facility** (`operator`); facility deve estar ACTIVE e dentro de `valid_from..valid_until`; `SLAUGHTERHOUSE` e `PROCESSING_FACILITY` aceitos |
| — sem rotação da autoridade | `transfer_config_authority` (assinam a autoridade atual e a nova; permite migrar para multisig) |
| V2-03 linhagem não verificada | `reserve_transformation_input` e `create_transformation_output` exigem prova Merkle (`leaf_position`, `leaf_quantity`, `leaf_index`, `proof`, e `leaf_role` nos outputs) contra `input_root`/`output_root`; profundidade máxima 16 |
| — insumos de terceiros | Reserva exige `asset.custodian == operator` |
| V2-04 sem custódia on-chain | Custódia em duas fases: intent `CUSTODY_TRANSFER` (tipo 2) do custodiante + `accept_custody_transfer` assinado pelo recebedor; payload = `custody_transfer_payload_hash` em `lastro_protocol::v2` |
| V2-05 intents decorativos | `create_intent` exige ser o custodiante e a `state_version` atual do ativo; `consume_intent` confere a versão do ativo |
| V2-07 observação em ativo terminal | `record_observation` rejeita CONSUMED/CLOSED/RETIRED e aplica `max_event_age_seconds` do deployment |
| V2-08 reserva sobrescrita/órfã | Reserva exige `reserved_by == 0`; `release` é permissionless após expirar/abortar, com rent devolvido ao owner |
| — mesmo insumo consumido duas vezes | A PDA de reserva não é mais fechada no consumo (`consumed_at`), impedindo re-reserva da mesma folha |
| — reservas além do manifesto | Limite passa a ser `reservado + consumido <= input_count/input_weight` |
| — subprodutos nunca finalizavam | Outputs com papel `BYPRODUCT` são contados em `created_byproduct_weight_grams` e conferidos contra `byproduct_weight_grams` |
| V2-10 finalizar após expirar | `finalize_transformation` exige `now <= expires_at` |
| V2-11 erro invertido | `expire_intent` usa `IntentNotExpired`; removido `status.max(1)` sem efeito |
| — sem revogação de Party | `set_party_status` |

Novos erros foram **acrescentados no fim** do enum, então os códigos existentes não mudam. Layouts alterados: `TransformationAnchor` (+8 B) e `TransformationReservation` (+8 B); `AssetState`, `EventAnchor` e `ProtocolConfigV2` continuam com o layout que a API decodifica.

### Validação deste corte

| Comando / verificação | Resultado |
|---|---|
| Testes host do programa (`--lib`, `layout`) | Passou (layout de todas as contas confere) |
| `cargo clippy --all-targets -- -D warnings` (programa e testes) | Passou |
| Compilação de `initial_state.rs` e `custody_lineage.rs` (LiteSVM) | Compila; `initial_state.rs` **não compilava antes** (argumento `&[u8;33]` e import de `InstructionData` ausentes) |
| Execução LiteSVM | **Pendente**: exige `cargo build-sbf` (toolchain Solana), indisponível no ambiente da correção |
| `--all-features` (`idl-build`) | **Pendente**: dependência fora do cache offline |
| `lastro-protocol` (novos `verify_merkle_proof`/`merkle_proof`/payload de custódia) | Passou: todos os testes, incluindo prova para árvores de 1 a 9 folhas |

## Corte P5 — pendências da auditoria (30/09/2026)

| Item | Mudança |
|---|---|
| Squatting de `initialize` (v1 e v2) | Contas `program` + `program_data`; só o upgrade authority inicializa (`UnauthorizedInitializer`). Programa imutável não aceita novos deployments |
| Taxa de prioridade | `require_only_trailing_compute_budget` (v1 e v2): até 2 ComputeBudget sem contas após o índice 1 |
| Custódia v2 pela API | `solana/v2_custody.rs`: intent determinístico por `transferId`; `transaction-data?phase=propose|accept`; `accept` exige transação finalizada do recebedor |
| Transformação v2 pela API | `POST /api/v2/transformations` + `POST /api/v2/transformations/{id}/sync`; decodificador da `TransformationAnchor` (274 B) |
| Lint pré-existente do v1 | `allow(unexpected_cfgs)` e aliases de CPI com `allow(unused_imports)`, como no v2 |

Novos testes: `initialize_rejects_signers_other_than_the_upgrade_authority` (v1), `initialize_is_reserved_to_the_program_upgrade_authority` (v2), `trailing_compute_budget_instructions_allow_priority_fees` e `only_compute_budget_may_follow_the_lastro_instruction` (v1), testes unitários de matcher/builder/custódia/provas na API, e `bounds the priority fee a custodian is asked to sign` no navegador.

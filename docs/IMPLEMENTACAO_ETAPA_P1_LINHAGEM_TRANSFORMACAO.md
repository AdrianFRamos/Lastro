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

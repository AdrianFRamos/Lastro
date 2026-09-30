# Backlog executável — Smart Contract Solana v2

**Projeto:** Lastro
**Objetivo:** implementar a arquitetura v2 para rastrear animal, lote, carcaça, transformação, produto, embalagem, expedição e recall na Solana, preservando integralmente os contratos v1 existentes.

**Status inicial:** todas as tarefas estão em `TODO`.
**Escopo principal:** `crates/lastro-protocol`, `chain/programs/lastro-v2`, testes Anchor/LiteSVM e integração mínima necessária para construir, verificar e operar as transações v2.
**Escopo secundário:** API, banco, Agent, frontend e verificador, somente nos pontos necessários para consumir os contratos v2.

---

## 1. Decisões que tornam o backlog executável

### 1.1 Estratégia de compatibilidade

O programa atual `chain/programs/lastro` permanecerá como **Lastro v1**. Ele continuará atendendo:

```text
initialize
origin
transfer
reidentify
```

O v2 será implementado em um novo pacote Anchor:

```text
chain/programs/lastro-v2
```

O v2 terá **novo program ID**, novo deployment e novas contas. O programa v1 continuará disponível para leitura durante a migração. Não será feito redimensionamento silencioso de `AnimalState` v1 e não será alterado o significado do `StationEvent` v1 de 276 bytes.

### 1.2 Fonte de verdade

A divisão de autoridade será:

```text
Station ou balança:
  observação física assinada

Carteira do custodiante:
  transferência e aceite de custódia

Facility autorizada:
  abate, transformação, produção e recall operacional

API/PostgreSQL:
  manifestos detalhados, documentos, projeções e consulta

Solana v2:
  invariantes, estado canônico, versões, hashes, reservas,
  consumo, status, anchors e bloqueios

Verifier:
  recomputação independente
```

O PostgreSQL não poderá transformar uma intenção em estado canônico sem confirmação final da Solana.

### 1.3 Primeira entrega de produção

O primeiro release v2 deverá conter:

1. configuração versionada;
2. registry de Stations;
3. registry de facilities;
4. registry de parties;
5. `AssetState`;
6. migração de animal v1;
7. envelope de evento v2;
8. intents e nonces;
9. observações físicas;
10. transferência de custódia;
11. lotes e linhagem;
12. carcaça;
13. transformação com reserva, chunks e finalização;
14. produto, embalagem e expedição;
15. quality hold e recall;
16. verificador on-chain v2;
17. fixtures Rust/TypeScript;
18. testes de concorrência, replay, massa e autorização.

Integrações oficiais externas, dados pessoais on-chain, marketplace e tokenização financeira ficam fora deste release.

### 1.4 Convenções do backlog

| Convenção | Significado |
|---|---|
| `P0` | Bloqueia qualquer deployment funcional v2 |
| `P1` | Necessário para o piloto controlado |
| `P2` | Necessário para escala ou operação avançada |
| `S` | Trabalho pequeno, com contrato já definido |
| `M` | Trabalho médio, exige código e testes de integração |
| `L` | Trabalho grande, exige design, implementação e múltiplos testes |
| `SEC` | Segurança ou integridade; não pode ser adiado sem decisão formal |
| `TEST` | Fixture, teste, benchmark ou gate |
| `INTEG` | Integração fora do programa on-chain |
| `DOC` | Documentação, operação ou decisão técnica |

A estimativa é uma banda de esforço relativo, não uma promessa de prazo. Uma tarefa `L` deve ser quebrada novamente se ultrapassar uma unidade de entrega revisável.

### 1.5 Definition of Done geral

Uma tarefa só pode ser marcada como concluída quando:

- o código estiver implementado no módulo correto;
- os invariantes estiverem explicitamente validados;
- houver teste positivo e teste negativo;
- o layout serializado estiver congelado por fixture;
- o erro retornado for específico;
- a tarefa não quebrar os testes v1;
- o `cargo fmt`, `cargo clippy` e `cargo test` aplicáveis passarem;
- a documentação de segurança e migração estiver atualizada;
- não houver dado pessoal no estado público;
- a revisão confirmar que a API não recebeu uma instrução genérica que contorne os guards específicos.

---

## 2. Estrutura de arquivos alvo

A estrutura proposta é:

```text
crates/lastro-protocol/src/v2/
  mod.rs
  constants.rs
  ids.rs
  asset.rs
  event.rs
  envelope.rs
  intent.rs
  lineage.rs
  transformation.rs
  hash.rs
  canonical.rs
  errors.rs

crates/lastro-protocol/tests/v2/
  envelope_vectors.rs
  lineage_vectors.rs
  transformation_vectors.rs
  canonical_vectors.rs

chain/programs/lastro-v2/
  Cargo.toml
  src/
    lib.rs
    constants.rs
    error.rs
    events.rs
    instructions/mod.rs
    instructions/initialize.rs
    instructions/stations.rs
    instructions/facilities.rs
    instructions/parties.rs
    instructions/assets.rs
    instructions/migration.rs
    instructions/observations.rs
    instructions/custody.rs
    instructions/lots.rs
    instructions/lineage.rs
    instructions/slaughter.rs
    instructions/transformations.rs
    instructions/products.rs
    instructions/shipments.rs
    instructions/quality.rs
    instructions/recalls.rs
    instructions/helpers.rs
    state/mod.rs
    state/config.rs
    state/registries.rs
    state/assets.rs
    state/events.rs
    state/intents.rs
    state/lineage.rs
    state/transformations.rs
    state/recalls.rs
    verify/mod.rs
    verify/secp256r1.rs
    verify/event.rs
  tests/
    common/mod.rs
    initialize_v2.rs
    registries.rs
    assets.rs
    migration.rs
    observations.rs
    custody.rs
    lots.rs
    transformations.rs
    products.rs
    shipments.rs
    recalls.rs
    authorization.rs
    replay.rs
    concurrency.rs
    transaction_size.rs
    layout.rs

apps/web/src/solana/v2/
  constants.ts
  pda.ts
  instructions.ts
  transaction.ts
  types.ts

apps/web/src/verify/
  verifyCanonicalChainStateV2.ts

services/api/src/solana/v2/
  pda.rs
  instruction_builder.rs
  transaction_builder.rs
  rpc.rs

services/api/src/domain/v2/
  assets.rs
  events.rs
  transformations.rs
  reconciliation.rs
```

O nome final do pacote pode ser `lastro-v2` ou `lastro_domain`, mas a decisão deve ser registrada antes de criar o program ID. O importante é que o crate v1 continue separado e compilável.

---

# 3. Épico V2-00 — Contrato, versionamento e bootstrap

## V2-001 — Registrar a decisão de novo program ID

**Prioridade:** P0 · **Tamanho:** S · **Tipo:** DOC/SEC
**Dependências:** nenhuma

### Implementação

Criar uma decisão técnica em `docs/DECISIONS.md` ou documento específico contendo:

- v1 permanece consultável;
- v2 usa novo program ID;
- contas v1 não são redimensionadas;
- `StationEvent` v1 permanece com 276 bytes;
- migração v1 → v2 é uma nova instrução e uma nova conta;
- data de corte e política de novas escritas v1 serão definidas no rollout.

### Aceite

- a estratégia não depender de uma mudança posterior no layout v1;
- o programa v1 continuar sendo compilado no CI;
- o plano de migração referenciar um `legacy_last_event_hash`;
- nenhuma tarefa futura usar `lastro` v1 para receber contas v2.

## V2-002 — Criar o crate Anchor `lastro-v2`

**Prioridade:** P0 · **Tamanho:** M · **Tipo:** P0
**Dependências:** V2-001

### Arquivos

```text
chain/Cargo.toml
chain/programs/lastro-v2/Cargo.toml
chain/programs/lastro-v2/src/lib.rs
chain/Anchor.toml
```

### Implementação

- adicionar `lastro-v2` ao workspace `chain`;
- fixar as mesmas versões Anchor/Solana aprovadas para o v1;
- criar o marker de bootstrap do program ID;
- habilitar `cdylib` e `lib`;
- copiar somente a infraestrutura mínima, não as instruções v1;
- expor um `initialize_v2` inicial;
- adicionar aliases Anchor necessários no crate root, como no programa atual.

### Aceite

```bash
cargo check --locked --manifest-path chain/Cargo.toml
cargo test --locked --manifest-path chain/Cargo.toml
anchor build --manifest-path chain/Anchor.toml
```

O crate v1 e o crate v2 devem compilar juntos. O IDL v2 não pode conter instruções v1 acidentalmente.

## V2-003 — Bootstrap determinístico do program ID v2

**Prioridade:** P0 · **Tamanho:** M · **Tipo:** SEC/DOC
**Dependências:** V2-002

### Arquivos

```text
scripts/bootstrap_program_id.sh
scripts/install_pinned_anchor_cli.sh
scripts/install_pinned_solana_cli.sh
chain/Anchor.toml
```

### Implementação

- estender o bootstrap para suportar `--program lastro-v2`;
- manter keypair fora do Git;
- derivar public key do keypair real;
- sincronizar `declare_id!` e configuração Anchor;
- produzir um manifest de deployment contendo program ID, deployment ID, autoridade, schema e hash do `.so`;
- impedir que o manifest de teste seja usado em staging ou produção.

### Aceite

- duas execuções limpas produzirem o mesmo resultado quando recebem o mesmo keypair;
- o script falhar se o marker não existir ou se o ID declarado divergir;
- o program ID não ser inventado no código;
- o CI detectar alteração não autorizada do IDL ou do marker.

## V2-004 — Definir matriz de versões e compatibilidade

**Prioridade:** P0 · **Tamanho:** S · **Tipo:** DOC/TEST
**Dependências:** V2-001

### Entregável

Criar `docs/PROTOCOL_V2_COMPATIBILITY.md` com:

```text
protocol v1 -> somente programa v1
protocol v2.0 -> programa v2
schema_version desconhecida -> rejeitar
account discriminator desconhecido -> rejeitar
program ID de outro deployment -> rejeitar
```

### Aceite

- cada versão possuir identificador explícito;
- o verificador não inferir v2 pelo tamanho do payload;
- fixtures de rejeição existirem para versão futura desconhecida;
- a mudança de layout exigir nova versão.

---

# 4. Épico V2-01 — Protocolo compartilhado e canonicalização

## V2-101 — Criar constantes e enums v2 no `lastro-protocol`

**Prioridade:** P0 · **Tamanho:** M · **Tipo:** P0
**Dependências:** V2-004

### Arquivos

```text
crates/lastro-protocol/src/v2/mod.rs
crates/lastro-protocol/src/v2/constants.rs
crates/lastro-protocol/src/v2/asset.rs
crates/lastro-protocol/src/v2/errors.rs
crates/lastro-protocol/src/lib.rs
```

### Tipos mínimos

```rust
AssetType
AssetStatus
FacilityType
FacilityStatus
StationStatus
EventType
IntentType
IntentStatus
TransformationStatus
RecallStatus
ProvenanceType
```

### Regras

- enum wire-level deve ter representação explícita, preferencialmente `u8` ou `u16`;
- nenhum enum deve depender da ordem acidental do Rust;
- cada enum deve possuir conversão que rejeite valor desconhecido;
- `UNKNOWN` não deve ser aceito como estado mutável;
- os valores devem ser documentados e congelados por fixture.

### Aceite

- Rust e TypeScript usarem os mesmos valores;
- valor não reconhecido retornar erro;
- enums não conterem strings livres dentro da transação;
- teste de overflow e conversão inválida passar.

## V2-102 — Implementar IDs e validação de identificadores

**Prioridade:** P0 · **Tamanho:** M · **Tipo:** SEC
**Dependências:** V2-101

### Implementação

Criar wrappers ou funções para validar:

```text
DeploymentId: 32 bytes
AssetId: 32 bytes
EventId: 32 bytes
StationId: 32 bytes
FacilityId: 32 bytes
PartyId: 32 bytes
TransformationId: 32 bytes
IntentId: 32 bytes
RecallId: 32 bytes
```

Adicionar derivação de PDA em um módulo separado do hash de payload. Não reutilizar uma função de hash com domínio incorreto.

### Domínios de hash

Definir constantes como:

```text
LASTRO_V2_ASSET
LASTRO_V2_EVENT
LASTRO_V2_PAYLOAD
LASTRO_V2_LINEAGE
LASTRO_V2_TRANSFORMATION
LASTRO_V2_INTENT
LASTRO_V2_MIGRATION
```

### Aceite

- domínio diferente produzir hash diferente;
- entrada com tamanho incorreto falhar;
- nenhum ID público depender de base64 não canônica;
- testes cross-language confirmarem bytes.

## V2-103 — Implementar `DomainEventEnvelope`

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** P0
**Dependências:** V2-101, V2-102

### Layout mínimo

```rust
pub struct DomainEventEnvelope {
    pub schema_version: u16,
    pub event_type: u16,
    pub deployment_id: [u8; 32],
    pub subject_id: [u8; 32],
    pub event_id: [u8; 32],
    pub state_version: u64,
    pub expected_previous_hash: [u8; 32],
    pub payload_hash: [u8; 32],
    pub source_id: [u8; 32],
    pub observed_at: i64,
    pub expires_at: i64,
}
```

### Implementação

- definir ordem dos campos;
- definir endianess;
- definir bytes reservados, se existirem;
- serializar sem `bincode` implícito;
- rejeitar schema desconhecido;
- validar `expires_at >= observed_at`;
- validar janela máxima conforme a política;
- validar predecessor zero somente em eventos de criação;
- calcular hash do envelope completo com domínio v2.

### Aceite

- `encode(decode(bytes)) == bytes`;
- comprimento fixo documentado;
- fixture Rust e TypeScript idêntica;
- evento com bytes alterados possuir hash diferente;
- evento expirado ser rejeitado no helper de validação;
- payload completo permanecer fora da transação.

## V2-104 — Criar canonicalização de manifestos

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** SEC
**Dependências:** V2-103

### Implementação

Criar canonicalização determinística para manifestos off-chain:

- campos ordenados;
- números como inteiros com unidade explícita;
- strings UTF-8 normalizadas conforme política;
- arrays ordenados pela regra do domínio;
- campos opcionais ausentes tratados de uma única forma;
- nenhum campo desconhecido aceito em schema fechado;
- hash produzido antes do upload ou anchor.

Não usar `serde_json::to_string` sem uma política de ordenação e normalização documentada.

### Aceite

- o mesmo manifesto produzido por Rust, TypeScript e Python gerar o mesmo `manifest_hash`;
- ordem diferente em campo que é semanticamente ordenado produzir o mesmo hash somente quando a especificação permitir;
- campo desconhecido ser rejeitado;
- alteração de peso, quantidade, input ou output alterar o hash.

## V2-105 — Criar fixtures cross-language

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** TEST
**Dependências:** V2-103, V2-104

### Arquivos

```text
crates/lastro-protocol/tests/v2/*.rs
apps/web/tests/protocol/v2/*.test.ts
schemas/fixtures/v2/*.json
scripts/check_vectors.py
```

### Fixtures obrigatórias

- envelope de observação;
- envelope de transferência;
- begin transformation;
- chunk 0, chunk 1 e chunk final;
- input root;
- output root;
- balanço de massa;
- intent válida;
- intent expirada;
- migration proof;
- recall commitment;
- bytes inválidos e schema futuro.

### Aceite

O CI deve falhar se qualquer linguagem produzir bytes ou hashes diferentes. A alteração de uma fixture deve exigir revisão de protocolo.

---

# 5. Épico V2-02 — Estado, configuração e registries on-chain

## V2-201 — Implementar `ProtocolConfigV2`

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** P0/SEC
**Dependências:** V2-002, V2-101

### Arquivo

```text
chain/programs/lastro-v2/src/state/config.rs
chain/programs/lastro-v2/src/instructions/initialize.rs
```

### Campos

```rust
pub struct ProtocolConfigV2 {
    pub authority: Pubkey,
    pub deployment_id: [u8; 32],
    pub schema_version: u16,
    pub station_registry: Pubkey,
    pub facility_registry: Pubkey,
    pub party_registry: Pubkey,
    pub document_registry: Pubkey,
    pub max_asset_weight_grams: u64,
    pub mass_tolerance_basis_points: u16,
    pub max_event_age_seconds: u64,
    pub bump: u8,
}
```

### Invariantes

- PDA: `['config-v2', deployment_id]`;
- criação única por deployment;
- `deployment_id` não pode ser zero;
- `schema_version` deve ser suportada;
- limites devem estar dentro de faixa segura;
- autoridade inicial é o signer da inicialização;
- parâmetros críticos não podem ser alterados por instrução genérica;
- atualização de parâmetro, se necessária, será uma instrução explícita com governança.

### Aceite

- segunda inicialização falhar;
- deployment diferente produzir PDA diferente;
- config de outro deployment ser rejeitada;
- limites inválidos falharem antes de criar estado;
- layout e `SPACE` possuírem fixture.

## V2-202 — Implementar governança da autoridade

**Prioridade:** P0 · **Tamanho:** M · **Tipo:** SEC
**Dependências:** V2-201

### Decisão

Para o MVP, a autoridade continua sendo um `Pubkey`, mas nenhuma operação deve aceitar autoridade arbitrária. Para piloto, o deployment deverá usar carteira ou multisig operacionalmente protegida.

### Implementação

- helper `assert_config_authority`;
- instrução explícita `transfer_config_authority`;
- instrução explícita `accept_config_authority` ou transferência direta documentada;
- registro de `authority_changed_at` se o layout permitir;
- nenhum admin oculto no programa;
- testes de signer errado e autoridade antiga.

### Aceite

- autoridade antiga não consegue registrar ou revogar depois da transferência;
- nenhuma conta pode alterar `authority` diretamente;
- mudança de autoridade possui evento on-chain;
- o deployment manifest registra a autoridade atual.

## V2-203 — Implementar registry de Stations

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** P0/SEC
**Dependências:** V2-201, V2-202, V2-102

### Arquivos

```text
chain/programs/lastro-v2/src/state/registries.rs
chain/programs/lastro-v2/src/instructions/stations.rs
chain/programs/lastro-v2/src/verify/secp256r1.rs
```

### Estado

```rust
pub struct StationRecord {
    pub station_id: [u8; 32],
    pub key_id: [u8; 32],
    pub pubkey33: [u8; 33],
    pub status: u8,
    pub valid_from: i64,
    pub valid_until: i64,
    pub firmware_hash: [u8; 32],
    pub bump: u8,
}
```

### Instruções

```text
register_station_v2
suspend_station_v2
revoke_station_v2
replace_station_key_v2
```

### Invariantes

- PDA: `['station-v2', deployment_id, station_id]`;
- `station_id == derive_station_id(pubkey33)`;
- chave P-256 comprimida deve ser válida;
- `valid_until` deve ser maior que `valid_from` quando presente;
- chave revogada não pode assinar evento futuro;
- evento histórico não deve ser invalidado somente porque a chave foi revogada depois;
- replacement deve apontar para a chave anterior e não apagar seu histórico.

### Aceite

- Station inexistente não passa na verificação;
- assinatura com chave diferente falha;
- chave expirada falha para evento novo;
- chave revogada falha para evento novo;
- registro duplicado falha;
- histórico com chave anteriormente válida permanece verificável.

## V2-204 — Implementar registry de facilities

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** P0/SEC
**Dependências:** V2-201, V2-202

### Estado

```rust
pub struct FacilityRecord {
    pub facility_id: [u8; 32],
    pub owner: Pubkey,
    pub facility_type: u8,
    pub status: u8,
    pub credential_hash: [u8; 32],
    pub valid_from: i64,
    pub valid_until: i64,
    pub bump: u8,
}
```

### Instruções

```text
register_facility
update_facility_credential
suspend_facility
revoke_facility
reinstate_facility
```

### Invariantes

- PDA: `['facility', deployment_id, facility_id]`;
- somente autoridade de configuração registra ou altera credencial;
- owner ou autoridade autorizada assina operação da facility;
- facility suspensa não pode iniciar ou finalizar transformação;
- tipo da facility limita instruções aceitas;
- validade é avaliada no timestamp do evento.

### Aceite

- frigorífico registrado pode executar somente operações compatíveis;
- facility `FARM` não finaliza desossa;
- facility suspensa não finaliza transformação;
- troca de credencial não altera histórico;
- carteira de outra facility falha.

## V2-205 — Implementar registry de parties

**Prioridade:** P1 · **Tamanho:** M · **Tipo:** P0/SEC
**Dependências:** V2-201

### Implementação

Criar uma conta ou estratégia de registro que relacione `party_id` a uma carteira e papéis compactos. Dados legais detalhados ficam off-chain.

### Instruções

```text
register_party
attach_party_role
revoke_party_role
```

### Aceite

- party ID não contém PII;
- papel revogado não autoriza operação nova;
- uma party pode ter mais de um papel dentro do escopo definido;
- carteira não pode representar outra party sem registro;
- histórico do papel permanece consultável.

## V2-206 — Congelar layouts e discriminators

**Prioridade:** P0 · **Tamanho:** M · **Tipo:** TEST/SEC
**Dependências:** V2-201 a V2-205

### Implementação

Criar testes que validem:

- discriminator Anchor;
- tamanho exato de cada conta;
- offsets críticos;
- PDA e bump;
- rejeição de conta truncada;
- rejeição de conta com owner incorreto;
- serialização de enums.

### Aceite

O CI deve falhar quando um campo for inserido, removido ou reordenado sem atualização de versão, fixture e migration plan.

---

# 6. Épico V2-03 — AssetState e migração do animal v1

## V2-301 — Implementar `AssetState`

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** P0
**Dependências:** V2-101, V2-201

### Estado

```rust
pub struct AssetState {
    pub asset_id: [u8; 32],
    pub asset_type: u8,
    pub status: u8,
    pub deployment_id: [u8; 32],
    pub custodian: Pubkey,
    pub parent_root: [u8; 32],
    pub lineage_root: [u8; 32],
    pub current_lot_id: [u8; 32],
    pub available_weight_grams: u64,
    pub event_sequence: u64,
    pub state_version: u64,
    pub last_event_hash: [u8; 32],
    pub reserved_by: [u8; 32],
    pub reserved_until: i64,
    pub flags: u16,
    pub bump: u8,
}
```

### Instruções

```text
register_asset
close_asset
```

### Invariantes

- PDA: `['asset', deployment_id, asset_id]`;
- asset ID único;
- deployment gravado na conta deve coincidir com a config;
- tipo e status devem ser válidos;
- asset fechado não aceita novas transições;
- `state_version` começa em zero ou um conforme decisão documentada;
- `last_event_hash` zero somente no estado inicial;
- custodian zero somente quando a regra do tipo permitir.

### Aceite

- ID duplicado falha;
- asset de outro deployment falha;
- tipo inválido falha;
- fechamento é terminal;
- conta criada possui layout e bump corretos;
- `register_asset` não aceita payload arbitrário que altere custodian sem regra.

## V2-302 — Criar helper de transição atômica

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** SEC
**Dependências:** V2-301, V2-103

### Helper interno

Criar funções internas para:

```text
assert_asset_version
assert_previous_hash
assert_status
assert_not_recalled
assert_not_consumed
advance_asset_version
set_last_event_hash
write_event_anchor
```

### Regras

A transição deve validar tudo antes de alterar qualquer conta. Anchor, asset e intent devem ser atualizados na mesma transação Solana.

### Aceite

- erro em qualquer validação deixa todas as contas no estado anterior;
- estado avançado possui exatamente uma nova versão;
- predecessor incorreto falha;
- replay do mesmo evento falha;
- hash final é determinístico.

## V2-303 — Implementar prova de migração v1 → v2

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** P0/SEC
**Dependências:** V2-201, V2-301, V2-302

### Estado

```rust
pub struct MigrationRecord {
    pub animal_id: [u8; 32],
    pub legacy_deployment_id: [u8; 32],
    pub legacy_animal_account: Pubkey,
    pub legacy_last_event_hash: [u8; 32],
    pub legacy_event_sequence: u64,
    pub legacy_identity_revision: u32,
    pub asset_id: [u8; 32],
    pub migrated_at: i64,
    pub bump: u8,
}
```

### Instrução

```text
migrate_animal_v1
```

### Validações

- conta v1 possui owner e discriminator corretos;
- `AnimalState` v1 corresponde ao animal informado;
- `ProtocolConfig` v1 corresponde ao deployment legado;
- `AssetState` v2 ainda não existe;
- migration PDA ainda não existe;
- RFID atual, custodian, revision, sequence e last hash são copiados sem alteração;
- peso, localização, status sanitário e documentos começam como desconhecidos;
- migration record aponta para a conta v1 e para o asset v2.

### Aceite

- migração duplicada falha;
- conta v1 adulterada falha;
- migration record é imutável;
- o verificador consegue atravessar v1 → v2;
- nenhum campo novo é inventado durante a migração.

## V2-304 — Adicionar transição de status e encerramento

**Prioridade:** P1 · **Tamanho:** M · **Tipo:** P0
**Dependências:** V2-301, V2-302

### Instruções

```text
change_asset_status
retire_asset
```

### Aceite

- somente transições permitidas por tipo são aceitas;
- asset em recall não pode ser retirado para apagar rastreabilidade;
- status terminal não volta a ativo;
- evento de status possui `event_id`, predecessor e versão;
- `retire_asset` não remove a conta nem a linhagem.

---

# 7. Épico V2-04 — Eventos físicos, observações e custódia

## V2-401 — Implementar verificação Secp256r1 v2

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** SEC
**Dependências:** V2-103, V2-203

### Implementação

Reutilizar somente a parte comprovada do verificador v1, criando módulo separado:

```text
chain/programs/lastro-v2/src/verify/secp256r1.rs
chain/programs/lastro-v2/src/verify/event.rs
```

Validar:

- instrução Secp256r1 no índice exigido;
- descriptor correto;
- assinatura sobre os bytes exatos do envelope v2;
- public key registrada;
- `station_id` derivado da chave;
- Station ativa no instante do evento;
- `source_id` igual ao Station esperado;
- sem assinatura de uma Station não relacionada.

### Aceite

Testar:

- assinatura válida;
- assinatura sobre bytes diferentes;
- public key errada;
- descriptor errado;
- offset errado;
- Station revogada;
- instrução precompile ausente;
- assinatura de evento expirado;
- Station de outro deployment.

## V2-402 — Implementar `record_observation`

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** P0
**Dependências:** V2-302, V2-401

### Instrução

```text
record_observation
```

### Dados

A instrução recebe somente o compromisso compacto e metadados necessários:

```text
DomainEventEnvelope
observation_kind
measurement_commitment ou valor inteiro autorizado
unit_code
expected_state_version
expected_previous_hash
```

Dados extensos e documentos ficam no manifesto off-chain.

### Invariantes

- asset existe e está no status correto;
- envelope pertence ao deployment e subject;
- versão e predecessor coincidem;
- timestamp está dentro da janela;
- fonte é Station ou balança registrada;
- peso não excede `max_asset_weight_grams`;
- unidade é aceita;
- estado é avançado uma única vez;
- EventAnchor é criado.

### Aceite

- peso negativo ou overflow falha;
- unidade desconhecida falha;
- evento expirado falha;
- replay falha;
- payload hash divergente falha;
- observação aceita atualiza asset e anchor atomicamente;
- histórico da observação fica verificável.

## V2-403 — Implementar observação de localização por commitment

**Prioridade:** P1 · **Tamanho:** M · **Tipo:** SEC
**Dependências:** V2-402

### Instrução

```text
record_location_observation
```

### Regras

- a cadeia guarda `location_commitment`, não coordenada precisa;
- o payload inclui tipo da fonte e nível de precisão em formato compacto;
- localização não altera custodian automaticamente;
- localização deve respeitar estado e autoridade da fonte;
- correção cria novo evento.

### Aceite

- coordenada precisa não aparecer nos dados on-chain;
- commitment alterado produzir novo hash;
- fonte não autorizada falhar;
- consulta pública não recuperar o valor privado.

## V2-404 — Implementar intents e nonces

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** P0/SEC
**Dependências:** V2-201, V2-301

### Estado

```rust
pub struct IntentState {
    pub intent_id: [u8; 32],
    pub subject_id: [u8; 32],
    pub intent_type: u16,
    pub expected_state_version: u64,
    pub nonce: u64,
    pub actor: Pubkey,
    pub status: u8,
    pub expires_at: i64,
    pub consumed_at: i64,
    pub payload_hash: [u8; 32],
    pub bump: u8,
}
```

### Instruções

```text
create_intent
cancel_intent
expire_intent
consume_intent // somente helper ou instrução específica
```

### Regras

- PDA: `['intent', deployment_id, subject_id, intent_id]`;
- intent é vinculada ao subject e ao actor;
- payload hash não pode mudar;
- intent expirada não pode ser consumida;
- cancelamento é terminal;
- consumo é único;
- nonce ou `expected_state_version` impede transação antiga;
- cancelamento fora da cadeia não é considerado suficiente.

### Aceite

- mesma intent consumida duas vezes falha;
- intent para subject diferente falha;
- payload diferente falha;
- intent expirada falha;
- transação com versão antiga falha mesmo que chegue depois;
- cancelamento impede consumo futuro.

## V2-405 — Implementar transferência de custódia

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** P0/SEC
**Dependências:** V2-301, V2-302, V2-404

### Instruções

```text
create_transfer_intent
accept_transfer_intent
finalize_custody_transfer
```

### Regras

- custodiante atual autoriza a criação;
- novo custodiante aceita quando a política exigir aceite bilateral;
- asset não pode estar fechado, em recall ou reservado de forma incompatível;
- `from_custodian` deve coincidir com asset;
- `to_custodian` não pode ser zero ou igual ao atual quando a mudança exigir troca;
- versão e predecessor devem coincidir;
- estado é avançado somente na finalização;
- transferência não muda RFID ou identity revision automaticamente.

### Aceite

- custodian antigo não autorizado falha;
- aceite de outra carteira falha;
- replay falha;
- duas transferências concorrentes, com mesma versão, deixam somente uma finalizada;
- PostgreSQL não aparece como autoridade de custódia.

---

# 8. Épico V2-05 — Lotes e linhagem

## V2-501 — Definir algoritmo de raízes de composição

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** P0/TEST
**Dependências:** V2-102, V2-104

### Decisão necessária

Escolher e documentar uma forma de root:

```text
Merkle root com folhas canônicas
```

ou

```text
hash encadeado de itens ordenados
```

A recomendação é Merkle root quando a verificação parcial for necessária. A folha deve incluir:

```text
asset_id
quantity
unit
weight_grams
role input/output
position ou regra de ordenação
```

### Aceite

- mesmo conjunto canônico produz a mesma root;
- reorder produz resultado definido pela regra;
- inclusão, remoção ou peso alteram a root;
- prova de inclusão possui fixture;
- root de input não pode ser reutilizada como root de output sem coincidência válida.

## V2-502 — Implementar `create_lot`

**Prioridade:** P1 · **Tamanho:** M · **Tipo:** P0
**Dependências:** V2-301, V2-501

### Instrução

```text
create_lot
```

### Regras

- cria `AssetState` do tipo `LOT`;
- registra composição root;
- define facility ou custodian responsável;
- não permite composição vazia quando o tipo exige inputs;
- não permite asset inexistente;
- não permite input já consumido;
- lote inicia com status coerente.

### Aceite

- lote pode ser consultado por PDA;
- composição root é recalculável;
- input é vinculado à linhagem;
- criação duplicada falha;
- lote vazio só é aceito por uma política explícita.

## V2-503 — Implementar split e merge de lotes

**Prioridade:** P1 · **Tamanho:** L · **Tipo:** P0
**Dependências:** V2-502

### Instruções

```text
split_lot
merge_lot
close_lot
```

### Invariantes

- quantidades não podem ser negativas;
- total dos filhos não pode exceder o saldo do pai;
- merge exige compatibilidade de unidade e tipo;
- lote fechado não pode ser dividido;
- filhos recebem novos asset IDs;
- linhagem cria arestas sem ciclos;
- operação possui root de entrada e saída;
- concorrência usa `state_version`.

### Aceite

- split por quantidade válida passa;
- split acima do saldo falha;
- merge de tipos incompatíveis falha;
- mesma parte não é alocada duas vezes;
- pai e filhos podem ser verificados por root.

## V2-504 — Implementar `LineageAnchor`

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** P0/SEC
**Dependências:** V2-501, V2-502

### Estado

```rust
pub struct LineageAnchor {
    pub asset_id: [u8; 32],
    pub lineage_root: [u8; 32],
    pub parent_root: [u8; 32],
    pub edge_count: u64,
    pub sequence: u64,
    pub last_transformation: [u8; 32],
    pub bump: u8,
}
```

### Regras

- PDA: `['lineage', deployment_id, asset_id]`;
- atualização é append/advance, nunca substituição silenciosa;
- edge count avança de forma monotônica;
- parent root deve ser compatível com os inputs;
- não permitir ciclo detectável pelo commitment;
- o detalhe da árvore permanece off-chain, mas a root final precisa ser ancorada.

### Aceite

- alteração de qualquer pai muda a root;
- saída de transformação aponta para a transformação correta;
- verificador detecta root divergente;
- tentativa de diminuir `edge_count` falha.

## V2-505 — Implementar `LineageAnchor` no verificador

**Prioridade:** P0 · **Tamanho:** M · **Tipo:** TEST/INTEG
**Dependências:** V2-504

### Aceite

O verificador deve:

- derivar PDA;
- validar owner, discriminator e bump;
- conferir asset ID;
- recomputar root do manifesto;
- conferir `edge_count` e `sequence`;
- marcar `LINEAGE_CONSISTENT` somente quando todos os vínculos necessários estiverem válidos;
- retornar `NOT_CHECKED` quando o manifesto não contiver dados suficientes.

---

# 9. Épico V2-06 — Frigorífico, carcaça e transformação

## V2-601 — Implementar `confirm_slaughter`

**Prioridade:** P1 · **Tamanho:** L · **Tipo:** P0/SEC
**Dependências:** V2-204, V2-301, V2-302, V2-405

### Instrução

```text
confirm_slaughter
```

### Regras

- signer deve ser facility do tipo `SLAUGHTERHOUSE`;
- facility deve estar ativa e dentro da validade;
- animal deve estar em status permitido;
- animal deve estar sob custódia ou recebimento da facility;
- evento deve incluir documento commitment quando exigido;
- animal entra em status que impede nova venda como animal vivo;
- a transição deve ser irreversível ou exigir correção explícita.

### Aceite

- facility errada falha;
- animal já abatido falha;
- animal em recall falha conforme política;
- status on-chain avança atomicamente;
- o anchor inclui facility e documento hash.

## V2-602 — Implementar `create_carcass`

**Prioridade:** P1 · **Tamanho:** M · **Tipo:** P0
**Dependências:** V2-601, V2-504

### Instrução

```text
create_carcass
```

### Regras

- cria `AssetState` do tipo `CARCASS`;
- referencia animal abatido;
- exige `parent_root` compatível;
- permite uma ou duas carcaças somente se a política do processo declarar isso;
- não permite carcaça duplicada para o mesmo evento;
- custodian inicial é a facility autorizada.

### Aceite

- carcaça sem abate finalizado falha;
- parent animal errado falha;
- duplicação de output falha;
- linhagem animal → carcaça é verificável.

## V2-603 — Implementar peso de carcaça

**Prioridade:** P1 · **Tamanho:** M · **Tipo:** P0
**Dependências:** V2-402, V2-602

### Regras

- peso em gramas inteiras;
- balança ou fonte autorizada;
- limite máximo da config;
- observação estável;
- correção somente por novo evento;
- peso não pode ser menor que zero;
- mudanças de peso não podem ultrapassar política sem hold.

### Aceite

- valor inválido falha;
- payload hash é verificável;
- peso final da carcaça fica refletido em `available_weight_grams`;
- correção mantém o valor anterior auditável.

## V2-604 — Implementar `TransformationAnchor`

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** P0/SEC
**Dependências:** V2-201, V2-204, V2-501

### Estado

```rust
pub struct TransformationAnchor {
    pub transformation_id: [u8; 32],
    pub facility_id: [u8; 32],
    pub transformation_type: u16,
    pub input_root: [u8; 32],
    pub output_root: [u8; 32],
    pub input_count: u32,
    pub output_count: u32,
    pub input_weight_grams: u64,
    pub output_weight_grams: u64,
    pub byproduct_weight_grams: u64,
    pub loss_weight_grams: u64,
    pub tolerance_basis_points: u16,
    pub status: u8,
    pub manifest_hash: [u8; 32],
    pub sequence: u64,
    pub expires_at: i64,
    pub bump: u8,
}
```

### Invariantes

- PDA: `['transformation', deployment_id, transformation_id]`;
- `input_root`, `output_root` e `manifest_hash` são imutáveis;
- facility não pode ser substituída após abertura;
- status segue `OPEN → FINALIZING → FINALIZED` ou `OPEN → ABORTED/EXPIRED`;
- transformação finalizada é terminal;
- transformação expirada não recebe novos chunks.

### Aceite

- criação duplicada falha;
- manifesto alterado não finaliza;
- status inválido falha;
- expiration é aplicada on-chain;
- layout possui fixture.

## V2-605 — Implementar `begin_transformation`

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** P0
**Dependências:** V2-204, V2-501, V2-604

### Regras

- somente facility autorizada;
- tipo de transformação compatível;
- contagens e pesos não excedem limites;
- tolerância vem de parâmetro permitido ou é bounded;
- input e output roots são não nulos;
- manifest hash corresponde ao payload assinado;
- expiração dentro da janela máxima;
- não criar outputs nesta etapa.

### Aceite

- facility suspensa falha;
- tolerância acima do máximo falha;
- expiração no passado falha;
- transformação aberta fica consultável;
- nenhuma entrada é consumida ainda.

## V2-606 — Implementar reserva de inputs

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** P0/SEC
**Dependências:** V2-301, V2-604, V2-605

### Instrução

```text
reserve_transformation_input
```

### Regras

- input existe e possui tipo compatível;
- input não está fechado, consumido, em recall ou reservado por outra transformação incompatível;
- `expected_state_version` coincide;
- peso reservado não excede saldo disponível;
- reservation PDA é derivada de transformação e input;
- reservation possui `reserved_until`;
- múltiplos inputs podem ser adicionados em transações separadas.

### Aceite

- duas reservas concorrentes do mesmo saldo não passam ambas;
- reserva expirada pode ser liberada;
- input reservado não pode ser vendido ou transformado em paralelo;
- tentativa de reservar acima do saldo falha;
- reserva não consome definitivamente o asset.

## V2-607 — Implementar chunks de transformação

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** P0/TEST
**Dependências:** V2-501, V2-604, V2-605

### Estado

```rust
pub struct TransformationChunk {
    pub transformation_id: [u8; 32],
    pub chunk_index: u32,
    pub previous_chunk_hash: [u8; 32],
    pub partial_root: [u8; 32],
    pub item_count: u32,
    pub weight_grams: u64,
    pub item_hash: [u8; 32],
    pub signer: Pubkey,
    pub bump: u8,
}
```

### Regras

- PDA: `['transformation-chunk', deployment_id, transformation_id, chunk_index]`;
- índice começa em zero;
- chunk anterior deve existir, exceto o primeiro;
- hash anterior deve coincidir;
- contagem total não pode exceder a anunciada;
- chunk não pode ser substituído;
- somente facility autorizada assina;
- chunk posterior à expiração falha.

### Aceite

- chunk fora de ordem falha;
- chunk duplicado falha;
- chunk com hash anterior errado falha;
- item count overflow falha;
- root final divergente impede finalização;
- tamanho de transação é medido para o maior chunk suportado.

## V2-608 — Implementar `finalize_transformation`

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** P0/SEC
**Dependências:** V2-606, V2-607, V2-504

### Validações

- transformação está aberta;
- facility ainda está ativa;
- todas as entradas exigidas estão reservadas;
- nenhuma entrada foi consumida por outra operação;
- chunks estão completos e ordenados;
- root final coincide;
- input/output count coincide;
- massa está dentro da tolerância;
- `output_weight + byproduct_weight + loss_weight` respeita a regra de balanço;
- manifest hash coincide;
- outputs não existem;
- asset não está em recall;
- intent ou autorização ainda é válida.

### Efeitos atômicos

- consome inputs;
- atualiza `available_weight_grams`;
- cria ou ativa outputs;
- cria edges de linhagem;
- atualiza `LineageAnchor`;
- marca transformação `FINALIZED`;
- libera reservas;
- grava `EventAnchor`.

### Aceite

- massa fora da tolerância falha sem alterar estado;
- output duplicado falha sem consumir inputs;
- finalização repetida falha;
- transformação finalizada não recebe novos inputs;
- todas as contas ficam consistentes após sucesso;
- teste de rollback atômico prova que nenhuma conta muda em erro.

## V2-609 — Implementar `abort_transformation` e expiração

**Prioridade:** P1 · **Tamanho:** M · **Tipo:** P0
**Dependências:** V2-606, V2-608

### Regras

- facility ou autoridade autorizada pode abortar;
- qualquer actor permitido pode marcar expiração somente quando `expires_at` passou;
- inputs são liberados;
- tentativa não é apagada;
- manifest hash permanece consultável;
- status terminal não volta a `OPEN`.

### Aceite

- reserva liberada após abort;
- input pode ser reservado por nova transformação depois da liberação;
- transformação abortada não cria outputs;
- histórico da tentativa permanece verificável.

## V2-610 — Testar balanço de massa

**Prioridade:** P0 · **Tamanho:** M · **Tipo:** TEST/SEC
**Dependências:** V2-608

### Casos

- balanço exato;
- perda dentro da tolerância;
- perda fora da tolerância;
- overflow de soma;
- unidade incompatível;
- peso zero;
- input maior que limite;
- output maior que input;
- byproduct e loss duplicados;
- arredondamento em basis points.

### Aceite

A fórmula usada no contrato e no protocolo compartilhado deve ser idêntica. O teste deve verificar limites sem overflow de `u64`.

---

# 10. Épico V2-07 — Produtos, embalagens e expedição

## V2-701 — Implementar criação de product lot

**Prioridade:** P1 · **Tamanho:** M · **Tipo:** P0
**Dependências:** V2-608

### Instrução

```text
create_product_lot
```

### Regras

- somente output de transformação finalizada;
- asset type `PRODUCT_LOT`;
- root e parent transformation corretos;
- quantidade e peso dentro do saldo produzido;
- data/validade não são armazenadas como texto livre sem validação;
- produto não pode ser criado duas vezes com o mesmo ID.

### Aceite

- output inexistente falha;
- transformação aberta falha;
- quantidade acima do output falha;
- produto possui linhagem para corte/carcaça/animal;
- `product_lot` entra com custodian definido.

## V2-702 — Implementar embalagem e commitment de QR

**Prioridade:** P1 · **Tamanho:** M · **Tipo:** P0/SEC
**Dependências:** V2-701

### Instrução

```text
create_package_commitment
```

### Regras

- asset type `PACKAGE`;
- QR/public identifier pode ser commitment ou ID opaco;
- não armazenar PII ou documento completo;
- package não pode conter quantidade acima do lote;
- package não pode ser criado após lote bloqueado sem autorização;
- QR deve apontar para consulta pública sem permitir enumeração previsível.

### Aceite

- package duplicado falha;
- QR alterado não encontra a mesma prova;
- consulta pública não revela custodian privado;
- package é rastreável até product lot.

## V2-703 — Implementar subproduto e perda

**Prioridade:** P1 · **Tamanho:** M · **Tipo:** P0
**Dependências:** V2-608

### Instruções

```text
create_byproduct_lot
record_transformation_loss
```

### Regras

- subproduto possui tipo e linhagem;
- perda não vira um asset comercial por acidente;
- massa total continua conciliável;
- correção cria evento posterior;
- subproduto bloqueado acompanha recall conforme política.

### Aceite

- subproduto aparece na massa;
- perda não pode ser usada como input;
- recall de input alcança subproduto quando a política exigir;
- balanço final não pode ser falsificado removendo perdas.

## V2-704 — Implementar shipment e aceite

**Prioridade:** P1 · **Tamanho:** L · **Tipo:** P0/SEC
**Dependências:** V2-405, V2-702

### Instruções

```text
create_shipment
accept_shipment
reject_shipment
```

### Regras

- origem possui custódia do asset ou escopo de expedição;
- destino é party autorizada;
- shipment vincula assets ou root de itens;
- item não pode estar em outro shipment ativo incompatível;
- aceite muda custodian somente após assinatura do destino;
- divergência de quantidade cria ocorrência e não desaparece;
- shipment finalizado não pode ser aceito duas vezes.

### Aceite

- origem errada falha;
- destino errado falha;
- duplicate shipment falha;
- aceite duplicado falha;
- transferência e expedição concorrentes não podem gastar a mesma versão;
- shipment possui linhagem e documentos por commitment.

---

# 11. Épico V2-08 — Quality hold e recall

## V2-801 — Implementar quality hold

**Prioridade:** P1 · **Tamanho:** M · **Tipo:** P0/SEC
**Dependências:** V2-301, V2-302

### Instruções

```text
place_quality_hold
release_quality_hold
```

### Regras

- hold bloqueia consumo, expedição ou transformação conforme flags;
- somente papel autorizado aplica ou libera;
- motivo é commitment/hash quando o detalhe for privado;
- release referencia o hold original;
- hold não altera a linhagem.

### Aceite

- asset em hold não é consumido por transformação;
- operador sem papel falha;
- release inválido falha;
- histórico de hold permanece.

## V2-802 — Implementar `RecallState`

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** P0/SEC
**Dependências:** V2-504, V2-801

### Estado

```rust
pub struct RecallState {
    pub recall_id: [u8; 32],
    pub issuer: Pubkey,
    pub affected_root: [u8; 32],
    pub reason_hash: [u8; 32],
    pub status: u8,
    pub opened_at: i64,
    pub closed_at: i64,
    pub sequence: u64,
    pub bump: u8,
}
```

### Instruções

```text
open_recall
acknowledge_recall
close_recall
```

### Regras

- PDA: `['recall', deployment_id, recall_id]`;
- issuer deve possuir papel compatível;
- affected root não pode ser alterada;
- recall aberto bloqueia operações definidas pela política;
- fechar recall não apaga o fato de que ele existiu;
- asset atingido pode receber flag on-chain ou ser validado por proof de inclusão.

### Aceite

- recall sem autoridade falha;
- affected root alterada falha;
- asset afetado não pode ser expedido quando o bloqueio for obrigatório;
- recall fechado continua consultável;
- prova de inclusão do asset é verificável.

## V2-803 — Implementar bloqueio de recall em operações

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** P0/SEC
**Dependências:** V2-802

### Operações bloqueadas

- transformação;
- criação de produto;
- embalagem;
- expedição;
- aceite;
- transferência de custódia, conforme política.

### Aceite

Cada instrução deve possuir teste específico. Não depender somente da API para bloquear recall.

## V2-804 — Implementar caminho de investigação reversa

**Prioridade:** P1 · **Tamanho:** M · **Tipo:** TEST/INTEG
**Dependências:** V2-802, V2-803

### Aceite

Dado um animal, carcaça, corte, product lot ou package, o sistema deve conseguir encontrar descendentes por root e anchors. O verificador deve diferenciar:

```text
linhagem comprovada
linhagem incompleta
linhagem não verificada
```

---

# 12. Épico V2-09 — Verificação on-chain e clientes

## V2-901 — Atualizar IDL e gerar cliente v2

**Prioridade:** P0 · **Tamanho:** M · **Tipo:** INTEG
**Dependências:** V2-201 a V2-803 conforme cada instrução

### Arquivos

```text
chain/target/idl/lastro_v2.json
apps/web/src/solana/v2/instructions.ts
services/api/src/solana/v2/instruction_builder.rs
```

### Aceite

- IDL corresponde ao código compilado;
- instruction discriminator possui fixture;
- cliente não aceita program ID v1 para instrução v2;
- accounts e flags são conferidos;
- nenhum builder esconde conta obrigatória.

## V2-902 — Criar derivação de PDAs em TypeScript e Rust

**Prioridade:** P0 · **Tamanho:** M · **Tipo:** TEST/INTEG
**Dependências:** V2-102, V2-201

### PDAs

```text
config-v2
station-v2
facility
party
asset
migration
event
intent
lineage
transformation
transformation-chunk
recall
```

### Aceite

A mesma entrada deve produzir a mesma PDA e bump em:

- Rust protocol;
- programa Anchor;
- API Rust;
- frontend TypeScript;
- script de verificação.

## V2-903 — Criar `verifyCanonicalChainStateV2.ts`

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** SEC/INTEG
**Dependências:** V2-206, V2-301, V2-504, V2-604, V2-802

### Verificações

- trusted deployment ID;
- trusted authority;
- owner e discriminator das contas;
- config v2;
- station/facility registry;
- AssetState;
- EventAnchor;
- IntentState;
- TransformationAnchor;
- LineageAnchor;
- RecallState;
- roots e hashes;
- versão e predecessor;
- assinaturas e transações finalizadas, quando disponíveis.

### Resultado

Adicionar camadas independentes:

```text
DOMAIN_EVENT_HASHES
LINEAGE_CONSISTENCY
MASS_BALANCE
FACILITY_CREDENTIAL
DOCUMENT_HASHES
RECALL_STATUS
ON_CHAIN_STATE
```

### Aceite

- RPC indisponível retorna `NOT_CHECKED`;
- conta divergente retorna `INVALID`;
- autoridade não provisionada não é inferida do package;
- `NOT_CHECKED` nunca torna o resultado global `VALID`;
- verifier v1 e v2 não compartilham offsets sem versão.

## V2-904 — Criar EvidencePackage v2

**Prioridade:** P1 · **Tamanho:** L · **Tipo:** INTEG/SEC
**Dependências:** V2-103, V2-501, V2-604, V2-802

### Conteúdo

- schema version;
- deployment ID;
- asset ou public product ID;
- eventos compactos;
- manifest hash;
- transformação;
- inputs e outputs por commitment;
- provas de root;
- transaction signatures;
- documentos por hash;
- status de recall;
- nível de proveniência.

### Regras

- limite de profundidade;
- limite de nós;
- limite de tamanho;
- sem PII por padrão;
- package assinado ou hashável;
- parser rejeita campos desconhecidos quando schema for fechado.

### Aceite

- package exportado pode ser verificado offline;
- tampering altera o resultado;
- pacote incompleto retorna `NOT_CHECKED` ou `INVALID` conforme o caso;
- package v1 continua sendo lido pelo verificador v1.

## V2-905 — Criar transaction builder v2

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** INTEG/TEST
**Dependências:** V2-901, V2-902, V2-607

### Arquivos

```text
services/api/src/solana/v2/transaction_builder.rs
apps/web/src/solana/v2/transaction.ts
```

### Regras

- construir somente instruções v2 permitidas;
- medir bytes serializados reais;
- rejeitar antes da assinatura se exceder limite;
- separar chunks grandes;
- preservar account metas e signer correto;
- usar `lastValidBlockHeight`;
- registrar `intent_id` e `payload_hash`;
- confirmar somente em commitment definido.

### Aceite

- transaction size benchmark passa;
- instrução v2 possui accounts esperadas;
- wallet assina somente após preflight;
- transaction expirada pode ser substituída somente quando a política permitir;
- rebroadcast não duplica evento.

---

# 13. Épico V2-10 — Integração API, banco e Agent

## V2-1001 — Criar migrações PostgreSQL v2

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** INTEG
**Dependências:** V2-301, V2-504, V2-604, V2-802

### Migrações

```text
0008_domain_assets.sql
0009_parties_roles.sql
0010_documents_provenance.sql
0011_domain_events.sql
0012_lots_and_lineage.sql
0013_transformations.sql
0014_carcasses_products_packages.sql
0015_shipments_and_receipts.sql
0016_quality_holds_recalls.sql
0017_onchain_anchors_v2.sql
0018_public_views_and_indexes.sql
0019_reconciliation_guards.sql
```

### Aceite

- banco novo sobe do zero;
- banco v1 atualiza sem perder dados;
- eventos são append-only;
- projeção só avança após finalização on-chain;
- constraints e triggers cobrem consumo duplicado;
- rollback de aplicação não exige apagar migração aplicada.

## V2-1002 — Criar API de intents e transações v2

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** INTEG/SEC
**Dependências:** V2-404, V2-905, V2-1001

### Endpoints mínimos

```http
POST /api/v2/intents
GET  /api/v2/intents/{intentId}
POST /api/v2/intents/{intentId}/cancel
GET  /api/v2/events/{eventId}/transaction-data
POST /api/v2/events/{eventId}/submit
POST /api/v2/events/{eventId}/confirm
```

### Regras

- payload hash é calculado antes da assinatura;
- intent fica vinculada a actor, subject e versão esperada;
- API não confirma sem RPC finalizado;
- retry exato é idempotente;
- payload divergente com mesmo ID retorna conflito;
- status da API não substitui status da Solana.

## V2-1003 — Criar reconciliador v2

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** INTEG/SEC
**Dependências:** V2-903, V2-1001, V2-1002

### Responsabilidades

- localizar intents pendentes;
- consultar transação e contas;
- verificar envelope;
- atualizar anchors locais;
- atualizar projeções;
- liberar ou manter reservas;
- detectar divergência;
- registrar incidentes;
- respeitar backoff e limite de RPC;
- não reprocessar evento terminal como novo evento.

### Aceite

- queda da API depois do submit é recuperável;
- queda do navegador não perde intenção;
- transação não finalizada não atualiza projeção final;
- divergência gera incidente sem sobrescrever histórico.

## V2-1004 — Estender Agent com outbox v2

**Prioridade:** P1 · **Tamanho:** L · **Tipo:** INTEG/SEC
**Dependências:** V2-103, V2-1002

### Regras

- não misturar `StationEvent` v1 com manifestos v2;
- usar `outbox_kind` explícito;
- persistir antes do upload;
- payload imutável;
- retry por classe de erro;
- quarentena terminal;
- ACK não pode transformar leitura antiga em observação fresca.

### Aceite

- reinício do Agent reenvia sem duplicar;
- observação inválida vai para quarentena;
- erro 5xx é retryable;
- erro de assinatura é terminal;
- outbox v1 continua passando seus testes.

## V2-1005 — Atualizar schemas OpenAPI e JSON Schema

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** INTEG/DOC
**Dependências:** V2-103, V2-1001, V2-1002

### Arquivos

```text
schemas/openapi-v2.yaml
schemas/domain-event.schema.json
schemas/transformation-manifest.schema.json
schemas/evidence-package-v2.schema.json
schemas/lineage.schema.json
```

### Aceite

- contrato descreve limites e enums;
- arrays possuem limites;
- `NOT_CHECKED` é representado;
- DTO público não herda campos privados;
- erro de conflito possui código distinto de indisponibilidade.

---

# 14. Épico V2-11 — Segurança, testes e performance

## V2-1101 — Criar harness LiteSVM v2

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** TEST
**Dependências:** V2-002, V2-201, V2-301

### Implementação

Criar `chain/programs/lastro-v2/tests/common/mod.rs` com:

- banco LiteSVM;
- program bytes v2;
- config inicializada;
- wallets determinísticas somente para teste;
- helper de Secp256r1;
- helper de PDA;
- helper de transação finalizada;
- leitura e comparação de contas;
- builders de eventos e manifestos.

### Aceite

- fixture de teste não pode ser aceita em deployment real;
- cada teste inicia estado isolado;
- helpers não escondem asserts críticos;
- falha mostra instrução e conta envolvidas.

## V2-1102 — Testar authorization boundaries

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** TEST/SEC
**Dependências:** V2-203, V2-204, V2-205, V2-405, V2-605

### Casos

- autoridade errada;
- facility errada;
- Station de outro deployment;
- Station suspensa;
- Station revogada;
- facility suspensa;
- party sem papel;
- custodian antigo;
- destino não autorizado;
- signer que não é owner;
- account owner errado;
- PDA com seed errada.

### Aceite

Cada caso falha com erro específico e não altera nenhuma conta.

## V2-1103 — Testar replay e concorrência

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** TEST/SEC
**Dependências:** V2-302, V2-404, V2-405, V2-606, V2-608

### Casos

- mesma transação reenviada;
- mesma intent consumida duas vezes;
- duas transferências com mesma versão;
- duas reservas para o mesmo saldo;
- duas finalizações da mesma transformação;
- shipment e transformação concorrentes;
- cancelamento simultâneo ao consumo;
- evento antigo depois de novo estado;
- transformação expirada durante a finalização.

### Aceite

Somente uma transição válida pode vencer. O estado final deve ser determinístico e nenhuma operação inválida pode consumir recursos.

## V2-1104 — Testar tampering e canonicalização

**Prioridade:** P0 · **Tamanho:** M · **Tipo:** TEST/SEC
**Dependências:** V2-103, V2-104, V2-903

### Casos

- payload hash alterado;
- manifest hash alterado;
- ordem de inputs alterada fora da regra;
- bytes reservados não zero;
- enum desconhecido;
- número com overflow;
- timestamp fora da janela;
- root truncada;
- documento com hash divergente;
- package alterado depois do anchor.

### Aceite

O resultado será `INVALID` quando a prova contradizer o estado e `NOT_CHECKED` quando faltar dado necessário sem contradição.

## V2-1105 — Medir tamanho, compute e rent

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** TEST
**Dependências:** V2-607, V2-608, V2-704, V2-905

### Medições

- bytes de cada instrução;
- bytes da transação legacy;
- bytes da transação v0;
- compute units;
- número máximo de accounts;
- rent de cada PDA;
- custo de transformação por chunk;
- custo de criação de package;
- custo de recall e proof.

### Aceite

- limites são registrados em fixture;
- o builder rejeita tamanho acima do limite;
- o chunk máximo é escolhido por benchmark, não por estimativa;
- nenhuma tarefa de produção depende de uma transação gigante;
- a política de custo é documentada.

## V2-1106 — Adicionar fuzz/property tests

**Prioridade:** P1 · **Tamanho:** L · **Tipo:** TEST/SEC
**Dependências:** V2-103, V2-501, V2-610

### Propriedades

- encode/decode preserva bytes;
- hash muda quando campo semântico muda;
- massa nunca aceita soma overflow;
- root não aceita lista ambígua;
- enum desconhecido é rejeitado;
- state version nunca diminui;
- status terminal não volta a aberto;
- replay nunca gera novo sucesso;
- canonicalização é determinística.

### Aceite

Fuzzing executado no CI ou em job noturno com corpus versionado e reproduzibilidade de seeds.

## V2-1107 — Auditar dependências e exceções

**Prioridade:** P0 · **Tamanho:** M · **Tipo:** SEC
**Dependências:** V2-002

### Regras

- manter exceções RustSec somente quando justificadas pelo LiteSVM/precompile;
- revisar versão do Anchor e Solana;
- gerar SBOM;
- auditar Node e Rust;
- verificar imagens por digest;
- não adicionar exceção genérica;
- documentar risco residual.

### Aceite

A CI falha para advisory não documentado e para alteração de lockfile sem revisão.

## V2-1108 — Criar invariantes de segurança como documentação executável

**Prioridade:** P1 · **Tamanho:** M · **Tipo:** DOC/TEST
**Dependências:** V2-1102, V2-1103

### Entregável

Criar `docs/SECURITY_INVARIANTS_V2.md` com a tabela:

```text
invariante
instruções que o aplicam
contas envolvidas
erro esperado
teste que o cobre
risco residual
```

### Aceite

Toda regra P0 do programa possuir ao menos um teste positivo e um negativo vinculados por nome.

---

# 15. Épico V2-12 — Deployment, migração e piloto

## V2-1201 — Criar deployment manifest v2

**Prioridade:** P0 · **Tamanho:** M · **Tipo:** DOC/SEC
**Dependências:** V2-003, V2-201, V2-203, V2-204

### Conteúdo

```json
{
  "protocol": "lastro-v2",
  "schemaVersion": 1,
  "programId": "...",
  "deploymentId": "...",
  "authority": "...",
  "configPda": "...",
  "stationRegistry": "...",
  "facilityRegistry": "...",
  "verifierVersion": "...",
  "programBinarySha256": "..."
}
```

### Aceite

- manifest é assinado ou distribuído por canal confiável;
- browser verifier não aceita authority vinda do EvidencePackage;
- program ID e binary hash podem ser auditados;
- staging e produção possuem manifests diferentes.

## V2-1202 — Implantar v2 em localnet e executar smoke test

**Prioridade:** P0 · **Tamanho:** M · **Tipo:** TEST
**Dependências:** V2-201, V2-203, V2-204, V2-301, V2-404

### Smoke test

```text
initialize_v2
register_station_v2
register_facility
register_party
register_asset
create_intent
record_observation
```

### Aceite

- deployment limpo funciona;
- todas as PDAs esperadas são derivadas;
- config lida pelo RPC bate com o manifest;
- transações finalizam;
- verificador retorna as camadas esperadas.

## V2-1203 — Criar fixture de migração animal v1 → v2

**Prioridade:** P0 · **Tamanho:** M · **Tipo:** TEST/INTEG
**Dependências:** V2-303, V2-903

### Aceite

Para cada animal fixture:

- comparar projeção API v1;
- comparar conta v1 RPC;
- executar migração;
- validar AssetState v2;
- validar MigrationRecord;
- exportar package combinado;
- verificar histórico v1 e continuação v2;
- bloquear animal com divergência.

## V2-1204 — Criar script de migração controlada

**Prioridade:** P1 · **Tamanho:** L · **Tipo:** INTEG/SEC
**Dependências:** V2-1203

### Arquivos

```text
scripts/migrate_animals_v1_to_v2.py
scripts/verify_migration_report.py
```

### Regras

- modo `--dry-run` obrigatório;
- gerar lista de elegíveis e bloqueados;
- exigir deployment manifest correto;
- não usar chave privada de produção no código;
- rate limit e retry;
- registrar tx signature;
- parar ao detectar divergência;
- produzir relatório assinado ou hashável.

### Aceite

- dry-run não escreve na chain;
- execução pode ser retomada sem duplicar migração;
- erro em um animal não corrompe os demais;
- relatório permite auditoria posterior.

## V2-1205 — Atualizar CI para v2

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** TEST/INTEG
**Dependências:** V2-1101 a V2-1107, V2-1202

### Alterações

Adicionar jobs ou passos para:

```text
cargo test --manifest-path chain/Cargo.toml
anchor build do v1 e v2
fixtures cross-language
LiteSVM v2
transaction-size v2
PDA fixtures
migration dry-run
verifier v2
E2E animal -> product -> recall
secret scanning
SBOM
```

### Aceite

- PR que quebra v1 falha;
- PR que muda layout sem fixture falha;
- PR que aceita replay falha;
- PR que inclui segredo em fixture falha;
- E2E localnet passa com program ID efêmero.

## V2-1206 — Criar runbooks de operação on-chain

**Prioridade:** P1 · **Tamanho:** M · **Tipo:** DOC
**Dependências:** V2-1201, V2-1202

### Runbooks

```text
RUNBOOK_V2_DEPLOY.md
RUNBOOK_V2_AUTHORITY_ROTATION.md
RUNBOOK_V2_STATION_REVOCATION.md
RUNBOOK_V2_FACILITY_SUSPENSION.md
RUNBOOK_V2_RECONCILIATION.md
RUNBOOK_V2_MIGRATION_BLOCKED.md
RUNBOOK_V2_PROGRAM_INCIDENT.md
```

### Aceite

Cada runbook deve informar:

- pré-condições;
- comando ou procedimento;
- signer necessário;
- dados que devem ser preservados;
- como validar o resultado;
- como pausar novas escritas;
- como reabrir a operação.

## V2-1207 — Gate de piloto controlado

**Prioridade:** P0 · **Tamanho:** L · **Tipo:** SEC/TEST
**Dependências:** todos os P0 e V2-1204

### Fluxo obrigatório

```text
animal v1 migrado
  -> observação de peso
  -> custódia
  -> recebimento no frigorífico
  -> abate
  -> carcaça
  -> transformação com múltiplos outputs
  -> product lot
  -> package
  -> shipment
  -> aceite
  -> recall
  -> investigação reversa
```

### Falhas obrigatórias

- input duplicado;
- duas transformações concorrentes;
- facility suspensa;
- Station revogada;
- payload alterado;
- massa fora da tolerância;
- RPC indisponível;
- API interrompida depois do submit;
- Agent reiniciado;
- recall bloqueando expedição;
- consulta pública sem PII.

### Aceite

O piloto só passa quando:

- nenhuma entrada for consumida duas vezes;
- nenhuma operação inválida for finalizada;
- linhagem puder ser verificada para frente e para trás;
- recall alcançar os descendentes;
- projeção PostgreSQL concordar com a Solana depois da reconciliação;
- backup puder ser restaurado;
- operação manual estiver documentada;
- `NOT_CHECKED` não for mostrado como sucesso.

---

# 16. Ordem de execução recomendada

A execução deve seguir esta sequência. Não iniciar uma fase enquanto o gate de saída da anterior não estiver verde.

## Fase 0 — congelamento v1

```text
V2-001
V2-004
V2-206 — somente fixtures v1 de compatibilidade
```

**Saída:** v1 protegido e decisão de program ID registrada.

## Fase 1 — protocolo e bootstrap

```text
V2-002
V2-003
V2-101
V2-102
V2-103
V2-104
V2-105
V2-201
```

**Saída:** crate protocol v2, program crate v2 e config inicializável.

## Fase 2 — autoridade e estado base

```text
V2-202
V2-203
V2-204
V2-205
V2-206
V2-301
V2-302
```

**Saída:** deployment registrável e `AssetState` mutável com guards atômicos.

## Fase 3 — migração e custódia

```text
V2-303
V2-304
V2-401
V2-402
V2-403
V2-404
V2-405
V2-1102
V2-1103
```

**Saída:** animal v1 continua para v2, recebe observação e pode mudar custódia sem replay.

## Fase 4 — linhagem e transformação

```text
V2-501
V2-502
V2-503
V2-504
V2-505
V2-601
V2-602
V2-603
V2-604
V2-605
V2-606
V2-607
V2-608
V2-609
V2-610
```

**Saída:** carcaça pode gerar múltiplos outputs com massa, reserva, chunks e linhagem verificáveis.

## Fase 5 — cadeia de produto

```text
V2-701
V2-702
V2-703
V2-704
V2-801
V2-802
V2-803
V2-804
```

**Saída:** produto e embalagem podem ser expedidos, recebidos e bloqueados por recall.

## Fase 6 — clientes e serviços

```text
V2-901
V2-902
V2-903
V2-904
V2-905
V2-1001
V2-1002
V2-1003
V2-1004
V2-1005
```

**Saída:** API, frontend, Agent e verificador conseguem construir, submeter e conferir operações v2.

## Fase 7 — hardening e piloto

```text
V2-1101
V2-1104
V2-1105
V2-1106
V2-1107
V2-1108
V2-1201
V2-1202
V2-1203
V2-1204
V2-1205
V2-1206
V2-1207
```

**Saída:** deployment controlado, migração ensaiada, gates verdes e piloto autorizado.

---

# 17. Backlog mínimo para o primeiro incremento de código

Se a equipe precisar começar por um incremento pequeno e revisável, executar somente esta fatia:

```text
V2-001  decisão de novo program ID
V2-002  crate lastro-v2
V2-003  bootstrap de ID
V2-101  enums e constantes
V2-102  IDs e domínios de hash
V2-103  DomainEventEnvelope
V2-105  fixtures cross-language mínimas
V2-201  ProtocolConfigV2
V2-203  StationRegistry mínimo
V2-204  FacilityRegistry mínimo
V2-301  AssetState
V2-302  helper de transição atômica
V2-1101 harness LiteSVM
V2-1102 authorization básica
V2-1103 replay e concorrência básica
V2-1202 smoke test localnet
```

Esse incremento **não deve** tentar implementar transformação, produto ou recall. Ele serve para provar que a base v2, os layouts, as PDAs, as assinaturas e os guards estão corretos antes de aumentar a superfície do programa.

---

# 18. Definition of Ready para cada instrução pública

Antes de implementar qualquer instrução Anchor, o ticket deve possuir:

```text
nome público
Context Anchor definido
PDA(s) envolvida(s)
contas mutáveis
contas somente leitura
signers obrigatórios
seeds e bumps
args serializados
invariantes de entrada
invariantes de saída
transições de status
erros esperados
limite de bytes
limite de compute
caso de replay
caso de concorrência
fixture cross-language
teste positivo
teste negativo
efeito na projeção PostgreSQL
efeito no verificador
plano de migração
```

Se algum desses campos estiver indefinido, o ticket deve permanecer em design e não entrar em implementação.

---

# 19. Erros on-chain mínimos

Criar em `chain/programs/lastro-v2/src/error.rs` erros específicos, sem reutilizar uma mensagem genérica:

```text
InvalidDeployment
UnsupportedSchemaVersion
ConfigAlreadyInitialized
InvalidConfigAuthority
InvalidPda
InvalidAccountOwner
InvalidAccountDiscriminator
InvalidAssetId
DuplicateAsset
InvalidAssetType
InvalidAssetStatus
InvalidStateVersion
InvalidPreviousHash
InvalidEventHash
EventAlreadyAnchored
EventExpired
EventFromFuture
InvalidStation
StationNotActive
StationKeyRevoked
StationKeyExpired
InvalidFacility
FacilityNotAuthorized
FacilitySuspended
FacilityExpired
InvalidPartyRole
UnauthorizedActor
IntentNotFound
IntentExpired
IntentCancelled
IntentAlreadyConsumed
IntentPayloadMismatch
AssetAlreadyReserved
AssetAlreadyConsumed
ReservationExpired
ReservationConflict
TransformationNotOpen
TransformationExpired
TransformationAlreadyFinalized
InvalidChunkIndex
InvalidChunkPredecessor
InvalidInputRoot
InvalidOutputRoot
InvalidManifestHash
InvalidCount
InvalidWeight
MassBalanceOutsideTolerance
DuplicateOutput
LineageCycleDetected
AssetInRecall
RecallNotFound
RecallAlreadyClosed
InvalidShipment
ShipmentAlreadyAccepted
InvalidMigration
LegacyStateMismatch
```

Cada erro deve ter teste e mapeamento para uma classe de resposta na API:

```text
400 invalid input
401/403 authorization
409 state conflict or replay
410 expired
422 domain rule
503 dependency unavailable
```

---

# 20. Riscos de execução e mitigação

| Risco | Sinal de alerta | Mitigação |
|---|---|---|
| Conta genérica grande demais | rent e compute crescem por item | usar PDAs de chunks e roots |
| Transformação em uma transação | falha por limite de bytes | begin/reserve/chunk/finalize |
| Dois estados canônicos | v1 e v2 divergem | migration record e data de corte |
| API vira autoridade | projeção avança antes da chain | reconciliador e testes de confirmação |
| Station revogada quebra histórico | verifier olha somente status atual | validade histórica |
| Recall não escala | lista on-chain de todos os afetados | affected root + proofs + projeção |
| PII na chain | payload contém documento ou nome | commitments e schema review |
| Replay após supersessão | nonce existe somente no PostgreSQL | IntentState on-chain |
| Massa manipulável | soma fora de faixa aceita | inteiros, tolerância e testes |
| IDL deriva do código | cliente aceita contas erradas | fixtures e CI de layout |
| Dependência vulnerável | exceção ampla no audit | exceções mínimas e revisão |
| Falha operacional | equipe não sabe pausar | runbooks e piloto controlado |

---

# 21. Critério de conclusão do backlog v2

O backlog estará concluído para um piloto quando todos os itens abaixo forem verdadeiros:

- `lastro-v2` possui program ID e deployment manifest próprios;
- v1 continua compilando e passando seus testes;
- todos os layouts v2 possuem fixtures;
- PDAs são iguais em Rust, TypeScript e verificador;
- `ProtocolConfigV2`, Stations e facilities possuem ciclo de vida;
- `AssetState` impede replay e concorrência inválida;
- animal v1 pode migrar sem perder predecessor ou custodian;
- observação física possui assinatura, freshness e source registry;
- transferência exige actor autorizado e versão esperada;
- lote e linhagem possuem roots verificáveis;
- transformação usa reserva, chunks, balanço de massa e finalização atômica;
- outputs não podem ser duplicados;
- produto, package e shipment possuem custódia e linhagem;
- recall bloqueia as operações definidas;
- verificador separa `VALID`, `INVALID` e `NOT_CHECKED`;
- API e PostgreSQL são projeções reconciliáveis;
- Agent mantém evidência offline sem replay indevido;
- testes de autorização, tampering, concorrência, massa e tamanho passam;
- backup, migração e restauração foram ensaiados;
- o piloto possui runbooks e critérios de pausa;
- nenhuma garantia jurídica, biológica ou regulatória é afirmada apenas pela existência da transação Solana.

**Resultado esperado:** o smart contract v2 funciona como motor de invariantes e âncora de compromissos. Ele não tenta armazenar todos os detalhes da cadeia nem substitui documentos, operação, governança ou verificação independente.

---

## Referências

[1]: ../docs/PLANO_ETAPA_3_SMART_CONTRACT.md "Plano da Etapa 3 — evolução do smart contract Solana"
[2]: ../docs/PLANO_ETAPA_4_SERVICOS_PRODUTO.md "Plano da Etapa 4 — evolução dos serviços e do produto"
[3]: ../docs/PLANO_ETAPA_5_SEGURANCA_ROLLOUT.md "Plano da Etapa 5 — segurança, governança, validação e rollout"
[4]: ../docs/SECURITY.md "Modelo de segurança atual do Lastro"
[5]: ../docs/HARDENING_STATUS.md "Status de hardening do Lastro"
[6]: https://www.anchor-lang.com/docs "Documentação do Anchor Framework"
[7]: https://solana.com/docs/core/transactions "Documentação de transações Solana"

**Autor:** Manus AI

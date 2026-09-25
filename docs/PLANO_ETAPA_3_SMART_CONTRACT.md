# Etapa 3 — Evolução do protocolo e do smart contract Solana

**Projeto:** Lastro
**Objetivo:** suportar animal individual, lote, carcaça, transformação, produto, embalagem, expedição e recall sem quebrar o `StationEvent` v1 de 276 bytes.

## 1. Decisão de compatibilidade

O programa atual tem uma superfície pequena:

```text
initialize
origin
transfer
reidentify
```

A instrução atual carrega o `StationEvent` inteiro, exige a instrução Secp256r1 no índice 0 e a instrução Lastro no índice 1. O frontend e o backend também congelam esse envelope em duas instruções e em uma transação de até 1232 bytes.

A decisão correta é:

> **Não alterar o significado, o tamanho ou os offsets do `StationEvent` v1. Criar um protocolo de domínio v2 separado.**

O v1 continua sendo responsável por:

- leitura física de RFID;
- origem do animal;
- transferência de custódia do animal vivo;
- reidentificação de RFID;
- sequência e predecessor do animal;
- assinatura P-256 da Station.

O v2 será responsável por:

- peso;
- localização;
- status bovino;
- lote;
- abate;
- carcaça;
- transformação;
- produto;
- embalagem;
- expedição;
- recall;
- documentos e compromissos criptográficos.

## 2. Estratégia de deployment

Não é recomendável alterar contas antigas em produção sem uma estratégia de migração explícita. A opção mais segura é:

```text
Lastro v1
  -> preservado para histórico e compatibilidade

Lastro v2
  -> novo schema/deployment/program ID
  -> novos tipos de ativo e eventos
```

No ambiente de desenvolvimento, pode existir um deployment v2 limpo. Em um rollout real, haverá duas possibilidades:

### Opção A — novo program ID e nova configuração

É a opção mais segura quando o layout antigo não pode ser expandido de forma compatível.

- cria novo `ProtocolConfigV2`;
- cria novo registro de deployment;
- mantém o v1 consultável;
- registra uma prova de migração do animal v1 para o ativo v2;
- bloqueia novas escritas v1 depois da data de corte;
- mantém o histórico v1 como origem da linhagem.

### Opção B — mesmo program ID com contas versionadas

É possível, mas exige uma migração formal:

- `AnimalState` antigo não é redimensionado silenciosamente;
- uma instrução cria `AssetStateV2`;
- a nova conta aponta para o `AnimalState` antigo;
- o programa impede duas fontes canônicas concorrentes;
- o verificador entende as duas versões;
- a migração é testada em LiteSVM e em um deployment real.

A recomendação é começar pela Opção A. Ela reduz o risco de misturar layouts antigos com regras novas.

## 3. Conta de configuração v2

O `ProtocolConfig` atual possui uma única chave Station imutável. Isso não é suficiente para um sistema industrial com rotação, revogação e múltiplas Stations, frigoríficos e fontes.

Criar:

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

PDA:

```text
["config-v2", deployment_id]
```

A configuração deve conter somente parâmetros de protocolo. Dados detalhados de fazenda, animal, corte ou pessoa devem ficar em suas contas próprias ou off-chain.

## 4. Registry de Stations e instalações

### 4.1 StationRegistry

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

PDA:

```text
["station-v2", deployment_id, station_id]
```

O status deve distinguir:

```text
ACTIVE
SUSPENDED
REVOKED
EXPIRED
```

A verificação histórica precisa conseguir perguntar se a chave era válida no momento do evento. Não basta olhar somente o status atual, porque uma Station revogada hoje pode ter produzido evidência válida ontem.

### 4.2 FacilityRegistry

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

PDA:

```text
["facility", deployment_id, facility_id]
```

Tipos de instalação:

```text
FARM
TRANSPORT_HUB
SLAUGHTERHOUSE
PROCESSING_FACILITY
DISTRIBUTION_CENTER
RETAIL
INSPECTION_SITE
```

O programa deve aceitar `confirm_slaughter` e `create_transformation_manifest` somente de um `FacilityRecord` ativo com papel compatível.

## 5. Conta genérica de ativo

Para evitar uma fonte canônica diferente para cada tipo, o v2 deve ter uma conta base:

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

PDA:

```text
["asset", deployment_id, asset_id]
```

Tipos iniciais:

```text
ANIMAL
LOT
CARCASS
CUT_BATCH
PRODUCT_LOT
PACKAGE
BYPRODUCT_LOT
SHIPMENT
```

O tipo não deve ser um texto livre. Deve ser um enum serializado como `u8` com versão de protocolo.

### Por que um estado genérico

Sem uma conta de ativo comum, o programa não consegue aplicar invariantes iguais a animal, carcaça e produto:

- sequência;
- status;
- custódia;
- consumo de entrada;
- reservation;
- bloqueio por recall;
- último hash;
- linhagem.

As tabelas especializadas continuam existindo off-chain para dados de negócio, mas a regra canônica de disponibilidade e consumo deve apontar para uma conta on-chain v2.

## 6. Compatibilidade do animal atual

O animal já possui `AnimalState` v1. Não devemos manter duas contas canônicas que possam divergir.

O rollout deve escolher uma das políticas:

### Política recomendada

- `AnimalState` v1 permanece somente como histórico compatível;
- `AssetState` v2 se torna canônico para novos eventos;
- `migrate_animal_v1` cria o ativo v2;
- a nova conta armazena `legacy_animal_state` ou `legacy_last_event_hash`;
- uma marca de migração impede repetir a operação;
- o verificador segue a origem v1 e a continuação v2.

PDA de migração:

```text
["migration", deployment_id, animal_id]
```

A migração deve copiar:

```text
animal_id
current_custodian
identity_revision
current_rfid_hash commitment
last_event_hash
last_event_sequence
```

A migração não deve inventar peso, localização, status sanitário ou documento. Esses campos começam como desconhecidos e recebem eventos v2 posteriormente.

## 7. Evento de domínio v2

O evento v2 não deve carregar JSON completo dentro da transação. A API canonicaliza o payload off-chain e envia uma representação compacta:

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

O manifesto completo fica no EvidencePackage. A Solana valida:

- versão conhecida;
- tipo permitido;
- subject correto;
- state version atual;
- predecessor correto;
- payload hash;
- source/authority;
- janela temporal;
- nonce/intent;
- status anterior;
- transição permitida.

A assinatura do domínio pode ser de dois tipos:

### Evento físico

Usado por Station ou balança:

```text
Secp256r1 precompile
  -> assina o envelope canônico v2
Lastro v2 instruction
  -> aplica o evento ao AssetState
custodian/facility wallet
  -> autoriza a transação
```

### Evento operacional

Usado por frigorífico, destino ou autoridade:

```text
wallet/credential do papel autorizado
  -> assina a transação
Lastro v2 instruction
  -> aplica o compromisso do manifesto
```

Não é necessário exigir assinatura de Station para um `TRANSFORMATION_MANIFEST_CREATED` se a origem de verdade desse evento for o sistema autorizado do frigorífico.

## 8. PDAs v2

### 8.1 AssetState

```text
["asset", deployment_id, asset_id]
```

Estado atual do ativo.

### 8.2 EventAnchor

```text
["event", deployment_id, event_id]
```

Guarda o compromisso mínimo de um evento finalizado:

```rust
pub struct EventAnchor {
    pub event_id: [u8; 32],
    pub subject_id: [u8; 32],
    pub event_type: u16,
    pub payload_hash: [u8; 32],
    pub previous_event_hash: [u8; 32],
    pub sequence: u64,
    pub signer: Pubkey,
    pub finalized_at: i64,
    pub bump: u8,
}
```

### 8.3 TransformationAnchor

```text
["transformation", deployment_id, transformation_id]
```

Guarda:

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
    pub bump: u8,
}
```

### 8.4 LineageAnchor

```text
["lineage", deployment_id, asset_id]
```

Pode guardar a raiz atual da árvore de ancestrais. A lista detalhada fica off-chain.

### 8.5 IntentState

```text
["intent", deployment_id, subject_id, intent_id]
```

Guarda:

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

### 8.6 RecallState

```text
["recall", deployment_id, recall_id]
```

O recall deve armazenar a raiz dos ativos afetados, motivo, emissor, status e data. Cada `AssetState` afetado deve consultar o compromisso ou receber um bloqueio explícito conforme o desenho de escala.

## 9. Instruções públicas v2

### Configuração e registros

```text
initialize_v2
register_station_v2
revoke_station_v2
register_facility
suspend_facility
register_party
```

### Identidade e animal

```text
migrate_animal_v1
register_asset
attach_identifier_commitment
record_observation
change_animal_status
```

### Lotes

```text
create_lot
split_lot
merge_lot
close_lot
```

### Movimentação e custódia

```text
create_transfer_intent
accept_transfer_intent
finalize_custody_transfer
create_shipment
accept_shipment
```

### Pós-abate

```text
confirm_slaughter
create_carcass
record_carcass_weight
begin_transformation
reserve_transformation_input
append_transformation_chunk
finalize_transformation
abort_transformation
create_product_lot
create_package_commitment
create_byproduct_lot
```

### Qualidade e recall

```text
place_quality_hold
release_quality_hold
open_recall
acknowledge_recall
retire_asset
```

Cada instrução deve possuir um `Context` Anchor específico. Não criar uma instrução genérica que aceite qualquer transição e um `event_type` sem aplicar regras específicas.

## 10. Fluxo recomendado para transformação

Uma desossa pode envolver muitos inputs e outputs. Não deve ser uma única instrução gigante.

### 10.1 Begin

```text
begin_transformation
```

Cria `TransformationAnchor` com:

- transformação;
- facility;
- input root;
- output root;
- quantidade esperada;
- peso esperado;
- tolerância;
- hash do manifesto;
- expiração;
- status `OPEN`.

### 10.2 Reservar entradas

```text
reserve_transformation_input
```

Para cada entrada, o programa verifica:

- asset existe;
- asset type é compatível;
- asset não está encerrado;
- asset não está em recall;
- asset version bate com `expected_state_version`;
- asset não está reservado por outra transformação;
- peso reservado está dentro do saldo;
- transformação não expirou.

A reserva não consome definitivamente o ativo. Ela possui prazo.

### 10.3 Anexar chunks

```text
append_transformation_chunk
```

Usado quando a lista é grande. Cada chunk contém:

- índice;
- hash do chunk anterior;
- raiz parcial;
- contagem;
- peso;
- hash de itens;
- assinatura do facility.

O payload detalhado permanece no EvidencePackage.

### 10.4 Finalizar

```text
finalize_transformation
```

A instrução exige:

- todas as reservas necessárias;
- root final igual ao anunciado;
- contagem consistente;
- peso dentro da tolerância;
- hash final do manifesto igual ao compromisso;
- saídas ainda não existentes;
- facility ativa;
- transformação não expirada.

A finalização:

- consome as entradas;
- cria ou ativa os outputs resumidos;
- atualiza raízes de linhagem;
- fecha a transformação;
- libera as reservas;
- grava `EventAnchor`.

### 10.5 Abortar

```text
abort_transformation
```

Permite liberar reservas expiradas ou canceladas. O aborto não apaga a tentativa; cria um status terminal e mantém o hash do manifesto original.

## 11. Nonce e concorrência

O sistema atual possui supersessão no PostgreSQL, mas uma transação Solana antiga pode continuar válida. O v2 deve resolver isso na cadeia.

Cada `AssetState` terá:

```text
state_version
last_event_hash
```

Cada instrução mutável recebe:

```text
expected_state_version
expected_previous_event_hash
intent_id ou nonce
```

A Solana aceita somente quando:

```text
expected_state_version == asset.state_version
expected_previous_event_hash == asset.last_event_hash
intent ainda não consumida
intent não expirada
```

Depois da aplicação:

```text
asset.state_version += 1
asset.last_event_hash = new_event_hash
intent.status = CONSUMED
```

Isso impede:

- duas vendas simultâneas;
- duas transformações consumindo a mesma carcaça;
- expedição duplicada da mesma caixa;
- transação antiga depois de supersessão;
- replay do mesmo aceite.

Cancelamento e expiração devem ser regras on-chain. Marcar uma intenção como cancelada apenas no PostgreSQL não é suficiente.

## 12. Limites de transação

O evento v2 não deve conter:

- JSON completo;
- lista de centenas de ativos;
- fotos;
- documentos;
- strings longas;
- coordenadas detalhadas;
- lista completa de caixas.

A instrução deve conter hashes, raízes, contagens, pesos e identificadores compactos.

A API deve medir cada transação como já mede a atual. O limite de 1232 bytes continua sendo um risco operacional para o envelope legacy. Para v2:

- usar instruções pequenas;
- dividir transformações em chunks;
- usar PDAs para estado intermediário;
- usar Address Lookup Tables somente se o envelope e a versão escolhida forem medidos;
- não confiar em estimativa manual;
- testar o tamanho serializado real;
- rejeitar a operação antes de pedir assinatura à carteira.

O número máximo de itens por chunk deve ser definido por benchmark em LiteSVM e em RPC real. Não congelar um número arbitrário sem teste.

## 13. Assinaturas e autoridade

### Station ou balança

Usar Secp256r1 quando o evento é uma observação física:

```text
Station/balança assina envelope v2
API verifica assinatura
Solana verifica precompile quando o evento precisa de garantia on-chain
```

### Frigorífico

Usar wallet ou credencial do `FacilityRegistry` para:

```text
confirm_slaughter
begin_transformation
finalize_transformation
create_product_lot
open_recall
```

### Destino

Usar a carteira do destino para:

```text
accept_shipment
accept_transfer_intent
acknowledge_recall
```

### Fonte oficial

Uma integração oficial não deve ser simulada como carteira de usuário comum. O sistema deve armazenar:

- `source_id`;
- `issuer_key_id`;
- `verification_status`;
- `retrieved_at`;
- `source_response_hash`.

## 14. Erros novos

Adicionar erros explícitos, por exemplo:

```text
InvalidAssetType
InvalidAssetStatus
InvalidStateVersion
InvalidPreviousHash
AssetAlreadyConsumed
AssetAlreadyReserved
ReservationExpired
TransformationExpired
TransformationNotOpen
InvalidInputRoot
InvalidOutputRoot
MassBalanceOutsideTolerance
FacilityNotAuthorized
FacilitySuspended
StationKeyRevoked
DocumentNotVerified
IntentExpired
IntentAlreadyConsumed
RecallActive
AssetInRecall
DuplicateAssetId
LineageCycleDetected
InvalidMigration
UnsupportedSchemaVersion
```

O frontend e a API devem distinguir:

- falha de validação;
- conflito de concorrência;
- dependência indisponível;
- credencial revogada;
- evento rejeitado on-chain;
- transação expirada;
- evidência inválida.

## 15. Verificador independente

O verificador atual consulta a história de um animal. O v2 precisa verificar:

1. evento físico e assinatura Station, quando houver;
2. evento de negócio e payload hash;
3. `previous_event_hash`;
4. `state_version`;
5. `AssetState` final;
6. `TransformationAnchor`;
7. raízes de input/output;
8. balanço de massa declarado;
9. linhagem para frente e para trás;
10. status de recall;
11. registry do frigorífico;
12. validade da chave no momento do evento;
13. documentos e hashes, quando disponíveis;
14. consistência entre Solana e EvidencePackage.

O resultado deve ser separado por propriedade:

```text
CRYPTOGRAPHICALLY_VALID
LINEAGE_CONSISTENT
MASS_BALANCE_CHECKED
FACILITY_CREDENTIAL_VALID
DOCUMENT_HASH_MATCHES
OFFICIAL_SOURCE_CONFIRMED
PHYSICAL_ORACLE_NOT_PROVEN
LEGAL_OWNERSHIP_NOT_DETERMINED
```

Não retornar apenas `VERIFIED: true`.

## 16. Testes da Etapa 3

### Protocol crate

- encode/decode de cada envelope v2;
- rejeição de schema desconhecido;
- hashes estáveis entre Rust e TypeScript;
- roots de input/output;
- balanço de massa;
- nonce e state version;
- intent expirada;
- chunk fora de ordem;
- predecessor incorreto;
- quantidade negativa ou overflow;
- unidade inválida.

### Anchor/LiteSVM

- criação de `AssetState`;
- migração v1 para v2;
- duas transições concorrentes;
- input reservado por duas transformações;
- finalização com output duplicado;
- massa fora da tolerância;
- facility suspensa;
- Station revogada;
- recall bloqueando expedição;
- abort liberando reserva;
- replay do mesmo intent;
- evento com predecessor antigo.

### Transação

- tamanho legacy/v0 medido;
- ordem do Secp256r1 e Lastro v2;
- offsets do envelope;
- criação e finalização de chunk;
- wallet deve assinar apenas depois da validação do payload;
- retry idempotente;
- transação expirada;
- rebroadcast seguro;
- confirmação finalizada.

## 17. Ordem de implementação do smart contract

1. Criar tipos v2 no `lastro-protocol`.
2. Criar fixtures cross-language.
3. Criar `ProtocolConfigV2`.
4. Criar `StationRegistry` e `FacilityRegistry`.
5. Criar `AssetState` e `EventAnchor`.
6. Criar `IntentState` e nonces.
7. Implementar `migrate_animal_v1`.
8. Implementar eventos simples de status e observação.
9. Implementar lotes e linhagem.
10. Implementar transformação com reservation/chunk/finalize.
11. Implementar carcaça, produto e embalagem.
12. Implementar expedição e aceite.
13. Implementar hold e recall.
14. Atualizar transaction builder e verificador.
15. Executar todos os gates antes de migrar qualquer deployment.

## 18. Decisão final da Etapa 3

O smart contract não deve tentar armazenar todos os detalhes da cadeia. Ele deve funcionar como um **motor de invariantes e âncora de compromissos**.

A divisão final é:

```text
Station / balança:
  observação física assinada

Frigorífico / destino:
  autorização operacional assinada

API:
  manifesto completo, documentos e projeções

Solana:
  estado canônico, versão, nonce, hashes, roots, status e bloqueios

Verifier:
  recomputação independente de integridade e linhagem
```

A alteração mais importante é criar o v2 como protocolo versionado e não transformar o evento físico v1 em um recipiente genérico para todo o negócio.

**Autor:** Manus AI

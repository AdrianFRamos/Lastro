# Etapa 4 — Evolução dos serviços e do produto

**Projeto:** Lastro
**Objetivo:** transformar o núcleo atual de captura de RFID e custódia individual em uma plataforma de rastreabilidade bovina que também acompanha abate, carcaças, cortes, produtos, embalagens, expedições e recall.

## 1. Resultado esperado

Ao terminar esta etapa de implementação, o sistema deverá conseguir executar o fluxo abaixo:

```text
cadastro do animal
  -> leitura RFID
  -> peso e localização
  -> movimento com documento
  -> transferência de custódia
  -> recebimento no frigorífico
  -> abate confirmado
  -> carcaça criada
  -> desossa e transformação
  -> lotes de cortes/subprodutos
  -> embalagem e pallet
  -> expedição
  -> aceite no destino
  -> consulta pública por QR code
  -> recall e investigação reversa
```

O sistema deverá separar cinco propriedades:

```text
identidade técnica
observação física
custódia operacional
proveniência documental
linhagem de transformação
```

A confirmação criptográfica de uma propriedade não deverá ser apresentada como prova automática das demais.

## 2. Princípios que devem guiar a implementação

### 2.1 Preservar o que já funciona

O `StationEvent` v1 de 276 bytes, o framing serial LSTR v1, o Agent outbox atual e o verificador do fluxo original devem continuar funcionando durante a migração.

Os arquivos atuais que não devem ser alterados semanticamente são:

```text
crates/lastro-protocol/src/event.rs
firmware/station/components/lastro_station/event.c
services/agent/src/serial/payload.rs
services/agent/src/serial/codec.rs
chain/programs/lastro/src/verify/event.rs
apps/web/src/protocol/stationEvent.ts
```

Eles podem receber correções de segurança, testes ou novos módulos paralelos, mas o contrato v1 não deve ganhar campos novos em seu formato existente.

### 2.2 Eventos em vez de edição

Peso, localização, status, abate, transformação, expedição e recall devem criar eventos append-only.

Não implementar:

```http
PATCH /api/animals/{animalId}
PATCH /api/products/{productId}
```

para alteração livre de estado canônico.

Implementar comandos de domínio que validem a transição:

```http
POST /api/animals/{animalId}/observations/weight
POST /api/animals/{animalId}/observations/location
POST /api/movements
POST /api/slaughter-receipts
POST /api/transformations
POST /api/shipments
POST /api/recalls
```

### 2.3 O backend não é a autoridade final

A API poderá criar, validar e projetar dados. Ela não poderá declarar que uma mudança canônica ocorreu apenas porque uma linha foi escrita no PostgreSQL.

O fluxo será:

```text
request
  -> validação de autorização
  -> criação de intenção
  -> payload canônico
  -> assinatura apropriada
  -> transação Solana
  -> confirmação finalizada
  -> projeção PostgreSQL
```

### 2.4 Privacidade por desenho

O perfil público deverá ser menor que o perfil autenticado. O DTO público não deve ser derivado automaticamente do DTO interno.

Dados como CPF/CNPJ, nomes legais, carteiras, coordenadas precisas, preços, rotas e documentos completos devem permanecer protegidos.

## 3. Banco PostgreSQL

### 3.1 Migrações incrementais

A sequência recomendada a partir da migração atual `0007` é:

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

Cada migração deve ser transacional quando o PostgreSQL permitir e deve possuir testes de upgrade a partir de um banco criado pelas migrações anteriores.

### 3.2 `assets`

Criar uma tabela comum para todos os objetos rastreáveis:

```sql
CREATE TABLE assets (
    asset_id                    BYTEA PRIMARY KEY CHECK (octet_length(asset_id) = 32),
    deployment_id               BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    asset_type                  SMALLINT NOT NULL,
    status                      TEXT NOT NULL,
    public_identifier           TEXT UNIQUE,
    current_custodian_party_id  BYTEA,
    current_location_commitment BYTEA CHECK (
        current_location_commitment IS NULL
        OR octet_length(current_location_commitment) = 32
    ),
    current_lot_id              BYTEA,
    event_sequence              BIGINT NOT NULL DEFAULT 0 CHECK (event_sequence >= 0),
    state_version               BIGINT NOT NULL DEFAULT 0 CHECK (state_version >= 0),
    last_event_hash             BYTEA CHECK (
        last_event_hash IS NULL OR octet_length(last_event_hash) = 32
    ),
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT now(),
    closed_at                   TIMESTAMPTZ
);
```

Regras:

- `asset_id` não pode ser reutilizado;
- `asset_type` deve ser enum lógico controlado pelo domínio;
- `status` deve ser validado pela máquina de estados;
- `state_version` deve avançar uma vez por transição canônica;
- `last_event_hash` deve ser atualizado somente após confirmação final;
- `closed_at` não pode ser removido depois de um estado terminal;
- não expor a tabela diretamente por endpoint.

### 3.3 `animals`

A tabela atual `animals` deve continuar existindo por compatibilidade. Ela deve receber colunas de relação com o novo domínio, sem duplicar a autoridade:

```sql
ALTER TABLE animals
    ADD COLUMN asset_id BYTEA UNIQUE REFERENCES assets(asset_id),
    ADD COLUMN identifier_namespace SMALLINT,
    ADD COLUMN official_identifier_commitment BYTEA,
    ADD COLUMN lifecycle_status TEXT;
```

Durante a migração:

```text
animals.current_custodian
  -> projeção compatível do AssetState v2

animals.current_rfid_hash
  -> projeção compatível do vínculo RFID v1/v2

animals.event_sequence
  -> sequência legada ou ponte para domain_events
```

Não atualizar o animal antigo e o `AssetState` novo de forma independente. O reconciliador deverá produzir a projeção compatível a partir da fonte canônica.

### 3.4 Participantes e papéis

Criar:

```sql
CREATE TABLE parties (
    party_id            BYTEA PRIMARY KEY CHECK (octet_length(party_id) = 32),
    party_type          TEXT NOT NULL,
    public_label        TEXT,
    legal_identity_ref  BYTEA,
    status              TEXT NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE party_roles (
    party_id             BYTEA NOT NULL REFERENCES parties(party_id),
    role_type            TEXT NOT NULL,
    scope_type           TEXT,
    scope_id             BYTEA,
    wallet_address       TEXT,
    credential_hash      BYTEA,
    valid_from           TIMESTAMPTZ NOT NULL,
    valid_until          TIMESTAMPTZ,
    status               TEXT NOT NULL,
    PRIMARY KEY (party_id, role_type, scope_type, scope_id, valid_from)
);
```

Papéis iniciais:

```text
PRODUCER
CUSTODIAN
SELLER
BUYER
TRANSPORTER
SLAUGHTERHOUSE
PROCESSING_FACILITY
DISTRIBUTOR
RETAILER
AUDITOR
OFFICIAL_SOURCE
PLATFORM_OPERATOR
```

`current_custodian` deve apontar para `party_id` pseudonimizado. Não deve guardar CPF/CNPJ ou nome em uma conta pública.

### 3.5 Documentos e proveniência

Criar:

```sql
CREATE TABLE official_documents (
    document_id          UUID PRIMARY KEY,
    document_type        TEXT NOT NULL,
    issuer_party_id      BYTEA,
    issuer_system        TEXT,
    issuer_record_id     TEXT,
    document_number      TEXT,
    access_key           TEXT,
    document_hash        BYTEA NOT NULL CHECK (octet_length(document_hash) = 32),
    issued_at            TIMESTAMPTZ,
    expires_at           TIMESTAMPTZ,
    verification_status  TEXT NOT NULL,
    source_response_hash  BYTEA,
    retrieved_at         TIMESTAMPTZ,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Status de proveniência:

```text
OBSERVED_BY_STATION
DECLARED_BY_OPERATOR
DOCUMENT_ATTACHED
DOCUMENT_VERIFIED
CONFIRMED_BY_OFFICIAL_SOURCE
DISPUTED
REVOKED
```

Documentos não devem ser substituídos silenciosamente. Uma nova versão deve apontar para o documento anterior e registrar o motivo.

### 3.6 Eventos de domínio

Criar:

```sql
CREATE TABLE domain_events (
    domain_event_id          UUID PRIMARY KEY,
    event_id                 BYTEA NOT NULL UNIQUE CHECK (octet_length(event_id) = 32),
    deployment_id            BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    subject_asset_id         BYTEA NOT NULL REFERENCES assets(asset_id),
    event_type               TEXT NOT NULL,
    schema_version           INTEGER NOT NULL,
    state_version            BIGINT NOT NULL,
    previous_event_hash      BYTEA,
    payload_hash             BYTEA NOT NULL CHECK (octet_length(payload_hash) = 32),
    manifest_hash            BYTEA,
    source_type              TEXT NOT NULL,
    source_id                BYTEA,
    actor_party_id           BYTEA,
    provenance_level         TEXT NOT NULL,
    document_id              UUID REFERENCES official_documents(document_id),
    status                   TEXT NOT NULL,
    tx_signature             TEXT UNIQUE,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
    finalized_at             TIMESTAMPTZ
);
```

Adicionar constraints para:

- impedir alteração de `event_id`, `payload_hash`, `subject_asset_id`, `event_type` e `state_version`;
- impedir exclusão;
- impedir `FINALIZED` sem `tx_signature`;
- impedir `SUBMITTED` sem assinatura;
- permitir somente transições de ciclo de vida válidas;
- exigir `manifest_hash` para eventos de transformação;
- exigir `document_id` para eventos que a política definir como document-dependent.

### 3.7 Lotes e linhagem

Criar:

```sql
CREATE TABLE lot_manifests (
    lot_id                 BYTEA PRIMARY KEY CHECK (octet_length(lot_id) = 32),
    version                BIGINT NOT NULL,
    purpose               TEXT NOT NULL,
    composition_root       BYTEA NOT NULL CHECK (octet_length(composition_root) = 32),
    quantity_declared      NUMERIC NOT NULL CHECK (quantity_declared >= 0),
    quantity_confirmed     NUMERIC CHECK (quantity_confirmed IS NULL OR quantity_confirmed >= 0),
    unit                   TEXT NOT NULL,
    origin_facility_id     BYTEA,
    destination_facility_id BYTEA,
    status                 TEXT NOT NULL,
    previous_lot_id        BYTEA,
    manifest_hash          BYTEA NOT NULL CHECK (octet_length(manifest_hash) = 32),
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (lot_id, version)
);

CREATE TABLE lineage_edges (
    lineage_edge_id        UUID PRIMARY KEY,
    parent_asset_id        BYTEA NOT NULL REFERENCES assets(asset_id),
    child_asset_id         BYTEA NOT NULL REFERENCES assets(asset_id),
    transformation_id      BYTEA,
    relationship_type      TEXT NOT NULL,
    quantity               NUMERIC NOT NULL CHECK (quantity >= 0),
    unit                   TEXT NOT NULL,
    weight_grams           BIGINT CHECK (weight_grams IS NULL OR weight_grams >= 0),
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (parent_asset_id, child_asset_id, transformation_id)
);
```

A aplicação deverá verificar ciclos antes de inserir uma aresta. Para operações críticas, também deverá validar a árvore/raiz no domínio, porque uma constraint SQL comum não resolve toda a transitividade.

### 3.8 Transformações

Criar:

```sql
CREATE TABLE transformation_manifests (
    transformation_id       BYTEA PRIMARY KEY CHECK (octet_length(transformation_id) = 32),
    facility_id             BYTEA NOT NULL,
    transformation_type     TEXT NOT NULL,
    input_root              BYTEA NOT NULL CHECK (octet_length(input_root) = 32),
    output_root             BYTEA NOT NULL CHECK (octet_length(output_root) = 32),
    manifest_hash           BYTEA NOT NULL CHECK (octet_length(manifest_hash) = 32),
    input_count             INTEGER NOT NULL CHECK (input_count >= 0),
    output_count            INTEGER NOT NULL CHECK (output_count >= 0),
    input_weight_grams      BIGINT NOT NULL CHECK (input_weight_grams >= 0),
    output_weight_grams     BIGINT NOT NULL CHECK (output_weight_grams >= 0),
    byproduct_weight_grams  BIGINT NOT NULL DEFAULT 0 CHECK (byproduct_weight_grams >= 0),
    loss_weight_grams       BIGINT NOT NULL DEFAULT 0 CHECK (loss_weight_grams >= 0),
    tolerance_basis_points  INTEGER NOT NULL CHECK (tolerance_basis_points >= 0),
    status                  TEXT NOT NULL,
    tx_signature            TEXT UNIQUE,
    started_at              TIMESTAMPTZ,
    finalized_at            TIMESTAMPTZ,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE transformation_inputs (
    transformation_id       BYTEA NOT NULL REFERENCES transformation_manifests(transformation_id),
    asset_id                BYTEA NOT NULL REFERENCES assets(asset_id),
    quantity                NUMERIC NOT NULL CHECK (quantity > 0),
    unit                    TEXT NOT NULL,
    weight_grams            BIGINT CHECK (weight_grams IS NULL OR weight_grams >= 0),
    reservation_status      TEXT NOT NULL,
    PRIMARY KEY (transformation_id, asset_id)
);

CREATE TABLE transformation_outputs (
    transformation_id       BYTEA NOT NULL REFERENCES transformation_manifests(transformation_id),
    asset_id                BYTEA NOT NULL REFERENCES assets(asset_id),
    asset_type              SMALLINT NOT NULL,
    quantity                NUMERIC NOT NULL CHECK (quantity > 0),
    unit                    TEXT NOT NULL,
    weight_grams            BIGINT CHECK (weight_grams IS NULL OR weight_grams >= 0),
    PRIMARY KEY (transformation_id, asset_id)
);
```

A transação de finalização deve usar `SELECT ... FOR UPDATE` nos inputs, verificando consumo e reserva em uma única transação.

### 3.9 Carcaças, produtos, embalagens e expedições

Criar tabelas especializadas:

```text
carcasses
product_lots
byproduct_lots
package_units
shipments
shipment_items
quality_holds
recalls
recall_assets
```

Campos mínimos:

```text
carcasses:
  asset_id
  source_animal_id
  facility_id
  carcass_number
  weight_grams
  inspection_status

product_lots:
  asset_id
  product_type
  production_date
  expiration_date
  quantity
  weight_grams
  quality_status

package_units:
  asset_id
  product_lot_id
  package_code
  packaging_level
  quantity
  weight_grams

shipments:
  shipment_asset_id
  origin_party_id
  destination_party_id
  vehicle_ref
  transport_document_id
  departure_at
  arrival_at
  status

recalls:
  recall_id
  reason
  source_event_hash
  affected_root
  status
  opened_by
  opened_at
```

## 4. API Rust

### 4.1 Organização dos módulos

Manter a composição atual em `services/api/src/lib.rs` e acrescentar:

```text
services/api/src/domain/assets.rs
services/api/src/domain/parties.rs
services/api/src/domain/documents.rs
services/api/src/domain/provenance.rs
services/api/src/domain/lots.rs
services/api/src/domain/observations.rs
services/api/src/domain/movements.rs
services/api/src/domain/slaughter.rs
services/api/src/domain/lineage.rs
services/api/src/domain/transformations.rs
services/api/src/domain/products.rs
services/api/src/domain/packages.rs
services/api/src/domain/shipments.rs
services/api/src/domain/quality.rs
services/api/src/domain/recalls.rs
services/api/src/domain/reconciliation.rs
```

Repositórios:

```text
services/api/src/repository/assets.rs
services/api/src/repository/parties.rs
services/api/src/repository/documents.rs
services/api/src/repository/domain_events.rs
services/api/src/repository/lots.rs
services/api/src/repository/lineage.rs
services/api/src/repository/transformations.rs
services/api/src/repository/products.rs
services/api/src/repository/packages.rs
services/api/src/repository/shipments.rs
services/api/src/repository/recalls.rs
services/api/src/repository/anchors.rs
```

Rotas:

```text
services/api/src/routes/assets.rs
services/api/src/routes/lots.rs
services/api/src/routes/observations.rs
services/api/src/routes/movements.rs
services/api/src/routes/slaughter.rs
services/api/src/routes/transformations.rs
services/api/src/routes/products.rs
services/api/src/routes/packages.rs
services/api/src/routes/shipments.rs
services/api/src/routes/recalls.rs
services/api/src/routes/public.rs
services/api/src/routes/admin_registry.rs
```

### 4.2 Limite HTTP atual

O router atual limita JSON a 1024 bytes porque o payload v1 é pequeno. Esse limite não serve para manifestos de transformação ou documentos.

Não aumentar globalmente o limite para vários megabytes. Usar limites por rota:

```text
AgentEvidence v1: 1024 bytes
DomainEvent compact: 4096 ou valor medido
Transformation manifest metadata: pequeno
Manifest upload: multipart ou object storage separado
Document upload: endpoint próprio com limite, streaming e autenticação
```

O router deve aplicar `DefaultBodyLimit` específico por rota ou extrator, mantendo o limite mínimo no fluxo Agent.

### 4.3 DTOs públicos e privados

Adicionar tipos em `services/api/src/model.rs`:

```rust
AssetType
AssetStatus
ProvenanceLevel
DomainEventType
WeightObservationRequest
LocationObservationRequest
MovementRequest
SlaughterReceiptRequest
TransformationBeginRequest
TransformationInputRequest
TransformationOutputRequest
TransformationFinalizeRequest
ProductResponse
PackageResponse
ShipmentRequest
RecallRequest
LineageResponse
PublicProductResponse
AuthorizedProductResponse
```

Cada request deve usar `deny_unknown_fields`.

Exemplo de transformação:

```rust
pub struct BeginTransformationRequest {
    pub transformation_id: Hex32,
    pub facility_id: Hex32,
    pub transformation_type: String,
    pub input_root: Hex32,
    pub output_root: Hex32,
    pub manifest_hash: Hex32,
    pub expected_input_count: u32,
    pub expected_output_count: u32,
    pub input_weight_grams: u64,
    pub tolerance_basis_points: u16,
    pub expires_at_unix: i64,
}
```

O endpoint de consulta pública deve retornar um DTO deliberadamente reduzido:

```rust
pub struct PublicProductResponse {
    pub public_product_id: String,
    pub product_type: String,
    pub production_date: String,
    pub expiration_date: Option<String>,
    pub public_region: Option<String>,
    pub status: String,
    pub recall_status: String,
    pub lineage_proof: PublicLineageProof,
}
```

### 4.4 Endpoints previstos

#### Animais e observações

```http
POST /api/animals
GET  /api/animals/{animalId}
GET  /api/animals/{animalId}/timeline
POST /api/animals/{animalId}/observations/weight
POST /api/animals/{animalId}/observations/location
POST /api/animals/{animalId}/status-transitions
```

#### Lotes e movimentos

```http
POST /api/lots
GET  /api/lots/{lotId}
POST /api/lots/{lotId}/split
POST /api/lots/merge
POST /api/movements
POST /api/movements/{movementId}/start
POST /api/movements/{movementId}/arrive
```

#### Frigorífico

```http
POST /api/slaughter/receipts
POST /api/slaughter/{receiptId}/confirm
POST /api/carcasses
POST /api/carcasses/{carcassId}/weigh
```

#### Transformação

```http
POST /api/transformations
POST /api/transformations/{id}/inputs
POST /api/transformations/{id}/outputs
POST /api/transformations/{id}/chunks
POST /api/transformations/{id}/finalize
POST /api/transformations/{id}/abort
GET  /api/transformations/{id}
```

#### Produtos e logística

```http
POST /api/product-lots
GET  /api/product-lots/{id}
POST /api/packages
GET  /api/packages/{id}
POST /api/shipments
POST /api/shipments/{id}/accept
GET  /api/shipments/{id}
```

#### Linhagem e verificação

```http
GET /api/assets/{id}/lineage/ancestors
GET /api/assets/{id}/lineage/descendants
GET /api/assets/{id}/evidence-package
GET /api/public/products/{publicId}
GET /api/public/packages/{publicId}
GET /api/recalls/{id}
```

#### Recall

```http
POST /api/recalls
POST /api/recalls/{id}/acknowledge
POST /api/recalls/{id}/close
GET  /api/recalls/{id}/affected-assets
```

### 4.5 Idempotência

Toda operação de escrita deve aceitar uma idempotency key ou `event_id` controlado pelo cliente.

A API deve devolver o mesmo resultado para uma repetição exata, mas rejeitar o mesmo identificador com payload divergente.

Isso é especialmente necessário para:

- finalização de transformação;
- criação de pacote;
- criação de expedição;
- aceite de recebimento;
- abertura de recall;
- upload de evidência;
- confirmação de transação Solana.

### 4.6 Reconciliador

Criar um worker server-side para:

1. encontrar intents não finalizadas;
2. consultar Solana;
3. verificar assinatura e envelope;
4. atualizar `domain_events`;
5. atualizar `assets` e projeções;
6. liberar ou manter reservas;
7. detectar divergência entre PostgreSQL e chain;
8. registrar incidentes sem alterar evidência;
9. reprocessar callbacks perdidos;
10. produzir métricas de atraso.

O navegador não deve ser responsável por finalizar uma transformação ou manter a consistência da cadeia.

## 5. Agent Rust

### 5.1 Manter o fluxo RFID

O `services/agent/src/worker.rs` atual executa uma captura por vez e persiste `OutboxRow` antes de enviar ACK ou HTTP. Esse comportamento deve ser preservado para RFID.

Não misturar no mesmo `OutboxRow`:

```text
StationEvent RFID v1
WeightObservation v2
TransformationManifest
Shipment
```

Criar uma outbox tipada:

```text
outbox_kind = RFID_EVENT | SENSOR_OBSERVATION | DOMAIN_COMMAND | MANIFEST_ANCHOR
```

### 5.2 Extensão do spool

Adicionar ao SQLite:

```text
outbox_v2
---------
row_id
kind
subject_id
event_id
payload_version
payload_bytes
payload_hash
source_id
state
attempts
last_error
created_at
updated_at
```

Estados:

```text
LOCAL
UPLOADING
SERVER_ACCEPTED
READY_TO_ANCHOR
SUBMITTED
FINALIZED
QUARANTINED
```

A evidência original deve ser imutável. Apenas o estado de entrega pode avançar.

### 5.3 Novos adaptadores

Criar traits separados:

```rust
trait RfidReader
trait ScaleReader
trait LocationSource
trait LabelPrinter
trait FacilityGateway
```

O Agent não deve confiar em JSON emitido por equipamento sem validar:

- versão;
- unidade;
- intervalo;
- timestamp;
- device id;
- assinatura ou vínculo seguro;
- calibração;
- duplicidade;
- contexto do comando;
- correlação com o asset correto.

### 5.4 API do Agent

Adicionar endpoints autenticados para:

```http
GET  /api/agent/v2/commands
POST /api/agent/v2/observations
POST /api/agent/v2/manifests
GET  /api/agent/v2/items/{id}/status
POST /api/agent/v2/acks
```

A autenticação do Agent deve continuar separada da autoridade da carteira e da Station. Token do Agent permite transporte; não permite inventar observação ou finalizar transferência.

### 5.5 Retry e quarentena

Classificar falhas:

```text
retryable:
  timeout
  5xx
  429
  RPC indisponível
  conexão serial interrompida

terminal:
  payload inválido
  assinatura inválida
  unidade inválida
  event_id divergente
  estado incompatível
  asset inexistente
  manifesto já finalizado
```

Falha terminal vai para quarentena e exige nova observação ou correção por evento. Nunca reaproveitar uma observação antiga como se fosse uma nova.

## 6. Firmware Station e hardware

### 6.1 Não aumentar o `StationEvent` v1

O firmware atual em `station.c` aceita comando, espera RFID, constrói, assina e aguarda ACK. Esse fluxo deve continuar separado.

Criar uma máquina de estados v2 ou um componente paralelo para sensores:

```text
IDLE
WAIT_SENSOR_CONTEXT
READ_RFID
READ_SCALE
READ_LOCATION
BUILD_OBSERVATION
SIGN_OBSERVATION
WAIT_ACK
```

Não permitir que o backend envie `new_rfid`, peso ou coordenada como se fossem observações locais.

### 6.2 Interfaces novas

Adicionar headers/components:

```text
include/lastro_station/scale.h
include/lastro_station/location.h
include/lastro_station/clock.h
include/lastro_station/observation.h
include/lastro_station/journal.h
include/lastro_station/label.h
```

Cada adapter deve expor:

```c
init
read
validate
serialize
sign
reset
```

### 6.3 Balança

A Station deve capturar:

- peso bruto;
- tara;
- peso líquido;
- unidade;
- resolução;
- identificador da balança;
- status de calibração;
- timestamp monotônico;
- operador ou sessão;
- resultado de estabilidade.

O firmware não deve aceitar uma pesagem instável como observação final. O valor deve usar inteiro com unidade fixa, preferencialmente gramas.

### 6.4 Localização

A localização pode vir de:

- GPS;
- estabelecimento configurado;
- leitor fixo;
- fonte oficial;
- operador autorizado.

Essas origens não devem ser confundidas. O payload deve dizer `location_source` e `precision_level`.

### 6.5 Relógio e freshness

O `StationEvent` v1 não possui timestamp ou nonce assinado. Para eventos v2, adicionar:

```text
observed_at
monotonic_counter
challenge_id
expires_at
```

A Station não deve depender apenas do relógio de parede. O Agent e a API devem rejeitar eventos fora da janela ou com contador repetido.

### 6.6 Provisionamento

Antes de operação real, ainda será necessário definir:

- modelo de leitor RFID;
- pinout;
- frames brutos;
- chave da Station;
- secure boot;
- eFuse;
- atualização de firmware;
- rotação/revogação;
- certificado de calibração;
- procedimento de troca de equipamento.

O código sozinho não prova que o hardware possui os eFuses ou a chave esperados.

## 7. Frontend Vue

### 7.1 Separar jornadas

A página atual `DemoPage.vue` é uma demonstração da transição física individual. Ela não deve receber toda a lógica pós-abate.

Criar páginas separadas:

```text
AnimalPage.vue
LotPage.vue
MovementPage.vue
SlaughterReceiptPage.vue
CarcassPage.vue
TransformationPage.vue
ProductLotPage.vue
PackagePage.vue
ShipmentPage.vue
RecallPage.vue
PublicProductPage.vue
```

### 7.2 Tipos TypeScript

Expandir `apps/web/src/api/types.ts` com:

```ts
export type AssetType =
  | 'ANIMAL'
  | 'LOT'
  | 'CARCASS'
  | 'CUT_BATCH'
  | 'PRODUCT_LOT'
  | 'PACKAGE'
  | 'BYPRODUCT_LOT'
  | 'SHIPMENT'

export type ProvenanceLevel =
  | 'OBSERVED_BY_STATION'
  | 'DECLARED_BY_OPERATOR'
  | 'DOCUMENT_ATTACHED'
  | 'DOCUMENT_VERIFIED'
  | 'CONFIRMED_BY_OFFICIAL_SOURCE'
  | 'DISPUTED'
  | 'REVOKED'

export interface LineageNode {
  assetId: Hex32
  assetType: AssetType
  status: string
  quantity?: number
  weightGrams?: number
  relation?: string
}

export interface TransformationManifest {
  transformationId: Hex32
  facilityId: Hex32
  transformationType: string
  inputRoot: Hex32
  outputRoot: Hex32
  manifestHash: Hex32
  inputWeightGrams: number
  outputWeightGrams: number
  byproductWeightGrams: number
  lossWeightGrams: number
  status: string
}
```

Não usar esses tipos transportados como prova. O cliente precisa validar formato, limites, enums, hashes e inteiros seguros antes de usar os dados em decisões.

### 7.3 Cliente API

O `apps/web/src/api/client.ts` deve ganhar funções para:

```text
getAsset
getLineageAncestors
getLineageDescendants
createWeightObservation
createLocationObservation
createMovement
createSlaughterReceipt
beginTransformation
appendTransformationChunk
finalizeTransformation
createPackage
createShipment
acceptShipment
getPublicProduct
getRecall
```

O parser deve rejeitar:

- campos desconhecidos quando o contrato exigir;
- números fora de `Number.MAX_SAFE_INTEGER`;
- hashes fora de 32 bytes;
- status desconhecido;
- payloads públicos com campos privados inesperados;
- raízes incompatíveis;
- respostas que misturem estados incompatíveis.

### 7.4 Jornada do frigorífico

A interface deve seguir etapas explícitas:

```text
1. Recebimento
2. Conferência documental
3. Conferência de identidade
4. Retenção ou liberação
5. Abate
6. Pesagem da carcaça
7. Preparação do manifesto
8. Validação de massa
9. Assinatura do frigorífico
10. Anchor Solana
11. Criação de lotes e caixas
12. Expedição
```

A interface deve mostrar divergências antes de solicitar assinatura.

### 7.5 Jornada de transformação

Mostrar ao operador:

```text
Entradas selecionadas
Peso total de entradas
Saídas cadastradas
Subprodutos
Perdas
Tolerância permitida
Diferença calculada
Status do manifesto
Hash do manifesto
Transação Solana
```

O botão de finalização deve ficar indisponível quando:

- houver entradas não reservadas;
- houver output duplicado;
- o balanço estiver fora da tolerância;
- o frigorífico estiver suspenso;
- faltar documento obrigatório;
- a transformação estiver expirada;
- a carteira conectada não for autorizada.

### 7.6 QR code e página pública

A página pública não deve depender do painel operacional. Criar uma rota própria:

```text
/public/product/:publicId
/public/package/:publicId
```

Ela deve exibir:

- produto;
- data de produção;
- validade;
- status de qualidade;
- região permitida;
- resumo da linhagem;
- prova criptográfica;
- recall, se houver.

Não exibir dados pessoais, preço, rota ou coordenada precisa.

## 8. Verificador independente

### 8.1 Expandir camadas

O tipo atual possui cinco camadas. Evoluir para:

```ts
export type VerificationLayerName =
  | 'RFID_EVIDENCE'
  | 'STATION_SIGNATURE'
  | 'IDENTITY_CONTINUITY'
  | 'CUSTODY'
  | 'DOMAIN_EVENT_HASHES'
  | 'LINEAGE_CONSISTENCY'
  | 'MASS_BALANCE'
  | 'FACILITY_CREDENTIAL'
  | 'DOCUMENT_HASHES'
  | 'RECALL_STATUS'
  | 'ON_CHAIN_STATE'
  | 'OFFICIAL_SOURCE_CONFIRMATION'
```

As camadas não devem ser reduzidas a um único booleano.

### 8.2 Verificação local

`verifyEvidencePackage.ts` deve ser dividido em módulos:

```text
verifyStationEvidence.ts
verifyDomainEvents.ts
verifyLineage.ts
verifyMassBalance.ts
verifyDocuments.ts
verifyRecall.ts
```

A verificação local deve:

- recomputar hashes;
- validar predecessor;
- validar sequência;
- validar payload canônico;
- validar raízes de input/output;
- verificar que não há ciclo;
- verificar que cada input foi consumido uma vez;
- verificar balanço dentro da tolerância;
- reconhecer eventos corrigidos sem apagar os originais;
- marcar fonte oficial como `NOT_CHECKED` quando não houver resposta verificável.

### 8.3 Verificação on-chain

O `verifyChain.ts` atual assume:

- uma única `ProtocolConfig`;
- um único `AnimalState`;
- uma conta `RfidBinding`;
- envelope fixo de duas instruções.

Criar uma implementação v2 separada:

```text
verifyCanonicalChainStateV2.ts
```

Ela deverá buscar e conferir:

```text
ProtocolConfigV2
AssetState
EventAnchor
TransformationAnchor
LineageAnchor
IntentState
FacilityRegistry
RecallState
```

Não misturar offsets v1 e v2 no mesmo bloco sem uma tag clara de versão.

### 8.4 Resultado compreensível

Exibir algo como:

```text
Integridade criptográfica: VALID
Linhagem animal-produto: VALID
Balanço de massa: VALID
Credencial do frigorífico: VALID
Documento anexado: VALID
Fonte oficial: NOT_CHECKED
Recall: CLEAR
Prova de propriedade jurídica: NOT_DETERMINED
Prova de identidade biológica: NOT_PROVEN
```

A UI nunca deve mostrar “propriedade confirmada” apenas porque uma carteira assinou a transação.

## 9. OpenAPI e schemas

O `schemas/openapi.yaml` atual descreve somente animais, capturas, eventos e EvidencePackage v1. Ele precisa ser ampliado ou versionado:

```text
schemas/openapi-v2.yaml
schemas/domain-event.schema.json
schemas/transformation-manifest.schema.json
schemas/lineage.schema.json
schemas/public-product.schema.json
schemas/evidence-package-v2.schema.json
```

A versão v2 deve definir:

- tipos e enums;
- limites de strings;
- limites de arrays;
- unidades;
- datas;
- campos de proveniência;
- campos públicos versus restritos;
- transições de status;
- erros de conflito;
- idempotency key;
- paginação de linhagem;
- limites de profundidade.

A consulta de linhagem deve ter limite de profundidade e quantidade de nós. Não permitir que um produto muito antigo cause uma resposta sem limite.

## 10. Testes

### 10.1 Banco

Criar testes para:

- imutabilidade de `domain_events`;
- proibição de delete;
- transição de status inválida;
- consumo duplicado de input;
- split e merge;
- ciclo de linhagem;
- balanço de massa fora da tolerância;
- expiração de reserva;
- recall bloqueando expedição;
- idempotência;
- documento revogado;
- isolamento entre instalações.

### 10.2 API

Criar testes de integração para:

- cadastro e migração de animal;
- peso com unidade inválida;
- localização privada não retornada no perfil público;
- recebimento no frigorífico;
- abate sem facility autorizada;
- transformação com input inexistente;
- transformação com output duplicado;
- finalização após expiração;
- duas finalizações concorrentes;
- envio de caixa já expedida;
- recall e consulta de afetados;
- divergência entre PostgreSQL e Solana;
- retry idempotente.

### 10.3 Agent

Adicionar testes para:

- persistência de observação antes do upload;
- queda de energia;
- retry 5xx;
- 4xx terminal;
- payload divergente;
- quarentena;
- replay após reinício;
- balança sem calibração;
- peso fora do limite;
- dispositivo desconhecido;
- observação duplicada.

### 10.4 Firmware

Adicionar testes C para:

- leitura RFID preservada;
- leitura de balança estável;
- timeout de sensor;
- payload com unidade inválida;
- contador monotônico repetido;
- challenge expirado;
- assinatura de envelope v2;
- ACK correto;
- queda durante `WAIT_ACK`;
- rejeição de comando que tenta injetar valor observado.

### 10.5 Frontend

Adicionar testes para:

- parser de DTOs v2;
- limites de `number` seguro;
- jornada de transformação;
- balanço fora da tolerância;
- transação expirada;
- recuperação após reload;
- consulta pública sem PII;
- recall visível;
- estado `NOT_CHECKED` não tratado como sucesso;
- árvore de linhagem com limite de profundidade.

### 10.6 Sistema completo

Criar cenários E2E:

```text
animal -> frigorífico -> carcaça -> corte -> caixa -> expedição
animal -> duas carcaças inválidas
carcaça -> duas transformações concorrentes
corte -> múltiplas caixas
múltiplas carcaças -> lote de produto
produto -> recall -> bloqueio de expedição
embalagem -> consulta reversa -> animal de origem
queda RPC durante finalização
queda API depois do anchor
alteração do manifesto após anchor
```

## 11. Observabilidade e operação

Adicionar métricas:

```text
capture_latency_ms
observation_ingest_latency_ms
transformation_open_duration_seconds
transformation_finalize_failures_total
mass_balance_rejections_total
lineage_query_depth
lineage_query_nodes
pending_intents
pending_onchain_anchors
reconciliation_lag_seconds
recall_affected_assets
agent_quarantine_rows
station_key_expirations
facility_credential_expirations
```

Adicionar logs estruturados com:

- correlation id;
- asset id pseudonimizado;
- event id;
- transformation id;
- facility id;
- status anterior e novo;
- motivo da rejeição;
- assinatura Solana sem dados pessoais;
- latency;
- versão do schema.

Nunca gravar tokens, chaves privadas, payloads pessoais completos ou URLs com credenciais nos logs.

## 12. Segurança e governança

Antes do piloto:

- criar RBAC/ABAC;
- separar tenants e instalações;
- registrar credenciais e validade;
- implementar rotação e revogação de Station;
- implementar rotação de token Agent;
- separar funções de operador, auditor e administrador;
- exigir aceite explícito em transferências;
- aplicar rate limit em consulta pública;
- impedir enumeração de RFID, animal e embalagem;
- criar política de retenção off-chain;
- definir controlador e operador de dados pessoais;
- executar avaliação de impacto quando aplicável;
- criar resposta a incidente;
- criar procedimento para correção e disputa;
- definir governança de fontes oficiais.

O administrador técnico não deve poder editar histórico canônico. Ele pode configurar infraestrutura e bloquear credenciais conforme a política, mas uma correção de negócio deve gerar um evento com autoridade própria.

## 13. Rollout em fases

### Fase A — compatibilidade

Objetivo: manter o produto atual funcionando.

- congelar fixtures v1;
- criar schemas v2 sem ativar endpoints;
- criar tabelas novas;
- criar registries;
- criar feature flag `DOMAIN_V2_ENABLED=false`;
- executar migrações em staging;
- validar que o fluxo RFID original não mudou.

### Fase B — domínio sem blockchain

Objetivo: testar modelo e operação com anchors simulados.

- cadastrar assets;
- criar eventos de domínio;
- criar lotes;
- criar carcaças e produtos;
- executar transformações;
- validar linhagem e massa;
- executar recall;
- validar perfis público/restrito.

Nesta fase, usar um `AnchorStore` de teste claramente marcado como não canônico.

### Fase C — Solana v2 em ambiente de teste

- implantar novo program ID;
- registrar Station e facilities;
- migrar animais de teste;
- executar intents;
- testar chunks;
- finalizar transformações;
- validar transações e verificador independente;
- medir custo e tamanho de transações.

### Fase D — piloto controlado

- uma fazenda;
- um transportador;
- um frigorífico;
- um distribuidor;
- poucos animais;
- produtos com lote rastreável;
- documentos de teste e procedimentos manuais de contingência.

### Fase E — expansão

Somente depois de evidenciar:

- reconciliação confiável;
- recall funcional;
- balanço de massa operacional;
- chave e credencial rotacionáveis;
- privacidade validada;
- auditoria completa;
- recuperação de falhas;
- custos aceitáveis;
- integração documental autorizada.

## 14. Ordem de implementação recomendada

A ordem técnica deve ser:

1. Criar enums, schemas e fixtures v2.
2. Criar `assets` e projeção compatível de animais.
3. Criar `domain_events` e guards append-only.
4. Criar parties, roles, documents e provenance.
5. Criar lotes e `lineage_edges`.
6. Criar carcaças e transformação off-chain.
7. Criar produtos, embalagens e shipments off-chain.
8. Criar verificador local de linhagem e massa.
9. Criar recall off-chain.
10. Implementar AssetState v2 e anchors Solana.
11. Implementar intents e nonces on-chain.
12. Implementar transformação em chunks on-chain.
13. Atualizar Agent e spool para observações v2.
14. Criar adapters reais de balança e localização.
15. Criar telas de frigorífico e consulta pública.
16. Atualizar verificador on-chain v2.
17. Executar piloto controlado.
18. Só então ativar integrações oficiais e escala.

## 15. O que não deve ser feito agora

Não fazer as seguintes alterações como primeiro passo:

- adicionar peso e localização diretamente ao `StationEvent` v1;
- transformar `current_custodian` em proprietário;
- colocar CPF/CNPJ, endereço ou coordenada em conta Solana pública;
- criar uma conta Solana por pedaço de carne sem medir custo e necessidade;
- confiar no navegador para finalizar operações;
- permitir `PATCH` livre de status;
- aceitar manifesto sem balanço de massa;
- guardar somente o último produto e perder a árvore;
- apagar eventos corrigidos;
- usar a API como autoridade final;
- tratar assinatura de Station como prova de identidade biológica;
- tratar hash de documento como confirmação oficial;
- integrar fonte oficial antes de definir escopo, credenciais e responsabilidades.

## 16. Critérios de pronto

A implementação poderá ser considerada pronta para piloto somente quando:

- o fluxo v1 continuar passando todos os testes;
- um animal puder ser migrado sem perder seu histórico;
- um frigorífico autorizado puder confirmar abate;
- uma carcaça puder gerar múltiplas saídas;
- uma saída puder ser combinada com outras entradas sem perder a linhagem;
- o balanço de massa for validado;
- a mesma entrada não puder ser consumida duas vezes;
- uma embalagem puder ser rastreada até o animal de origem;
- um animal puder ser rastreado até suas expedições;
- recall bloquear produtos afetados;
- consulta pública não vazar dados protegidos;
- intents e nonces bloquearem replay e concorrência;
- o reconciliador recuperar operações após queda do navegador;
- o verificador independente validar anchors e linhagem;
- credenciais de Station e facility puderem ser revogadas;
- eventos e manifestos permanecerem append-only;
- os erros forem observáveis e recuperáveis;
- o piloto tiver procedimento de contingência manual.

## 17. Parecer final da Etapa 4

O projeto atual não precisa ser descartado. Ele precisa ser dividido em duas camadas:

```text
Lastro Core v1
  identidade RFID
  Station
  Agent
  custódia individual
  verificador original

Lastro Domain v2
  animais e lotes
  documentos
  peso e localização
  frigorífico
  carcaça e produto
  linhagem
  expedição
  recall
  anchors e verificador v2
```

A alteração mais importante fora do smart contract será criar um **motor de domínio append-only** no backend, com projeções, linhagem e reconciliação. Sem ele, o sistema conseguiria gravar transações, mas não conseguiria explicar corretamente como um animal se transformou em centenas de produtos.

A segunda alteração mais importante será separar o Agent/Station de RFID dos dispositivos e sistemas de produção. RFID, balança, GPS, etiqueta, linha de desossa e ERP do frigorífico têm contratos, falhas e autoridades diferentes.

A terceira será criar uma experiência pública que mostra provas e linhagem sem expor dados pessoais, comerciais ou operacionais.

## Referências internas

[1]: ../docs/ARCHITECTURE.md "Arquitetura atual do Lastro"
[2]: ../docs/HARDENING_STATUS.md "Status de hardening do Lastro"
[3]: ../docs/PLANO_ETAPA_3_SMART_CONTRACT.md "Plano da Etapa 3 — evolução do smart contract Solana"
[4]: ../docs/ESTUDO_POS_ABATE_LINHAGEM.md "Adendo sobre pós-abate, desossa e linhagem de produtos"

**Autor:** Manus AI

# Estudo de evolução do Lastro para rastreabilidade bovina

**Projeto analisado:** Lastro  
**Objetivo deste estudo:** adaptar a arquitetura atual para acompanhar bovinos individualmente e em lotes, registrar peso, localização, status e cadeia de custódia, permitir consulta segura pela internet e representar compras e movimentações com evidência verificável.  
**Conclusão curta:** o Lastro pode ser a camada de **rastreabilidade, integridade e custódia operacional** de uma cadeia bovina. Ele não deve ser apresentado automaticamente como SISBOV, PNIB, GTA, registro público ou matrícula legal de propriedade.

> **Recomendação principal.** Não transformar o smart contract em um formulário com campos mutáveis como `peso`, `local`, `status` e `proprietário`. O modelo correto é um **livro de eventos append-only**: cada observação ou mudança produz um evento assinado, o programa Solana valida a transição e o estado atual é apenas uma projeção derivada do histórico.

## 1. A visão de produto

A ideia proposta é um sistema em que cada bovino tenha uma identidade digital e uma história verificável. Ao longo da cadeia, o sistema deve conseguir responder:

- qual animal ou lote está sendo tratado;
- qual identificador físico ou oficial está associado ao animal;
- qual era o peso em determinada data e quem realizou a medição;
- em qual estabelecimento, região ou etapa da cadeia o animal estava;
- qual é o status operacional ou sanitário conhecido;
- quem recebeu, vendeu, transportou ou custodiu o animal;
- quais documentos sustentam cada movimentação;
- quais eventos foram observados diretamente por uma Station;
- quais dados foram apenas declarados por um operador;
- quais informações foram confirmadas por uma fonte oficial;
- se a história foi alterada, corrigida, contestada ou revogada.

A proposta é tecnicamente valiosa porque combina identificação física, registros de negócio, documentos e uma âncora criptográfica. Porém, é necessário separar quatro conceitos que não são equivalentes:

| Conceito | Significado para o produto |
|---|---|
| **Identidade** | Qual registro representa aquele bovino e qual identificador está associado a ele. |
| **Rastreabilidade** | Capacidade de acompanhar animal ou lote ao longo das etapas da cadeia. |
| **Custódia** | Quem está responsável pelo animal ou lote em determinado momento. |
| **Propriedade** | Relação jurídica decorrente de negócio, tradição, contrato e demais documentos aplicáveis. |

O Lastro consegue controlar muito bem os três primeiros quando recebe evidências adequadas. O quarto depende de documentação, representação das partes e legislação aplicável. Uma transação Solana não converte automaticamente uma pessoa em proprietária do bovino.

## 2. O que o Lastro atual já possui

A base atual é uma boa fundação para essa evolução.

O `AnimalState` já representa um estado canônico individual por `AnimalID`. Ele mantém `current_rfid_hash`, `current_custodian`, `identity_revision`, `event_sequence` e `last_event_hash`. Essa estrutura já suporta duas ideias essenciais: preservar a identidade lógica quando o RFID muda e encadear eventos sem sobrescrever o histórico.

O `StationEvent` possui tamanho fixo de 276 bytes. Ele contém ação, deployment, animal, Station, sequência, revisão, predecessor, RFID antigo, RFID novo, custodiante de origem e custodiante de destino. O evento é assinado pela Station e verificado pelo Agent, pela API, pelo navegador e pelo programa Solana.

A arquitetura também já possui:

- Agent com outbox SQLite;
- API com PostgreSQL;
- EvidencePackage;
- transferência e reidentificação;
- integração com carteira Solana;
- verificação independente;
- simulador de hardware;
- separação entre evidência, projeção e estado canônico.

A arquitetura declarada do projeto já define que a Station prova apenas que recebeu uma leitura e assinou um evento. Ela não prova a identidade biológica do animal, a propriedade civil, a validade de uma GTA ou a regularidade sanitária. Essa fronteira deve ser mantida na nova versão.

## 3. O que precisa mudar conceitualmente

### 3.1 O lote não deve substituir o animal

Um lote é uma forma de agrupar animais ou quantidades para transporte, venda, engorda, quarentena ou abate. Ele não é necessariamente a identidade de cada animal.

Se o produto quer registrar o peso individual, a localização individual, o status individual e a transferência individual, o **animal precisa continuar sendo a entidade canônica**. O lote deve funcionar como um manifesto versionado que aponta para animais ou para uma quantidade agregada.

O desenho recomendado é:

```text
AnimalState individual
    ├── pertence ao LotManifest v1
    ├── pode sair do lote por venda, morte, abate ou separação
    └── pode entrar em outro LotManifest v2

LotManifest v1
    ├── tem raiz criptográfica dos animais/quantidades
    ├── referencia GTA, documentos e finalidade
    └── pode gerar LotManifest v2 por split ou merge
```

Isso evita um problema comum: manter simultaneamente um estado canônico do animal e um estado canônico do lote, sem uma regra clara para resolver divergências entre os dois.

### 3.2 O lote deve ter linhagem

Operações de lote não devem editar uma lista de animais em lugar. Elas devem criar novos manifestos:

- `CREATE_LOT`: cria o lote inicial;
- `SPLIT_LOT`: separa parte do lote em um novo manifesto;
- `MERGE_LOT`: combina manifestos compatíveis;
- `MOVE_LOT`: move a custódia de um lote;
- `CLOSE_LOT`: encerra o lote por recebimento, abate, venda ou outro motivo;
- `CORRECT_LOT`: cria correção vinculada ao manifesto anterior.

Cada manifesto precisa conter:

- `lot_id` aleatório;
- versão do manifesto;
- deployment;
- raiz Merkle ou hash do conjunto de animais;
- quantidade declarada;
- quantidade confirmada;
- espécie;
- sexo e faixas etárias quando aplicável;
- finalidade;
- origem e destino;
- GTA ou referência documental;
- custodiante remetente;
- custodiante recebedor;
- evento anterior;
- status;
- data de criação e de encerramento;
- assinatura do emissor;
- nível de confirmação.

A lista completa de `AnimalID`s deve ficar no PostgreSQL ou em armazenamento de evidência. A Solana deve guardar a raiz ou o compromisso criptográfico do manifesto, não uma lista arbitrariamente grande em uma única conta.

### 3.3 Peso deve ser uma observação, não uma variável livre

O peso de um bovino possui contexto. Uma medição precisa responder:

- qual animal ou lote foi pesado;
- qual balança foi usada;
- qual era a unidade;
- qual foi o valor bruto;
- qual foi a tara;
- qual foi o peso líquido;
- quando a pesagem ocorreu;
- onde ocorreu;
- quem operou o equipamento;
- qual era a calibração da balança;
- qual método foi usado;
- se o peso foi individual ou estimado por lote;
- se a medição foi confirmada ou apenas declarada.

Por isso, o evento deve ser semelhante a `WEIGHT_OBSERVED`, com valores inteiros em gramas ou quilogramas com precisão fixa. O sistema não deve aceitar um valor decimal livre sem unidade e sem origem.

A assinatura da balança ou da Station prova que determinado dispositivo assinou aquele valor. Ela não prova que a balança estava calibrada, que o animal correto estava sobre a plataforma ou que o operador não adulterou a entrada. Para uma garantia maior, é necessário um identificador de equipamento, certificado de calibração, atestado do operador e política de exceção.

### 3.4 Localização não deve significar GPS público em tempo real

A localização é necessária para rastreabilidade, mas a exposição pública da coordenada exata de uma fazenda pode criar risco físico, comercial e de privacidade. Ela também pode revelar estoque, rotas e rotina operacional.

A recomendação é separar três níveis:

1. **Localização operacional privada:** coordenada, endereço ou código interno completo, disponível somente a usuários autorizados.
2. **Localização oficial referenciada:** código de estabelecimento, exploração, município e UF conforme a fonte competente.
3. **Localização pública reduzida:** região, município, UF ou célula geográfica generalizada, eventualmente com atraso temporal.

No smart contract público, prefira armazenar:

- `location_commitment`;
- identificador opaco do estabelecimento;
- região generalizada;
- `location_source`;
- nível de precisão;
- momento da observação;
- versão da política de exposição.

Não grave nome, CPF/CNPJ, endereço, latitude, longitude ou polígono rural diretamente no estado público. Se o deployment for em uma rede Solana pública, isso deve ser considerado informação potencialmente permanente.

Além disso, GPS é um oráculo. Um receptor pode ser enganado, estar deslocado ou ter sua leitura falsificada. Uma localização assinada pelo dispositivo prova o que o dispositivo reportou. Não prova, por si só, presença física verdadeira em uma propriedade.

### 3.5 Status deve ser uma máquina de estados

Não deve existir um campo `status` livre que qualquer operador altere para qualquer texto. Os status precisam ser enumerados e derivados de eventos autorizados.

Um conjunto inicial possível é:

```text
REGISTERED
ACTIVE
IN_TRANSIT
RECEIVED
IN_QUARANTINE
CLEARED
SOLD_PENDING_ACCEPTANCE
CUSTODY_TRANSFERRED
DISPUTED
MISSING
DEAD
SLAUGHTERED
RETIRED
```

A lista final depende do domínio e da autoridade sanitária. O importante é que cada transição tenha:

- status anterior permitido;
- evento que pode causar a mudança;
- papel autorizado;
- documento ou evidência exigida;
- prazo de validade;
- consequência operacional;
- possibilidade de correção ou disputa.

Exemplos:

| Evento | Status anterior | Novo status | Autoridade mínima recomendada |
|---|---|---|---|
| `MOVE_STARTED` | `ACTIVE` ou `CLEARED` | `IN_TRANSIT` | Custodiante + referência de GTA/documento |
| `ARRIVAL_CONFIRMED` | `IN_TRANSIT` | `RECEIVED` | Destino ou fonte oficial |
| `QUARANTINE_ENTERED` | `IN_TRANSIT` ou `RECEIVED` | `IN_QUARANTINE` | Fonte/ator sanitário autorizado |
| `QUARANTINE_RELEASED` | `IN_QUARANTINE` | `CLEARED` | Fonte sanitária autorizada |
| `SALE_PROPOSED` | `ACTIVE` ou `RECEIVED` | `SOLD_PENDING_ACCEPTANCE` | Vendedor/custodiante atual |
| `SALE_ACCEPTED` | `SOLD_PENDING_ACCEPTANCE` | `CUSTODY_TRANSFERRED` | Comprador + vendedor + documentos |
| `SLAUGHTER_CONFIRMED` | `RECEIVED` ou `CLEARED` | `SLAUGHTERED` | Estabelecimento habilitado/fonte aplicável |
| `DEATH_REPORTED` | `ACTIVE`, `RECEIVED` ou `IN_QUARANTINE` | `DEAD` | Papel autorizado e evidência adequada |
| `DISPUTE_OPENED` | qualquer estado não terminal | `DISPUTED` | Parte autorizada ou mecanismo de disputa |

O smart contract deve rejeitar uma mudança incompatível mesmo que o chamador tenha uma chave válida.

## 4. A diferença entre rastreabilidade e propriedade

### 4.1 O que a blockchain pode provar

Com uma política de chaves adequada, o sistema pode provar que:

- determinados bytes foram assinados por uma chave específica;
- um evento foi encadeado ao evento anterior;
- a sequência não foi alterada sem quebrar a cadeia;
- um custodiante autorizado assinou uma intenção;
- um comprador aceitou uma transferência, se o modelo exigir aceite;
- um documento ou pacote existia em determinado momento, por meio de hash;
- a transação foi submetida e finalizada na Solana;
- o estado atual é consistente com os eventos reconhecidos pelo programa.

### 4.2 O que ela não prova sozinha

O sistema não prova sozinho que:

- o RFID estava fisicamente no animal correto;
- o RFID não foi clonado;
- a balança estava calibrada;
- a coordenada era verdadeira;
- a pessoa controladora da carteira tinha poder de representação;
- uma compra e venda era válida;
- a tradição ocorreu;
- o comprador pagou;
- a GTA era válida ou não estava cancelada;
- um órgão público reconhece a operação;
- a propriedade civil foi transmitida;
- o animal estava sanitariamente liberado;
- uma pessoa tinha posse legítima do estabelecimento.

A lei brasileira trata a rastreabilidade de bovinos e búfalos como o registro e acompanhamento da cadeia, usando instrumentos como identificação, GTA, nota fiscal, registros oficiais de inspeção e registros privados. A própria lei permite sistemas voluntários com instrumentos adicionais, mas isso não equivale a autorizar uma plataforma privada a substituir os instrumentos oficiais. [1]

O SISBOV é apresentado pelo MAPA como o sistema oficial de identificação individual de bovinos e búfalos. A adesão é, em regra, voluntária, salvo obrigação normativa, controle ou programa sanitário oficial. [2]

O PNIB está em implantação progressiva. A Portaria SDA/MAPA nº 1.331/2025 prevê uma Base Central de Dados, interoperabilidade com sistemas estaduais e um cronograma que chega à identificação individual de todos os bovinos e búfalos antes da primeira movimentação a partir de 1º de janeiro de 2033, observadas as regulamentações, fases transitórias e possíveis requisitos estaduais complementares. [3]

O Decreto nº 7.623/2011 atribui ao MAPA a responsabilidade pelo sistema público informatizado e pela numeração de identificação individual. Também exige avaliação e homologação de protocolos voluntários quando utilizados para certificação oficial. [4]

Consequentemente, o nome correto para o produto, até existir reconhecimento específico, é algo como:

> **Registro digital de rastreabilidade, evidências e custódia operacional de bovinos.**

Nomes como **matrícula legal**, **registro oficial de propriedade**, **SISBOV na blockchain** ou **certificação PNIB** não devem ser usados sem base normativa, homologação e integração formal com as autoridades competentes.

### 4.3 Compra, custódia e propriedade devem ser entidades diferentes

O modelo atual possui `current_custodian`. Esse campo é útil, mas deve ser definido como **custodiante operacional**, e não como proprietário civil.

O domínio deve separar:

```text
Party
  ├── pessoa física ou jurídica
  ├── organização
  ├── carteira(s)
  └── credenciais e papéis

RoleAssignment
  ├── proprietário alegado ou documentado
  ├── produtor responsável
  ├── possuidor/detentor
  ├── custodiante operacional
  ├── vendedor
  ├── comprador
  ├── transportador
  └── estabelecimento de destino

TransferIntent
  ├── proposta de negócio ou custódia
  ├── documentos
  ├── aceite do remetente
  ├── aceite do recebedor
  ├── entrega/tradição declarada
  ├── confirmação documental
  └── finalização on-chain
```

O Código Civil diferencia compra e venda, posse, detenção, propriedade e tradição de bens móveis. [5] A GTA, por sua vez, representa o trânsito animal e seus requisitos sanitários; não deve ser tratada como prova única de domínio civil.

O fluxo de venda recomendado é:

1. o custodiante atual cria uma intenção de transferência;
2. a API consulta estado canônico, documentos e regras da origem/destino;
3. o vendedor assina o contexto da intenção;
4. o comprador recebe o resumo e assina um aceite explícito;
5. a documentação de compra, entrega e trânsito é anexada off-chain;
6. a API valida referências documentais e status conhecidos;
7. a transação Solana grava a transferência de custódia;
8. a projeção muda `current_custodian` somente depois da finalização;
9. eventual propriedade jurídica é apresentada como **alegada ou documentada**, não como verdade automática do smart contract.

Se a operação não for uma venda, o evento deve dizer isso. Transporte, depósito, comodato, leilão, engorda, quarentena e guarda temporária não devem ser rotulados como transferência de propriedade.

## 5. Autoridade dos documentos e integrações

A GTA/e-GTA deve ser tratada como documento oficial de trânsito. O manual do MAPA trabalha com uma GTA por espécie, origem, destino, finalidade e veículo. Também exige informações de produtor, estabelecimento, exploração, município, UF, quantidade por sexo e faixa etária, além de requisitos sanitários. [6]

O Lastro deve armazenar uma entidade `OfficialDocumentReference` com:

- tipo do documento;
- fonte emissora;
- UF;
- órgão ou unidade;
- série e número;
- chave de acesso quando existir;
- origem;
- destino;
- finalidade;
- veículo e transportador quando aplicável;
- data de emissão;
- validade;
- status de verificação;
- hash do documento recebido;
- versão do parser;
- momento da consulta;
- resposta da fonte;
- identidade do operador ou integração.

A blockchain deve registrar um compromisso do documento, não necessariamente o documento completo.

As integrações devem ser implementadas como adapters separados:

```text
OfficialSourceAdapter
  ├── SISBOV/BND, quando autorizado
  ├── Base Central PNIB, quando disponível e autorizado
  ├── OESA/SVO estadual
  ├── BDU/PGA/GTA
  ├── SEFAZ/NF-e ou documento fiscal aplicável
  └── fonte de inspeção SIF/SIE/SIM
```

Cada adapter deve retornar não apenas dados, mas também proveniência:

```json
{
  "source": "OESA-SP",
  "source_record_id": "opaque-id",
  "retrieved_at": "2026-09-23T00:00:00Z",
  "verification_status": "CONFIRMED_BY_SOURCE",
  "parser_version": "gta-sp-v3",
  "payload_hash": "..."
}
```

A aplicação deve distinguir os níveis abaixo:

| Nível | Significado |
|---|---|
| `OBSERVED_BY_STATION` | Uma Station assinou uma observação física. |
| `DECLARED_BY_OPERATOR` | Uma pessoa ou organização informou o dado. |
| `DOCUMENT_ATTACHED` | Existe documento anexado, ainda não confirmado pela fonte. |
| `DOCUMENT_VERIFIED` | O documento passou por validação de formato, assinatura ou chave de acesso. |
| `CONFIRMED_BY_OFFICIAL_SOURCE` | A fonte oficial retornou confirmação. |
| `DISPUTED` | Existe conflito ou contestação. |
| `REVOKED` | A credencial, documento ou evento foi invalidado. |

A validade criptográfica não deve ser exibida como se fosse confirmação oficial.

## 6. Proposta de arquitetura de dados

### 6.1 Entidades principais

| Entidade | Fonte canônica | Função |
|---|---|---|
| `Animal` | Solana + projeção | Identidade lógica de um bovino. |
| `AnimalIdentifier` | Protocolo + fonte oficial | Relação entre RFID, identificador oficial e AnimalID. |
| `AnimalEvent` | Evento assinado + transação | Histórico imutável de mudança. |
| `WeightObservation` | Station/balança + EvidencePackage | Medição individual ou estimativa de lote. |
| `LocationObservation` | Dispositivo, estabelecimento ou fonte | Localização com nível de precisão e proveniência. |
| `AnimalStatus` | Máquina de estados | Estado operacional ou sanitário conhecido. |
| `LotManifest` | PostgreSQL + commitment Solana | Agrupamento versionado de animais ou quantidades. |
| `Movement` | Documentos + eventos | Deslocamento entre origens e destinos. |
| `OfficialDocumentReference` | Fonte oficial/documento | GTA, e-GTA, NF-e, atestado, inspeção e similares. |
| `TransferIntent` | PostgreSQL + intenção on-chain | Proposta e aceite de venda ou custódia. |
| `Party` | Diretório protegido | Pessoa, empresa, organização ou órgão. |
| `RoleAssignment` | Diretório + documentos | Relação entre parte e papel. |
| `EvidencePackage` | PostgreSQL/object storage | Pacote verificável em camadas de exposição. |
| `Dispute` | PostgreSQL + evento de bloqueio | Contestação, investigação e resolução. |

### 6.2 Estado individual recomendado

Uma futura versão do `AnimalState` poderia manter apenas o estado operacional mínimo e compromissos:

```rust
pub struct BovineState {
    pub animal_id: [u8; 32],
    pub deployment_id: [u8; 32],
    pub identifier_commitment: [u8; 32],
    pub identifier_namespace: u16,
    pub lifecycle_status: u8,
    pub custodian_party_id: [u8; 32],
    pub official_establishment_id: [u8; 32],
    pub current_lot_id: [u8; 32],
    pub last_weight_grams: u64,
    pub last_weight_event_hash: [u8; 32],
    pub location_commitment: [u8; 32],
    pub last_event_hash: [u8; 32],
    pub event_sequence: u64,
    pub identity_revision: u32,
    pub schema_version: u16,
    pub bump: u8,
}
```

Esse exemplo é conceitual. Antes de adotá-lo, é preciso decidir quais campos devem ser públicos e quais devem permanecer apenas em commitment. O `custodian_party_id` não deve ser um CPF, nome ou identificador diretamente reidentificável.

A principal mudança em relação ao `AnimalState` atual é semântica:

- `current_custodian` passa a representar uma parte operacional pseudonimizada;
- `lifecycle_status` substitui status livre;
- `last_weight_grams` é um resumo do último evento, não o histórico;
- `last_weight_event_hash` vincula o resumo ao evento completo;
- `location_commitment` não expõe coordenada precisa;
- `official_establishment_id` deve ser um compromisso ou ID público aprovado, conforme a política de exposição;
- `current_lot_id` é uma projeção, não substitui o manifesto e seus eventos de linhagem.

Se a exposição pública desses valores for inadequada, o estado on-chain deve conter somente hashes e códigos opacos. O PostgreSQL/EvidencePackage mantém o valor detalhado sob controle de acesso.

### 6.3 Eventos de negócio

Não se deve estender silenciosamente o `StationEvent` v1 de 276 bytes. O projeto deve manter compatibilidade e criar uma versão ou uma camada de eventos bovinos.

A separação recomendada é:

```text
PhysicalObservationEvent
  -> assinado pela Station/balança

BusinessTransitionEvent
  -> assinado pela parte autorizada

OfficialAttestationEvent
  -> ancorado por fonte oficial ou credencial reconhecida

EvidenceAnchorEvent
  -> registra hash, versão e referência do pacote
```

Eventos iniciais:

| Tipo | Dados mínimos | Assinantes ou fonte |
|---|---|---|
| `ANIMAL_REGISTERED` | animal, identificador, origem, schema, documento | produtor/operador + fonte aplicável |
| `IDENTIFIER_ATTACHED` | antigo/novo, namespace, dispositivo, data | Station + operador autorizado |
| `IDENTIFIER_REPLACED` | antigo, novo, motivo, revisão | Station + custodiante + documento |
| `WEIGHT_OBSERVED` | animal/lote, gramas, balança, método, hora | balança/Station + operador |
| `LOCATION_OBSERVED` | local commitment, precisão, fonte, hora | dispositivo/estabelecimento/fonte |
| `STATUS_CHANGED` | anterior, novo, motivo, papel, documento | papel autorizado |
| `MOVEMENT_CREATED` | origem, destino, finalidade, lote, GTA | remetente + documento |
| `MOVEMENT_STARTED` | carga, veículo, lacre, hora | transportador/remetente |
| `ARRIVAL_CONFIRMED` | destino, quantidade recebida, hora | destinatário/fonte |
| `QUARANTINE_ENTERED` | estabelecimento, documento, início | autoridade aplicável |
| `QUARANTINE_RELEASED` | liberação, documento, data | autoridade sanitária |
| `TRANSFER_PROPOSED` | vendedor, comprador, animais/lote, preço commitment | vendedor |
| `TRANSFER_ACCEPTED` | intent, comprador, aceite, hora | comprador |
| `CUSTODY_TRANSFERRED` | origem/destino, documentos, predecessor | programa Solana |
| `SLAUGHTER_RECEIVED` | abatedouro, GTA, lote, quantidade | estabelecimento/fonte |
| `SLAUGHTER_CONFIRMED` | abate, lote de produto, data | fonte/estabelecimento habilitado |
| `ANIMAL_DEAD` | causa/categoria, data, documento | papel autorizado |
| `DISPUTE_OPENED` | evento contestado, razão, parte | parte autorizada |
| `EVENT_CORRECTED` | evento anterior, correção, razão | autoridade competente |

### 6.4 Peso, local e status no histórico

Para cada atualização, o banco e o EvidencePackage devem guardar:

```json
{
  "event_id": "random-id",
  "animal_id": "opaque-animal-id",
  "event_type": "WEIGHT_OBSERVED",
  "value": {
    "weight_grams": 487300,
    "measurement_kind": "INDIVIDUAL",
    "uncertainty_grams": 500
  },
  "instrument": {
    "device_id": "opaque-scale-id",
    "calibration_reference": "document-hash",
    "firmware_version": "..."
  },
  "observed_at": "timestamp",
  "received_at": "timestamp",
  "source": "STATION",
  "provenance": "OBSERVED_BY_STATION",
  "previous_event_hash": "...",
  "payload_hash": "...",
  "signature": "..."
}
```

A cadeia deve conservar o valor original e o valor corrigido como eventos separados. Nunca substituir silenciosamente `487300` por `490000`.

## 7. Smart contract Solana proposto

### 7.1 Contas ou PDAs

Uma futura versão pode usar contas semelhantes a:

```text
ProtocolConfig
  -> deployment, versão, autoridade, registry

BovineState
  -> estado individual e commitments

IdentifierBinding
  -> namespace + identificador -> animal

LotState
  -> manifesto atual, quantidade, raiz, estado e custódia

EventAnchor
  -> hash, sequência, tipo, predecessor e referência mínima

PartyRegistry
  -> parte/organização pseudonimizada e status

StationRegistry
  -> Station, chave, firmware, validade e revogação

TransferIntent
  -> intenção, nonce, expiracão, remetente, recebedor e estado

DocumentAnchor
  -> hash, tipo, emissor, versão e status de confirmação
```

A implementação pode evitar uma conta `EventAnchor` para cada evento se o histórico for recuperado por transações e pelo EvidencePackage. Porém, se a verificação independente precisar funcionar sem um indexador próprio, será necessário decidir como tornar o histórico consultável, com custo e limite bem conhecidos.

### 7.2 Instruções

Uma superfície inicial de instruções poderia ser:

```text
initialize_deployment
register_station
register_party
register_animal
attach_identifier
replace_identifier
record_weight
record_location_commitment
change_status
create_lot_manifest
split_lot
merge_lot
create_movement
propose_transfer
accept_transfer
finalize_transfer
attach_document_commitment
record_arrival
record_quarantine
record_slaughter
open_dispute
record_correction
retire_animal
```

A lista não deve ser implementada inteira de uma vez. A primeira versão deve priorizar:

1. cadastro individual;
2. leitura/identificação;
3. peso;
4. movimento com GTA referenciada;
5. transferência de custódia com aceite;
6. lote versionado;
7. correção e disputa.

### 7.3 Nonce e concorrência

A evolução precisa resolver um problema que já existe na arquitetura atual: uma transação antiga pode chegar à Solana depois que uma captura foi substituída no PostgreSQL.

O estado individual deve possuir uma versão lógica ou nonce:

```text
AnimalState.intent_nonce = n

TransferIntent A -> nonce n
TransferIntent B -> só pode usar n+1 depois de invalidar/expirar A

Programa Solana:
  aceita somente o nonce vigente
  consome o nonce uma vez
  rejeita replay e transação antiga
```

A supersessão não pode ser somente um status PostgreSQL. Se uma transação antiga ainda é válida on-chain, o cancelamento precisa ser refletido na regra canônica.

### 7.4 Peso e localização em transações

Não é necessário colocar o documento completo, a foto, o laudo, a coordenada precisa ou o histórico total em cada instrução.

O padrão deve ser:

```text
payload completo off-chain
  -> canonicalização
  -> hash do payload
  -> assinatura da origem
  -> transação Solana com hash, tipo, versão e predecessor
```

Para o peso, o chain pode guardar `weight_grams` atual e o hash do evento, se a consulta pública do último peso for requisito essencial. Para a localização, é mais seguro guardar apenas um commitment e uma região pública reduzida.

### 7.5 Lote na Solana

Há duas alternativas.

**Alternativa A — animal canônico, lote derivado.** Cada animal possui o estado canônico. O lote guarda somente um manifesto e uma raiz. É a opção recomendada para o produto descrito, porque peso, status e localização são individuais.

**Alternativa B — lote canônico, animal derivado.** O lote possui todo o estado e os animais são apenas itens de um manifesto. É mais eficiente para operações de grande volume, mas dificulta alterações individuais, reidentificação, peso e disputas.

Para o Lastro, a alternativa A é mais segura. O lote serve como compromisso de composição e como unidade operacional, enquanto o `AnimalState` continua sendo a fonte canônica de cada bovino.

## 8. Internet, acesso público e segurança

### 8.1 Camadas de acesso

O sistema deve possuir três experiências diferentes:

**Consulta pública.** Mostra prova de integridade, identificador público, status permitido, região generalizada, data aproximada, eventos autorizados e links de documentos públicos. Não mostra CPF, nome, endereço, coordenada exata, preço, rota, credencial ou histórico comercial completo.

**Consulta autenticada da cadeia.** Permite ao comprador, produtor, transportador, abatedouro ou auditor visualizar os documentos e eventos que o seu papel autoriza. O acesso precisa ser limitado por organização, finalidade e período.

**Console operacional.** Permite capturar, autorizar, corrigir, disputar e reconciliar. Exige autenticação forte, carteira ou credencial adequada, MFA quando aplicável e trilha de auditoria.

A API pública não deve aceitar busca aberta por sequência de RFID. O acesso deve usar um identificador público aleatório, QR code ou token de consulta com escopo e expiração.

### 8.2 Modelo de autorização

O sistema deve usar autorização baseada em papel e contexto:

```text
PublicViewer
  -> prova pública mínima

ProducerOperator
  -> animais e documentos de suas explorações

Transporter
  -> movimentos atribuídos e documentos de transporte

Buyer
  -> lotes/animais recebidos ou em negociação

Slaughterhouse
  -> recebimento, inspeção e abate do seu estabelecimento

Auditor
  -> histórico e EvidencePackage autorizado

OfficialAuthority
  -> integração e confirmação conforme convênio/credencial

PlatformAdmin
  -> configuração técnica, sem poder editar evidência canônica
```

O administrador da plataforma não deve conseguir alterar o histórico de um animal por um endpoint administrativo. Correções precisam gerar eventos e registrar autor, razão e autoridade.

### 8.3 Dados públicos e dados privados

A LGPD se aplica ao tratamento digital de dados pessoais. Ela exige finalidade, adequação, necessidade, transparência, segurança e responsabilização, além de direitos dos titulares e medidas técnicas e administrativas. [7]

No contexto rural, nomes, CPF/CNPJ, carteiras, localização, documentos, logs e relações de produtor/custodiante podem identificar pessoas direta ou indiretamente. Um hash não é automaticamente anônimo. A correlação com uma tabela de referência, tempo, região e cadeia comercial pode reidentificar o titular.

O `EvidencePackage` deve ter três perfis:

| Perfil | Conteúdo |
|---|---|
| **Public proof** | Hashes, sequência, assinatura verificável, status público, versão e localização generalizada. |
| **Restricted evidence** | Documentos e dados necessários para contraparte, auditor ou cliente autorizado. |
| **Internal/legal** | Identidade, base legal, logs, mapa de chaves, disputas, incidentes e informações protegidas. |

O conteúdo público deve ser mínimo por padrão. A regra deve ser “publicar prova suficiente”, não “publicar todo o histórico”.

O sistema também precisa de um fluxo de incidente. A Resolução CD/ANPD nº 15/2024 prevê comunicação de incidente que possa causar risco ou dano relevante e define, em regra, prazo de três dias úteis para comunicação pelo controlador, além do conteúdo mínimo e manutenção de registro do incidente. [8]

## 9. Segurança da verdade física

A blockchain torna o histórico difícil de adulterar. Ela não torna automaticamente verdadeiro o fato de origem.

### 9.1 RFID

O RFID pode ser perdido, trocado, duplicado, clonado ou lido no animal errado. O sistema deve registrar:

- fabricante e modelo do leitor;
- protocolo do leitor;
- frame bruto quando permitido;
- valor lógico canônico;
- qualidade ou repetição da leitura;
- Station e firmware;
- motivo da leitura;
- operador;
- horário monotônico e horário de parede;
- reidentificação e motivo;
- status do identificador anterior.

### 9.2 Balança

O peso precisa de um ciclo de vida próprio:

- registro do equipamento;
- certificado de calibração;
- versão de firmware;
- teste de tara;
- identificação da estação de pesagem;
- operador;
- incerteza da medição;
- evidência de erro;
- correção por evento.

### 9.3 Localização

A localização deve separar “observada por GPS”, “informada pelo operador”, “associada ao estabelecimento cadastrado” e “confirmada por fonte oficial”. Não se deve apresentar um GPS assinado como prova definitiva de presença física.

### 9.4 Chaves

A Station deve possuir registry, provisionamento, firmware conhecido, rotação e revogação. A chave de uma Station substituída não pode continuar autorizada indefinidamente. O mesmo vale para carteiras, tokens de Agent, credenciais de operadores e chaves de integração.

## 10. Fluxo ponta a ponta recomendado

```text
1. Produtor cadastra animal ou lote
2. Identificador é associado com namespace e fonte
3. Station lê RFID e, se aplicável, balança coleta peso
4. Station assina a observação física
5. Agent grava evidência no outbox SQLite
6. API valida bytes, assinatura, contexto e limites
7. Operador informa ou integra documento de origem
8. Sistema classifica proveniência e cria EvidencePackage
9. Vendedor cria intenção de transferência
10. Comprador lê resumo e assina aceite
11. API valida GTA/documentos e nonce on-chain
12. Solana aplica a transição de custódia
13. Transporte registra carregamento e movimento
14. Destino confirma chegada ou fonte oficial reconcilia
15. Status e localização são atualizados por eventos
16. Lote pode ser dividido, fundido ou encerrado
17. Reconciliador atualiza a projeção sem depender do navegador
18. Usuários consultam prova pública ou evidência autorizada
```

Uma versão mais detalhada está disponível no diagrama separado deste estudo.

## 11. Alterações concretas no repositório

### 11.1 `crates/lastro-protocol`

Adicionar tipos de protocolo sem quebrar o evento v1:

```text
BovineEventType
IdentifierNamespace
LifecycleStatus
ProvenanceLevel
DocumentType
MovementPurpose
LotOperation
TransferMode
```

Criar encode/decode versionado para eventos bovinos. O parser deve rejeitar versões desconhecidas e impedir campos ambíguos.

Adicionar vetores para:

- peso em gramas;
- valores máximos e mínimos;
- localização commitment;
- transferência com aceite;
- split e merge de lote;
- documento cancelado;
- correção e disputa;
- replay;
- duas transações com o mesmo nonce;
- evento com fonte não autorizada.

### 11.2 `chain`

Evoluir `AnimalState` para `BovineState` ou manter `AnimalState` com uma versão explícita. Criar registry de Stations e partes. Criar intent/nonce on-chain. Adicionar regras de status e documentos.

Não colocar CPF, nome, endereço ou documento completo em contas públicas. Definir tamanho de contas e custo de rent antes de congelar o layout.

### 11.3 `services/api`

Adicionar módulos:

```text
routes/animals.rs
routes/lots.rs
routes/movements.rs
routes/weights.rs
routes/documents.rs
routes/transfers.rs
routes/disputes.rs
routes/public_verify.rs
routes/official_sources.rs

domain/lots.rs
domain/movements.rs
domain/weights.rs
domain/status.rs
domain/transfers.rs
domain/provenance.rs
domain/reconciliation.rs

repository/lots.rs
repository/documents.rs
repository/transfers.rs
repository/parties.rs
repository/disputes.rs
repository/official_sources.rs
```

As rotas de escrita devem exigir idempotency key, credencial e escopo. As rotas públicas devem responder com DTOs mínimos e nunca devolver a linha crua do banco.

### 11.4 `services/agent`

Além de RFID, o Agent poderá integrar balança e localização. Ele deve validar:

- unidade e intervalo de peso;
- identificação do dispositivo;
- calibração vigente;
- timestamp;
- duplicidade;
- assinatura;
- contexto recebido da API;
- sequência e nonce;
- estado de quarentena.

### 11.5 `firmware/station`

Adicionar interfaces separadas:

```text
rfid_reader
scale_reader
location_source
secure_clock
journal
attestation
```

O firmware não deve permitir que o backend injete `new_rfid`, peso ou coordenada como se fossem observações locais. O backend fornece contexto; o dispositivo fornece observação.

### 11.6 `apps/web`

Criar telas para:

- cadastro individual;
- lotes e manifestos;
- pesagem;
- localização e nível de precisão;
- movimento e GTA;
- proposta de venda;
- aceite do comprador;
- disputa;
- histórico de proveniência;
- consulta pública;
- consulta autorizada;
- verificação do EvidencePackage.

A UI deve mostrar os rótulos de proveniência. “Assinado” não deve aparecer como “oficial”. “Custodiante” não deve aparecer automaticamente como “proprietário”.

### 11.7 `schemas`

Evoluir o OpenAPI e o JSON Schema com:

- `AnimalPublicView`;
- `AnimalAuthorizedView`;
- `LotManifest`;
- `WeightObservation`;
- `LocationObservation`;
- `Movement`;
- `OfficialDocumentReference`;
- `TransferIntent`;
- `Provenance`;
- `Dispute`;
- `EvidencePackagePublic`;
- `EvidencePackageRestricted`.

O esquema público deve ser diferente do esquema interno. Isso reduz o risco de um endpoint vazar campos apenas porque o banco possui a coluna.

## 12. Roadmap recomendado

### Fase 1 — Modelo correto sem mudar ainda o hardware

- definir `Animal`, `LotManifest`, `Movement`, `WeightObservation`, `LocationObservation`, `StatusEvent` e `TransferIntent`;
- separar custodiante, comprador, vendedor e proprietário alegado;
- criar níveis de proveniência;
- definir EvidencePackage em perfis público, restrito e interno;
- manter eventos bovinos fora do `StationEvent` v1 enquanto o formato não estiver congelado.

### Fase 2 — Lote e cadeia de custódia

- implementar manifestos de lote com raiz criptográfica;
- implementar split, merge e encerramento;
- criar transferência em duas partes;
- exigir documentos por finalidade;
- adicionar nonce on-chain;
- criar reconciliação server-side.

### Fase 3 — Peso e local

- escolher balança e protocolo;
- implementar adapter e calibração;
- definir precisão e unidade;
- criar localização privada e pública generalizada;
- implementar regras de exposição;
- testar dispositivo falso, leitura duplicada, perda de conexão e correção.

### Fase 4 — Fontes oficiais

- definir estado e UF piloto;
- confirmar APIs e credenciais com OESA/MAPA;
- criar adapters de GTA/e-GTA e documentos fiscais aplicáveis;
- validar dados de origem/destino;
- armazenar resposta e proveniência;
- apresentar separadamente “Lastro válido” e “fonte oficial confirmada”.

### Fase 5 — Segurança e governança

- fazer inventário LGPD;
- definir controlador e operador;
- elaborar RIPD se aplicável;
- aplicar segregação de tenant;
- criar RBAC/ABAC;
- definir retenção e descarte off-chain;
- implementar incident response;
- criar registry de Stations e rotação de chaves.

### Fase 6 — Produto de cadeia completa

- acompanhar chegada e abate;
- criar lineage de produto após abate;
- separar lote vivo, carcaça e produtos derivados;
- integrar inspeção e documentos do abatedouro;
- criar portais para produtor, comprador, transportador, auditor e autoridade;
- testar auditoria ponta a ponta em operação real.

## 13. Testes que passam a ser obrigatórios

| Área | Cenário mínimo |
|---|---|
| Identidade | RFID trocado com preservação do mesmo `AnimalID`. |
| RFID | Frame corrompido, duplicado, ausente, clonado e fora do formato. |
| Peso | Unidade inválida, valor fora do limite, balança sem calibração e correção posterior. |
| Local | GPS inconsistente, coordenada precisa bloqueada no perfil público e fonte divergente. |
| Status | Tentativa de transição impossível ou executada por papel incorreto. |
| Lote | Split, merge, item ausente, duplicidade e divergência de quantidade. |
| Movimento | GTA vencida, finalidade incompatível, origem/destino inválidos e chegada parcial. |
| Transferência | Vendedor assina e comprador rejeita; comprador aceita e RPC cai; replay do aceite. |
| Concorrência | Duas vendas do mesmo animal, transação antiga após supersessão e mesmo nonce. |
| Documentos | Hash alterado, documento cancelado, fonte indisponível e parser incompatível. |
| Privacidade | Consulta pública, consulta de comprador, exportação em massa e tentativa de enumeração. |
| Recuperação | Queda da Station, Agent, API, banco, RPC e navegador em cada etapa. |
| Auditoria | Reconstrução do histórico a partir do EvidencePackage e da Solana. |
| Hardware | Queda de energia entre leitura, assinatura, persistência e ACK. |

## 14. Decisão recomendada para a nomenclatura

Até existir reconhecimento oficial, usar:

- **Passaporte digital de rastreabilidade do bovino**;
- **Registro digital de custódia e evidências**;
- **Histórico verificável do animal ou lote**;
- **Manifesto digital de lote**;
- **Prova criptográfica de eventos**.

Evitar:

- matrícula de propriedade;
- título de propriedade on-chain;
- registro oficial de propriedade;
- SISBOV na blockchain;
- certificado PNIB;
- GTA digital do Lastro;
- animal oficialmente regularizado, se a fonte oficial não confirmou.

Uma comunicação comercial segura seria:

> O Lastro cria um registro digital verificável da identidade técnica, das observações, dos documentos e da custódia operacional de bovinos e lotes. Ele preserva a integridade do histórico e permite consulta controlada pela cadeia. Documentos oficiais, requisitos sanitários e titularidade jurídica continuam dependendo das fontes competentes e das partes envolvidas.

## 15. Parecer técnico final

A visão de negócio é viável, mas precisa ser ajustada em um ponto de linguagem e em três pontos de arquitetura.

O ajuste de linguagem é substituir “matrícula de propriedade” por **registro de rastreabilidade e custódia**, a menos que exista no futuro um convênio ou reconhecimento legal específico.

O primeiro ponto arquitetural é manter `AnimalID` individual como identidade canônica. O lote deve ser um manifesto versionado com raiz criptográfica, porque lote pode ser dividido, fundido e movimentado.

O segundo é transformar peso, localização, status e venda em eventos com proveniência e autorização. O estado atual deve ser uma projeção validada, nunca um campo livre que um usuário edita.

O terceiro é separar o que é observado pela Station, declarado pelo operador, sustentado por documento e confirmado por fonte oficial. Sem essa proveniência, a interface fará uma afirmação mais forte do que a evidência permite.

A arquitetura do Lastro já possui as melhores bases para essa evolução: bytes canônicos, assinatura P-256, Agent durável, API, Solana, carteira e verificador. O próximo passo não é simplesmente adicionar mais campos ao `AnimalState`. É criar um modelo de domínio bovino com eventos, lotes, documentos, papéis, aceites, fontes e privacidade.

Se esse desenho for seguido, a blockchain terá um papel útil e realista: **ancorar a história, impedir alterações silenciosas, permitir auditoria e dar confiança verificável entre participantes da cadeia**. Ela não precisará fingir que substitui o órgão sanitário, o documento fiscal ou o direito civil para entregar valor.

## Referências

[1]: https://www.planalto.gov.br/ccivil_03/_ato2007-2010/2009/lei/L12097.htm "Lei nº 12.097/2009 — rastreabilidade na cadeia produtiva de carnes bovinas e bubalinas"
[2]: https://www.gov.br/agricultura/pt-br/assuntos/sanidade-animal-e-vegetal/saude-animal/cgtqa/dpc/sisbov "SISBOV — Ministério da Agricultura e Pecuária"
[3]: https://www.in.gov.br/en/web/dou/-/portaria-sda/mapa-n-1.331-de-21-de-julho-de-2025-643581903 "Portaria SDA/MAPA nº 1.331/2025 — cronograma do PNIB"
[4]: https://www.planalto.gov.br/ccivil_03/_ato2011-2014/2011/decreto/d7623.htm "Decreto nº 7.623/2011 — rastreabilidade de carnes bovinas e bubalinas"
[5]: https://www2.camara.leg.br/legin/fed/lei/2002/lei-10406-10-janeiro-2002-432893-norma-pl.html "Código Civil — Lei nº 10.406/2002"
[6]: https://wikisda.agricultura.gov.br/pt-br/Sa%C3%BAde-Animal/tr%C3%A2nsito_bovinos "MAPA — Manual de procedimentos para trânsito de bovinos e bubalinos"
[7]: https://www.planalto.gov.br/ccivil_03/_ato2015-2018/2018/lei/l13709.htm "Lei nº 13.709/2018 — Lei Geral de Proteção de Dados Pessoais"
[8]: https://www.in.gov.br/en/web/dou/-/resolucao-cd/anpd-n-15-de-24-de-abril-de-2024-556243024 "Resolução CD/ANPD nº 15/2024 — comunicação de incidentes de segurança"

**Autor:** Manus AI

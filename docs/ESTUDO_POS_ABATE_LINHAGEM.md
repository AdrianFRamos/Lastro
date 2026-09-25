# Adendo técnico: pós-abate, desossa e linhagem de produtos

**Projeto:** Lastro
**Escopo:** tratamento de um bovino que chega ao frigorífico, é abatido e é transformado em carcaça, cortes, lotes, caixas, pallets, expedições e eventualmente produtos derivados.

## 1. Resposta direta

Quando o boi chega ao frigorífico, ele não deve continuar sendo representado pelo mesmo objeto que representa os cortes. O modelo correto é criar uma **linhagem de transformação**.

```text
Bovino vivo
    -> recebimento no frigorífico
    -> abate confirmado
    -> carcaça
    -> meia-carcaça ou peça primária
    -> cortes
    -> lote de produção
    -> caixa ou pallet
    -> expedição
    -> cliente ou estabelecimento seguinte
```

O animal original permanece no histórico e muda para um estado terminal, como `SLAUGHTERED` ou `RETIRED`. A partir do abate, surgem novos ativos ou lotes derivados. Cada saída aponta para uma ou mais entradas por meio de uma relação de linhagem.

A blockchain não deve guardar uma conta Solana para cada pedaço de carne. Ela deve ancorar os compromissos criptográficos dos manifestos e das transformações. O detalhe operacional fica no PostgreSQL e no EvidencePackage.

## 2. Por que não basta alterar o status do animal

Uma abordagem simplista seria:

```text
AnimalState.status = SLAUGHTERED
AnimalState.products = [picanha, acém, costela, ...]
```

Esse desenho tem problemas:

1. Um animal pode produzir dezenas ou centenas de unidades físicas.
2. Cada corte pode receber um lote, uma embalagem, uma validade e uma expedição diferente.
3. Uma caixa pode conter cortes de várias carcaças.
4. Um produto pode ser transformado novamente em carne moída, embutido ou outro lote.
5. Parte do animal pode virar subproduto ou perda de processo.
6. Um recall precisa descobrir tanto a origem de um produto quanto todos os produtos derivados de uma origem contaminada.
7. A custódia do boi vivo é diferente da custódia do alimento produzido.

Por isso, o domínio deve separar:

```text
identidade do animal
estado de abate
identidade da carcaça
lote de transformação
lote de produto
unidade logística
expedição
custódia do produto
```

## 3. Modelo de ativos

O sistema deve adotar um tipo explícito para cada ativo.

| Ativo | Exemplo de identificador | Significado |
|---|---|---|
| `Animal` | `ANIMAL-123` | Bovino vivo identificado individualmente. |
| `SlaughterBatch` | `SLT-2026-001` | Recebimento ou programa de abate. |
| `Carcass` | `CARCASS-123-A` | Resultado do abate de um animal ou unidade definida pelo processo. |
| `PrimalCut` | `PRIMAL-123-A-01` | Peça primária da carcaça. |
| `CutBatch` | `CUT-2026-0001` | Lote homogêneo de cortes. |
| `ProductLot` | `PROD-2026-0001` | Lote comercial refrigerado, congelado ou processado. |
| `PackageUnit` | `PKG-2026-0001` | Caixa, peça embalada, pallet ou unidade logística. |
| `Shipment` | `SHIP-2026-0100` | Expedição para outro estabelecimento ou cliente. |
| `ByproductLot` | `BYP-2026-0001` | Couro, sebo, ossos, miúdos ou outro subproduto. |

O `AnimalID` não deve ser reutilizado como identificador de corte. O relacionamento deve ser explícito:

```text
ANIMAL-123
  -> CARCASS-123-A
  -> CUT-2026-0001
  -> PKG-2026-0001
  -> SHIP-2026-0100
```

O produto pode ter mais de um pai. Por exemplo, um lote de carne moída pode ser produzido a partir de cortes de várias carcaças:

```text
CARCASS-123-A ─┐
CARCASS-124-A ─┼─> CUT-2026-0008 ─> PROD-2026-0100
CARCASS-125-A ─┘
```

Isso exige um grafo acíclico de linhagem. Uma transformação nunca pode apontar para um produto que foi criado depois dela.

## 4. O que é on-chain e o que é off-chain

### 4.1 Solana

A Solana deve manter somente os dados necessários para prova de integridade, estado canônico e verificação de regras:

- `asset_id` ou `lot_id` opaco;
- `asset_type`;
- relação de linhagem ou raiz do manifesto;
- hash do manifesto de transformação;
- hash das entradas;
- hash das saídas;
- quantidade resumida;
- unidade de medida;
- estabelecimento pseudonimizado;
- status do ativo;
- sequência e nonce;
- credencial que autorizou o evento;
- timestamp lógico;
- status de bloqueio, recall ou revogação.

### 4.2 PostgreSQL e EvidencePackage

O backend deve guardar o detalhe necessário para operar e auditar:

- lista completa de entradas e saídas;
- pesos bruto, líquido e de perdas;
- documentos sanitários;
- documentos fiscais;
- laudos e inspeções;
- operador;
- linha de produção;
- equipamento;
- turno;
- fotos ou anexos;
- caixas e pallets;
- clientes e destinatários;
- coordenadas e endereços;
- dados pessoais;
- parâmetros de processo que não devem ser públicos.

A Solana recebe o hash de um manifesto canônico. Se o manifesto for alterado depois, o hash não coincidirá com o compromisso on-chain.

## 5. O frigorífico como autoridade de processamento

O frigorífico deve ter uma identidade técnica e permissões próprias. Não basta usar uma carteira administrativa genérica.

```text
ProcessingFacilityRegistry
  -> facility_id
  -> tipo e escopo de operação
  -> credenciais e carteiras
  -> validade
  -> status de habilitação
  -> chaves revogadas
  -> integrações documentais
```

O frigorífico pode receber permissões para:

- confirmar chegada;
- confirmar quantidade recebida;
- confirmar abate;
- criar carcaça;
- registrar peso de carcaça;
- criar manifesto de desossa;
- criar lote de produto;
- criar subprodutos;
- registrar perdas;
- registrar retenção de qualidade;
- liberar lote;
- criar expedição;
- abrir recall.

Essas permissões não devem permitir que o frigorífico altere o histórico do produtor, reescreva a identidade do animal ou declare automaticamente propriedade civil.

## 6. Eventos pós-abate

Os eventos mínimos são:

```text
SLAUGHTER_RECEIVED
SLAUGHTER_CONFIRMED
CARCASS_CREATED
CARCASS_WEIGHED
CARCASS_SPLIT
PRIMAL_CUT_CREATED
TRANSFORMATION_MANIFEST_CREATED
PRODUCT_LOT_CREATED
PACKAGE_CREATED
PACKAGE_RELABELED
BYPRODUCT_CREATED
WASTE_RECORDED
QUALITY_HOLD_PLACED
QUALITY_HOLD_RELEASED
SHIPMENT_CREATED
SHIPMENT_ACCEPTED
PRODUCT_RECALLED
PRODUCT_RETIRED
```

Cada evento deve conter, conforme o tipo:

- identificador do evento;
- tipo do ativo;
- entradas;
- saídas;
- unidades e quantidades;
- pesos;
- estabelecimento;
- linha de processo;
- intervalo temporal;
- operador ou sistema responsável;
- documento associado;
- proveniência;
- evento anterior;
- hash do manifesto;
- assinatura ou credencial;
- nonce;
- versão do esquema.

## 7. Manifesto de transformação

A desossa deve ser uma operação de transformação com entrada e saída. Não deve ser registrada como vários eventos independentes sem vínculo.

Exemplo conceitual:

```json
{
  "transformation_id": "TRF-2026-0001",
  "event_type": "TRANSFORMATION_MANIFEST_CREATED",
  "facility_id": "FACILITY-001",
  "process_type": "DEBONING",
  "started_at": "2026-09-24T10:00:00Z",
  "completed_at": "2026-09-24T14:00:00Z",
  "inputs": [
    {
      "asset_id": "CARCASS-123-A",
      "asset_type": "CARCASS",
      "quantity": 1,
      "unit": "UNIT",
      "net_weight_grams": 312400
    }
  ],
  "outputs": [
    {
      "asset_id": "CUT-2026-0001",
      "asset_type": "CUT_BATCH",
      "product_type": "BEEF_CUT",
      "quantity": 80,
      "unit": "KG",
      "net_weight_grams": 213800
    },
    {
      "asset_id": "BYP-2026-0001",
      "asset_type": "BYPRODUCT_LOT",
      "product_type": "OFFAL",
      "quantity": 1,
      "unit": "LOT",
      "net_weight_grams": 42800
    }
  ],
  "loss": {
    "net_weight_grams": 20800,
    "category": "PROCESS_LOSS"
  },
  "manifest_hash": "...",
  "previous_event_hash": "...",
  "signature": "..."
}
```

Os números são ilustrativos. O ponto importante é que a operação seja fechada por entradas, saídas, subprodutos, perdas e unidade de medida.

## 8. Balanço de massa

A transformação deve ser validada por uma regra de balanço:

```text
peso de entrada
  = produtos principais
  + subprodutos
  + perdas
  + ajuste documentado
```

Não se deve exigir igualdade matemática absoluta em todos os processos. Podem existir tolerâncias por água, aparas, evaporação, tara, arredondamento e diferenças de balanças. Essas tolerâncias precisam ser configuradas e auditadas.

Exemplo:

```text
Entrada:             312.400 g
Cortes:              213.800 g
Subprodutos:          42.800 g
Perdas de processo:   20.800 g
Ajuste documentado:   35.000 g
```

O ajuste precisa informar motivo, unidade, responsável e evidência. O sistema não deve aceitar um ajuste silencioso.

A Solana pode validar somente o resumo e os limites. A API executa o cálculo detalhado e grava o hash do manifesto.

## 9. Estados separados por tipo de ativo

O mesmo texto `ACTIVE` não deve significar a mesma coisa para um animal vivo e para uma embalagem.

### Animal

```text
REGISTERED
ACTIVE
IN_TRANSIT
RECEIVED
SLAUGHTER_RECEIVED
SLAUGHTERED
DISPUTED
RETIRED
```

### Carcaça

```text
CREATED
WEIGHED
QUALITY_HOLD
RELEASED
SPLIT
RETIRED
```

### Lote de produto

```text
CREATED
IN_PROCESS
QUALITY_HOLD
RELEASED
IN_STORAGE
IN_TRANSIT
RECEIVED
RECALLED
EXPIRED
RETIRED
```

### Unidade logística

```text
CREATED
PACKED
STORED
SHIPPED
RECEIVED
DAMAGED
RECALLED
CLOSED
```

O programa deve validar transições por `asset_type`. Uma carcaça não deve receber uma transição que só faz sentido para um animal vivo.

## 10. Divisão e combinação de lotes

O pós-abate possui duas operações diferentes:

### Split

Um lote de produção é dividido em vários lotes ou caixas.

```text
CUT-2026-0001
  -> PKG-0001
  -> PKG-0002
  -> PKG-0003
```

O evento deve consumir ou reduzir a quantidade disponível do lote-pai e criar identificadores-filhos.

### Merge

Um novo lote é criado com entradas de vários pais.

```text
CUT-2026-0001 ─┐
CUT-2026-0002 ─┼─> PROD-2026-0100
CUT-2026-0003 ─┘
```

O evento deve registrar todos os pais. Depois do merge, a consulta precisa conseguir retornar a origem de cada componente.

O sistema deve impedir:

- consumo duplicado de uma entrada;
- saída maior que a quantidade disponível sem ajuste;
- ciclo na árvore de linhagem;
- uso de lote expirado ou bloqueado;
- transformação de produto sob recall sem autorização;
- reutilização de um `asset_id`.

## 11. Custódia depois do abate

Depois do abate, o fluxo de custódia muda:

```text
Frigorífico
  -> operador logístico
  -> distribuidor
  -> atacadista
  -> supermercado/restaurante
  -> consumidor
```

A entidade principal deixa de ser apenas `Animal` e passa a ser `ProductLot`, `PackageUnit` ou `Shipment`.

Cada expedição deve conter:

- origem;
- destino;
- produto ou caixas transportadas;
- quantidade;
- peso;
- temperatura, se aplicável;
- veículo e transportador;
- documento de trânsito ou documento comercial;
- data de saída;
- data de chegada;
- aceite do destino;
- divergência de quantidade;
- status de qualidade.

O sistema deve diferenciar:

```text
custódia física
propriedade comercial
transporte
entrega
recebimento
```

Uma carteira que assina o recebimento representa o aceite digital de determinada operação. Ela não deve ser apresentada automaticamente como prova de titularidade jurídica sem os documentos correspondentes.

## 12. Recall e rastreabilidade reversa

A linhagem precisa funcionar em duas direções.

### Rastreabilidade para frente

A partir de um animal, deve ser possível descobrir:

```text
Animal
  -> carcaça
  -> cortes
  -> lotes de produto
  -> caixas
  -> pallets
  -> expedições
  -> estabelecimentos autorizados
```

### Rastreabilidade para trás

A partir de uma embalagem, deve ser possível descobrir:

```text
Embalagem
  -> lote de produto
  -> transformação
  -> corte
  -> carcaça
  -> animal
  -> origem e documentos
```

Um recall deve criar um novo evento, por exemplo:

```text
PRODUCT_RECALLED
```

Esse evento não apaga o histórico. Ele altera o estado de disponibilidade e bloqueia novas movimentações conforme a política.

Exemplo:

```text
ANIMAL-123
  -> CARCASS-123-A
  -> CUT-2026-0001
  -> PKG-01, PKG-02, PKG-03
  -> SHIP-10, SHIP-11
  -> clientes autorizados
```

Se houver problema em `CUT-2026-0001`, o sistema identifica as embalagens e expedições afetadas. Se houver problema no `ANIMAL-123`, o sistema pode encontrar os produtos que derivaram dele.

## 13. QR code e consulta pública

O QR code de uma embalagem não deve conter todo o histórico nem dados pessoais. Ele deve apontar para um identificador público aleatório:

```text
https://verificador.exemplo/p/opaque-package-id
```

A página pública pode mostrar:

- lote público;
- tipo de produto;
- data de produção;
- validade;
- região de processamento;
- status de liberação;
- prova criptográfica;
- resumo da linhagem;
- aviso de recall, quando aplicável.

Ela não deve mostrar por padrão:

- CPF/CNPJ;
- preço;
- carteira da pessoa;
- coordenada exata da fazenda;
- rota do caminhão;
- nome de todos os operadores;
- histórico comercial completo;
- documentos internos do frigorífico.

A consulta autenticada pode mostrar mais informações a compradores, auditores e autoridades, conforme escopo.

## 14. Contas Solana e instruções sugeridas

Uma evolução do programa pode usar contas como:

```text
ProcessingFacilityRegistry
AnimalState
CarcassState
ProductLotState
PackageCommitment
TransformationAnchor
LineageAnchor
ShipmentState
DocumentAnchor
RecallState
```

Instruções possíveis:

```text
register_processing_facility
confirm_slaughter
create_carcass
record_carcass_weight
create_transformation_manifest
consume_transformation_inputs
create_product_lot
create_package_commitment
create_shipment
accept_shipment
place_quality_hold
release_quality_hold
open_recall
retire_product_lot
```

A instrução `create_transformation_manifest` deve receber o hash do manifesto completo. A instrução `consume_transformation_inputs` deve impedir que uma entrada seja consumida duas vezes.

O `TransformationAnchor` pode conter:

```rust
pub struct TransformationAnchor {
    pub transformation_id: [u8; 32],
    pub facility_id: [u8; 32],
    pub input_root: [u8; 32],
    pub output_root: [u8; 32],
    pub manifest_hash: [u8; 32],
    pub input_weight_grams: u64,
    pub output_weight_grams: u64,
    pub loss_weight_grams: u64,
    pub status: u8,
    pub sequence: u64,
    pub previous_event_hash: [u8; 32],
    pub bump: u8,
}
```

Esse é um exemplo conceitual. Os campos precisam ser ajustados ao limite de tamanho, custo de rent, precisão, privacidade e necessidade de consulta independente.

## 15. Alterações no repositório Lastro

### Protocolo compartilhado

Adicionar tipos para:

```text
AssetType
LineageRelation
TransformationType
ProductStatus
PackagingLevel
MassUnit
QualityStatus
RecallStatus
```

Criar codificação canônica para manifestos de transformação. A mesma sequência de entradas, saídas e pesos precisa gerar o mesmo hash em Rust, TypeScript e qualquer componente do frigorífico.

### Programa Solana

Adicionar estados e instruções para:

```text
carcaça
manifesto
produto
embalagem
expedição
recall
facility registry
```

### API

Adicionar módulos:

```text
services/api/src/domain/slaughter.rs
services/api/src/domain/lineage.rs
services/api/src/domain/transformations.rs
services/api/src/domain/products.rs
services/api/src/domain/packages.rs
services/api/src/domain/shipments.rs
services/api/src/domain/recalls.rs

services/api/src/repository/lineage.rs
services/api/src/repository/product_lots.rs
services/api/src/repository/transformations.rs
services/api/src/repository/shipments.rs
services/api/src/repository/recalls.rs
```

### Frontend

Adicionar telas para:

- recebimento no frigorífico;
- confirmação de abate;
- pesagem da carcaça;
- manifesto de transformação;
- árvore de linhagem;
- lotes de produto;
- caixas e pallets;
- expedições;
- bloqueio e recall;
- verificação pública por QR code.

## 16. Primeira versão recomendada

Não é necessário começar rastreando cada pedaço individual de carne on-chain.

A primeira versão prática deve ser:

1. O frigorífico confirma a chegada do lote e dos animais.
2. O sistema registra `SLAUGHTER_CONFIRMED`.
3. Cada carcaça recebe um identificador interno.
4. A carcaça é pesada.
5. O frigorífico cria um `TransformationManifest` de desossa.
6. O manifesto contém carcaças de entrada, cortes de saída, subprodutos e perdas.
7. O manifesto é canonicalizado e recebe um hash.
8. O hash é ancorado na Solana.
9. O PostgreSQL guarda a árvore detalhada.
10. Caixas e pallets recebem identificadores logísticos.
11. A expedição registra origem, destino, quantidade e aceite.
12. O QR code aponta para uma página pública de verificação.
13. O sistema consegue fazer consulta para frente e para trás.
14. Um recall pode bloquear lote, caixa e expedição sem apagar histórico.

Essa versão entrega rastreabilidade real sem criar custos e complexidade de uma conta blockchain por embalagem.

## 17. Testes obrigatórios

| Área | Teste |
|---|---|
| Abate | Animal recebido, mas não autorizado para abate. |
| Identidade | Dois animais gerando carcaças distintas. |
| Desossa | Uma carcaça gerando vários lotes de corte. |
| Merge | Cortes de várias carcaças formando um lote de produto. |
| Split | Um lote formando várias caixas e pallets. |
| Massa | Saídas acima da entrada sem ajuste autorizado. |
| Massa | Perda acima da tolerância configurada. |
| Duplicidade | Mesma carcaça consumida em duas transformações. |
| Ciclo | Produto apontando para si mesmo como ancestral. |
| Status | Produto em recall sendo expedido. |
| Recall | Produto derivado localizado a partir do animal de origem. |
| Origem | Animal localizado a partir de uma embalagem pública. |
| Privacidade | Consulta pública sem CPF, endereço ou rota. |
| Concorrência | Duas expedições consumindo a mesma caixa. |
| Recuperação | Queda da API após o manifesto ser salvo e antes do anchor Solana. |
| Idempotência | Reenvio do mesmo manifesto sem criar produtos duplicados. |
| Evidência | Alteração do manifesto quebrando o hash on-chain. |
| Auditoria | Reconstrução completa do grafo usando EvidencePackage e Solana. |

## 18. Conclusão

O frigorífico deve ser modelado como uma transformação de ativos, não como uma simples atualização de cadastro.

A sequência correta é:

```text
animal vivo
  -> abate
  -> carcaça
  -> transformação
  -> produtos
  -> embalagens
  -> expedições
  -> recall e auditoria
```

A blockchain deve garantir que a linhagem não seja alterada silenciosamente. O banco de dados deve guardar o detalhe operacional. O frigorífico deve assinar as transformações conforme seu papel. O sistema deve manter balanço de massa, estados separados, identificadores novos para cada classe de ativo e consulta reversa.

A decisão mais importante é esta:

> **O `AnimalID` é a origem da história, mas não é o identificador dos cortes. Cada transformação cria ativos derivados com novos identificadores e uma relação criptograficamente verificável com os pais.**

Essa estrutura permite que o Lastro acompanhe o animal vivo, a carcaça, os cortes, as embalagens e a distribuição sem confundir identidade, custódia, propriedade, produção e consumo.

## Referências

[1]: ../docs/ESTUDO_RASTREABILIDADE_BOVINA.md "Estudo principal do Lastro para rastreabilidade bovina"
[2]: https://www.planalto.gov.br/ccivil_03/_ato2007-2010/2009/lei/L12097.htm "Lei nº 12.097/2009 — rastreabilidade na cadeia produtiva de carnes bovinas e bubalinas"
[3]: https://wikisda.agricultura.gov.br/pt-br/Sa%C3%BAde-Animal/tr%C3%A2nsito_bovinos "MAPA — Manual de procedimentos para trânsito de bovinos e bubalinos"

**Autor:** Manus AI

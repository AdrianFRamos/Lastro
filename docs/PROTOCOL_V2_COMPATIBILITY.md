# Compatibilidade e canonicalização do protocolo v2

**Projeto:** Lastro
**Estado:** contrato inicial congelado para a primeira etapa de linhagem e transformação
**Data:** 25 de setembro de 2026

## 1. Regra de compatibilidade

O protocolo v1 e o protocolo v2 são contratos independentes. O `StationEvent` v1 continua com 276 bytes, e o programa Solana v1 continua responsável pelas contas e transições legadas. O v2 utiliza o envelope `DomainEventEnvelope` de 220 bytes, o programa `lastro-v2` e contas com seeds próprias.

Um consumidor não deve inferir a versão somente pelo tamanho do payload. A versão deve ser lida do campo `schema_version` e validada contra o contrato esperado. Schema desconhecido, enum desconhecido, ID nulo, janela temporal inválida ou discriminator de conta desconhecido devem ser rejeitados.

| Contrato | Programa | Estado | Regra |
|---|---|---|---|
| `StationEvent` v1 | `lastro` | contas v1 | compatibilidade congelada; não receber campos v2 |
| `DomainEventEnvelope` v2 schema 1 | `lastro-v2` | contas v2 | versão explícita; eventos e IDs fixos |
| Manifesto de transformação schema 1 | API, Agent, verificador | off-chain com hash | bytes canônicos e hash de domínio separado |

## 2. Identificadores e enums

IDs públicos são arrays de 32 bytes e não devem conter PII. O protocolo v2 rejeita identificadores formados somente por zeros. Os enums possuem representação explícita (`u8` ou `u16`) e conversão que rejeita valores não conhecidos.

Os tipos principais são `Animal`, `Lot`, `Carcass`, `CutBatch`, `ProductLot`, `Package`, `ByproductLot` e `Shipment`. Os papéis de folhas de linhagem são `Input`, `Output`, `Byproduct` e `Loss`. A classificação de facility, status, unidade e tipo de evento também é wire-level e não deve ser substituída por strings livres dentro de transações.

## 3. Hashes com separação de domínio

Cada compromisso usa um prefixo próprio terminado por byte nulo:

```text
LASTRO_V2_EVENT\0
LASTRO_V2_PAYLOAD\0
LASTRO_V2_ASSET\0
LASTRO_V2_LINEAGE\0
LASTRO_V2_LINEAGE_LEAF\0
LASTRO_V2_LINEAGE_NODE\0
LASTRO_V2_TRANSFORMATION\0
LASTRO_V2_INTENT\0
LASTRO_V2_MIGRATION\0
```

A mesma sequência de bytes em dois domínios diferentes deve produzir hashes diferentes. Nenhum hash de payload deve ser reutilizado como ID de PDA sem aplicar a regra de derivação correspondente.

## 4. Folha canônica de linhagem

Uma folha de linhagem possui exatamente 53 bytes, sempre em little-endian:

| Offset | Tamanho | Campo |
|---:|---:|---|
| 0 | 32 | `asset_id` |
| 32 | 1 | `role` |
| 33 | 4 | `position` |
| 37 | 8 | `quantity` |
| 45 | 8 | `weight_grams` |

A folha é válida somente quando o ID não é nulo e quantidade ou peso é maior que zero. O hash da folha é `SHA-256("LASTRO_V2_LINEAGE_LEAF\\0" || bytes_da_folha)`.

## 5. Merkle root

A raiz é calculada com as folhas ordenadas pelo campo `position`. Posições duplicadas são rejeitadas. Em cada nível, os nós são combinados aos pares usando:

```text
SHA-256("LASTRO_V2_LINEAGE_NODE\\0" || left_hash || right_hash)
```

Quando um nível possui quantidade ímpar, o último nó é duplicado. A regra é deliberada e deve ser repetida no TypeScript, Python, verificador e qualquer integração industrial. A ordem do array de entrada não altera a raiz porque a ordem semântica está no `position`; alterar asset, role, quantidade, peso ou posição altera a raiz.

## 6. Manifesto de transformação

O manifesto canônico inicial possui exatamente 188 bytes. A ordem é:

```text
transformation_id       [32]
facility_id             [32]
transformation_type     u16 LE
input_root              [32]
output_root             [32]
input_count             u32 LE
output_count            u32 LE
input_weight_grams      u64 LE
output_weight_grams     u64 LE
byproduct_weight_grams  u64 LE
loss_weight_grams       u64 LE
tolerance_basis_points  u16 LE
manifest_nonce           u64 LE
expires_at               i64 LE
```

O hash do manifesto é:

```text
SHA-256("LASTRO_V2_TRANSFORMATION\\0" || canonical_manifest_bytes)
```

O manifesto exige IDs, roots, contagens, tipo e timestamp válidos. O payload completo, documentos e detalhes dos cortes permanecem fora da transação; a cadeia recebe o commitment e os números compactos necessários para aplicar os invariantes.

## 7. Balanço de massa

O balanço é validado em inteiros de gramas usando aritmética de 128 bits:

```text
input_weight
  = output_weight + byproduct_weight + loss_weight
    dentro de tolerance_basis_points
```

A tolerância máxima do protocolo é de 1.000 basis points, equivalente a 10%. A configuração de deployment pode usar limite menor. Valores negativos não são representáveis, overflow é rejeitado e uma transformação sem peso de entrada é inválida.

O sistema deve representar perdas e subprodutos explicitamente. Não é permitido fazer uma transformação parecer válida omitindo a massa que deixou de existir ou que saiu por uma linha de produção diferente.

## 8. Fixture de referência

A fixture [transformation-manifest.json](../schemas/fixtures/v2/transformation-manifest.json) congela um exemplo com:

- comprimento canônico de 188 bytes;
- duas entradas de 600 g e 400 g;
- um output de 800 g;
- um subproduto de 150 g;
- uma perda de 50 g;
- roots de entrada e saída;
- hash final do manifesto.

O teste Rust `checked_in_fixture_matches_rust_canonicalization` valida os roots, o comprimento e o hash contra essa fixture. Implementações TypeScript e Python devem usar os mesmos bytes e devem adicionar testes equivalentes antes de participar da produção.

## 9. Regras para versões futuras

A inclusão de um campo, mudança de endianess, alteração da regra de ordenação, mudança de padding, alteração do limite de tolerância ou mudança de política de árvore exige nova versão de schema ou novo domínio de hash. Não se deve alterar o significado do schema 1 retroativamente.

Uma migração deve preservar o histórico anterior, registrar o predecessor e produzir novo commitment. A correção de massa, linhagem ou documento ocorre por novo evento; não há edição destrutiva de uma folha, manifesto ou anchor já confirmado.

# Etapa 4A — Integração da API com o protocolo v2

**Projeto:** Lastro
**Escopo:** ingestão autenticada de observações v2, persistência idempotente e consulta pública de anchors
**Estado:** concluída para revisão

## Resultado

A API agora possui um vertical slice funcional para o protocolo v2. Um Agent autenticado pode enviar um envelope de domínio assinado pela Station. A API decodifica os 220 bytes, valida a versão, o tipo de evento, o deployment, a origem da Station, a janela temporal e a assinatura P-256 low-S. Depois disso, persiste o evento como evidência imutável e devolve um anchor verificável.

A API também expõe uma leitura pública da evidência por hash e uma timeline cronológica por `subject_id`. Essas leituras são limitadas ao deployment configurado no processo, evitando que uma instância misture dados de deployments diferentes.

## Endpoints adicionados

| Método | Endpoint | Acesso | Função |
|---|---|---|---|
| `POST` | `/api/v2/agent/observations` | Bearer do Agent | Valida e persiste uma observação `ObservationRecorded` assinada |
| `GET` | `/api/v2/events/{eventHash}` | Público | Recupera um anchor v2 pelo hash do envelope |
| `GET` | `/api/v2/assets/{assetId}/timeline` | Público | Recupera até 256 anchors ordenados por versão de estado |

O endpoint de ingestão aceita somente `ObservationRecorded` neste primeiro corte. Eventos de custódia, transformação, abate, criação de carcaça, cortes, pacotes, remessas e recall terão endpoints e regras próprias. Isso evita que uma rota genérica aceite transições de negócio sem autorização específica.

## Banco de dados

A migration `0008_domain_v2.sql` adiciona três blocos conceituais:

1. `v2_assets`, que prepara a projeção local dos ativos genéricos do domínio bovino.
2. `v2_event_anchors`, que armazena envelope, hash, Station, assinatura, status e eventual assinatura Solana.
3. Índices e trigger de imutabilidade, incluindo unicidade por `(subject_id, state_version)`.

A inserção é idempotente. Uma repetição byte a byte do mesmo evento retorna sucesso sem duplicar a linha. Uma colisão de `event_id`, hash ou versão com evidência diferente retorna conflito. O banco não pode alterar os bytes criptográficos, a Station, a assinatura, o predecessor ou os identificadores depois da inserção.

## Segurança implementada

O endpoint de ingestão usa a mesma comparação em tempo constante do transporte Agent v1. O token não concede autoridade de custódia; ele apenas autentica o canal operacional entre Agent e API. A autoridade canônica continua sendo validada contra a configuração da Station no RPC Solana.

A API rejeita Station diferente da registrada, `source_id` incompatível com a chave P-256, envelope fora da janela de validade, schema inválido, tipo de evento diferente de observação e assinaturas high-S. A resposta pública não expõe token operacional.

## Validações executadas

| Verificação | Resultado |
|---|---|
| `cargo check --locked -p lastro-api` | Passou |
| `cargo test --locked -p lastro-api --lib` | Passou: 13 testes |
| Teste de admissão v2 com assinatura real P-256 | Passou |
| Teste de rejeição de Station incompatível | Passou |
| `cargo test --locked -p lastro-protocol` | Passou |
| `cargo fmt --all -- --check` | Passou |
| `git diff --check` | Passou |
| YAML OpenAPI | Atualizado; validação automática não executada porque Python YAML e Ruby não estão instalados no Windows |
| Suíte completa de testes da API | Não concluída: o compilador MSVC sofreu `STATUS_STACK_BUFFER_OVERRUN`/falha de alocação ao compilar muitos testes de integração simultaneamente. O erro ocorreu fora do código v2; a biblioteca e os testes unitários passaram.

## Arquivos principais

- `services/api/migrations/0008_domain_v2.sql`
- `services/api/src/domain/v2.rs`
- `services/api/src/repository/domain_v2.rs`
- `services/api/src/routes/domain_v2.rs`
- `services/api/src/model.rs`
- `services/api/src/routes/mod.rs`
- `schemas/openapi.yaml`
- `crates/lastro-protocol/src/crypto.rs`

## Limites desta etapa

Esta entrega ainda não cria transações Solana v2 para o envelope de 220 bytes. A API apenas admite e consulta evidência. O Agent ainda usa o spool e os payloads v1 de 276 bytes. O frontend ainda não chama os endpoints v2. O modelo de `v2_assets` está preparado, mas a criação e atualização de ativos ainda dependem da camada de domínio e das instruções específicas do contrato.

## Próxima etapa sujeita à revisão

A próxima etapa é a integração do Agent. Ela deverá adicionar um payload serial v2 compatível, persistir observações assinadas no SQLite antes do ACK, fazer retry idempotente para `/api/v2/agent/observations` e enviar status sem misturar as filas v1 e v2. Depois disso será possível implementar o fluxo de wallet/transação e a UI v2.

[1]: ../PLANO_ETAPA_4_SERVICOS_PRODUTO.md "Plano de evolução dos serviços e do produto"
[2]: ../IMPLEMENTACAO_ETAPA_P0_SMART_CONTRACT_V2.md "Implementação da fundação do smart contract v2"
[3]: ../../schemas/openapi.yaml "Contrato OpenAPI do Lastro"

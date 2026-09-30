# Implementação do reconciliador Solana/PostgreSQL

**Projeto:** Lastro
**Etapa:** R0/R1 — reconciliação durável v1 e observação v2
**Status:** implementado e validado em compilação e testes unitários

## Resultado

A API agora inicia um worker de reconciliação depois de conectar ao PostgreSQL e aplicar todas as migrations. O worker procura eventos legados que já foram submetidos a uma transação Solana, cria jobs idempotentes e tenta finalizar a projeção PostgreSQL somente depois de repetir as mesmas verificações criptográficas e canônicas do endpoint síncrono.

A confirmação HTTP e a confirmação em background usam a função `reconcile_finalized_event`. Isso evita que o caminho automático use regras mais permissivas do que o caminho manual.

## Garantias implementadas

- A fila é persistida em PostgreSQL e sobrevive a reinícios da API.
- Os workers usam `FOR UPDATE SKIP LOCKED`, portanto múltiplas réplicas podem processar lotes sem disputar o mesmo job.
- Cada job tem lease de 30 segundos. Um lease vencido volta para `RETRY`.
- Falhas transitórias de PostgreSQL, RPC ou rate limit retornam para retry com backoff exponencial limitado.
- Conflitos de assinatura, transação, identidade, predecessor ou projeção vão para `QUARANTINED`.
- Jobs concluídos não podem regredir para estados anteriores.
- A identidade do job e o hash-alvo são imutáveis.
- O reprocessamento de um evento já finalizado confirma novamente o estado terminal em vez de aplicar uma segunda transição.
- O worker não marca uma projeção como concluída apenas porque encontrou uma linha no banco; ele exige a transação exata e o estado canônico final na Solana.
- O fluxo v2 `record_observation` usa o mesmo princípio: a API constrói o envelope exato, valida a transação confirmada, exige a transação finalizada e compara `EventAnchor` e `AssetState` antes de atualizar PostgreSQL.

## Arquivos alterados

- `services/api/migrations/0011_reconciliation_jobs.sql`: tabela de jobs, índices e guardas de ciclo de vida.
- `services/api/src/repository/reconciliation.rs`: claim, lease, retry, quarentena e requeue explícito.
- `services/api/src/reconciliation.rs`: worker periódico e política de processamento.
- `services/api/src/routes/events.rs`: função compartilhada entre confirmação HTTP e reconciliação automática.
- `services/api/src/routes/domain_v2.rs`: endpoints de transaction-data, submit, confirm e adapter finalizado de observações v2.
- `services/api/src/solana/v2_transaction_builder.rs`: instruções canônicas v2 e contas PDA.
- `services/api/src/solana/rpc.rs`: decoders estritos de `ProtocolConfigV2`, `EventAnchor` e `AssetState`.
- `services/api/src/repository/domain_v2.rs`: transições de status e projeção transacional de assets.
- `services/api/src/main.rs`: inicialização do worker após migrations e RPC.
- `services/api/src/lib.rs` e `services/api/src/repository/mod.rs`: composição dos módulos.

## Limite conhecido desta etapa

O adapter automático processa `LEGACY_EVENT`, que corresponde ao fluxo de `StationEvent` v1 já integrado à confirmação Solana e à projeção `animals/events`.

Para `DOMAIN_EVENT`, o corte R1 cobre somente `ObservationRecorded` admitido pela rota `/api/v2/agent/observations` e ancorado por `record_observation` do programa v2. O caminho possui endpoints de preparação, submissão e confirmação. Depois de uma transação `SUBMITTED`, o worker valida a transação finalizada, o `EventAnchor` e o `AssetState`, e atualiza a projeção de asset na mesma transação PostgreSQL do status do evento.

Outros tipos de evento v2, incluindo transformações, abate, cortes, subprodutos, perdas, expedição e recall, continuam fora deste adapter. Eles permanecem em quarentena até que cada instrução tenha uma prova on-chain específica e uma projeção transacional correspondente.

## Validação executada

- `cargo check --locked -p lastro-api`: passou.
- `cargo test --locked -p lastro-api --lib`: passou com 19 testes.
- `cargo fmt --all -- --check`: passou.

A validação de integração que requer PostgreSQL real e uma transação Solana finalizada ainda precisa ser executada no ambiente de staging. Os testes unitários não substituem esse cenário.

## Operação

Para investigar um job, consultar `reconciliation_jobs` filtrando por `status`, `target_hash` e `last_error`. Um job `QUARANTINED` só deve voltar para a fila depois da análise da causa. O requeue deve ser uma ação explícita de operador, não uma rotina automática que possa esconder adulteração ou erro de configuração.

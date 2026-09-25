# Implementação da Etapa P0 — Smart Contract Solana v2

**Projeto:** Lastro
**Escopo:** fundação do programa v2 para rastreabilidade bovina, intents, prova física e ancoragem de eventos
**Estado:** implementado e pronto para revisão; a execução LiteSVM permanece pendente por dependência ausente no ambiente local

## Resultado

A Etapa P0 criou um programa Anchor separado do programa v1. O programa v1 não foi substituído nem teve o seu `StationEvent` de 276 bytes alterado. O v2 utiliza um envelope de domínio independente, com 220 bytes, versionamento explícito, identificadores de 32 bytes, janela temporal e hashes com separação de domínio.

A identidade local declarada para o v2 é `7H5tixrcDMrAFGhbbXJ2sTy9sYPmezQKexhJ6FMBvD8F`. O arquivo de chave privada permanece fora do controle de versão, em `chain/target/deploy/lastro-v2-keypair.json`. Essa chave deve ser armazenada em cofre seguro antes de qualquer deployment real. O program ID não deve ser trocado depois que o deployment for usado por clientes ou documentos de rastreabilidade.

## Componentes implementados

### Protocolo compartilhado

O crate `lastro-protocol` agora expõe `v2`. Foram implementados os enums de ativos e eventos, os domínios de hash e o `DomainEventEnvelope`. A codificação usa little-endian e offsets fixos. A validação rejeita schema desconhecido, tipos desconhecidos, identificadores nulos, janelas temporais negativas e janelas maiores que 24 horas.

O mesmo contrato foi implementado no frontend em `apps/web/src/protocol/v2/domainEvent.ts`. O navegador consegue codificar, decodificar e calcular os hashes do mesmo envelope sem depender de uma representação JSON intermediária.

### Programa Anchor v2

Foi criado `chain/programs/lastro-v2` como programa independente. A configuração `ProtocolConfigV2` vincula o deployment aos registries de Station, facilities, parties e documentos. Cada registry tem uma PDA raiz com o `deployment_id` gravado.

O estado `AssetState` representa o ativo genérico que poderá ser um animal, lote, carcaça, lote de cortes, produto, pacote, subproduto ou remessa. A conta mantém custodian, raízes de pai e linhagem, peso disponível, sequência, versão de estado, hash do último evento e reserva de intent.

Foram implementados os registros de Station, facility e party. A Station só é aceita quando a chave P-256 comprimida é válida e quando o `station_id` coincide com `SHA-256("LASTRO_STATION\\0" || pubkey33)`.

### Intents e replay

O `IntentState` possui actor, nonce, versão esperada, hash do payload e expiração. A implementação inclui criação, cancelamento pelo actor, expiração depois do prazo e consumo one-shot. Uma intent consumida, cancelada ou expirada não pode ser usada novamente.

### Prova física e EventAnchor

A instrução `record_observation` valida o envelope v2, exige que a Station esteja ativa e dentro da validade, verifica a posição da instrução Secp256r1 no sysvar de instruções e garante que a assinatura se refere exatamente aos 220 bytes processados pelo programa.

Depois da validação, a instrução atualiza atomicamente o `AssetState` e cria um `EventAnchor` imutável. O predecessor, a versão de estado, a Station, o deployment, o subject e a janela temporal são comparados antes da alteração. A tentativa de reutilizar o mesmo `event_id` encontra a mesma PDA e falha sem criar uma segunda âncora.

## Arquivos principais adicionados ou alterados

| Área | Arquivos | Finalidade |
|---|---|---|
| Programa v2 | `chain/programs/lastro-v2/` | Programa Anchor, estados, instruções e verificação |
| Protocolo | `crates/lastro-protocol/src/v2/` | Contrato binário e hashes compartilhados |
| Testes de protocolo | `crates/lastro-protocol/tests/envelope_v2.rs` | Round-trip, tamanhos, enums, janela e domínio de hash |
| Frontend | `apps/web/src/protocol/v2/domainEvent.ts` | Codec TypeScript de produção |
| Teste frontend | `apps/web/tests/protocol/v2/envelope.test.ts` | Compatibilidade do layout no navegador |
| Bootstrap | `scripts/bootstrap_program_id_v2.sh` | Geração, verificação e proteção do program ID |
| Configuração | `chain/Anchor.toml`, `Makefile` | Registro do v2 e comandos reproduzíveis |
| Fixture | `schemas/fixtures/v2/domain-event-envelope.json` | Vetor legível para integração entre linguagens |
| Harness | `chain/programs/lastro-v2/tests/` | Cenários LiteSVM de autorização, intents e observação |

## Validações executadas

| Verificação | Resultado |
|---|---|
| `cargo check --offline --manifest-path chain/Cargo.toml -p lastro-v2` | Passou |
| `cargo test --locked -p lastro-protocol` | Passou |
| `cargo test --locked -p lastro-protocol --test envelope_v2` | Passou: 5 testes |
| `cargo fmt --all -- --check` | Passou |
| `git diff --check` | Deve ser executado no gate final da branch |
| `cargo test -p lastro-v2` com LiteSVM | Pendente: `solana-program-runtime v4.2.2` não está no cache e o download ficou bloqueado pelo limite do ambiente |
| Build SBF/Anchor | Pendente: o CLI `anchor` não está instalado nesta máquina; `cargo check` valida o crate host, não o artefato SBF |

## Limitações que continuam explícitas

A Etapa P0 ainda não integra essas novas instruções à API Rust, ao Agent, ao firmware ou às telas do produto. Também não implementa transformação, desossa, linhagem pai-filho, conservação de massa, lotes de cortes, pacotes, remessas, recalls ou consulta pública v2. Esses itens são a próxima etapa e não devem ser considerados prontos apenas porque as contas base já existem.

A `record_observation` exige autoridade de configuração como payer e Station proof. O modelo final de autorização por facility, party, operador e documentação oficial ainda será implementado na camada de serviço e nas instruções de transformação. O armazenamento de manifestos, documentos regulatórios e dados detalhados permanece fora da cadeia; a blockchain guarda estado canônico e compromissos verificáveis.

## Próxima etapa sujeita à aprovação

A próxima etapa deve integrar o v2 à API e ao Agent. Ela deverá criar endpoints de registro e consulta de ativos, preparar transações v2, transportar o envelope no spool offline, publicar a observação depois da confirmação da Station e expor uma consulta pública que diferencie estado confirmado em Solana de dados ainda pendentes na API. Somente depois dessa integração deve ser implementada a transformação pós-abate.

## Referências internas

[1]: ../PLANO_ETAPA_3_SMART_CONTRACT.md "Plano da evolução do smart contract Solana v2"
[2]: ../PLANO_ETAPA_4_SERVICOS_PRODUTO.md "Plano de evolução dos serviços e do produto"
[3]: ../PLANO_ETAPA_5_SEGURANCA_ROLLOUT.md "Plano de segurança, governança, validação e rollout"
[4]: ../BACKLOG_IMPLEMENTACAO_SMART_CONTRACT_V2.md "Backlog executável do smart contract v2"

# Etapa 5 — Segurança, governança, validação e rollout

**Projeto:** Lastro
**Objetivo:** definir os controles necessários para transformar a arquitetura v2 em um sistema operável com segurança, privacidade, auditoria, recuperação de falhas e piloto real.

## 1. Conclusão executiva

O Lastro pode avançar para um piloto somente quando deixar de ser tratado como uma demonstração de transação e passar a ser tratado como um sistema de registro operacional com múltiplas autoridades.

A segurança do produto não depende apenas da assinatura P-256 ou da confirmação Solana. Ela depende de cinco camadas:

```text
hardware e chaves
protocolo e smart contract
API, banco e autorização
operação e governança
privacidade e resposta a incidentes
```

A regra de lançamento é:

> **Nenhum dado crítico pode depender de uma única chave, uma única tabela, um único operador, um único navegador ou uma única fonte de verdade.**

O piloto deve começar com escopo pequeno e controlado. A plataforma deverá conseguir parar novas operações sem perder a capacidade de consultar e auditar o histórico já finalizado.

## 2. O que já existe e o que ainda falta

O projeto atual já possui controles relevantes:

- o evento físico é assinado e verificado contra os bytes exatos;
- sequência e predecessor impedem replay de um estado finalizado;
- o custodiante é verificado pelo programa Solana;
- o PostgreSQL não pode reescrever a evidência criptográfica;
- o Agent possui outbox SQLite durável;
- há retry e quarentena;
- o navegador diferencia `VALID`, `INVALID` e `NOT_CHECKED`;
- o verificador usa trust anchor independente para deployment e autoridade;
- há limites de eventos, RPC e payload;
- a CI executa testes Rust, Python, web, E2E e auditoria de dependências.

Ainda faltam para o produto bovino:

- registry versionado de Stations e facilities;
- rotação e revogação de chaves;
- freshness assinado para eventos v2;
- intents e nonces on-chain;
- autorização por papel e escopo;
- isolamento entre organizações;
- linhagem e recall;
- política de dados públicos e privados;
- backup e restauração testados;
- resposta a incidentes;
- governança para correção, disputa e bloqueio;
- procedimento de operação manual quando Solana, API ou hardware estiverem indisponíveis.

As pendências v1 já registradas em `docs/HARDENING_STATUS.md` não devem ser esquecidas durante a construção do v2. O novo protocolo deve corrigir as limitações de supersessão, freshness e rotação sem declarar que o v1 possui garantias que ele não possui.

## 3. Modelo de ameaça

### 3.1 Ativos que precisam de proteção

| Ativo | Risco principal | Controle prioritário |
|---|---|---|
| Chave da Station | Assinatura falsa de observação | Secure element, registry, validade e revogação |
| Carteira de custodiante | Transferência não autorizada | Challenge, MFA operacional, nonce e monitoramento |
| Credencial do frigorífico | Criação falsa de abate ou produto | Facility registry e segregação de papéis |
| Agent token | Injeção de comandos ou leitura de dados | Escopo por Agent, rotação e rate limit |
| PostgreSQL | Vazamento ou adulteração de projeção | Controle de acesso, append-only, backups e reconciliação |
| EvidencePackage | Tampering ou exposição indevida | Hash, assinatura, criptografia e perfis de acesso |
| Manifesto de transformação | Alteração de entradas, saídas ou perdas | Canonicalização, hash, balanço de massa e anchor |
| Linhagem | Perda de origem ou ciclo | Raízes, arestas imutáveis e validação acíclica |
| Dados pessoais | Reidentificação e exposição | Minimização, pseudonimização e acesso por escopo |
| Consulta pública | Enumeração de animais e produtos | IDs opacos, rate limit e respostas mínimas |
| Recall | Falha em localizar afetados | Grafo reversível, bloqueio e auditoria |

### 3.2 Adversários

O desenho deve considerar:

- operador com carteira roubada;
- Station clonada ou com firmware adulterado;
- Agent comprometido;
- frigorífico tentando registrar uma transformação inexistente;
- usuário tentando registrar o mesmo input duas vezes;
- API comprometida tentando modificar uma projeção;
- administrador tentando apagar uma evidência;
- atacante enumerando RFID, AnimalID ou QR codes;
- fonte oficial indisponível ou retornando resposta divergente;
- operador fazendo upload de documento malicioso;
- insider com acesso a dados de várias organizações;
- comprometimento da chave de autoridade do deployment;
- erro operacional sem intenção maliciosa;
- indisponibilidade prolongada de RPC, banco ou internet.

### 3.3 Garantias que não devem ser alegadas

Mesmo após o piloto, o sistema não deve afirmar sozinho:

- identidade biológica do animal;
- impossibilidade de remoção ou troca física do brinco;
- verdade absoluta do GPS;
- calibração correta da balança sem certificado e procedimento;
- propriedade jurídica do boi ou produto;
- conformidade sanitária sem confirmação da fonte competente;
- validade de documento apenas porque o hash coincide;
- intenção comercial de uma parte que não assinou o ato correspondente.

## 4. Governança de identidades e chaves

### 4.1 Hierarquia de identidades

Separar os seguintes tipos de identidade:

```text
DeploymentAuthority
  -> autoriza configuração do deployment

StationKey
  -> assina observações físicas

ScaleKey
  -> assina pesagens, quando o equipamento suportar

FacilityCredential
  -> autoriza abate, transformação e produção

CustodianWallet
  -> autoriza custódia ou aceite de transferência

AgentCredential
  -> permite transporte autenticado

OperatorIdentity
  -> identifica a pessoa que executou a ação

OfficialSourceKey
  -> identifica resposta de uma fonte integrada
```

Uma dessas identidades não deve ser usada para representar todas as outras.

### 4.2 Ciclo de vida da chave

Toda chave deverá ter:

```text
key_id
owner_type
owner_id
algorithm
public_key
valid_from
valid_until
status
created_by
revoked_at
revocation_reason
replacement_key_id
```

Estados:

```text
PENDING
ACTIVE
SUSPENDED
REVOKED
EXPIRED
RETIRED
```

A verificação histórica deverá usar a validade da chave no momento do evento, e não somente o status atual.

### 4.3 Registro e revogação de Station

Antes de aceitar uma observação v2:

1. o `station_id` deve existir;
2. a chave deve estar registrada;
3. a chave deve ser válida no momento do evento;
4. o firmware conhecido deve coincidir com a política;
5. o dispositivo não pode estar revogado;
6. a assinatura deve ser verificada;
7. o `source_id` deve corresponder ao contexto reservado.

Revogar uma Station não deve invalidar automaticamente eventos históricos já finalizados. Deve impedir novos eventos e sinalizar a credencial como revogada para operações futuras.

### 4.4 Carteiras e facilities

O registro do frigorífico deve indicar:

- identidade técnica da facility;
- papel permitido;
- organização controladora;
- escopo de operação;
- validade da autorização;
- chave ou carteira ativa;
- procedimento de substituição;
- status de suspensão;
- auditor responsável.

A carteira de um frigorífico não deve ter autoridade global sobre todos os animais. A autorização precisa ser limitada por deployment, facility, papel, período e operação.

### 4.5 Segredos

Não armazenar no Git:

- chaves privadas;
- tokens reais;
- frases-semente;
- certificados privados;
- dumps de produção;
- dados reais de produtores;
- coordenadas reais de propriedades usadas em testes.

O ambiente de CI atual usa chaves determinísticas para o sistema local. Essas chaves devem permanecer exclusivas para CI e nunca ser aceitas por um deployment de piloto ou produção.

## 5. Autorização da API

### 5.1 Autenticação não é autorização

O fato de um usuário estar autenticado não autoriza qualquer operação. Cada comando deve avaliar:

```text
quem é o ator
qual papel ele possui
qual organização possui o ativo
qual escopo foi concedido
qual estado atual existe
qual documento é exigido
qual assinatura é necessária
qual operação está sendo tentada
```

### 5.2 RBAC e ABAC

Usar RBAC para o papel geral e ABAC para o contexto.

Exemplo:

```text
Buyer
  pode consultar um produto se:
    recebeu o shipment
    ou está na negociação autorizada
    ou possui escopo de auditoria

Slaughterhouse
  pode finalizar transformação se:
    facility está ativa
    transformação pertence à facility
    entradas foram recebidas
    documento exigido está anexado
    balanço está dentro da tolerância
```

### 5.3 Isolamento entre organizações

Toda query de negócio deve aplicar escopo de organização no repositório, não somente na rota.

Não confiar em:

```rust
WHERE asset_id = $1
```

quando o dado é multi-tenant.

Exigir algo equivalente a:

```sql
WHERE asset_id = $1
  AND organization_id = $2
  AND access_scope @> $3
```

Criar testes que tentem consultar:

- animal de outra fazenda;
- produto de outro frigorífico;
- documento de outro comprador;
- expedição de outro transportador;
- recall de uma organização sem escopo.

### 5.4 Operações sensíveis

Exigir step-up authorization para:

- mudar custódia;
- confirmar abate;
- finalizar transformação;
- liberar quality hold;
- abrir recall;
- revogar Station;
- alterar configuração de facility;
- exportar EvidencePackage restrito;
- corrigir um evento de negócio.

A autorização deve estar vinculada ao payload exato. Um challenge genérico para “confirmar operação” não é suficiente.

## 6. Privacidade e LGPD

A LGPD se aplica ao tratamento digital de dados pessoais. O produto deve aplicar finalidade, necessidade, transparência, segurança e responsabilização desde o desenho. A legislação também prevê direitos dos titulares e obrigações de segurança para o controlador e os operadores. [1]

### 6.1 Dados potencialmente pessoais

Tratar como potencialmente pessoal:

- nome de produtor, comprador e operador;
- CPF/CNPJ;
- carteira associada a pessoa;
- coordenada de propriedade;
- histórico de presença;
- telefone e e-mail;
- documento fiscal;
- logs que permitam correlação;
- hash de um identificador quando a organização possui a tabela de referência;
- localização temporal de transporte.

### 6.2 Nunca colocar na blockchain pública

Não armazenar diretamente em conta ou evento público:

```text
CPF
CNPJ
nome legal
endereço
latitude/longitude precisa
telefone
email
preço
contrato completo
nota fiscal completa
GTA completa
laudo completo
```

Usar identificadores opacos e commitments. O dado detalhado fica off-chain, criptografado e sujeito a política de acesso.

### 6.3 Perfis de exposição

O sistema deve definir três perfis:

```text
PUBLIC
  prova mínima e status público

AUTHORIZED
  histórico, documentos e linhagem conforme escopo

INTERNAL_LEGAL
  identidade, logs, incidentes, disputas e chaves de correlação
```

Cada endpoint precisa declarar o perfil. Não permitir que uma mudança no modelo interno apareça automaticamente na resposta pública.

### 6.4 Retenção e eliminação

Dados on-chain são difíceis ou impossíveis de remover. Portanto:

- dados pessoais não devem ser gravados on-chain;
- a cadeia deve armazenar somente commitments ou IDs não reidentificáveis por terceiros;
- o PostgreSQL deve possuir política de retenção;
- documentos devem ter prazo por finalidade;
- backups devem ter prazo e controle de acesso;
- referências off-chain devem poder ser desassociadas quando houver base legal;
- preservação por auditoria, disputa ou obrigação legal deve ser registrada;
- o sistema deve responder a solicitações de titulares conforme orientação jurídica.

Um pedido de eliminação off-chain não deve alterar o hash on-chain. A aplicação deve comunicar que a prova criptográfica permanece, enquanto o conteúdo detalhado controlado pode ser removido ou restringido conforme a base legal.

### 6.5 Incidentes

Criar um plano de resposta para:

```text
vazamento de dados pessoais
chave de Station comprometida
token Agent comprometido
carteira de facility comprometida
manifesto incorreto
recall sanitário
RPC falso ou endpoint adulterado
corrupção de banco
ransomware em storage
```

A Resolução CD/ANPD nº 15/2024 disciplina a comunicação de incidentes de segurança e o registro das informações relevantes. O prazo e a necessidade de comunicação devem ser avaliados pelo controlador com orientação jurídica conforme o risco e o caso concreto. [2]

## 7. Integridade de dados e operação

### 7.1 Append-only

Criar guards para:

- impedir `DELETE` de eventos finalizados;
- impedir alteração de payload e hash;
- impedir mudança de pai ou filho depois do anchor;
- impedir alteração retroativa de peso;
- impedir alteração retroativa de documento;
- impedir redução silenciosa de quantidade;
- impedir retirada de um asset já consumido;
- impedir fechamento sem manifestação de saída;
- impedir recall sem motivo e autoridade.

Correção significa novo evento:

```text
WEIGHT_OBSERVED 487300 g
WEIGHT_CORRECTED 490000 g
reason = calibration adjustment
```

O primeiro valor continua visível para auditoria.

### 7.2 Reconciliação

O reconciliador deve comparar:

```text
PostgreSQL projection
Solana AssetState
Solana EventAnchor
Solana TransformationAnchor
EvidencePackage
Document status
```

Divergências devem gerar:

- alerta;
- incident id;
- bloqueio seletivo quando necessário;
- preservação das duas versões;
- operação manual de resolução;
- relatório de causa.

Nunca corrigir uma divergência sobrescrevendo a linha original.

### 7.3 Relógio e freshness

O v2 deve ter:

- `observed_at` assinado;
- contador monotônico por dispositivo;
- `challenge_id` ou nonce;
- `expires_at`;
- janela tolerada;
- sincronização de relógio registrada;
- política para clock inválido.

O v1 deve continuar sendo descrito como sem prova de tempo de parede. Não usar o timestamp de recepção da API como se fosse o momento físico da leitura.

## 8. Backup, restauração e continuidade

### 8.1 PostgreSQL

Configurar:

- backup completo periódico;
- WAL/PITR quando o ambiente suportar;
- cópia criptografada fora do host;
- retenção definida;
- teste de restauração periódico;
- checksum do backup;
- controle de acesso separado;
- conta de recuperação sem acesso desnecessário ao app.

### 8.2 Object storage

EvidencePackage, documentos e manifestos devem possuir:

- versão;
- hash;
- criptografia em repouso;
- retenção;
- object lock quando necessário;
- política de acesso;
- cópia independente;
- verificação periódica de integridade.

### 8.3 Agent

O SQLite do Agent deve ser recuperável por:

- cópia local segura;
- exportação de quarentena;
- reprocessamento idempotente;
- procedimento de troca de máquina;
- retenção da evidência original;
- não reutilização de observação expirada como nova.

### 8.4 Solana

A Solana não substitui backup de documentos e projeções. O sistema deve preservar:

- transações;
- deployment manifest;
- program ID;
- deployment ID;
- autoridade confiável;
- registry de chaves;
- versão do schema;
- configuração do verificador.

## 9. Migração e rollback

### 9.1 Migração sem destruição

A migração v1 → v2 deve ser aditiva:

1. criar novas tabelas;
2. criar `AssetStateV2` para animais elegíveis;
3. registrar `legacy_animal_id` e último hash v1;
4. validar que a projeção v1 corresponde ao RPC;
5. criar evento de migração;
6. ancorar a relação v1 → v2;
7. ativar novos eventos apenas depois da confirmação;
8. manter consulta v1;
9. registrar versão do migrador;
10. produzir relatório de ativos migrados e rejeitados.

### 9.2 Ativos não migráveis

Um animal não deve ser migrado se:

- a projeção não corresponde à Solana;
- existe evento não finalizado;
- o RFID atual está em conflito;
- o `last_event_hash` está ausente ou inválido;
- a autoridade do deployment não foi validada;
- a identificação visual está duplicada;
- há disputa aberta que impede a política de migração.

Esses ativos ficam em `MIGRATION_BLOCKED` e exigem reconciliação manual.

### 9.3 Rollback de aplicação

Rollback de código deve ser possível antes de criar novos eventos incompatíveis. Rollback de chain não deve ser tratado como rollback de banco.

Se o v2 apresentar problema:

```text
1. pausar novas escritas v2
2. manter consultas e verificação
3. manter captura física local em modo seguro ou offline
4. preservar outbox e manifestos
5. corrigir código ou regra
6. executar migração/reprocessamento em staging
7. reabrir somente após aprovação
```

Não apagar eventos v2 finalizados para “voltar” a uma versão antiga.

### 9.4 Rollback de manifesto

Um manifesto finalizado incorreto deve receber:

```text
TRANSFORMATION_CORRECTED
```

ou:

```text
TRANSFORMATION_VOIDED
```

Os outputs afetados devem entrar em `QUALITY_HOLD` ou `RECALLED` conforme o caso. A operação corretiva deve apontar para o manifesto original.

## 10. CI e gates obrigatórios

A CI atual já verifica Rust, web, E2E, vetores e dependências. Para v2, adicionar gates separados por risco.

### 10.1 Gate de protocolo

```text
cargo fmt --check
cargo clippy -D warnings
cargo test --workspace
cross-language vectors
schema compatibility
protocol version rejection
```

### 10.2 Gate de smart contract

```text
anchor build
LiteSVM tests
PDA derivation fixtures
account layout fixtures
transaction size benchmarks
nonce/replay tests
mass-balance tests
lineage consumption tests
facility authorization tests
recall blocking tests
```

### 10.3 Gate de banco

```text
fresh migration
upgrade from v1 fixture
rollback rehearsal in disposable database
append-only triggers
concurrent finalization
isolation tests
backup restore test
```

### 10.4 Gate web

```text
format
TypeScript typecheck
unit tests
parser fuzz cases
public DTO privacy tests
lineage depth limits
QR public route
recall presentation
NOT_CHECKED semantics
```

### 10.5 Gate de dependências e segredos

Manter auditoria Rust e npm, mas acrescentar:

- secret scanning no histórico e no working tree;
- SBOM de Rust, Node e containers;
- verificação de imagens por digest;
- scan de container;
- licença de dependências;
- alerta para exceções RustSec expiradas;
- revisão obrigatória de qualquer nova exceção;
- teste que falha se segredo real aparecer em fixture público.

As exceções atuais do LiteSVM/RustSec devem continuar restritas aos advisories já documentados. Não aumentar a lista para fazer o gate passar.

### 10.6 Gate de compatibilidade

Adicionar ao CI:

```text
StationEvent v1 vectors unchanged
serial LSTR v1 vectors unchanged
EvidencePackage v1 parser unchanged
old verifier fixtures unchanged
v1 demo flow unchanged
v2 parser rejects unknown schema
v2 verifier does not treat v1 as v2
```

## 11. Testes de aceitação do piloto

### 11.1 Fluxo positivo

O piloto deverá executar:

```text
Animal A
  -> identificação
  -> pesagem
  -> localização generalizada
  -> movimento
  -> recebimento no frigorífico
  -> abate
  -> carcaça
  -> transformação
  -> corte
  -> embalagem
  -> expedição
  -> recebimento
  -> consulta pública
```

Em cada etapa, a equipe deve guardar:

- id da operação;
- operador;
- dispositivo;
- documento;
- status;
- hash;
- assinatura;
- transação;
- tempo de processamento;
- exceções.

### 11.2 Falhas obrigatórias

O piloto deve simular:

- RFID indisponível;
- balança sem leitura estável;
- GPS indisponível;
- Agent sem internet;
- API indisponível;
- RPC indisponível;
- transaction expired;
- input já consumido;
- manifesto fora da tolerância;
- facility suspensa;
- credencial revogada;
- documento vencido;
- package enviado duas vezes;
- recall aberto antes da expedição;
- recall aberto depois da expedição;
- divergência de quantidade na chegada;
- alteração do manifesto;
- tentativa de consulta cross-tenant;
- perda do computador do Agent;
- rotação de chave da Station.

### 11.3 Critérios de sucesso

O piloto só será aprovado se:

- nenhuma transição inválida for finalizada;
- nenhuma entrada for consumida duas vezes;
- toda saída tiver linhagem consultável;
- todo produto puder retornar ao animal ou lote de origem conforme a granularidade definida;
- recall encontrar todos os afetados dentro do limite operacional;
- uma queda de rede não perder evidência local;
- uma repetição não criar duplicidade;
- dados públicos não revelarem PII;
- o verificador independente concordar com o estado canônico;
- divergências forem detectadas e não ocultadas;
- a equipe conseguir operar em contingência;
- a restauração de backup for demonstrada.

## 12. Operação e runbooks

Criar runbooks versionados para:

```text
RUNBOOK_STATION_COMPROMISED.md
RUNBOOK_AGENT_LOST.md
RUNBOOK_RPC_UNAVAILABLE.md
RUNBOOK_DATABASE_RESTORE.md
RUNBOOK_TRANSFORMATION_STUCK.md
RUNBOOK_MASS_BALANCE_DIVERGENCE.md
RUNBOOK_RECALL.md
RUNBOOK_KEY_ROTATION.md
RUNBOOK_FACILITY_SUSPENSION.md
RUNBOOK_DATA_INCIDENT.md
RUNBOOK_MIGRATION_BLOCKED.md
```

Cada runbook deve informar:

- como detectar;
- quem pode declarar incidente;
- quais operações bloquear;
- quais dados preservar;
- como revogar credencial;
- como recuperar;
- como validar a recuperação;
- como registrar a decisão;
- quando comunicar parceiros e autoridades;
- como reabrir o serviço.

## 13. Observabilidade e SLOs do piloto

Não definir SLOs de escala antes de medir o processo real. Para o piloto, medir:

```text
p95 de ingestão de observação
p95 de finalização on-chain
tempo de reconciliação
idade máxima do outbox
quantidade de quarentena
falhas de balanço de massa
tempo de consulta de linhagem
tempo de localização de recall
erro de integração documental
percentual de operações manuais
```

Alertas iniciais:

- outbox acima do limite;
- evento submetido sem finalização;
- transformação aberta próxima da expiração;
- divergência PostgreSQL/Solana;
- chave próxima do vencimento;
- facility suspensa com tentativa de escrita;
- recall sem reconhecimento;
- consulta pública com padrão de enumeração;
- falha de backup;
- aumento de payload rejeitado.

## 14. Validação de mercado e operação

A documentação atual exige conversas estruturadas com produtores, participante de rastreabilidade/certificação, frigorífico/exportação/compliance e participante de risco, seguro ou financiamento. Esse requisito deve ser mantido.

Para o piloto, cada conversa ou operação deve registrar:

- processo atual;
- falha concreta;
- frequência;
- custo ou risco;
- sistemas existentes;
- documento aceito;
- quem compra a informação;
- objeções;
- interesse real em piloto;
- responsabilidade de cada parte.

O objetivo não é confirmar que a ideia parece interessante. É descobrir quais dados a cadeia realmente aceita, quem é responsável por cada declaração e quais documentos não podem ser substituídos.

## 15. Governança de mudanças

Toda alteração de protocolo deve possuir:

```text
RFC ou decisão técnica
versão de schema
impacto em v1/v2
vetores atualizados
plano de migração
plano de rollback de aplicação
impacto de privacidade
impacto de custos
testes de compatibilidade
aprovação técnica
aprovação operacional
```

Alterações que exigem revisão ampliada:

- formato de evento;
- layout de conta Solana;
- significado de status;
- autoridade de facility;
- campos públicos;
- regras de recall;
- fonte oficial integrada;
- política de retenção;
- chave de deployment;
- alteração de tolerância de massa.

Não permitir que uma mudança de UI altere silenciosamente a semântica do protocolo.

## 16. Critérios de lançamento por ambiente

### Desenvolvimento

Pode usar:

- dados sintéticos;
- chaves determinísticas;
- RPC local;
- Station simulada;
- imagens locais;
- logs detalhados.

Não pode usar:

- dados reais sem autorização;
- chave de produção;
- endpoint público com credencial real;
- documento real em fixture versionado.

### Staging

Deve usar:

- deployment separado;
- credenciais separadas;
- database isolado;
- storage isolado;
- facility fictícia ou autorizada;
- backups testáveis;
- observabilidade próxima da produção.

### Piloto

Deve possuir:

- participantes identificados;
- contrato ou termo operacional;
- escopo de dados;
- procedimento de contingência;
- suporte;
- janela de operação;
- canal de incidente;
- critério de pausa;
- revisão após cada lote.

### Produção

Somente depois de:

- auditoria técnica;
- revisão jurídica e de privacidade;
- chaves provisionadas com procedimento de rotação;
- backup restaurado;
- recall ensaiado;
- verificador independente publicado;
- monitoramento ativo;
- responsabilidades definidas;
- fontes oficiais integradas ou explicitamente marcadas como não integradas.

## 17. Critérios de bloqueio do lançamento

O lançamento deve ser bloqueado se qualquer item abaixo ocorrer:

- sistema grava dado pessoal em estado on-chain;
- facility pode finalizar transformação sem autorização;
- mesma entrada pode ser consumida duas vezes;
- recall não consegue localizar descendentes;
- banco pode avançar projeção antes da finalização Solana;
- transação antiga pode vencer supersessão sem nonce on-chain;
- chave revogada continua autorizando novos eventos;
- consulta pública permite enumeração de ativos;
- backup não pode ser restaurado;
- verificador apresenta `NOT_CHECKED` como válido;
- evento corrigido apaga a versão anterior;
- integração oficial é simulada sem identificação clara;
- payload do frigorífico pode ser alterado depois do anchor;
- o piloto não possui procedimento manual para indisponibilidade.

## 18. Ordem final para sair da demonstração

A sequência recomendada é:

1. congelar contratos v1;
2. concluir protocolo v2;
3. implementar banco e domínio off-chain;
4. implementar verificador local;
5. implementar autorização e isolamento;
6. implementar facility/station registries;
7. implementar anchors, intents e nonces;
8. executar testes concorrentes e de recuperação;
9. executar migração sintética;
10. restaurar backup em ambiente limpo;
11. executar fluxo completo com simulador;
12. testar recall;
13. testar rotação e revogação;
14. revisar privacidade e exposição pública;
15. executar piloto pequeno;
16. revisar métricas e falhas;
17. corrigir sem apagar histórico;
18. expandir para mais participantes.

## 19. Parecer final da Etapa 5

O maior risco do projeto não é a falta de blockchain. É lançar uma cadeia de custódia com autoridade, privacidade e operação ainda indefinidas.

A arquitetura deve tratar a Solana como uma camada de invariantes e prova. O PostgreSQL deve tratar o detalhe operacional. O Agent deve proteger a continuidade offline. O frigorífico deve responder pelos eventos de produção que assina. O verificador deve declarar exatamente o que comprovou e o que não comprovou.

O sistema estará pronto para piloto quando conseguir fazer quatro coisas simultaneamente:

```text
preservar o histórico
bloquear transições inválidas
recuperar-se de falhas
explicar seus limites
```

A aprovação do piloto não deve ser baseada apenas em uma transação finalizada. Ela deve ser baseada na capacidade de operar, auditar, corrigir por novos eventos, bloquear produtos, proteger dados e recuperar o serviço sem inventar fatos.

## Referências

[1]: https://www.planalto.gov.br/ccivil_03/_ato2015-2018/2018/lei/l13709.htm "Lei nº 13.709/2018 — Lei Geral de Proteção de Dados Pessoais"
[2]: https://www.in.gov.br/en/web/dou/-/resolucao-cd/anpd-n-15-de-24-de-abril-de-2024-556243024 "Resolução CD/ANPD nº 15/2024 — comunicação de incidentes de segurança"
[3]: https://www.planalto.gov.br/ccivil_03/_ato2007-2010/2009/lei/L12097.htm "Lei nº 12.097/2009 — rastreabilidade na cadeia produtiva de carnes bovinas e bubalinas"
[4]: ../docs/SECURITY.md "Modelo de segurança atual do Lastro"
[5]: ../docs/HARDENING_STATUS.md "Status de hardening do Lastro"
[6]: ../docs/VALIDATION.md "Validação de mercado do Lastro"
[7]: ../docs/PLANO_ETAPA_3_SMART_CONTRACT.md "Plano da Etapa 3 — smart contract Solana"
[8]: ../docs/PLANO_ETAPA_4_SERVICOS_PRODUTO.md "Plano da Etapa 4 — serviços e produto"

**Autor:** Manus AI

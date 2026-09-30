# Revisão sênior de engenharia do projeto Lastro

**Projeto analisado:** Lastro  
**Base:** árvore de trabalho atual sobre `b8546f3`, com alterações locais não commitadas  
**Objetivo desta revisão:** avaliar organização, arquitetura, ideia, configuração, correção, segurança, confiabilidade, operação e evolução futura sob a perspectiva de engenharia de software sênior.

## 1. Veredito executivo

O Lastro tem uma arquitetura de protótipo tecnicamente acima da média. Ele não trata blockchain como um banco mágico nem RFID como prova absoluta de identidade. A proposta separa evidência física, transporte, coordenação, autorização do custodiante, projeção local e estado canônico on-chain.

A estrutura central é coerente:

```text
Station física
  -> evento binário assinado
  -> Agent durável
  -> API de admissão
  -> PostgreSQL como projeção
  -> carteira do custodiante
  -> transação Solana verificada
  -> estado canônico
  -> verificador independente
```

A atualização recente melhorou pontos importantes. Agora a criação de captura exige um challenge assinado pela carteira correta. O Agent possui timeout e quarentena. O navegador retém o block height da transação assinada. A recaptura de `REIDENTIFY` é explícita. O verificador exige uma autoridade de deployment configurada independentemente.

Mesmo assim, o sistema **ainda não é um produto físico de produção**. O principal motivo não é a Solana. O maior risco está na fronteira entre o mundo físico e o software: o driver RFID real ainda não está implementado, a chave Station não possui um ciclo completo de provisionamento/rotação/revogação, a Station não possui journal persistente e o modelo não evita completamente a concorrência entre uma transação antiga e uma recaptura nova.

Minha avaliação é:

| Dimensão | Avaliação |
|---|---|
| Clareza arquitetural | Forte |
| Separação de responsabilidades | Forte |
| Contratos binários e compatibilidade | Forte |
| Segurança contra adulteração de software | Boa para protótipo |
| Segurança da origem física | Incompleta |
| Recuperação de falhas | Boa, mas ainda centrada no navegador e no Agent |
| Operação contínua | Incompleta |
| Escalabilidade | Não demonstrada |
| Multi-tenancy e governança | Não implementadas de forma suficiente |
| Prontidão para produção | Não atingida |

A conclusão mais importante é esta:

> **O projeto possui um excelente esqueleto de sistema verificável, mas ainda precisa fechar as garantias de dispositivo, reconciliação, autorização operacional, revogação e observabilidade antes de ser tratado como infraestrutura de produção.**

## 2. A ideia do sistema e o que ela realmente prova

O Lastro quer responder a uma pergunta específica:

> “Este estado digital de identidade e custódia pode ser explicado por uma sequência de observações físicas assinadas, autorizadas e finalizadas de forma verificável?”

Ele não responde, sozinho, às perguntas seguintes:

- O animal é biologicamente aquele animal?
- A pessoa que operou a Station tinha intenção legítima?
- A tag RFID não foi clonada ou removida?
- O custodiante possui propriedade legal do animal?
- A transferência foi aceita pelo destinatário?
- O leitor RFID estava instalado corretamente?
- A placa física estava executando firmware íntegro?
- A observação ocorreu em um horário específico?

Essa diferença precisa permanecer explícita na comunicação do produto. O sistema prova uma cadeia de evidência digital e autoridade criptográfica. Ele não transforma um RFID em identidade biológica nem uma assinatura em contrato jurídico.

A ideia é boa porque não tenta colocar toda a confiança em um único componente. A Station não pode inventar um destino de custódia sozinha. O Agent não pode trocar os bytes depois que foram assinados. A API não pode avançar a projeção sem finalização. A carteira não pode autorizar uma transação cujo signer não corresponda ao custodiante exigido. O programa Solana não aceita apenas um JSON: ele valida bytes, instruções, contas, predecessor e estado.

O ponto fraco é que essa cadeia só é tão forte quanto a menor garantia. Se a Station aceita uma leitura física sem driver confiável, toda a camada posterior pode estar correta e ainda assim a origem factual ser fraca.

## 3. Modelo mental da arquitetura

### 3.1 Camadas e autoridades

A arquitetura possui seis camadas principais:

| Camada | Responsabilidade | O que ela não deve fazer |
|---|---|---|
| Station | Observar RFID, construir `StationEvent` e assinar os 276 bytes. | Não deve escolher arbitrariamente o novo RFID nem decidir custódia fora do comando. |
| Agent | Transportar, validar, persistir e reenviar evidência. | Não deve editar bytes nem transformar erro terminal em retry infinito. |
| API | Criar intents, admitir evidência, consultar Solana e coordenar estados. | Não deve ser tratada como autoridade final de custódia. |
| PostgreSQL | Armazenar contexto, evidência aceita e projeção. | Não deve avançar o estado canônico por conta própria. |
| Carteira | Autorizar intent e assinar a transação Solana. | Não deve assinar uma mensagem diferente da mostrada/validada pelo navegador. |
| Solana/programa | Aplicar invariantes canônicos e manter estado final. | Não deve confiar em campos off-chain sem verificar o envelope on-chain. |

O navegador tem duas funções diferentes. No modo operador, ele orquestra a operação. No modo verificador, ele tenta agir como uma parte independente que não confia no resultado calculado pelo backend.

Essa distinção é importante. O navegador operador não é uma autoridade confiável por si só. Ele é um cliente que ajuda a montar e verificar a transação antes de pedir assinatura. O programa on-chain continua sendo a barreira final.

### 3.2 Fluxo de estados

Existem pelo menos quatro máquinas de estado sobrepostas:

```text
Capture:
PENDING -> DISPATCHED -> EVIDENCE_ACCEPTED -> terminal
                          |                    |
                          +--------------------+

Event:
EVIDENCE_ACCEPTED -> SUBMITTED -> FINALIZED
        |
        +-------> REJECTED

Agent outbox:
LOCAL -> SERVER -> FINALIZED
   |        |
   +------> QUARANTINED

Station:
IDLE -> WAIT_RFID -> BUILD_EVENT -> SIGN -> WAIT_ACK -> IDLE
```

A arquitetura está correta ao separar essas máquinas. Uma captura física aceita não é a mesma coisa que um evento admitido. Um evento admitido não é a mesma coisa que uma transação submetida. Uma transação submetida não é a mesma coisa que estado finalizado.

O problema atual é de coordenação entre essas máquinas. Elas vivem em processos e bancos diferentes, com falhas diferentes:

- Station pode perder energia;
- Agent pode morrer antes ou depois do ACK;
- API pode aceitar evidência e cair;
- RPC pode confirmar uma transação sem a API registrar;
- uma transação antiga pode continuar válida enquanto o backend prepara uma recaptura;
- o navegador pode ser fechado em qualquer etapa.

A solução atual cobre vários cenários, mas ainda não existe um reconciliador central que observe a Solana e corrija estados intermediários sem depender do navegador.

## 4. Organização do código

### 4.1 `crates/lastro-protocol`

Este é o módulo mais importante do ponto de vista de arquitetura. Ele deve ser tratado como uma biblioteca de protocolo, não como uma biblioteca de domínio da API.

Ele concentra:

- `Action` e as três transições;
- IDs e arrays de tamanho fixo;
- offsets do evento;
- encode/decode do `StationEvent`;
- regras de semântica e sucessor;
- canonicalização e hash do RFID;
- derivação do StationID;
- verificação da assinatura P-256;
- validação do `EvidencePackage`.

A decisão de manter esse crate sem HTTP, banco, RPC ou porta serial é correta. Ela reduz o risco de a regra canônica depender de detalhes de infraestrutura.

O principal risco futuro é drift entre as implementações. A regra existe em Rust, C, Python, TypeScript e Rust on-chain. Vetores congelados ajudam, mas não substituem testes de propriedade e geração automática de bindings. Quanto mais a regra evoluir, maior a chance de uma linguagem aceitar algo que outra rejeita.

### 4.2 `firmware/station`

O firmware tem boa separação interna:

- `station.c`: máquina de estados;
- `event.c`: layout dos bytes;
- `signer.c`: chave e assinatura;
- `transport.c`: framing e CRC;
- `runtime.c`: integração com o transporte;
- `rfid.c`: fronteira do leitor.

A máquina de estados é clara e segura para um protótipo. Ela não assina sem observar um RFID. Ela aceita uma única observação. Ela permanece vinculada ao evento até ACK correto. Ela rejeita comando diferente enquanto está ocupada.

O problema é que a fronteira RFID ainda é uma interface vazia. Esse é o maior gap funcional de todo o projeto. O código consegue simular a consequência de uma leitura, mas ainda não provou a entrada física.

Além disso, a Station mantém o estado em memória. Se cair depois de assinar e antes de o Agent persistir a evidência, o evento é perdido. O projeto documenta isso, mas a consequência operacional é séria: uma leitura física pode precisar ser repetida, e o sistema não consegue provar que a primeira observação ocorreu.

### 4.3 `services/agent`

O Agent é uma boa fronteira de durabilidade. A regra “persistir antes de ACK e antes de HTTP” é correta.

A validação em `worker.rs` também é bem posicionada. O Agent não deve confiar somente no que a Station diz nem somente no que a API entregou. Ele reconstrói e compara o contexto do comando com o evento recebido.

O novo estado `QUARANTINED` é importante. Sem ele, uma falha de contrato poderia consumir recursos indefinidamente e mascarar a causa original.

O risco operacional é que a quarentena ainda depende de inspeção e reconciliação de operador. Não existe uma fila de incidentes, métrica obrigatória, alerta ou procedimento automatizado para decidir entre recaptura, correção de configuração ou substituição da Station.

Também existe uma assimetria de segurança no transporte serial. CRC32C detecta corrupção acidental, mas não autentica o remetente. Se um processo local conseguir injetar bytes na porta serial, ele pode tentar enviar comandos ou ACKs. A Station possui validações semânticas, mas não existe autenticação criptográfica do canal Agent-Station.

Isso pode ser aceitável em um host dedicado e fisicamente protegido. Não é aceitável como suposição silenciosa em um computador compartilhado, quiosque público ou rede industrial conectada.

### 4.4 `services/api`

A API está dividida de maneira saudável:

- `routes`: transporte HTTP e parsing;
- `domain`: regras de negócio;
- `repository`: SQL e persistência;
- `solana`: RPC e construção de transações;
- `crypto`: validação de evidência;
- `state`: dependências compartilhadas;
- `config`: fail-closed no startup.

A nova divisão entre `authorization_challenge` e `create` é uma melhora arquitetural. O challenge congela a intenção e o `create` verifica que a assinatura corresponde ao mesmo contexto no momento de consumir.

O ponto mais delicado é que a API continua sendo uma combinação de coordenador, admission controller, projetor e gateway Solana. Isso é aceitável para o escopo atual, mas aumenta a complexidade do processo único. Em produção, a parte de ingestão, a parte de construção de transação e a parte de reconciliação provavelmente precisarão de responsabilidades operacionais separadas, mesmo que compartilhem crates.

Outro risco é a dependência síncrona do RPC durante operações HTTP. A criação de captura consulta estado canônico para preparar a sequência. Isso garante correção, mas torna a disponibilidade da Solana parte do caminho de reserva da Station. Um RPC lento pode impedir uma operação física legítima antes de existir qualquer evidência.

### 4.5 `chain`

O programa Anchor é relativamente pequeno, o que é uma qualidade. As contas e invariantes são fáceis de localizar.

A verificação do precompile está mais forte que uma implementação ingênua. Ela não confia apenas no fato de que existe uma instrução Secp256r1. Ela confere programa, contas, descriptor, offsets, bytes, instrução atual e ausência de uma terceira instrução.

O desenho das PDAs também é compreensível:

```text
config = PDA("config", deployment_id)
animal = PDA("animal", deployment_id, animal_id)
rfid   = PDA("rfid", deployment_id, rfid_hash)
```

O risco de longo prazo é a imutabilidade excessiva. Um único `ProtocolConfig` possui uma única Station pública e uma única autoridade por deployment. Isso é simples e seguro para uma demo, mas inadequado para uma operação com várias estações, rotação de chaves, revogação, manutenção e incidentes.

### 4.6 `apps/web`

O frontend separa bem componentes visuais de código protocol-aware. `DemoPage.vue` orquestra, enquanto `protocol`, `solana` e `verify` concentram regras técnicas.

O frontend faz uma quantidade incomum de validação antes da carteira assinar. Isso é positivo. Ele verifica o evento, o descriptor, os PDAs, o signer, os account roles e o tamanho da transação.

O problema é que o frontend concentra uma parte relevante da recuperação. A operação é persistida no `localStorage`, mas esse armazenamento é apenas uma dica não autoritativa. Se o navegador for limpo, a API precisa possuir uma maneira de descobrir e reconciliar operações pendentes sem depender do `captureId` salvo.

Outro ponto de atenção é a experiência de assinatura. O challenge de autorização é uma mensagem textual com prefixo de domínio e campos técnicos. Isso é verificável, mas muitas carteiras exibem mensagens de forma pouco amigável. O usuário pode assinar um texto que não entende. Para uma operação de alto risco, o produto precisa mostrar claramente ação, animal, origem, destino, deployment e expiração antes e durante a assinatura.

### 4.7 `hardware-simulator`

O simulador cumpre bem seu papel de integração. Ele permite testar o Agent Rust real contra uma Station Python, em vez de substituir todos os componentes por mocks.

O risco é de governança: um simulador funcional pode ser confundido com prova de hardware. O CI deve manter os testes simulados separados dos gates de firmware e hardware. O produto também deve exibir explicitamente quando está em modo simulado.

## 5. Configuração atual e riscos de configuração

### 5.1 API

A API exige, entre outras, estas variáveis:

```text
LASTRO_API_BIND
LASTRO_DATABASE_URL
LASTRO_AGENT_TOKEN
LASTRO_SOLANA_RPC_URL
LASTRO_DEPLOYMENT_ID_HEX
LASTRO_STATION_PUBKEY_HEX
LASTRO_PROGRAM_ID
```

A configuração é validada no startup. O deployment tem 32 bytes em hex minúsculo. A Station tem chave P-256 comprimida de 33 bytes. O Program ID precisa ser um endereço Solana válido. O token precisa ter comprimento mínimo.

O ponto positivo é não haver fallback silencioso para outro deployment. O ponto negativo é que as variáveis continuam sendo strings independentes, sem um manifesto de deployment assinado que relacione Program ID, deployment ID, autoridade, Station e cluster.

Isso permite uma configuração internamente válida, mas semanticamente errada: todas as strings podem possuir o formato correto e ainda apontar para recursos de deployments diferentes.

### 5.2 Agent

O Agent exige:

```text
LASTRO_AGENT_API_URL
LASTRO_AGENT_TOKEN
LASTRO_AGENT_SERIAL_PORT
LASTRO_AGENT_SERIAL_BAUD
LASTRO_STATION_PUBKEY_HEX
LASTRO_AGENT_SQLITE_URL
LASTRO_AGENT_POLL_INTERVAL_MS
LASTRO_AGENT_REQUEST_TIMEOUT_MS
LASTRO_AGENT_STATION_RESPONSE_TIMEOUT_MS
```

A validação de URL, token, baud rate, chave pública, esquema SQLite e tempos é fail-closed.

Ainda faltam políticas operacionais de rotação de token, identidade por Agent, revogação de um Agent comprometido e distribuição segura de configuração. O token atual autentica o processo, não estabelece uma identidade de dispositivo forte.

### 5.3 Frontend

A configuração pública exige:

```text
VITE_API_BASE_URL
VITE_SOLANA_RPC_URL
VITE_SOLANA_CHAIN
VITE_LASTRO_PROGRAM_ID
VITE_LASTRO_DEPLOYMENT_ID_HEX
VITE_LASTRO_AUTHORITY
```

Esses valores são públicos e não devem conter segredos. O navegador usa essa configuração para rejeitar respostas de API/RPC que apontem para outra implantação.

O risco é operacional: configuração pública não é segredo, mas ainda é uma âncora de confiança. Se for gerada com valores errados no build, a aplicação pode compilar e operar contra um cluster ou deployment incorreto. É necessário validar o manifesto no pipeline de release, não apenas durante o build.

### 5.4 Compose e ambiente local

O Compose possui profiles separados para `app` e `hardware-sim`. A API usa RPC com endereço interno de container, enquanto o browser e os testes usam `127.0.0.1`. Essa distinção é necessária e foi tratada em `local_dev.py`.

O script local cria identidades descartáveis em `.lastro-local/` e impede que variáveis de um ambiente externo sejam herdadas. Isso reduz um risco comum: executar um teste achando que está no local, mas enviar transações para Devnet ou Mainnet.

A fragilidade é a quantidade de configuração espalhada entre `.env.example`, Compose, scripts, workflow CI, workflow E2E, Anchor e frontend. A solução futura deveria gerar os arquivos derivados a partir de um único manifesto de deployment.

## 6. Principais problemas atuais, por severidade

### P0 — A integração RFID real ainda não existe

**Problema.** O adaptador RFID não possui modelo de leitor, protocolo de frames, pinagem, transporte elétrico, checksum ou regra de integridade definidos. A Station recebe um valor canônico por uma fronteira de teste.

**Impacto.** Todo o restante pode estar criptograficamente correto e ainda assim o fato físico não ser confiável. Também não é possível avaliar duplicação de leitura, distância, ruído, tag clone, tag ausente, colisão ou leitura parcial.

**Correção.** Escolher um leitor real e escrever um contrato físico versionado. O contrato deve incluir frame bruto, checksum, timeouts, retries, erro de comunicação, inicialização, reset, valor lógico extraído e evidências de teste.

**Critério de conclusão.** Uma placa real, com leitor real, deve produzir vetores idênticos aos do simulador, sobreviver a ruído e timeout e demonstrar que `new_rfid_hash` sempre vem da leitura local.

### P0 — Não existe root of trust completo para a Station

**Problema.** O protocolo conhece a chave pública Station, mas o ciclo de vida físico da chave ainda não está completo. eFuse/secure boot são capacidades preparadas, não propriedades comprovadas de cada placa.

**Impacto.** Um processo que controla a chave privada de desenvolvimento pode produzir eventos que parecem vir da Station. Mesmo com a chave correta, o sistema não sabe se a chave está em hardware protegido, se foi clonada ou se precisa ser revogada.

**Correção.** Definir provisionamento de fábrica, geração de chave dentro do dispositivo ou importação controlada, secure boot, leitura protegida de eFuse, atestado de chave pública, inventário de dispositivo, revogação e rotação.

**Critério de conclusão.** O deployment deve aceitar somente Stations registradas, e uma Station comprometida deve poder ser revogada sem invalidar toda a história legítima.

### P0 — Recaptura não revoga uma transação antiga

**Problema.** A supersessão ocorre no PostgreSQL. Ela marca evidência anterior como `REJECTED` e cria nova captura, mas não cancela uma transação Solana que já tenha sido assinada ou enviada.

**Impacto.** A transação antiga pode chegar à rede depois da criação da captura substituta. O banco e a Solana podem discordar sobre qual intent deve vencer.

**Correção.** Introduzir um estado de intent on-chain ou nonce por `AnimalState`. A transação deve consumir o nonce atual. Uma operação cancelada ou supersedida deve ser rejeitada on-chain, não apenas no backend.

**Critério de conclusão.** Para duas transações concorrentes da mesma posição lógica, no máximo uma pode finalizar, independentemente da ordem de chegada ao RPC.

### P0 — Não há reconciliação server-side completa

**Problema.** O fluxo de finalização é acionado principalmente pelo navegador. Se a Solana finalizar uma transação e o navegador desaparecer antes de chamar `confirm`, o evento pode permanecer `SUBMITTED` e a projeção local pode não avançar.

**Impacto.** O estado canônico existe, mas a API fica stale. A captura seguinte pode ser bloqueada. O operador precisa recarregar ou intervir manualmente. Em produção, operações não podem depender da aba do navegador permanecer aberta.

**Correção.** Criar um reconciliador persistente no backend. Ele deve buscar eventos `SUBMITTED`, consultar commitment `finalized`, verificar o envelope e aplicar a projeção de forma idempotente. O reconciliador também deve detectar transações expiradas, falhas definitivas e divergência entre PostgreSQL e Solana.

**Critério de conclusão.** Fechar o navegador em qualquer instante depois do broadcast não pode impedir a conclusão do estado do sistema.

### P0 — Não existe journal persistente na Station

**Problema.** A Station guarda captura, evento e assinatura em RAM. Uma queda antes do Agent persistir o `EVENT_READY` perde a evidência.

**Impacto.** Uma observação física real pode desaparecer. Isso gera recaptura, perda de auditoria e possível divergência entre o que aconteceu fisicamente e o que o sistema consegue provar.

**Correção.** Adicionar armazenamento local mínimo para o evento assinado, com status `SIGNED`, `ACKED` e checksum. O armazenamento precisa suportar recuperação após reboot e proteção contra corrupção.

**Critério de conclusão.** Após queda entre assinatura e ACK, a Station deve reenviar exatamente os mesmos bytes e não consumir uma segunda leitura.

### P1 — Um único `ProtocolConfig` não atende operação real

**Problema.** O deployment possui uma Station pública e uma autoridade imutáveis. Não há registro de várias Stations, status de revogação, data de validade, substituição controlada ou histórico de chaves.

**Impacto.** Trocar uma placa, revogar uma placa roubada ou operar múltiplos pontos exige novo deployment ou solução fora do protocolo.

**Correção.** Criar um registry de Stations versionado. Cada Station deve ter ID, chave pública, status, autoridade registradora, versão de firmware e janela de validade. O verificador deve validar a chave apropriada ao momento do evento.

### P1 — A autoridade da captura e a autoridade da transação são diferentes

**Problema.** O challenge de captura é assinado por uma carteira com `signMessage`; depois a transação Solana é assinada pela carteira. O sistema verifica que ambas correspondem, mas são duas operações e dois prompts.

**Impacto.** O usuário pode autorizar o trabalho físico e depois rejeitar ou abandonar a transação. Isso é permitido pelo desenho, mas deixa evidência aceita sem finalização e consome capacidade operacional.

**Correção.** Manter as duas etapas, mas tornar a UX e a política explícitas. Mostrar que a primeira autorização reserva a Station e a segunda grava o estado canônico. Avaliar uma autorização de intent que possa ser vinculada à futura transação e expirar no mesmo ciclo.

### P1 — O modelo de transferência não tem aceitação do destinatário

**Problema.** `TRANSFER` exige assinatura do custodiante atual e coloca outro endereço como destino. O destinatário não assina nem aceita formalmente.

**Impacto.** O sistema prova que A transferiu para B em termos do protocolo, mas não prova que B recebeu, aceitou ou controla a carteira. Isso pode ser suficiente para custódia operacional, mas não para propriedade, venda, entrega ou responsabilidade legal.

**Correção.** Separar “mudança de custodiante autorizada pelo atual” de “aceite do próximo custodiante”. Se o domínio exigir aceitação bilateral, usar um fluxo em duas fases com prazo e intent on-chain.

### P1 — O verificador e o builder não têm uma política única de versão de transação

**Problema.** O contrato de `TransactionData` permite `legacy` e `v0`, e o frontend possui lógica para ambos. O verificador independente de transações finalizadas rejeita transações que não sejam legacy e rejeita address table lookups.

**Impacto.** Uma transação v0 pode ser válida para a operação e ainda assim não ser aceita pelo verificador. Isso é uma inconsistência de contrato.

**Correção.** Escolher uma das opções:

1. congelar formalmente o protocolo em legacy e remover `v0` dos DTOs e da UI; ou
2. implementar verificação completa de v0, incluindo message version, static account keys, address tables e resolução das contas.

Para um hackathon, a opção 1 é mais simples. Para produção, a opção 2 é mais flexível.

### P1 — O banco e a Solana podem divergir por falha de processo

**Problema.** PostgreSQL é projeção, mas não há uma rotina de reconciliação completa e contínua documentada como requisito operacional.

**Impacto.** Um bug, queda ou erro de migration pode deixar `animals`, `events` e contas Solana em estados diferentes. A arquitetura sabe que isso pode acontecer, mas o sistema precisa detectar e resolver automaticamente.

**Correção.** Criar uma tabela de reconciliação, snapshots do estado canônico, jobs idempotentes e alertas. Nenhuma correção deve editar silenciosamente o histórico; deve registrar divergência e decisão.

### P1 — Os endpoints públicos têm superfície de abuso

**Problema.** Registro de animal, recovery lookup, challenge e alguns endpoints de leitura são públicos ou pouco autenticados. Existem budgets, mas não há uma política completa de identidade, quota por IP, CAPTCHA operacional, API key ou gateway de rate limit.

**Impacto.** Um atacante pode consumir budgets, enumerar `visualRecoveryId`, gerar carga de RPC e preencher tabelas de auditoria.

**Correção.** Colocar rate limiting na borda, quotas por tenant/operator, autenticação para funções operacionais e uma política de privacidade para recovery IDs. Os budgets no PostgreSQL devem ser a última barreira, não a primeira.

### P1 — `visualRecoveryId` pode ser enumerável

**Problema.** O identificador visual é usado para recuperar `AnimalID` e a projeção. Se for previsível ou curto, um terceiro pode consultar IDs de outros animais.

**Impacto.** Exposição de existência, custodian, RFID hash, sequência e histórico. Isso pode ser grave em um sistema com dados de produtores, transporte ou propriedade.

**Correção.** Separar identificador de recuperação público de segredo de recuperação. Usar IDs não enumeráveis, autenticação do operador, escopo por tenant e respostas com menor quantidade de dados.

### P1 — Falta ciclo de vida de secrets

**Problema.** O token do Agent e configurações sensíveis são providos por environment. O ambiente local cria um token, mas não há desenho completo de rotação, revogação, expiração e auditoria de tokens em produção.

**Impacto.** Um token vazado pode permitir submissão de evidências, polling e consumo da API até intervenção manual.

**Correção.** Usar identidade por Agent, tokens curtos ou mTLS, rotação automática, revogação e armazenamento em secret manager. Registrar qual Agent admitiu cada evidência.

### P1 — O transporte serial tem integridade, mas não autenticidade

**Problema.** CRC32C detecta corrupção, mas não impede injeção ou replay por quem controla o host serial.

**Impacto.** Em uma máquina comprometida, um processo pode tentar produzir comandos ou ACKs indevidos. As verificações posteriores reduzem o impacto, mas não isolam a Station.

**Correção.** Adicionar handshake autenticado Agent-Station, contador monotônico ou challenge de sessão e, se o risco justificar, assinatura de comandos. O desenho deve impedir replay de um comando antigo em uma Station reiniciada.

### P1 — A arquitetura depende muito de RPC público

**Problema.** Captura, construção de transação, submit, confirm e verificação dependem do RPC. Há timeout e budget, mas não há estratégia de múltiplos RPCs, fallback ou operação offline completa.

**Impacto.** Uma falha de RPC é percebida como indisponibilidade do sistema, mesmo que a Station e a carteira estejam funcionando.

**Correção.** Usar RPC redundante, health scoring, retry com limites, cache de contas somente para leitura e um reconciliador que possa usar um indexer confiável. Nunca usar cache para decidir autoridade sem confirmação adequada.

### P1 — Histórico e evidência dependem de disponibilidade off-chain

**Problema.** O `EvidencePackage` é montado pela API a partir do PostgreSQL. A Solana contém a transação, mas não necessariamente toda a informação de RFID observado, chave pública e pacote pronto para verificação.

**Impacto.** Se o banco for perdido ou uma linha for corrompida, a prova independente fica incompleta mesmo que as transações estejam na cadeia.

**Correção.** Exportar pacotes assinados ou content-addressed para armazenamento durável independente. Manter backups testados, retenção, hashes de snapshot e procedimento de recuperação. Considerar armazenar um commitment mínimo on-chain para o pacote completo.

### P1 — O limite de 128 eventos é uma limitação de produto escondida no verificador

**Problema.** O browser recusa pacotes maiores que 128 eventos para proteger recursos.

**Impacto.** Um animal com histórico longo pode ter evidência verdadeira e ainda assim ser impossível de verificar na interface.

**Correção.** Criar verificação incremental, snapshots de estado, provas por segmentos e uma ferramenta CLI/verificador de referência. O limite deve ser por lote de processamento, não por tamanho total da história.

### P2 — Contratos de configuração estão espalhados

**Problema.** Configuração é distribuída entre `.env.example`, workflows, Compose, Anchor.toml, scripts e variáveis Vite.

**Impacto.** É fácil compilar frontend com deployment diferente da API ou inicializar `ProtocolConfig` com Station diferente da esperada.

**Correção.** Criar um manifesto de deployment versionado, por exemplo:

```text
cluster
program_id
deployment_id
authority
station_registry
supported_transaction_versions
protocol_version
```

Gerar `.env`, configurações de CI, frontend e comandos de inicialização a partir desse manifesto. Validar o manifesto antes de qualquer deploy.

### P2 — Protocolo e envelope têm rigidez útil, mas pouca evolução

**Problema.** O programa exige duas instruções, o evento é v1, o descriptor é fixo e a política de contas é fechada.

**Impacto.** Adicionar nonce, múltiplas Stations, cancelamento, aceite bilateral ou uma terceira instrução exigirá nova versão coordenada em C, Rust, TypeScript, API, Solana e vetores.

**Correção.** Definir política de versionamento antes de precisar dela. Versionar envelope, evento, semântica e capabilities. Um parser futuro deve rejeitar versões desconhecidas sem ambiguidade.

### P2 — Cobertura de testes ainda não corresponde a todas as falhas de produção

**Problemas observáveis:**

- não há validação com leitor RFID real;
- não há teste físico de queda de energia entre assinatura e ACK;
- não há teste de eFuse e secure boot em placa real;
- não há prova com carteira de extensão real;
- não há teste de carga da API e dos budgets sob concorrência alta;
- não há teste de failover de PostgreSQL;
- não há teste de RPC alternativo ou divergente;
- não há property testing/fuzzing sistemático do parser C/Rust/TypeScript;
- não há teste de reorg ou transação antiga concorrendo com supersessão;
- não há teste de recuperação server-side sem navegador.

**Correção.** Classificar testes por garantia e por ambiente. Um teste unitário não deve aparecer no mesmo selo que um teste físico ou E2E contra validator real.

### P2 — O checkout não está pronto como release reprodutível

**Problema.** Há muitas alterações locais não commitadas e `MANIFEST.sha256` está stale em relação à árvore atual.

**Impacto.** Não há baseline auditável único. Não é possível afirmar que o relatório, lockfiles, scripts e arquivos novos pertencem a uma revisão reproduzível.

**Correção.** Fechar uma branch/commit, regenerar manifestos, executar todos os gates, publicar os hashes dos artefatos e separar mudanças de produto de artefatos de análise.

### P2 — Não há observabilidade suficiente para operar o fluxo

**Problema.** O sistema possui logs e estados, mas não há indicação de uma camada completa de métricas, tracing distribuído, incident IDs, dashboards ou alertas obrigatórios.

**Impacto.** Uma operação pode ficar em `DISPATCHED`, `LOCAL`, `SERVER`, `SUBMITTED` ou `QUARANTINED` sem que o time saiba automaticamente por quê.

**Correção.** Instrumentar `capture_id`, `event_hash`, `tx_signature`, Station ID, Agent ID e deployment como correlation IDs. Nunca registrar segredo ou bytes sensíveis completos. Criar métricas de latência, retry, quarentena, expiração e divergência.

## 7. Problemas futuros de produto e domínio

### 7.1 Custódia não é propriedade

Hoje o modelo chama o campo de `custodian`. Isso é bom, porque evita declarar propriedade. Porém, a interface “transfer” pode ser interpretada como transferência de posse ou propriedade.

O produto precisa definir se uma transferência representa:

- custódia física;
- responsabilidade operacional;
- propriedade legal;
- posse temporária;
- autorização de transporte;
- entrega comercial.

Cada interpretação exige atores, provas e aceites diferentes.

### 7.2 Uma carteira não representa necessariamente uma organização

O modelo atual transforma uma chave pública em custodiante. Em produção, uma organização pode ter usuários, delegação, multisig, expiração de mandato, revogação e auditoria.

A evolução natural é separar:

```text
wallet key -> operator identity -> organization -> custody role
```

A carteira continua sendo um mecanismo de assinatura, mas não precisa ser o único modelo de identidade do domínio.

### 7.3 Multi-tenancy não está resolvido

O deployment ID separa parte do estado on-chain, mas o modelo off-chain precisa garantir isolamento por cliente, produtor ou organização.

Sem tenant explícito, um `visualRecoveryId`, AnimalID ou endpoint público pode atravessar fronteiras de clientes. Isso é um risco de autorização e privacidade, não apenas uma questão de banco.

### 7.4 Muitas Stations exigem governança

Um único deployment com várias Stations precisa responder:

- quem registra uma Station;
- quem revoga uma Station;
- como validar eventos históricos de uma Station revogada;
- qual chave vale em cada data;
- como substituir uma Station quebrada;
- como distinguir uma Station em manutenção de uma comprometida;
- como auditar firmware e versão de hardware.

Sem registry versionado, a escala física ficará limitada ao cenário de uma Station fixa.

### 7.5 Offline e conectividade intermitente

A arquitetura atual presume API e RPC disponíveis para criar captura e concluir o ciclo. Em fazendas, currais, transportes ou áreas remotas, a operação pode começar offline.

Um modo offline exigiria:

- autorização pré-carregada e limitada;
- nonce ou sequência local segura;
- armazenamento durável na Station/Agent;
- sincronização posterior;
- resolução de conflitos;
- limites de tempo e validade;
- prevenção de reutilização da autorização offline.

Não se deve adicionar modo offline sem primeiro decidir quem pode autorizar operações quando o estado canônico não está disponível.

### 7.6 Escala de histórico e custo

O desenho atual consulta a história e pode exportar todo o `EvidencePackage`. Isso funciona para demo e pequenos históricos. Em escala, haverá custo de armazenamento, tempo de verificação, indexação e retenção.

A evolução deve separar:

- estado terminal;
- checkpoints assinados;
- histórico completo;
- prova de inclusão de um evento;
- pacote de auditoria sob demanda.

A cadeia pode armazenar commitments e o armazenamento off-chain pode manter os bytes completos, desde que a relação seja verificável.

### 7.7 Freshness e tempo

O protocolo v1 usa sequência e predecessor, mas não possui nonce ou timestamp assinado. Isso limita a capacidade de provar que uma leitura ocorreu durante uma janela operacional específica.

Para operações reguladas, pode ser necessário provar:

- momento da leitura;
- local da Station;
- versão do firmware;
- operador presente;
- janela de validade;
- contexto de lote ou transporte.

Esses campos não devem ser adicionados informalmente ao JSON. Eles precisam entrar no material assinado e nos invariantes on-chain ou em um commitment verificável.

## 8. Avaliação de segurança por fronteira

### 8.1 Station comprometida

Se a chave privada Station for comprometida, um atacante pode criar eventos assinados que parecem físicos. O programa on-chain pode rejeitar inconsistências com o estado anterior, mas não consegue distinguir uma leitura falsa de uma leitura verdadeira se o atacante controla a chave e produz um RFID coerente.

A mitigação é principalmente fora do parser: secure boot, eFuse, proteção de chave, atestado, registry e revogação.

### 8.2 Agent comprometido

Um Agent comprometido pode tentar enviar evidência inventada. A API deve rejeitar bytes, assinatura, StationID ou contexto inválidos. Essa fronteira está bem defendida.

Ainda assim, o Agent pode causar negação de serviço, gastar budgets, atrasar comandos ou colocar linhas em quarentena. É necessário isolamento operacional, identidade individual e monitoramento.

### 8.3 API comprometida

Uma API comprometida pode tentar retornar transaction-data alterado ou projection falsa. O navegador e o programa on-chain reduzem o impacto, desde que o custodiante assine apenas o que o navegador validou e a configuração de deployment esteja correta.

Uma API comprometida também pode censurar eventos, impedir capturas ou negar recovery. Blockchain não resolve disponibilidade nem disponibilidade do histórico off-chain.

### 8.4 Banco comprometido

O banco não deve conseguir alterar evidência criptográfica por causa de triggers e verificações posteriores. Porém, um administrador de banco pode excluir tabelas, negar leituras ou apagar projeção. A disponibilidade e a recuperação dependem de backups, WORM/object storage e auditoria externa.

### 8.5 Carteira comprometida

A carteira é a autoridade de custódia. Se sua chave for comprometida, o protocolo pode aceitar uma transição assinada pelo custodiante correto. O sistema precisa de revogação de custodian, multisig ou governança externa para tratar esse caso.

### 8.6 Solana/RPC indisponível

A indisponibilidade não falsifica a evidência, mas impede finalização ou verificação. O estado `NOT_CHECKED` é correto. A arquitetura ainda precisa de redundância e reconciliação para não transformar uma falha temporária em incidente manual permanente.

## 9. Recomendações de arquitetura futura

### 9.1 Criar um serviço de reconciliação

O próximo serviço importante não é outra tela. É um reconciliador server-side.

Ele deve:

1. buscar eventos `SUBMITTED` e intents ativos;
2. consultar RPC com commitment adequado;
3. verificar assinatura e envelope completo;
4. detectar sucesso, falha, expiração ou ausência;
5. aplicar projeção de forma idempotente;
6. registrar cada decisão;
7. emitir alerta quando não conseguir decidir.

Esse serviço deve ser seguro para executar várias vezes. A reconciliação não pode depender da aba do browser.

### 9.2 Introduzir intent/nonce on-chain

Cada operação deve possuir uma intenção canônica associada ao `AnimalState` ou a uma conta de operação. A transação deve consumir uma versão específica.

Um modelo possível:

```text
AnimalState.intent_nonce = n

capture authorization -> intent n
StationEvent            -> intent n
wallet transaction      -> intent n
program on-chain        -> accepts only n
finalization             -> increments nonce
```

Uma recaptura cria intent `n+1` somente depois de invalidar ou expirar formalmente `n`. Isso resolve o principal problema da supersessão atual.

### 9.3 Criar registry de Stations

Substituir o único `station_pubkey33` por um registry versionado. O estado deve registrar chave, status, firmware, deployment, data de ativação e revogação.

Eventos históricos devem continuar verificáveis com a chave válida no momento da captura. Eventos novos devem rejeitar Stations revogadas.

### 9.4 Fechar o dispositivo físico

A ordem correta é:

1. escolher leitor;
2. documentar protocolo elétrico e frame;
3. implementar parser com testes de corrupção;
4. mapear para RFID canônico;
5. validar repetição e timeout;
6. implementar journal mínimo;
7. validar secure boot/eFuse;
8. executar testes de energia e reboot;
9. registrar firmware hash e Station ID.

Não é recomendável começar por mais telas ou mais endpoints antes desse fechamento, porque a principal hipótese do produto é física.

### 9.5 Gerar configuração a partir de manifesto

Criar um manifesto único de deployment e gerar:

- `.env` da API;
- env do Agent;
- `VITE_*`;
- configuração de Compose;
- argumentos de inicialização on-chain;
- trust anchors do verificador;
- documentação de deployment.

O pipeline deve rejeitar combinações inconsistentes de cluster, Program ID, deployment ID, autoridade e Station.

### 9.6 Definir uma política de dados

Antes de produção, classificar os dados:

- RFID observado;
- hash do RFID;
- AnimalID;
- recovery ID;
- custodian address;
- localização;
- operador;
- assinatura;
- histórico de transações.

Definir retenção, criptografia em repouso, acesso por tenant, exportação, exclusão compatível com a finalidade e o que precisa permanecer imutável para auditoria. A imutabilidade da prova não deve ser confundida com publicação irrestrita de todos os dados.

## 10. Roadmap priorizado

### Fase 0 — Fechar a baseline de engenharia

**Objetivo:** tornar a revisão e os builds auditáveis.

- criar um commit/branch limpo para o hardening atual;
- regenerar `MANIFEST.sha256`;
- decidir quais arquivos novos pertencem ao produto;
- executar `spec_check`, repository, protocol, API, Agent, web e chain tests;
- atualizar o índice de testes;
- publicar artefatos e hashes;
- separar claramente simulador, localnet, Devnet e hardware real.

**Saída:** uma revisão reprodutível que possa ser comparada com a próxima.

### Fase 1 — Corrigir confiabilidade de operação

**Objetivo:** nenhuma operação depender do navegador.

- implementar reconciliador server-side;
- adicionar idempotency key e estado de operação;
- tratar RPC multi-endpoint;
- criar dashboards de estados;
- automatizar quarentena e incidentes;
- testar queda em cada transição;
- testar PostgreSQL indisponível e recuperação.

**Saída:** fechar o browser não impede finalização nem deixa estado silenciosamente preso.

### Fase 2 — Corrigir concorrência e revogação

**Objetivo:** impedir que intent antigo e recaptura concorram.

- adicionar nonce/versionamento on-chain;
- definir cancelamento e expiração de intent;
- registrar a intenção da captura no material assinado;
- alinhar supersessão PostgreSQL com aceitação on-chain;
- decidir se transferência exige aceite bilateral.

**Saída:** a regra de “uma transição válida por posição” é verdadeira mesmo sob concorrência e atraso de RPC.

### Fase 3 — Fechar identidade da Station

**Objetivo:** transformar chave de teste em identidade de dispositivo.

- registry de Stations;
- provisionamento de fábrica;
- secure boot;
- eFuse ou elemento seguro;
- atestado e inventário;
- rotação e revogação;
- assinatura de comando ou canal autenticado;
- firmware hash versionado.

**Saída:** o sistema consegue distinguir Station autorizada, Station antiga, Station substituída e Station comprometida.

### Fase 4 — Integrar hardware real

**Objetivo:** provar a hipótese física.

- leitor real escolhido;
- frames documentados;
- parser em firmware;
- testes de ruído, tag ausente e tag duplicada;
- teste de energia e reboot;
- journal persistente;
- calibração e procedimento de campo.

**Saída:** a leitura física real produz evidência compatível com os vetores e mantém as mesmas garantias do simulador.

### Fase 5 — Preparar produto multi-tenant

**Objetivo:** proteger dados e organizar atores reais.

- tenant e organização no modelo;
- roles e delegações;
- recovery não enumerável;
- autenticação de operadores;
- aceite de transferência, se necessário;
- retenção e privacidade;
- auditoria e suporte operacional.

**Saída:** um cliente não consegue consultar ou operar dados de outro cliente.

### Fase 6 — Escalar verificação e histórico

**Objetivo:** suportar histórico longo e volume alto.

- indexer próprio ou serviço especializado;
- checkpoints;
- verificação incremental;
- pacotes content-addressed;
- object storage durável;
- provas por segmento;
- política de compactação sem perder verificabilidade.

**Saída:** a verificação não depende de materializar toda a história em uma única aba do browser.

## 11. O que eu não faria agora

Eu não adicionaria mais ações de negócio antes de resolver a semântica de intent e supersessão. Cada nova ação multiplicaria os estados que podem ficar presos.

Eu não declararia “hardware-backed” apenas porque existe caminho de eFuse no código. A afirmação precisa de placa, provisionamento e teste físico.

Eu não trataria `confirmed` como finalização. O sistema está correto ao separar os dois estados; essa distinção deve permanecer em toda a UI e em toda a documentação.

Eu não colocaria o EvidencePackage completo somente na Solana para resolver disponibilidade sem medir custo. Primeiro é necessário definir commitment, retenção, storage externo e provas.

Eu não usaria o simulador como substituto de validação física. Ele deve continuar sendo uma ferramenta de desenvolvimento e integração.

Eu não faria uma grande refatoração de microserviços agora. O maior valor imediato está em fechar invariantes, reconciliação, dispositivo e governança. Separar processos antes disso aumentaria a superfície operacional sem resolver os riscos principais.

## 12. Critérios para considerar o sistema pronto para um piloto controlado

Um piloto controlado deveria exigir, no mínimo:

1. um leitor RFID real documentado;
2. pelo menos duas Stations físicas ou uma Station com procedimento de substituição;
3. chave Station protegida e inventariada;
4. journal de recuperação após reboot;
5. reconciliador server-side;
6. política de revogação de carteira, Agent e Station;
7. intent/nonce que elimine supersessão insegura;
8. backup e restauração testados do PostgreSQL e EvidencePackage;
9. observabilidade de toda a cadeia;
10. teste de RPC indisponível e troca de endpoint;
11. teste de transação antiga concorrente com recaptura;
12. isolamento de tenant ou operação explicitamente single-tenant;
13. revisão da política de privacidade;
14. uma release commitada com manifesto regenerado;
15. registro explícito das garantias que continuam fora do escopo.

Um piloto não precisa resolver toda a escala global. Ele precisa provar que o caminho escolhido é operável, recuperável e honesto sobre o que a evidência significa.

## 13. Diagnóstico final de engenheiro sênior

O Lastro está bem pensado onde muitos protótipos falham: contratos são explícitos, bytes são congelados, autoridades são separadas, estados não são misturados e a UI não é a única camada de validação.

Os problemas mais graves não estão na organização básica do código. Eles estão nas garantias que aparecem quando o sistema sai do caminho feliz:

- uma leitura física perdida;
- uma Station comprometida;
- uma chave que precisa ser revogada;
- duas transações concorrentes;
- um browser fechado;
- um banco restaurado de backup;
- um RPC que responde atrasado;
- um destinatário que não aceita a transferência;
- uma consulta pública que expõe dados;
- um deployment configurado com âncoras incompatíveis.

A arquitetura atual já possui pontos de extensão para resolver esses casos. O risco seria interpretar essa extensibilidade como solução pronta. Ainda faltam componentes e procedimentos concretos.

Minha ordem de prioridade seria:

```text
1. baseline reprodutível e manifesto limpo
2. reconciliador server-side
3. nonce/intent on-chain e supersessão segura
4. root of trust e registry de Stations
5. leitor RFID real e journal físico
6. secrets, tenants, privacidade e roles
7. observabilidade, carga e escala de histórico
```

Se esses sete pontos forem tratados na ordem correta, o projeto pode evoluir de uma demo criptograficamente cuidadosa para uma plataforma de evidência física realmente operável. Se forem ignorados, o sistema continuará parecendo forte no caminho feliz, mas dependerá de intervenção manual e confiança implícita exatamente nos momentos mais importantes.

## Referências internas

[1]: ./ANALISE_PROJETO.md "Análise técnica atualizada do projeto Lastro"
[2]: ./ARCHITECTURE.md "Arquitetura do Lastro"
[3]: ./PROTOCOL.md "Protocolo canônico de bytes e transições"
[4]: ./API.md "API e ciclo de submissão de eventos"
[5]: ./SECURITY.md "Fronteiras de segurança e ameaças"
[6]: ./HARDENING_STATUS.md "Status atual do hardening"
[7]: ./LOCAL_DEVELOPMENT.md "Desenvolvimento local"
[8]: ./TESTING.md "Estratégia de testes"
[9]: ../schemas/openapi.yaml "Contrato OpenAPI atual"
[10]: ../services/api/src/routes/captures.rs "Autorização e criação de capturas"
[11]: ../services/api/src/repository/capture_authorizations.rs "Persistência dos challenges de autorização"
[12]: ../services/api/src/solana/transaction_builder.rs "Construção do envelope Solana"
[13]: ../apps/web/src/solana/transaction.ts "Validação e assinatura da transação no navegador"
[14]: ../apps/web/src/verify/verifyChain.ts "Verificação independente do estado on-chain"
[15]: ../services/agent/src/worker.rs "Worker e recuperação do Agent"
[16]: ../firmware/station/components/lastro_station/station.c "Máquina de estados da Station"
[17]: ../chain/programs/lastro/src/verify/secp256r1.rs "Vinculação do precompile Secp256r1"

**Autor:** Manus AI

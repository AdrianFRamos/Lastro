# Plano consolidado de mudanÃ§as previstas do Lastro

**Projeto:** Lastro
**Data:** 24 de setembro de 2026
**Escopo:** evoluÃ§Ã£o do sistema de prova de identidade fÃ­sica e custÃ³dia individual para uma plataforma de rastreabilidade bovina, linhagem pÃ³s-abate e registro digital de custÃ³dia.
**Estado de referÃªncia:** a fundaÃ§Ã£o do protocolo e do programa Solana v2 jÃ¡ foi implementada; os incrementos de domÃ­nio bovino completo continuam previstos.

## 1. Resumo executivo

O Lastro estÃ¡ sendo evoluÃ­do de um sistema focado em capturar um evento fÃ­sico de RFID e registrar a custÃ³dia de um animal para um sistema capaz de acompanhar uma cadeia produtiva completa. O fluxo pretendido comeÃ§a no animal vivo e pode seguir por lote, transporte, recebimento no frigorÃ­fico, abate, carcaÃ§a, desossa, cortes, subprodutos, lotes de produto, embalagens, expediÃ§Ãµes, recebimentos e recall.

A mudanÃ§a central Ã© separar corretamente trÃªs responsabilidades. A **Solana** deve manter o estado canÃ´nico mÃ­nimo e os invariantes que nÃ£o podem ser substituÃ­dos silenciosamente. A **API e o PostgreSQL** devem guardar manifestos, documentos, projeÃ§Ãµes e dados operacionais detalhados. O **Agent e a Station** devem preservar evidÃªncias fÃ­sicas mesmo quando o local estiver sem internet. O **verificador independente** deve recomputar as provas e informar exatamente quais propriedades foram comprovadas.

O sistema nÃ£o deve tratar a blockchain como um banco para armazenar todos os dados da cadeia. O smart contract deve funcionar como um motor de transiÃ§Ãµes, consumo, concorrÃªncia, hashes, raÃ­zes de composiÃ§Ã£o, status, reservas e bloqueios. O manifesto completo de uma transformaÃ§Ã£o, seus documentos e a lista detalhada de cortes permanecem fora da transaÃ§Ã£o, protegidos por hashes e commitments ancorados na Solana.

TambÃ©m Ã© importante delimitar o significado de â€œmatrÃ­cula de propriedadeâ€. A arquitetura pode oferecer uma **matrÃ­cula digital de identidade tÃ©cnica, custÃ³dia, proveniÃªncia e linhagem**. Ela nÃ£o prova automaticamente propriedade jurÃ­dica, identidade biolÃ³gica, conformidade sanitÃ¡ria ou veracidade absoluta de uma declaraÃ§Ã£o. Essas propriedades exigem autoridades, documentos, procedimentos e integraÃ§Ãµes prÃ³prias.

## 2. SituaÃ§Ã£o atual apÃ³s a fundaÃ§Ã£o v2

A versÃ£o v1 foi preservada para compatibilidade. O `StationEvent` v1 continua sendo o contrato fÃ­sico de 276 bytes usado para RFID, origem, transferÃªncia de custÃ³dia e reidentificaÃ§Ã£o. O programa Solana v1 continua separado do programa v2, e o v1 nÃ£o deve receber campos bovinos novos dentro do evento legado.

A fundaÃ§Ã£o v2 foi adicionada em paralelo. O programa `chain/programs/lastro-v2` possui uma superfÃ­cie nova de configuraÃ§Ã£o, registries, ativos, intents e ancoragem de observaÃ§Ãµes. O protocolo compartilhado possui um envelope de evento de domÃ­nio com layout fixo, hashes com domÃ­nios separados e validaÃ§Ã£o de versÃ£o, IDs e janela temporal.

A integraÃ§Ã£o inicial tambÃ©m alcanÃ§ou a API, o Agent e o frontend. A API possui uma migration v2 inicial, admissÃ£o de eventos de domÃ­nio, persistÃªncia idempotente e consulta de timeline. O Agent possui uma outbox SQLite separada, payloads seriais v2, ACK por hash, retry e quarentena. O frontend possui codec TypeScript do envelope v2 e teste de compatibilidade.

| Ãrea | SituaÃ§Ã£o atual | Limite atual |
|---|---|---|
| Programa v1 | Preservado e compilÃ¡vel | Continua limitado ao fluxo individual legado |
| Protocolo v2 | Envelope, hashes, enums e validaÃ§Ãµes iniciais implementados | Ainda faltam canonicalizaÃ§Ã£o completa de manifestos e todos os tipos de evento |
| Programa Solana v2 | ConfiguraÃ§Ã£o, registries iniciais, `AssetState`, `IntentState`, `EventAnchor` e observaÃ§Ã£o fÃ­sica inicial implementados | Ainda faltam migraÃ§Ã£o, lotes, transformaÃ§Ã£o, produtos, expediÃ§Ã£o e recall |
| API | AdmissÃ£o v2, armazenamento idempotente e timeline inicial implementados | O domÃ­nio completo de rastreabilidade ainda nÃ£o estÃ¡ exposto |
| Agent | Outbox v2, transporte, retry e quarentena iniciais implementados | Ainda faltam adaptadores de balanÃ§a, localizaÃ§Ã£o, manifesto e operaÃ§Ã£o industrial |
| Firmware | Contrato v1 preservado | A mÃ¡quina de sensores v2 ainda precisa ser implementada e validada em hardware |
| Frontend | Codec v2 e integraÃ§Ã£o inicial implementados | Ainda faltam jornadas operacionais e pÃ¡gina pÃºblica de produtos |
| Verificador | VerificaÃ§Ã£o legada e limites atuais preservados | O verificador de linhagem, massa, facility e recall v2 ainda precisa ser criado |
| OperaÃ§Ã£o | Manifesto e gates bÃ¡sicos existentes | Ainda faltam piloto, runbooks, backup restaurÃ¡vel e governanÃ§a operacional |

## 3. Arquitetura alvo

A arquitetura alvo terÃ¡ duas versÃµes convivendo durante a migraÃ§Ã£o:

```text
Lastro v1
  StationEvent de 276 bytes
  AnimalState e RfidBinding legados
  origem, transferÃªncia e reidentificaÃ§Ã£o

Lastro v2
  DomainEventEnvelope
  ProtocolConfigV2 e registries
  AssetState, EventAnchor e IntentState
  lotes, carcaÃ§as, transformaÃ§Ãµes, produtos e recall
```

A consulta poderÃ¡ atravessar as duas versÃµes. Um animal criado no v1 poderÃ¡ receber uma prova explÃ­cita de migraÃ§Ã£o para um ativo v2. Essa migraÃ§Ã£o deve copiar identidade, custÃ³dia, sequÃªncia, revisÃ£o e Ãºltimo hash conhecidos. Ela nÃ£o pode inventar peso, localizaÃ§Ã£o, status sanitÃ¡rio ou documento que nÃ£o existiam no histÃ³rico legado.

A divisÃ£o de autoridade prevista Ã© a seguinte:

| Fonte | Responsabilidade |
|---|---|
| Station ou balanÃ§a | Assinar observaÃ§Ãµes fÃ­sicas, como RFID e pesagem, quando o equipamento possuir essa capacidade |
| Carteira do custodiante | Autorizar transferÃªncia e aceite de custÃ³dia |
| Facility autorizada | Autorizar recebimento, abate, transformaÃ§Ã£o, produÃ§Ã£o, expediÃ§Ã£o e aÃ§Ãµes operacionais de recall |
| API e PostgreSQL | Receber requests, guardar manifestos, documentos, projeÃ§Ãµes e evidÃªncias detalhadas |
| Solana v2 | Aplicar invariantes, versÃ£o, predecessor, nonce, reservas, consumo, status e anchors |
| Verificador | Recalcular hashes, conferir contas, reconstruir linhagem e separar propriedades vÃ¡lidas de propriedades nÃ£o verificadas |

## 4. Modelo de ativos e linhagem

O v2 utilizarÃ¡ um `AssetState` genÃ©rico para evitar que animal, carcaÃ§a e produto tenham fontes canÃ´nicas incompatÃ­veis. Os tipos previstos sÃ£o `ANIMAL`, `LOT`, `CARCASS`, `CUT_BATCH`, `PRODUCT_LOT`, `PACKAGE`, `BYPRODUCT_LOT` e `SHIPMENT`.

Cada ativo deverÃ¡ possuir, no mÃ­nimo, um ID de 32 bytes, tipo, status, deployment, custodiante, raiz de pai, raiz de linhagem, lote atual, saldo de peso, sequÃªncia, versÃ£o de estado, Ãºltimo hash de evento e dados de reserva. Esses campos sÃ£o suficientes para aplicar regras de concorrÃªncia e disponibilidade sem colocar o manifesto completo na blockchain.

A linhagem serÃ¡ um grafo dirigido de relaÃ§Ãµes pai-filho. Um animal pode gerar uma ou mais carcaÃ§as conforme a polÃ­tica do processo. Uma carcaÃ§a pode gerar vÃ¡rios lotes de cortes. Um lote pode gerar diversas embalagens e expediÃ§Ãµes. Subprodutos e perdas devem ser representados explicitamente para que a transformaÃ§Ã£o nÃ£o pareÃ§a simplesmente â€œmultiplicarâ€ massa.

O detalhe da Ã¡rvore ficarÃ¡ no PostgreSQL e no EvidencePackage. A Solana guardarÃ¡ roots e anchors que permitam verificar se o manifesto consultado Ã© o mesmo que foi comprometido. O verificador deverÃ¡ ser capaz de navegar para trÃ¡s, atÃ© o animal ou lote de origem, e para frente, atÃ© produtos, embalagens e expediÃ§Ãµes afetadas.

## 5. PÃ³s-abate e transformaÃ§Ã£o

O frigorÃ­fico nÃ£o deve registrar uma desossa como uma instruÃ§Ã£o Ãºnica contendo centenas de entradas e saÃ­das. A operaÃ§Ã£o serÃ¡ dividida em uma mÃ¡quina de estados:

1. `confirm_slaughter` confirma o recebimento e o abate por uma facility autorizada.
2. `create_carcass` cria os ativos de carcaÃ§a relacionados ao animal abatido.
3. `record_carcass_weight` registra a pesagem da carcaÃ§a por fonte autorizada.
4. `begin_transformation` cria o manifesto de transformaÃ§Ã£o com facility, roots, quantidades, pesos, tolerÃ¢ncia e expiraÃ§Ã£o.
5. `reserve_transformation_input` reserva cada entrada para impedir consumo concorrente.
6. `append_transformation_chunk` registra chunks de entradas e saÃ­das quando a lista for grande.
7. `finalize_transformation` valida roots, contagens, pesos, tolerÃ¢ncia, facility, reservas e outputs antes de consumir as entradas.
8. `create_product_lot`, `create_package_commitment` e `create_byproduct_lot` materializam os ativos derivados.
9. `abort_transformation` encerra uma tentativa expirada ou cancelada sem apagar o manifesto original.

A regra de balanÃ§o de massa serÃ¡ explÃ­cita:

```text
peso de entradas
  = peso de produtos
  + peso de subprodutos
  + peso de perdas
  dentro da tolerÃ¢ncia aprovada
```

O smart contract deve validar os nÃºmeros compactos e os commitments. O manifesto detalhado deve explicar quais cortes foram produzidos, em qual quantidade, com quais unidades e quais documentos sustentam a operaÃ§Ã£o. Uma alteraÃ§Ã£o posterior deve criar um evento corretivo ou de invalidaÃ§Ã£o; nunca deve sobrescrever silenciosamente a transformaÃ§Ã£o original.

## 6. MudanÃ§as previstas no protocolo e no smart contract

### 6.1 Protocolo compartilhado

Ainda estÃ£o previstos os seguintes componentes no `lastro-protocol`:

- wrappers e validaÃ§Ãµes para `AssetId`, `EventId`, `FacilityId`, `PartyId`, `TransformationId`, `IntentId` e `RecallId`;
- canonicalizaÃ§Ã£o determinÃ­stica de manifestos;
- roots de composiÃ§Ã£o com algoritmo documentado, preferencialmente Merkle quando provas parciais forem necessÃ¡rias;
- envelopes para transferÃªncia, transformaÃ§Ã£o, migraÃ§Ã£o, recall e eventos de qualidade;
- fixtures Rust, TypeScript e JSON para todos os bytes e hashes crÃ­ticos;
- rejeiÃ§Ã£o explÃ­cita de schemas futuros, enums desconhecidos, unidades invÃ¡lidas e janelas temporais invÃ¡lidas;
- regras compartilhadas para balanÃ§o de massa, predecessor, versÃ£o de estado e consumo Ãºnico.

A canonicalizaÃ§Ã£o precisa fixar ordem de campos, unidades, tratamento de opcionais, ordenaÃ§Ã£o de arrays e representaÃ§Ã£o de inteiros. NÃ£o serÃ¡ suficiente serializar JSON sem polÃ­tica determinÃ­stica, porque duas linguagens poderiam gerar hashes diferentes para o mesmo manifesto lÃ³gico.

### 6.2 Contas e registries

O programa v2 deverÃ¡ evoluir os registries jÃ¡ iniciados:

- `ProtocolConfigV2` para parÃ¢metros de deployment e limites seguros;
- `StationRegistry` para chave P-256, validade, firmware, suspensÃ£o, revogaÃ§Ã£o e substituiÃ§Ã£o;
- `FacilityRegistry` para tipo de instalaÃ§Ã£o, credencial, validade e status;
- `PartyRegistry` para carteiras e papÃ©is compactos sem PII;
- registry de documentos ou commitments de documentos;
- `AssetState` para ativos vivos e derivados;
- `EventAnchor` para compromissos imutÃ¡veis de eventos;
- `IntentState` para nonce, actor, payload hash, expiraÃ§Ã£o, cancelamento e consumo;
- `LineageAnchor`, `TransformationAnchor` e `RecallState` para os fluxos posteriores.

A validade histÃ³rica de uma chave deve ser preservada. Revogar uma Station hoje deve impedir eventos futuros, mas nÃ£o invalidar automaticamente um evento que foi produzido enquanto a chave estava vÃ¡lida.

### 6.3 InstruÃ§Ãµes de domÃ­nio

A superfÃ­cie futura deve possuir uma instruÃ§Ã£o Anchor especÃ­fica por transiÃ§Ã£o relevante. A intenÃ§Ã£o Ã© evitar uma instruÃ§Ã£o genÃ©rica que receba `event_type` e permita contornar os guards.

As instruÃ§Ãµes previstas incluem migraÃ§Ã£o do animal v1, alteraÃ§Ã£o de status, observaÃ§Ã£o fÃ­sica, observaÃ§Ã£o de localizaÃ§Ã£o, transferÃªncia de custÃ³dia, criaÃ§Ã£o e divisÃ£o de lotes, merge, confirmaÃ§Ã£o de abate, carcaÃ§a, transformaÃ§Ã£o, produto, embalagem, expediÃ§Ã£o, aceite, quality hold, recall e retirada terminal.

Toda instruÃ§Ã£o mutÃ¡vel deverÃ¡ validar `expected_state_version`, `expected_previous_event_hash` e a intent ou nonce correspondente. Depois da aplicaÃ§Ã£o, a versÃ£o deve avanÃ§ar exatamente uma vez e o Ãºltimo hash deve apontar para o novo evento. Isso impede que uma transaÃ§Ã£o antiga seja aceita depois de uma operaÃ§Ã£o concorrente.

## 7. MudanÃ§as previstas na API e no PostgreSQL

A migration v2 inicial Ã© uma fundaÃ§Ã£o. O modelo completo deverÃ¡ ser incrementado sem quebrar as tabelas v1. A sequÃªncia planejada inclui assets, parties e papÃ©is, documentos, eventos de domÃ­nio, lotes, linhagem, transformaÃ§Ãµes, carcaÃ§as, produtos, embalagens, expediÃ§Ãµes, quality holds, recalls, anchors e reconciliaÃ§Ã£o.

As entidades principais serÃ£o:

- `assets`, com uma projeÃ§Ã£o comum de todos os objetos rastreÃ¡veis;
- `parties` e `party_roles`, com escopo por organizaÃ§Ã£o, facility e operaÃ§Ã£o;
- `official_documents`, com hash, emissor, validade e status de verificaÃ§Ã£o;
- `domain_events`, append-only e ligados a `AssetState` e `EventAnchor`;
- `lot_manifests` e `lineage_edges`, para composiÃ§Ã£o e relaÃ§Ãµes pai-filho;
- `transformation_manifests`, `transformation_inputs` e `transformation_outputs`;
- tabelas especializadas de carcaÃ§as, produtos, subprodutos, embalagens e expediÃ§Ãµes;
- `quality_holds`, `recalls` e seus ativos afetados;
- projeÃ§Ãµes pÃºblicas com somente os campos autorizados.

A API deverÃ¡ deixar de oferecer alteraÃ§Ã£o livre de estado canÃ´nico. Peso, localizaÃ§Ã£o, status, movimento, abate, transformaÃ§Ã£o, expediÃ§Ã£o e recall devem ser comandos de domÃ­nio que criam eventos. O fluxo de escrita serÃ¡:

```text
request
  -> autenticaÃ§Ã£o e autorizaÃ§Ã£o
  -> criaÃ§Ã£o da intent
  -> canonicalizaÃ§Ã£o do payload
  -> assinatura apropriada
  -> transaÃ§Ã£o Solana
  -> confirmaÃ§Ã£o finalizada
  -> projeÃ§Ã£o PostgreSQL
```

Os DTOs pÃºblicos nÃ£o devem ser derivados automaticamente dos DTOs internos. Dados como CPF, CNPJ, nomes legais, carteiras, coordenadas precisas, preÃ§os, rotas e documentos completos devem permanecer protegidos.

TambÃ©m serÃ¡ necessÃ¡rio um reconciliador server-side. Ele deverÃ¡ localizar intents pendentes, consultar a Solana, verificar anchors, atualizar projeÃ§Ãµes, liberar reservas, detectar divergÃªncias entre PostgreSQL e blockchain e registrar incidentes sem apagar versÃµes anteriores.

## 8. MudanÃ§as previstas no Agent e no firmware

O Agent manterÃ¡ o outbox RFID v1 separado do outbox de domÃ­nio v2. Uma observaÃ§Ã£o fÃ­sica, um manifesto de transformaÃ§Ã£o e uma expediÃ§Ã£o nÃ£o devem compartilhar uma mesma linha com semÃ¢ntica ambÃ­gua.

A outbox v2 deverÃ¡ conservar bytes originais, hash, tipo, versÃ£o, estado de entrega, tentativas e erro. Falhas de timeout, 5xx, 429, RPC indisponÃ­vel ou perda de serial devem ser reprocessÃ¡veis. Payload invÃ¡lido, assinatura invÃ¡lida, unidade invÃ¡lida, estado incompatÃ­vel, asset inexistente ou manifesto jÃ¡ finalizado devem ir para quarentena e exigir uma nova decisÃ£o de domÃ­nio.

No firmware, o `StationEvent` v1 continuarÃ¡ isolado. Os sensores v2 deverÃ£o possuir adapters prÃ³prios para balanÃ§a, localizaÃ§Ã£o, relÃ³gio, impressÃ£o e journal local. A mÃ¡quina de estados prevista Ã©:

```text
IDLE
  -> WAIT_SENSOR_CONTEXT
  -> READ_RFID
  -> READ_SCALE
  -> READ_LOCATION
  -> BUILD_OBSERVATION
  -> SIGN_OBSERVATION
  -> WAIT_ACK
```

Uma pesagem sÃ³ deverÃ¡ ser aceita quando estiver estÃ¡vel, com unidade conhecida, calibraÃ§Ã£o vÃ¡lida, timestamp monotÃ´nico e identificador do equipamento. O backend nÃ£o pode enviar peso, coordenada ou novo RFID para a Station e tratar esses valores como observaÃ§Ã£o local.

Antes de um piloto fÃ­sico ainda serÃ¡ necessÃ¡rio definir leitor RFID, balanÃ§a, pinout, frames, secure boot, eFuse, chave da Station, rotaÃ§Ã£o de chaves, atualizaÃ§Ã£o de firmware, certificado de calibraÃ§Ã£o e procedimento de troca do equipamento.

## 9. MudanÃ§as previstas no frontend e no verificador

A `DemoPage.vue` deve continuar representando o fluxo individual atual, mas nÃ£o deve concentrar toda a operaÃ§Ã£o industrial. O produto deverÃ¡ ganhar jornadas separadas para animal, lote, movimento, recebimento, abate, carcaÃ§a, transformaÃ§Ã£o, produto, embalagem, expediÃ§Ã£o, recall e consulta pÃºblica.

A jornada de transformaÃ§Ã£o deverÃ¡ mostrar entradas, peso total, outputs, subprodutos, perdas, tolerÃ¢ncia, diferenÃ§a calculada, status, hash do manifesto e transaÃ§Ã£o Solana. O botÃ£o de finalizaÃ§Ã£o deve ficar bloqueado quando houver input nÃ£o reservado, output duplicado, massa fora da tolerÃ¢ncia, facility suspensa, documento obrigatÃ³rio ausente, transformaÃ§Ã£o expirada ou carteira nÃ£o autorizada.

O verificador deverÃ¡ ampliar as camadas atuais. AlÃ©m de RFID, assinatura Station, continuidade de identidade e custÃ³dia, ele deverÃ¡ verificar hashes de eventos de domÃ­nio, linhagem, balanÃ§o de massa, credencial da facility, hashes documentais, recall, estado on-chain e confirmaÃ§Ã£o de fonte oficial.

O resultado deve ser granular. Exemplos de estados esperados sÃ£o `VALID`, `INVALID`, `NOT_CHECKED` e `NOT_DETERMINED`. A interface nÃ£o deve exibir um Ãºnico `VERIFIED: true`, porque uma assinatura vÃ¡lida nÃ£o comprova automaticamente a linhagem, a balanÃ§a, o documento ou a propriedade jurÃ­dica.

A consulta pÃºblica por QR code deverÃ¡ usar IDs opacos e retornar somente um resumo seguro do produto ou embalagem. O perfil pÃºblico pode mostrar data, tipo, validade, regiÃ£o generalizada, status de qualidade, resumo da linhagem, prova criptogrÃ¡fica e recall. NÃ£o deve revelar PII, preÃ§o, rota completa ou coordenada precisa.

## 10. SeguranÃ§a, privacidade e governanÃ§a

A versÃ£o v2 deverÃ¡ separar DeploymentAuthority, StationKey, ScaleKey, FacilityCredential, CustodianWallet, AgentCredential, OperatorIdentity e OfficialSourceKey. Uma identidade nÃ£o pode representar todas as outras.

A autorizaÃ§Ã£o deve combinar RBAC, que define o papel geral, com ABAC, que define organizaÃ§Ã£o, facility, ativo, perÃ­odo, operaÃ§Ã£o e estado atual. AutenticaÃ§Ã£o nÃ£o Ã© autorizaÃ§Ã£o. Uma carteira de frigorÃ­fico nÃ£o pode obter autoridade global sobre todos os animais.

A plataforma tambÃ©m deverÃ¡ aplicar isolamento entre organizaÃ§Ãµes no repositÃ³rio, rate limit para consultas pÃºblicas, IDs opacos, perfis `PUBLIC`, `AUTHORIZED` e `INTERNAL_LEGAL`, rotaÃ§Ã£o de credenciais, revogaÃ§Ã£o histÃ³rica, backup criptografado e resposta a incidentes.

Nenhum dado pessoal deve ser gravado diretamente na Solana. O desenho deve usar commitments, IDs pseudonimizados e conteÃºdo off-chain criptografado. Um pedido de eliminaÃ§Ã£o ou restriÃ§Ã£o de conteÃºdo off-chain nÃ£o deve alterar o hash on-chain; a aplicaÃ§Ã£o deve explicar que a prova criptogrÃ¡fica e o conteÃºdo detalhado possuem ciclos de vida diferentes.

O sistema nÃ£o deve alegar, sem base adicional, identidade biolÃ³gica do animal, verdade absoluta do GPS, calibraÃ§Ã£o correta de balanÃ§a, propriedade jurÃ­dica, conformidade sanitÃ¡ria ou validade de documento apenas porque o hash coincide.

## 11. Testes e critÃ©rios de aceitaÃ§Ã£o

A evoluÃ§Ã£o deve acrescentar testes negativos alÃ©m dos fluxos positivos. Os principais critÃ©rios sÃ£o:

- duas transiÃ§Ãµes concorrentes nÃ£o podem finalizar sobre a mesma versÃ£o;
- uma entrada nÃ£o pode ser consumida por duas transformaÃ§Ãµes;
- output duplicado deve ser rejeitado;
- massa fora da tolerÃ¢ncia deve bloquear a finalizaÃ§Ã£o;
- facility suspensa ou credencial revogada nÃ£o pode operar;
- intent expirada ou consumida nÃ£o pode ser reutilizada;
- alteraÃ§Ã£o de manifesto deve produzir hash diferente;
- queda de rede nÃ£o pode perder evidÃªncia local;
- retry idempotente nÃ£o pode criar duplicidade;
- recall deve localizar todos os descendentes dentro do limite operacional;
- consulta pÃºblica nÃ£o pode expor PII;
- o verificador deve marcar dependÃªncia indisponÃ­vel como `NOT_CHECKED`, nunca como sucesso;
- o fluxo v1, seus bytes e seus fixtures devem continuar passando.

Ainda serÃ¡ necessÃ¡rio executar testes de layout de contas, derivaÃ§Ã£o de PDA, tamanho de transaÃ§Ãµes, precompile Secp256r1, migraÃ§Ã£o v1 para v2, banco vazio, upgrade de banco, concorrÃªncia, recuperaÃ§Ã£o de backup, Agent offline, firmware, frontend, E2E de transformaÃ§Ã£o e recall.

## 12. Ordem recomendada de execuÃ§Ã£o

### Fase A â€” FundaÃ§Ã£o v2

Esta fase estÃ¡ implementada em grande parte. Ela inclui o crate de protocolo v2, o programa Anchor v2, configuraÃ§Ã£o, registries iniciais, `AssetState`, `IntentState`, `EventAnchor`, observaÃ§Ã£o fÃ­sica, API inicial, outbox Agent e codec frontend.

### Fase B â€” Identidade, migraÃ§Ã£o e custÃ³dia

A prÃ³xima fase deve concluir registry de Station e facility com ciclo de vida completo, governanÃ§a de autoridade, migraÃ§Ã£o explÃ­cita de animal v1, transferÃªncia de custÃ³dia, observaÃ§Ãµes de peso e localizaÃ§Ã£o e verificaÃ§Ã£o histÃ³rica de chaves.

### Fase C â€” Lotes e transformaÃ§Ã£o

Depois, devem ser implementados roots de composiÃ§Ã£o, lotes, split, merge, linhagem, abate, carcaÃ§as e o ciclo de transformaÃ§Ã£o com reserva, chunks, balanÃ§o de massa, finalizaÃ§Ã£o e aborto.

### Fase D â€” Produtos, embalagens e logÃ­stica

Em seguida, o sistema deverÃ¡ criar lotes de produto, subprodutos, embalagens, pallets, expediÃ§Ãµes, recebimento e aceites de custÃ³dia. Cada etapa deverÃ¡ preservar a linhagem e impedir consumo ou envio duplicado.

### Fase E â€” Recall, consulta pÃºblica e verificador

A fase posterior deve entregar recall reversÃ­vel, bloqueios, consulta pÃºblica por QR code, Ã¡rvores de linhagem com limites e verificador independente com camadas granulares.

### Fase F â€” Piloto controlado

O piloto somente deve comeÃ§ar depois de migraÃ§Ã£o sintÃ©tica, restauraÃ§Ã£o de backup, testes de queda de API/RPC/internet, rotaÃ§Ã£o de chave, facility suspensa, massa divergente, recall e consulta cross-tenant. O escopo inicial deve ser pequeno, com participantes identificados e procedimento manual de contingÃªncia.

## 13. Bloqueios para lanÃ§amento

O lanÃ§amento deve ser bloqueado se qualquer uma destas condiÃ§Ãµes ocorrer:

1. dados pessoais forem gravados no estado pÃºblico on-chain;
2. uma facility puder finalizar transformaÃ§Ã£o sem autorizaÃ§Ã£o de papel e escopo;
3. a mesma entrada puder ser consumida duas vezes;
4. o recall nÃ£o conseguir localizar descendentes;
5. o PostgreSQL puder declarar finalizaÃ§Ã£o antes da Solana;
6. transaÃ§Ã£o antiga puder vencer a supersessÃ£o sem nonce ou versÃ£o on-chain;
7. chave revogada puder autorizar novos eventos;
8. consulta pÃºblica permitir enumeraÃ§Ã£o de animais ou produtos;
9. backup nÃ£o puder ser restaurado;
10. o verificador tratar `NOT_CHECKED` como vÃ¡lido;
11. um evento corretivo apagar o histÃ³rico original;
12. uma integraÃ§Ã£o oficial for simulada sem identificaÃ§Ã£o clara;
13. o manifesto puder ser alterado depois do anchor;
14. o piloto nÃ£o possuir procedimento de operaÃ§Ã£o sem internet ou RPC.

## 14. ValidaÃ§Ã£o conhecida e pendÃªncias tÃ©cnicas

No estado publicado, foram validados o manifesto de arquivos, o whitespace do staged, a formataÃ§Ã£o Rust, os testes do envelope v2, a compilaÃ§Ã£o da API e a compilaÃ§Ã£o do programa `lastro-v2`. O checkout local foi sincronizado com `origin/main` apÃ³s a execuÃ§Ã£o de commits automÃ¡ticos de artefatos.

O `spec_check` completo deve ser executado novamente em um ambiente estÃ¡vel. Durante esta implementaÃ§Ã£o, a varredura do checkout montado ficou sujeita a bloqueios de I/O e o processo foi encerrado pelo ambiente. Isso Ã© uma limitaÃ§Ã£o de execuÃ§Ã£o do gate, nÃ£o uma evidÃªncia de que o gate completo passou.

TambÃ©m continuam pendentes a execuÃ§Ã£o completa do LiteSVM do programa v2, os testes de integraÃ§Ã£o da API v2 em PostgreSQL real, os testes de domÃ­nio v2 do Agent, o build de firmware em ESP32-C5 e os testes E2E do fluxo bovino completo. O cÃ³digo compilado Ã© uma fundaÃ§Ã£o; ainda nÃ£o representa o produto industrial completo.

## 15. ConclusÃ£o

A mudanÃ§a prevista nÃ£o Ã© apenas adicionar campos de peso, local e status ao animal. O sistema precisa representar uma cadeia de ativos derivados e preservar a relaÃ§Ã£o entre cada entrada e cada saÃ­da. O ponto mais importante Ã© modelar a transformaÃ§Ã£o como uma operaÃ§Ã£o verificÃ¡vel, com inputs reservados, outputs comprometidos, balanÃ§o de massa, linhagem e autorizaÃ§Ã£o da facility.

A estratÃ©gia mais segura continua sendo preservar o v1 e construir o v2 como protocolo e deployment separados. A Solana deve proteger invariantes e compromissos compactos. O PostgreSQL deve guardar o detalhe operacional. O Agent deve impedir perda de evidÃªncia offline. O frontend deve mostrar o fluxo sem prometer mais do que a prova permite. O verificador deve declarar os limites da evidÃªncia.

Quando todas as fases forem concluÃ­das, o Lastro poderÃ¡ oferecer uma matrÃ­cula digital de custÃ³dia e proveniÃªncia ao longo da cadeia bovina. Essa matrÃ­cula serÃ¡ forte tecnicamente porque permitirÃ¡ verificar identidade, transiÃ§Ãµes, documentos comprometidos, linhagem, massa, facility, embalagem, expediÃ§Ã£o e recall. Ela continuarÃ¡ dependendo de governanÃ§a, fontes autorizadas e procedimentos legais para qualquer afirmaÃ§Ã£o de propriedade ou conformidade oficial.

## ReferÃªncias

[1]: ./PLANO_ETAPA_3_SMART_CONTRACT.md "Plano da Etapa 3 â€” evoluÃ§Ã£o do protocolo e do smart contract Solana"
[2]: ./PLANO_ETAPA_4_SERVICOS_PRODUTO.md "Plano da Etapa 4 â€” evoluÃ§Ã£o dos serviÃ§os e do produto"
[3]: ./PLANO_ETAPA_5_SEGURANCA_ROLLOUT.md "Plano da Etapa 5 â€” seguranÃ§a, governanÃ§a, validaÃ§Ã£o e rollout"
[4]: ./BACKLOG_IMPLEMENTACAO_SMART_CONTRACT_V2.md "Backlog executÃ¡vel do smart contract Solana v2"
[5]: ./ESTUDO_RASTREABILIDADE_BOVINA.md "Estudo de evoluÃ§Ã£o do Lastro para rastreabilidade bovina"
[6]: ./ESTUDO_POS_ABATE_LINHAGEM.md "Adendo tÃ©cnico de pÃ³s-abate, desossa e linhagem de produtos"
[7]: ./IMPLEMENTACAO_ETAPA_P0_SMART_CONTRACT_V2.md "Registro da implementaÃ§Ã£o da Etapa P0"
[8]: ./IMPLEMENTACAO_ETAPA_4A_API_V2.md "Registro da integraÃ§Ã£o da API v2"

**Autor:** Manus AI

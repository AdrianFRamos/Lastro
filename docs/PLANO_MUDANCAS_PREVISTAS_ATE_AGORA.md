# Plano consolidado de mudanças previstas do Lastro

**Projeto:** Lastro
**Data:** 24 de setembro de 2026
**Escopo:** evolução do sistema de prova de identidade física e custódia individual para uma plataforma de rastreabilidade bovina, linhagem pós-abate e registro digital de custódia.
**Estado de referência:** a fundação do protocolo e do programa Solana v2 já foi implementada; os incrementos de domínio bovino completo continuam previstos.

## 1. Resumo executivo

O Lastro está sendo evoluído de um sistema focado em capturar um evento físico de RFID e registrar a custódia de um animal para um sistema capaz de acompanhar uma cadeia produtiva completa. O fluxo pretendido começa no animal vivo e pode seguir por lote, transporte, recebimento no frigorífico, abate, carcaça, desossa, cortes, subprodutos, lotes de produto, embalagens, expedições, recebimentos e recall.

A mudança central é separar corretamente três responsabilidades. A **Solana** deve manter o estado canônico mínimo e os invariantes que não podem ser substituídos silenciosamente. A **API e o PostgreSQL** devem guardar manifestos, documentos, projeções e dados operacionais detalhados. O **Agent e a Station** devem preservar evidências físicas mesmo quando o local estiver sem internet. O **verificador independente** deve recomputar as provas e informar exatamente quais propriedades foram comprovadas.

O sistema não deve tratar a blockchain como um banco para armazenar todos os dados da cadeia. O smart contract deve funcionar como um motor de transições, consumo, concorrência, hashes, raízes de composição, status, reservas e bloqueios. O manifesto completo de uma transformação, seus documentos e a lista detalhada de cortes permanecem fora da transação, protegidos por hashes e commitments ancorados na Solana.

Também é importante delimitar o significado de “matrícula de propriedade”. A arquitetura pode oferecer uma **matrícula digital de identidade técnica, custódia, proveniência e linhagem**. Ela não prova automaticamente propriedade jurídica, identidade biológica, conformidade sanitária ou veracidade absoluta de uma declaração. Essas propriedades exigem autoridades, documentos, procedimentos e integrações próprias.

## 2. Situação atual após a fundação v2

A versão v1 foi preservada para compatibilidade. O `StationEvent` v1 continua sendo o contrato físico de 276 bytes usado para RFID, origem, transferência de custódia e reidentificação. O programa Solana v1 continua separado do programa v2, e o v1 não deve receber campos bovinos novos dentro do evento legado.

A fundação v2 foi adicionada em paralelo. O programa `chain/programs/lastro-v2` possui uma superfície nova de configuração, registries, ativos, intents e ancoragem de observações. O protocolo compartilhado possui um envelope de evento de domínio com layout fixo, hashes com domínios separados e validação de versão, IDs e janela temporal.

A integração inicial também alcançou a API, o Agent e o frontend. A API possui uma migration v2 inicial, admissão de eventos de domínio, persistência idempotente e consulta de timeline. O Agent possui uma outbox SQLite separada, payloads seriais v2, ACK por hash, retry e quarentena. O frontend possui codec TypeScript do envelope v2 e teste de compatibilidade.

| Área | Situação atual | Limite atual |
|---|---|---|
| Programa v1 | Preservado e compilável | Continua limitado ao fluxo individual legado |
| Protocolo v2 | Envelope, hashes, enums e validações iniciais implementados | Ainda faltam canonicalização completa de manifestos e todos os tipos de evento |
| Programa Solana v2 | Configuração, registries iniciais, `AssetState`, `IntentState`, `EventAnchor` e observação física inicial implementados | Ainda faltam migração, lotes, transformação, produtos, expedição e recall |
| API | Admissão v2, armazenamento idempotente e timeline inicial implementados | O domínio completo de rastreabilidade ainda não está exposto |
| Agent | Outbox v2, transporte, retry e quarentena iniciais implementados | Ainda faltam adaptadores de balança, localização, manifesto e operação industrial |
| Firmware | Contrato v1 preservado | A máquina de sensores v2 ainda precisa ser implementada e validada em hardware |
| Frontend | Codec v2 e integração inicial implementados | Ainda faltam jornadas operacionais e página pública de produtos |
| Verificador | Verificação legada e limites atuais preservados | O verificador de linhagem, massa, facility e recall v2 ainda precisa ser criado |
| Operação | Manifesto e gates básicos existentes | Ainda faltam piloto, runbooks, backup restaurável e governança operacional |

## 3. Arquitetura alvo

A arquitetura alvo terá duas versões convivendo durante a migração:

```text
Lastro v1
  StationEvent de 276 bytes
  AnimalState e RfidBinding legados
  origem, transferência e reidentificação

Lastro v2
  DomainEventEnvelope
  ProtocolConfigV2 e registries
  AssetState, EventAnchor e IntentState
  lotes, carcaças, transformações, produtos e recall
```

A consulta poderá atravessar as duas versões. Um animal criado no v1 poderá receber uma prova explícita de migração para um ativo v2. Essa migração deve copiar identidade, custódia, sequência, revisão e último hash conhecidos. Ela não pode inventar peso, localização, status sanitário ou documento que não existiam no histórico legado.

A divisão de autoridade prevista é a seguinte:

| Fonte | Responsabilidade |
|---|---|
| Station ou balança | Assinar observações físicas, como RFID e pesagem, quando o equipamento possuir essa capacidade |
| Carteira do custodiante | Autorizar transferência e aceite de custódia |
| Facility autorizada | Autorizar recebimento, abate, transformação, produção, expedição e ações operacionais de recall |
| API e PostgreSQL | Receber requests, guardar manifestos, documentos, projeções e evidências detalhadas |
| Solana v2 | Aplicar invariantes, versão, predecessor, nonce, reservas, consumo, status e anchors |
| Verificador | Recalcular hashes, conferir contas, reconstruir linhagem e separar propriedades válidas de propriedades não verificadas |

## 4. Modelo de ativos e linhagem

O v2 utilizará um `AssetState` genérico para evitar que animal, carcaça e produto tenham fontes canônicas incompatíveis. Os tipos previstos são `ANIMAL`, `LOT`, `CARCASS`, `CUT_BATCH`, `PRODUCT_LOT`, `PACKAGE`, `BYPRODUCT_LOT` e `SHIPMENT`.

Cada ativo deverá possuir, no mínimo, um ID de 32 bytes, tipo, status, deployment, custodiante, raiz de pai, raiz de linhagem, lote atual, saldo de peso, sequência, versão de estado, último hash de evento e dados de reserva. Esses campos são suficientes para aplicar regras de concorrência e disponibilidade sem colocar o manifesto completo na blockchain.

A linhagem será um grafo dirigido de relações pai-filho. Um animal pode gerar uma ou mais carcaças conforme a política do processo. Uma carcaça pode gerar vários lotes de cortes. Um lote pode gerar diversas embalagens e expedições. Subprodutos e perdas devem ser representados explicitamente para que a transformação não pareça simplesmente “multiplicar” massa.

O detalhe da árvore ficará no PostgreSQL e no EvidencePackage. A Solana guardará roots e anchors que permitam verificar se o manifesto consultado é o mesmo que foi comprometido. O verificador deverá ser capaz de navegar para trás, até o animal ou lote de origem, e para frente, até produtos, embalagens e expedições afetadas.

## 5. Pós-abate e transformação

O frigorífico não deve registrar uma desossa como uma instrução única contendo centenas de entradas e saídas. A operação será dividida em uma máquina de estados:

1. `confirm_slaughter` confirma o recebimento e o abate por uma facility autorizada.
2. `create_carcass` cria os ativos de carcaça relacionados ao animal abatido.
3. `record_carcass_weight` registra a pesagem da carcaça por fonte autorizada.
4. `begin_transformation` cria o manifesto de transformação com facility, roots, quantidades, pesos, tolerância e expiração.
5. `reserve_transformation_input` reserva cada entrada para impedir consumo concorrente.
6. `append_transformation_chunk` registra chunks de entradas e saídas quando a lista for grande.
7. `finalize_transformation` valida roots, contagens, pesos, tolerância, facility, reservas e outputs antes de consumir as entradas.
8. `create_product_lot`, `create_package_commitment` e `create_byproduct_lot` materializam os ativos derivados.
9. `abort_transformation` encerra uma tentativa expirada ou cancelada sem apagar o manifesto original.

A regra de balanço de massa será explícita:

```text
peso de entradas
  = peso de produtos
  + peso de subprodutos
  + peso de perdas
  dentro da tolerância aprovada
```

O smart contract deve validar os números compactos e os commitments. O manifesto detalhado deve explicar quais cortes foram produzidos, em qual quantidade, com quais unidades e quais documentos sustentam a operação. Uma alteração posterior deve criar um evento corretivo ou de invalidação; nunca deve sobrescrever silenciosamente a transformação original.

## 6. Mudanças previstas no protocolo e no smart contract

### 6.1 Protocolo compartilhado

Ainda estão previstos os seguintes componentes no `lastro-protocol`:

- wrappers e validações para `AssetId`, `EventId`, `FacilityId`, `PartyId`, `TransformationId`, `IntentId` e `RecallId`;
- canonicalização determinística de manifestos;
- roots de composição com algoritmo documentado, preferencialmente Merkle quando provas parciais forem necessárias;
- envelopes para transferência, transformação, migração, recall e eventos de qualidade;
- fixtures Rust, TypeScript e JSON para todos os bytes e hashes críticos;
- rejeição explícita de schemas futuros, enums desconhecidos, unidades inválidas e janelas temporais inválidas;
- regras compartilhadas para balanço de massa, predecessor, versão de estado e consumo único.

A canonicalização precisa fixar ordem de campos, unidades, tratamento de opcionais, ordenação de arrays e representação de inteiros. Não será suficiente serializar JSON sem política determinística, porque duas linguagens poderiam gerar hashes diferentes para o mesmo manifesto lógico.

### 6.2 Contas e registries

O programa v2 deverá evoluir os registries já iniciados:

- `ProtocolConfigV2` para parâmetros de deployment e limites seguros;
- `StationRegistry` para chave P-256, validade, firmware, suspensão, revogação e substituição;
- `FacilityRegistry` para tipo de instalação, credencial, validade e status;
- `PartyRegistry` para carteiras e papéis compactos sem PII;
- registry de documentos ou commitments de documentos;
- `AssetState` para ativos vivos e derivados;
- `EventAnchor` para compromissos imutáveis de eventos;
- `IntentState` para nonce, actor, payload hash, expiração, cancelamento e consumo;
- `LineageAnchor`, `TransformationAnchor` e `RecallState` para os fluxos posteriores.

A validade histórica de uma chave deve ser preservada. Revogar uma Station hoje deve impedir eventos futuros, mas não invalidar automaticamente um evento que foi produzido enquanto a chave estava válida.

### 6.3 Instruções de domínio

A superfície futura deve possuir uma instrução Anchor específica por transição relevante. A intenção é evitar uma instrução genérica que receba `event_type` e permita contornar os guards.

As instruções previstas incluem migração do animal v1, alteração de status, observação física, observação de localização, transferência de custódia, criação e divisão de lotes, merge, confirmação de abate, carcaça, transformação, produto, embalagem, expedição, aceite, quality hold, recall e retirada terminal.

Toda instrução mutável deverá validar `expected_state_version`, `expected_previous_event_hash` e a intent ou nonce correspondente. Depois da aplicação, a versão deve avançar exatamente uma vez e o último hash deve apontar para o novo evento. Isso impede que uma transação antiga seja aceita depois de uma operação concorrente.

## 7. Mudanças previstas na API e no PostgreSQL

A migration v2 inicial é uma fundação. O modelo completo deverá ser incrementado sem quebrar as tabelas v1. A sequência planejada inclui assets, parties e papéis, documentos, eventos de domínio, lotes, linhagem, transformações, carcaças, produtos, embalagens, expedições, quality holds, recalls, anchors e reconciliação.

As entidades principais serão:

- `assets`, com uma projeção comum de todos os objetos rastreáveis;
- `parties` e `party_roles`, com escopo por organização, facility e operação;
- `official_documents`, com hash, emissor, validade e status de verificação;
- `domain_events`, append-only e ligados a `AssetState` e `EventAnchor`;
- `lot_manifests` e `lineage_edges`, para composição e relações pai-filho;
- `transformation_manifests`, `transformation_inputs` e `transformation_outputs`;
- tabelas especializadas de carcaças, produtos, subprodutos, embalagens e expedições;
- `quality_holds`, `recalls` e seus ativos afetados;
- projeções públicas com somente os campos autorizados.

A API deverá deixar de oferecer alteração livre de estado canônico. Peso, localização, status, movimento, abate, transformação, expedição e recall devem ser comandos de domínio que criam eventos. O fluxo de escrita será:

```text
request
  -> autenticação e autorização
  -> criação da intent
  -> canonicalização do payload
  -> assinatura apropriada
  -> transação Solana
  -> confirmação finalizada
  -> projeção PostgreSQL
```

Os DTOs públicos não devem ser derivados automaticamente dos DTOs internos. Dados como CPF, CNPJ, nomes legais, carteiras, coordenadas precisas, preços, rotas e documentos completos devem permanecer protegidos.

Também será necessário um reconciliador server-side. Ele deverá localizar intents pendentes, consultar a Solana, verificar anchors, atualizar projeções, liberar reservas, detectar divergências entre PostgreSQL e blockchain e registrar incidentes sem apagar versões anteriores.

## 8. Mudanças previstas no Agent e no firmware

O Agent manterá o outbox RFID v1 separado do outbox de domínio v2. Uma observação física, um manifesto de transformação e uma expedição não devem compartilhar uma mesma linha com semântica ambígua.

A outbox v2 deverá conservar bytes originais, hash, tipo, versão, estado de entrega, tentativas e erro. Falhas de timeout, 5xx, 429, RPC indisponível ou perda de serial devem ser reprocessáveis. Payload inválido, assinatura inválida, unidade inválida, estado incompatível, asset inexistente ou manifesto já finalizado devem ir para quarentena e exigir uma nova decisão de domínio.

No firmware, o `StationEvent` v1 continuará isolado. Os sensores v2 deverão possuir adapters próprios para balança, localização, relógio, impressão e journal local. A máquina de estados prevista é:

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

Uma pesagem só deverá ser aceita quando estiver estável, com unidade conhecida, calibração válida, timestamp monotônico e identificador do equipamento. O backend não pode enviar peso, coordenada ou novo RFID para a Station e tratar esses valores como observação local.

Antes de um piloto físico ainda será necessário definir leitor RFID, balança, pinout, frames, secure boot, eFuse, chave da Station, rotação de chaves, atualização de firmware, certificado de calibração e procedimento de troca do equipamento.

## 9. Mudanças previstas no frontend e no verificador

A `DemoPage.vue` deve continuar representando o fluxo individual atual, mas não deve concentrar toda a operação industrial. O produto deverá ganhar jornadas separadas para animal, lote, movimento, recebimento, abate, carcaça, transformação, produto, embalagem, expedição, recall e consulta pública.

A jornada de transformação deverá mostrar entradas, peso total, outputs, subprodutos, perdas, tolerância, diferença calculada, status, hash do manifesto e transação Solana. O botão de finalização deve ficar bloqueado quando houver input não reservado, output duplicado, massa fora da tolerância, facility suspensa, documento obrigatório ausente, transformação expirada ou carteira não autorizada.

O verificador deverá ampliar as camadas atuais. Além de RFID, assinatura Station, continuidade de identidade e custódia, ele deverá verificar hashes de eventos de domínio, linhagem, balanço de massa, credencial da facility, hashes documentais, recall, estado on-chain e confirmação de fonte oficial.

O resultado deve ser granular. Exemplos de estados esperados são `VALID`, `INVALID`, `NOT_CHECKED` e `NOT_DETERMINED`. A interface não deve exibir um único `VERIFIED: true`, porque uma assinatura válida não comprova automaticamente a linhagem, a balança, o documento ou a propriedade jurídica.

A consulta pública por QR code deverá usar IDs opacos e retornar somente um resumo seguro do produto ou embalagem. O perfil público pode mostrar data, tipo, validade, região generalizada, status de qualidade, resumo da linhagem, prova criptográfica e recall. Não deve revelar PII, preço, rota completa ou coordenada precisa.

## 10. Segurança, privacidade e governança

A versão v2 deverá separar DeploymentAuthority, StationKey, ScaleKey, FacilityCredential, CustodianWallet, AgentCredential, OperatorIdentity e OfficialSourceKey. Uma identidade não pode representar todas as outras.

A autorização deve combinar RBAC, que define o papel geral, com ABAC, que define organização, facility, ativo, período, operação e estado atual. Autenticação não é autorização. Uma carteira de frigorífico não pode obter autoridade global sobre todos os animais.

A plataforma também deverá aplicar isolamento entre organizações no repositório, rate limit para consultas públicas, IDs opacos, perfis `PUBLIC`, `AUTHORIZED` e `INTERNAL_LEGAL`, rotação de credenciais, revogação histórica, backup criptografado e resposta a incidentes.

Nenhum dado pessoal deve ser gravado diretamente na Solana. O desenho deve usar commitments, IDs pseudonimizados e conteúdo off-chain criptografado. Um pedido de eliminação ou restrição de conteúdo off-chain não deve alterar o hash on-chain; a aplicação deve explicar que a prova criptográfica e o conteúdo detalhado possuem ciclos de vida diferentes.

O sistema não deve alegar, sem base adicional, identidade biológica do animal, verdade absoluta do GPS, calibração correta de balança, propriedade jurídica, conformidade sanitária ou validade de documento apenas porque o hash coincide.

## 11. Testes e critérios de aceitação

A evolução deve acrescentar testes negativos além dos fluxos positivos. Os principais critérios são:

- duas transições concorrentes não podem finalizar sobre a mesma versão;
- uma entrada não pode ser consumida por duas transformações;
- output duplicado deve ser rejeitado;
- massa fora da tolerância deve bloquear a finalização;
- facility suspensa ou credencial revogada não pode operar;
- intent expirada ou consumida não pode ser reutilizada;
- alteração de manifesto deve produzir hash diferente;
- queda de rede não pode perder evidência local;
- retry idempotente não pode criar duplicidade;
- recall deve localizar todos os descendentes dentro do limite operacional;
- consulta pública não pode expor PII;
- o verificador deve marcar dependência indisponível como `NOT_CHECKED`, nunca como sucesso;
- o fluxo v1, seus bytes e seus fixtures devem continuar passando.

Ainda será necessário executar testes de layout de contas, derivação de PDA, tamanho de transações, precompile Secp256r1, migração v1 para v2, banco vazio, upgrade de banco, concorrência, recuperação de backup, Agent offline, firmware, frontend, E2E de transformação e recall.

## 12. Ordem recomendada de execução

### Fase A — Fundação v2

Esta fase está implementada em grande parte. Ela inclui o crate de protocolo v2, o programa Anchor v2, configuração, registries iniciais, `AssetState`, `IntentState`, `EventAnchor`, observação física, API inicial, outbox Agent e codec frontend.

### Fase B — Identidade, migração e custódia

A próxima fase deve concluir registry de Station e facility com ciclo de vida completo, governança de autoridade, migração explícita de animal v1, transferência de custódia, observações de peso e localização e verificação histórica de chaves.

### Fase C — Lotes e transformação

Depois, devem ser implementados roots de composição, lotes, split, merge, linhagem, abate, carcaças e o ciclo de transformação com reserva, chunks, balanço de massa, finalização e aborto.

### Fase D — Produtos, embalagens e logística

Em seguida, o sistema deverá criar lotes de produto, subprodutos, embalagens, pallets, expedições, recebimento e aceites de custódia. Cada etapa deverá preservar a linhagem e impedir consumo ou envio duplicado.

### Fase E — Recall, consulta pública e verificador

A fase posterior deve entregar recall reversível, bloqueios, consulta pública por QR code, árvores de linhagem com limites e verificador independente com camadas granulares.

### Fase F — Piloto controlado

O piloto somente deve começar depois de migração sintética, restauração de backup, testes de queda de API/RPC/internet, rotação de chave, facility suspensa, massa divergente, recall e consulta cross-tenant. O escopo inicial deve ser pequeno, com participantes identificados e procedimento manual de contingência.

## 13. Bloqueios para lançamento

O lançamento deve ser bloqueado se qualquer uma destas condições ocorrer:

1. dados pessoais forem gravados no estado público on-chain;
2. uma facility puder finalizar transformação sem autorização de papel e escopo;
3. a mesma entrada puder ser consumida duas vezes;
4. o recall não conseguir localizar descendentes;
5. o PostgreSQL puder declarar finalização antes da Solana;
6. transação antiga puder vencer a supersessão sem nonce ou versão on-chain;
7. chave revogada puder autorizar novos eventos;
8. consulta pública permitir enumeração de animais ou produtos;
9. backup não puder ser restaurado;
10. o verificador tratar `NOT_CHECKED` como válido;
11. um evento corretivo apagar o histórico original;
12. uma integração oficial for simulada sem identificação clara;
13. o manifesto puder ser alterado depois do anchor;
14. o piloto não possuir procedimento de operação sem internet ou RPC.

## 14. Validação conhecida e pendências técnicas

No estado publicado, foram validados o manifesto de arquivos, o whitespace do staged, a formatação Rust, os testes do envelope v2, a compilação da API e a compilação do programa `lastro-v2`. O checkout local foi sincronizado com `origin/main` após a execução de commits automáticos de artefatos.

O `spec_check` completo deve ser executado novamente em um ambiente estável. Durante esta implementação, a varredura do checkout montado ficou sujeita a bloqueios de I/O e o processo foi encerrado pelo ambiente. Isso é uma limitação de execução do gate, não uma evidência de que o gate completo passou.

Também continuam pendentes a execução completa do LiteSVM do programa v2, os testes de integração da API v2 em PostgreSQL real, os testes de domínio v2 do Agent, o build de firmware em ESP32-C5 e os testes E2E do fluxo bovino completo. O código compilado é uma fundação; ainda não representa o produto industrial completo.

## 15. Conclusão

A mudança prevista não é apenas adicionar campos de peso, local e status ao animal. O sistema precisa representar uma cadeia de ativos derivados e preservar a relação entre cada entrada e cada saída. O ponto mais importante é modelar a transformação como uma operação verificável, com inputs reservados, outputs comprometidos, balanço de massa, linhagem e autorização da facility.

A estratégia mais segura continua sendo preservar o v1 e construir o v2 como protocolo e deployment separados. A Solana deve proteger invariantes e compromissos compactos. O PostgreSQL deve guardar o detalhe operacional. O Agent deve impedir perda de evidência offline. O frontend deve mostrar o fluxo sem prometer mais do que a prova permite. O verificador deve declarar os limites da evidência.

Quando todas as fases forem concluídas, o Lastro poderá oferecer uma matrícula digital de custódia e proveniência ao longo da cadeia bovina. Essa matrícula será forte tecnicamente porque permitirá verificar identidade, transições, documentos comprometidos, linhagem, massa, facility, embalagem, expedição e recall. Ela continuará dependendo de governança, fontes autorizadas e procedimentos legais para qualquer afirmação de propriedade ou conformidade oficial.

## Referências

[1]: ./PLANO_ETAPA_3_SMART_CONTRACT.md "Plano da Etapa 3 — evolução do protocolo e do smart contract Solana"
[2]: ./PLANO_ETAPA_4_SERVICOS_PRODUTO.md "Plano da Etapa 4 — evolução dos serviços e do produto"
[3]: ./PLANO_ETAPA_5_SEGURANCA_ROLLOUT.md "Plano da Etapa 5 — segurança, governança, validação e rollout"
[4]: ./BACKLOG_IMPLEMENTACAO_SMART_CONTRACT_V2.md "Backlog executável do smart contract Solana v2"
[5]: ./ESTUDO_RASTREABILIDADE_BOVINA.md "Estudo de evolução do Lastro para rastreabilidade bovina"
[6]: ./ESTUDO_POS_ABATE_LINHAGEM.md "Adendo técnico de pós-abate, desossa e linhagem de produtos"
[7]: ./IMPLEMENTACAO_ETAPA_P0_SMART_CONTRACT_V2.md "Registro da implementação da Etapa P0"
[8]: ./IMPLEMENTACAO_ETAPA_4A_API_V2.md "Registro da integração da API v2"

**Autor:** Manus AI

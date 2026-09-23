# Análise técnica atualizada do projeto Lastro

**Projeto:** Lastro  
**Checkout analisado:** `main` em `b8546f3` (`first commit`) com alterações locais não commitadas  
**Data da atualização:** 23 de setembro de 2026  
**Escopo:** organização do repositório, arquitetura, código, contratos, fluxo ponta a ponta, segurança, alterações de hardening, testes, execução local e limitações verificáveis.

> **Conclusão executiva.** O Lastro é um sistema de rastreabilidade de um ativo físico cujo estado digital só deve avançar quando cinco coisas concordam: uma leitura RFID observada por uma Station, um evento binário assinado, uma admissão durável pelo Agent e pela API, uma autorização explícita do custodiante e uma transação finalizada na Solana. A atualização recente fortaleceu principalmente a fronteira de autorização, a recuperação de operações, os limites de recursos e a verificação independente. Ela também deixou mais explícitas algumas limitações que ainda impedem alegar segurança física ou produção completa.

## 1. O que o projeto faz

O Lastro transforma uma observação física de RFID em um histórico de identidade e custódia verificável. O ativo inicial do domínio é um animal, mas o protocolo não depende de biologia: ele trabalha com um `AnimalID` lógico, uma identificação física RFID e uma sequência de transições.

A ideia central é não permitir que uma aplicação altere simplesmente os campos “RFID atual” ou “custodiante atual”. Cada mudança precisa ser explicada por um `StationEvent` de bytes fixos, assinado pela Station, encadeado ao evento anterior e aceito pelo programa Lastro na Solana.

O fluxo principal é:

```text
identidade visual ou registro off-chain
  -> intenção de captura
  -> challenge de autorização assinado pela carteira
  -> captura física reservada
  -> comando Agent -> Station
  -> RFID realmente observado
  -> StationEvent de 276 bytes
  -> assinatura P-256 da Station
  -> outbox SQLite do Agent
  -> admissão criptográfica na API
  -> PostgreSQL como projeção/evidência off-chain
  -> transação de duas instruções na Solana
  -> assinatura da carteira do custodiante
  -> confirmação confirmed
  -> confirmação finalized
  -> projeção local atualizada
  -> EvidencePackage verificável independentemente
```

As únicas ações de domínio são:

| Ação | Finalidade |
|---|---|
| `ORIGIN` | Criar a primeira identidade on-chain, vinculando o primeiro RFID e custodiante. |
| `TRANSFER` | Trocar o custodiante sem trocar o RFID. |
| `REIDENTIFY` | Trocar o RFID sem trocar o `AnimalID` nem o custodiante. |

O sistema separa cinco autoridades. A Station é autoridade sobre o fato de ter observado e assinado uma leitura. O Agent é responsável pelo transporte e pela durabilidade local. A API coordena a captura e valida a admissão, mas não é a autoridade canônica. A carteira demonstra quem autorizou a transição. A Solana mantém o estado canônico de custódia, sequência, revisão e vínculo RFID.

Essa separação é o principal valor arquitetural do projeto. Ela evita que um banco ou uma interface seja confundido com a fonte definitiva de verdade.

## 2. O que mudou desde a análise anterior

A alteração atual não é apenas cosmética. Ela adiciona uma camada de autorização anterior à captura física e torna os caminhos de falha mais explícitos.

| Área | Situação anterior | Situação atual |
|---|---|---|
| Autorização de captura | A API podia reservar uma captura a partir da intenção HTTP. | A API emite um challenge de curta duração e exige assinatura da carteira correspondente ao custodiante exigido antes de criar a captura. |
| Repetição de operação | Havia idempotência para algumas repetições. | Repetição da mesma intenção continua idempotente; a substituição de uma captura `REIDENTIFY` aceita exige um `supersedeCaptureId` explícito e uma nova autorização. |
| Captura física abandonada | O Agent podia ficar dependente de uma Station presa em `WAIT_RFID`. | A Station tem deadline monotônico de 180 segundos e gera erro de timeout recuperável. |
| Outbox | Estados locais cobriam entrega e finalização. | Existe estado `QUARANTINED` para falhas terminais. Evidência imutável não pode voltar ao fluxo nem ser transformada em evidência nova. |
| Transação Solana | O navegador aguardava confirmação usando apenas a assinatura persistida. | O envelope assinado retém `lastValidBlockHeight`; o navegador pode rebroadcastar os mesmos bytes e reconhecer expiração antes de pedir nova assinatura. |
| Evidência aceita sem assinatura de carteira | Podia ficar aguardando assinatura de transação. | A UI mantém a evidência física aceita, mas permite autorizar novamente a mesma transição sem repetir a observação física. |
| Limites de recursos | Os limites estavam menos explícitos. | Existem budgets de challenges, retenção limitada, corpo HTTP de 1024 bytes, máximo de 128 eventos no verificador e orçamento total de RPC de 30 segundos. |
| Verificação on-chain independente | Comparava contas e estado terminal. | Também exige deployment confiável e autoridade do `ProtocolConfig` provisionados independentemente; sem essa âncora, o resultado é `NOT_CHECKED`. |
| Inicialização | Podia construir uma transação de inicialização antes de a conta do pagador estar observavelmente financiada. | O script espera funding em commitment `finalized` antes de enviar `initialize`. |
| Reprodutibilidade | Dependia mais do ambiente instalado. | CI instala Anchor 1.2.0 e Solana CLI 4.2.0 com checksum fixo e verifica versões. |

O hardening é coerente com o objetivo do sistema: proteger não apenas a criptografia, mas também o consumo de recursos, o replay, a recuperação após crash e a diferença entre “evidência física aceita” e “transição canônica finalizada”.

## 3. Organização do monorepo

O projeto é um monorepo com dois workspaces Rust separados e um workspace npm. A separação do workspace de protocolo/API/Agent do workspace Anchor é necessária porque o programa Solana tem dependências e artefatos de build próprios.

| Diretório | Papel | Tecnologia |
|---|---|---|
| `crates/lastro-protocol` | Contrato puro de bytes, hashes, IDs, eventos, assinatura e pacote de evidência. | Rust |
| `firmware/station` | Firmware da Station, máquina de estados, transporte serial, assinatura e fronteira RFID. | C / ESP-IDF |
| `chain` | Programa on-chain, contas Anchor, PDAs e transições canônicas. | Rust / Anchor / Solana |
| `services/agent` | Ponte entre Station e API, serial, validação, retry e SQLite outbox. | Rust / Tokio / SQLite |
| `services/api` | HTTP, PostgreSQL, admissão criptográfica, RPC Solana e construção de transações. | Rust / Axum / SQLx |
| `apps/web` | Console do operador, Wallet Standard, assinatura de captura, transação e verificador. | Vue / TypeScript / Vite |
| `hardware-simulator` | Station simulada e ponte de hardware para desenvolvimento. | Python |
| `schemas` | Contratos públicos HTTP e de evidência. | OpenAPI / JSON Schema |
| `test-vectors` | Bytes e pacotes congelados para compatibilidade entre linguagens. | Binário / JSON |
| `tests/repository` | Verificações estruturais e de consistência do checkout. | Python / pytest |
| `tests/system` | Fluxos locais de sistema, recovery, tampering e estabilidade. | Python / pytest |
| `docs` | Arquitetura, protocolo, desenvolvimento, segurança, hardware, testes e hardening. | Markdown |
| `infra` | Docker Compose e imagens dos serviços. | Docker |
| `scripts` | Bootstrap, doctor, inicialização, vetores, ambiente local e ferramentas derivadas. | Python / shell |
| `.github/workflows` | CI, E2E, chain, firmware, Devnet e sincronização de artefatos. | GitHub Actions |

Os principais arquivos de entrada são `README.md`, `docs/ARCHITECTURE.md`, `docs/PROTOCOL.md`, `docs/API.md`, `docs/SECURITY.md`, `docs/HARDENING_STATUS.md`, `schemas/openapi.yaml`, `schemas/evidence-package.schema.json`, `Cargo.toml`, `package.json` e `Makefile`.

O relatório está em `docs/ANALISE_PROJETO.md`, porque os gates estruturais do projeto não permitem Markdown de análise solto na raiz.

## 4. Stack atual e versões fixadas

A configuração atual tenta tornar o build reproduzível e reduzir divergências entre máquina local e CI.

| Camada | Versão ou tecnologia atual |
|---|---|
| Rust | 1.98.1, edition 2024 |
| Anchor CLI | 1.2.0 |
| Solana CLI | 4.2.0 |
| Anchor runtime/dependências | `anchor-lang = 1.2.0` |
| Testes on-chain | LiteSVM 0.16.0 com precompiles |
| Firmware | ESP-IDF 6.1.x, alvo ESP32-C5 |
| API/Agent | Axum, Tokio, SQLx 0.9, reqwest |
| Banco principal | PostgreSQL 18.6 no Compose/CI |
| Banco local do Agent | SQLite com WAL e `synchronous=FULL` |
| Node | 24.21.0 |
| npm | 11.19.0 |
| TypeScript | 6.0.2 |
| Vue | 3.5.43 |
| Vite | 8.2.2 |
| Solana no navegador | `@solana/kit` 8.3.0 e plugins RPC/Wallet |
| Assinatura P-256 web | `@noble/curves` 2.4.0 |
| Testes web | Vitest 5.0.1, Vue Test Utils, Playwright 1.63.0 |
| Testes Python | pytest e `cryptography` |

Os instaladores `scripts/install_pinned_anchor_cli.sh` e `scripts/install_pinned_solana_cli.sh` baixam binários específicos, conferem SHA-256 e confirmam a versão executável. Isso é mais forte do que apenas dizer “use Anchor” ou “use Solana”, pois a geração de SBF, o layout de transações e os testes dependem do toolchain exato.

A CI também audita os dois lockfiles Rust e o lockfile npm. Ainda assim, checksum de ferramenta não substitui auditoria de segurança do binário baixado; ele garante que o CI recebeu exatamente o artefato declarado.

## 5. O protocolo canônico

### 5.1 `StationEvent`: o objeto central

O `StationEvent` tem exatamente 276 bytes. O layout não depende do layout de memória de C ou Rust. Cada campo é gravado e lido por offset explícito.

| Offset | Tamanho | Campo |
|---:|---:|---|
| `0` | 4 | Magic `LSTR` |
| `4` | 1 | Versão `1` |
| `5` | 1 | Ação: `1`, `2` ou `3` |
| `6` | 2 | Reservado, sempre zero |
| `8` | 32 | `deployment_id` |
| `40` | 32 | `animal_id` |
| `72` | 32 | `station_id` |
| `104` | 8 | `event_sequence`, little-endian |
| `112` | 4 | `identity_revision`, little-endian |
| `116` | 32 | `previous_event_hash` |
| `148` | 32 | `old_rfid_hash` |
| `180` | 32 | `new_rfid_hash` |
| `212` | 32 | `from_custodian` |
| `244` | 32 | `to_custodian` |
| `276` | — | Fim |

O mesmo layout aparece em quatro implementações independentes:

- Rust em `crates/lastro-protocol/src/event.rs`;
- C em `firmware/station/components/lastro_station/event.c`;
- Rust on-chain em `chain/programs/lastro/src/verify/event.rs`;
- TypeScript em `apps/web/src/protocol/stationEvent.ts`.

A atualização preserva uma regra importante: o programa on-chain opera sobre os 276 bytes originais. Ele não desserializa bytes arbitrários em uma struct e depois não reserializa o evento antes de calcular o hash. Isso evita que uma mudança de representação altere o material assinado.

### 5.2 Semântica das três ações

`ORIGIN` exige sequência 1, revisão 1, predecessor zero, RFID antigo zero, custodiante de origem zero, RFID novo não-zero e custodiante destino não-zero.

`TRANSFER` exige sequência a partir de 2, revisão preservada, predecessor não-zero, RFID antigo igual ao RFID novo, custodiante de origem não-zero, custodiante destino não-zero e destinos diferentes. O destino não é aceito como autoridade apenas porque veio no JSON: a carteira exigida é a do custodiante atual, e a conta signer da transação é verificada.

`REIDENTIFY` exige sequência a partir de 2, revisão pelo menos 2, predecessor não-zero, RFID antigo e novo não-zero e diferentes, além de custodiante de origem igual ao custodiante destino. O antigo binding fica `RETIRED` e um novo binding `ACTIVE` é criado.

A continuidade entre eventos exige que a sequência aumente exatamente uma unidade, que o `previous_event_hash` seja o hash do evento anterior, que o RFID antigo seja o RFID novo anterior e que a revisão só aumente em `REIDENTIFY`.

### 5.3 Identificadores e hashes

O `AnimalID` é aleatório. Ele não é derivado do RFID. Essa decisão mantém a identidade lógica do animal quando o brinco é perdido ou substituído.

O RFID físico passa por uma fronteira de canonicalização. O adaptador deve validar o frame do leitor, extrair o identificador lógico e representá-lo como oito bytes big-endian. O hash usa domain separation:

```text
rfid_hash  = SHA-256("LASTRO_RFID\0" || canonical_rfid[8])
station_id = SHA-256("LASTRO_STATION\0" || compressed_p256_pubkey[33])
event_hash  = SHA-256(StationEvent[276])
```

A assinatura P-256 é feita sobre os 276 bytes do evento. O `event_hash` é usado para encadeamento, mas não substitui o evento como mensagem assinada. Essa diferença é necessária porque o precompile Secp256r1 da Solana também precisa apontar para os 276 bytes originais.

### 5.4 Assinatura da Station

A Station usa uma chave P-256 SEC1 comprimida de 33 bytes e assinatura compacta `r || s` de 64 bytes. O protocolo exige assinatura low-S. Rust, Agent, API, navegador e programa on-chain rejeitam uma assinatura high-S ou incompatível.

A chave pública também é vinculada ao `station_id`. O `ProtocolConfig` armazena uma chave Station imutável por deployment. Portanto, uma chave diferente não pode simplesmente aparecer em um `EVENT_READY` e ser aceita.

### 5.5 Envelope serial LSTR

A comunicação Station-Agent usa o envelope:

```text
magic[4] | version[1] | type[1] | reserved[2]
        | payload_len[4 little-endian] | payload | crc32c[4 little-endian]
```

A CRC32C/Castagnoli cobre versão, tipo, reservado, tamanho e payload. O parser rejeita tipo inválido, tamanho inesperado, payload acima do limite e CRC incorreta. Ele também descarta bytes até reencontrar o magic para recuperar sincronização depois de ruído serial.

| Tipo | Payload |
|---:|---:|
| `COMMAND` | 224 bytes |
| `EVENT_READY` | 397 bytes |
| `ACK` | 48 bytes |
| `ERROR` | 20 bytes |

O `COMMAND` contém apenas contexto esperado. Ele não contém o RFID novo observado. O `EVENT_READY` contém o evento, o RFID observado, a chave pública e a assinatura. O Agent só envia `ACK` depois de gravar a evidência no SQLite.

### 5.6 `EvidencePackage`

O pacote exportado por `/api/animals/{animalId}/evidence-package` contém apenas evidência original:

- versão;
- deployment;
- `AnimalID`;
- eventos com bytes do `StationEvent` em Base64;
- RFID observado;
- chave pública Station;
- assinatura Station;
- assinatura da transação Solana, quando existir.

O pacote deliberadamente não possui um campo `valid` produzido pelo backend. O verificador recalcula tudo. Isso impede que o servidor declare válido um pacote cuja assinatura, histórico ou estado canônico não corresponda aos bytes.

O pacote agora tem um limite de verificação de 128 eventos no navegador. Esse limite protege a memória, o tempo e o orçamento de RPC. Ele é um limite operacional do verificador, não uma afirmação de que o protocolo histórico só possa ter 128 eventos.

## 6. Fluxo ponta a ponta atual

### 6.1 Criação ou recuperação

`POST /api/animals` cria uma associação off-chain entre um `AnimalID` aleatório e um `visualRecoveryId`. Essa chamada não cria `AnimalState`, não atribui RFID e não estabelece custodiante.

A recuperação visual permite reencontrar o mesmo `AnimalID` quando o RFID atual não está disponível. Ela não concede autoridade. Para autoridade e estado atual, o fluxo ainda depende do custodiante e da Solana.

O lookup por hash RFID consulta o `RfidBinding` canônico e só retorna uma projeção cujo binding está `ACTIVE`. Um binding `RETIRED` não pode reidentificar silenciosamente outro animal.

### 6.2 Challenge de autorização da captura

A mudança mais importante do fluxo começa em `/api/captures/authorization-challenge`.

O navegador envia a intenção de ação, `animalId`, próximo custodiante quando aplicável e, apenas na recaptura explícita de `REIDENTIFY`, o ID da captura aceita que será substituída. A API consulta o estado canônico atual e deriva o contexto correto. Ela não aceita sequência, predecessor ou RFID fornecidos pelo cliente.

A API determina o `required_signer`:

- em `ORIGIN`, é o custodiante inicial;
- em `TRANSFER`, é o custodiante atual;
- em `REIDENTIFY`, é o custodiante atual.

Depois, gera uma mensagem textual versionada que inclui challenge ID, deployment, Program ID, ação, `AnimalID`, destino, eventual `supersedeCaptureId`, signer exigido e expiração. O challenge expira em dois minutos e é de uso único.

O frontend não assina cegamente a mensagem. `wallet.ts` confere o deployment, o signer, a expiração e reconstrói localmente a mensagem esperada. Só depois pede `signMessage` à carteira Wallet Standard.

Em seguida, o cliente envia `challengeId` e assinatura Base64 para `POST /api/captures`. A API bloqueia o registro do challenge, confere cada campo contra a intenção atual, verifica a assinatura Ed25519 do signer exigido e consome o challenge atomicamente.

Essa ordem é importante: a Station só recebe trabalho físico depois de existir autorização explícita do custodiante para aquela intenção exata.

### 6.3 Criação idempotente, supersessão e recaptura

A função `prepare_capture` separa três situações.

A primeira é a repetição idempotente. Se existe uma evidência não finalizada para o mesmo animal e a intenção é exatamente a mesma, a API retorna a captura/evento já existente. Isso evita uma nova leitura física e um segundo evento.

A segunda é uma tentativa conflitante. Se existe uma transição não finalizada com intenção diferente, a API bloqueia. Se a transição já tem assinatura de transação ou está `SUBMITTED`, ela não pode ser simplesmente substituída pelo backend.

A terceira é a recaptura explícita de `REIDENTIFY`. Nesse caso, a nova requisição precisa nomear o ID da captura aceita anterior. A API exige que a ação seja `REIDENTIFY`, marca o evento anterior como `REJECTED`, cancela a captura anterior e cria uma nova captura em uma transação PostgreSQL. Um índice parcial impede dois eventos não rejeitados com a mesma sequência por animal.

Essa função resolve um caso real de operação: a Station pode observar um RFID incorreto durante uma reidentificação. A evidência física aceita não deve ser apagada, mas o custodiante precisa de uma forma explícita de pedir nova observação antes da assinatura on-chain.

Existe uma limitação importante. A Solana não sabe que o PostgreSQL “substituiu” uma evidência. Se o cliente já assinou e transmitiu a transação antiga, ela ainda pode chegar à rede. Por isso a supersessão é um caminho de recuperação de demo para evidência aceita e ainda não assinada, não uma revogação criptográfica universal. Uma futura versão precisaria de nonce, cancelamento ou intent state on-chain.

### 6.4 Agent e Station

A API entrega comandos ao Agent por polling autenticado com Bearer token. O Agent valida novamente o comando antes de serializá-lo. Essa segunda validação evita que um erro ou comprometimento parcial da API atravesse a fronteira física sem ser detectado.

A Station entra em `WAIT_RFID` somente depois de um `COMMAND` válido. Ela aceita exatamente uma observação física. Depois calcula o hash, verifica as restrições locais da ação, constrói o `StationEvent`, assina e entra em `WAIT_ACK`.

A Station tem agora deadline monotônico de 180 segundos para `WAIT_RFID`. O firmware chama `lastro_station_elapse` com o tempo decorrido medido por `esp_timer_get_time`. Ao expirar, a Station volta a `IDLE` e disponibiliza um `RFID_TIMEOUT` associado ao mesmo `capture_id`.

A Station não expira evidência que já foi assinada. O deadline vale apenas para a espera pela leitura. Isso preserva a possibilidade de o Agent reiniciar ou repetir a entrega de um evento já produzido.

A Station também trata a repetição do mesmo comando de forma idempotente enquanto está ocupada. Um comando diferente recebe `BUSY`. Em `WAIT_ACK`, o mesmo comando pode produzir novamente o mesmo `EVENT_READY`, fechando a janela de crash entre envio da evidência e ACK.

### 6.5 Durabilidade do Agent

O Agent valida `EVENT_READY` em várias etapas:

1. `capture_id` igual ao comando ativo;
2. chave pública igual à Station configurada;
3. tamanho e semântica dos 276 bytes;
4. `StationID` derivado da chave;
5. assinatura P-256 low-S válida;
6. hash do RFID observado igual ao `new_rfid_hash`;
7. todos os campos iguais ao contexto imutável do comando.

Depois grava o evento no SQLite com estado `LOCAL`. A gravação ocorre antes do ACK e antes do POST HTTP. O SQLite usa WAL e `synchronous=FULL`, e triggers impedem a alteração dos bytes da evidência.

A máquina de estados é:

```text
LOCAL -> SERVER -> FINALIZED
  |        |
  +------> QUARANTINED
```

`LOCAL` significa que a evidência está duravelmente no Agent. `SERVER` significa que a API a aceitou. `FINALIZED` significa que o evento já foi finalizado no fluxo canônico. `QUARANTINED` significa falha terminal que não deve ser retentada indefinidamente.

Falhas HTTP transitórias produzem retry com backoff crescente até 30 segundos. Falhas terminais são registradas e movem a linha para quarentena. Uma linha em quarentena fica disponível para inspeção, mas não pode ser usada depois da expiração da captura para transformar uma observação antiga em evidência nova.

Após restart, o Agent reemite ACK para linhas `LOCAL` e `SERVER`. Essa decisão fecha a janela em que a API aceitou a evidência, mas o processo morreu antes de confirmar o ACK na Station.

A limitação deliberada continua existindo: se a Station perder energia antes de o Agent gravar `LOCAL`, a captura física precisa ser repetida. O firmware não possui um journal persistente completo.

### 6.6 Admissão na API e PostgreSQL

`POST /api/agent/evidence` exige o token do Agent. A API decodifica e valida o conteúdo antes de confiar nele. Ela confere Base64 canônico, tamanhos fixos, RFID, chave P-256, assinatura, StationID, deployment, contexto da captura e estado atual.

A evidência admitida entra em `events`, e a captura vai para `EVIDENCE_ACCEPTED`. Os bytes criptográficos são append-only. Triggers impedem mudar ação, animal, sequência, hashes, bytes, RFID, chave ou assinatura. Outro trigger impede exclusões indevidas.

O ciclo de evento é:

```text
EVIDENCE_ACCEPTED -> SUBMITTED -> FINALIZED
          |
          +-------> REJECTED
```

`REJECTED` é usado pela supersessão explícita de uma evidência aceita de `REIDENTIFY`. A restrição de consistência exige que eventos em `EVIDENCE_ACCEPTED` ou `REJECTED` ainda não tenham assinatura Solana, enquanto `SUBMITTED` e `FINALIZED` precisam ter assinatura.

As tabelas principais são:

| Tabela | Responsabilidade |
|---|---|
| `animals` | Projeção de leitura, `AnimalID`, recovery ID e estado materializado após finalização. |
| `captures` | Contexto da captura, expiração, dispatch, aceitação e cancelamento. |
| `events` | Bytes de evidência e ciclo de vida criptográfico. |
| `capture_authorization_challenges` | Mensagens de autorização de uso único, intent, signer, expiração e supersessão. |

A nova tabela de challenges limita o tamanho da mensagem a 1024 bytes, mantém índices por data, signer e animal e retém apenas uma janela curta de auditoria. A inserção usa advisory lock transacional para aplicar budgets de forma serializada entre réplicas da API.

### 6.7 Limites de recursos

Os budgets atuais são parte da defesa, não apenas uma otimização:

- até 240 challenges globais por minuto;
- até 8 challenges por signer por minuto;
- até 8 challenges por animal por minuto;
- limpeza limitada de até 128 challenges antigos por operação;
- mensagem de authorization com 1 a 1024 bytes;
- corpo JSON global limitado a 1024 bytes;
- no máximo 128 eventos materializados pelo verificador web;
- orçamento total de 30 segundos para a verificação RPC no navegador.

A API faz um preflight barato antes de consultar a Solana, mas a inserção transacional permanece a autoridade final. Isso evita que duas réplicas passem simultaneamente por um simples `SELECT count(*)` e excedam o mesmo limite.

### 6.8 Construção da transação Solana

`GET /api/events/{eventHash}/transaction-data` retorna exatamente duas instruções:

```text
instruction[0] = precompile oficial Secp256r1
instruction[1] = instrução Lastro correspondente à ação
```

A primeira instrução contém o descriptor, assinatura e chave pública. O descriptor aponta para os 276 bytes dentro da segunda instrução. O programa on-chain verifica que a ocorrência do evento é única e que não existe terceira instrução.

A segunda instrução contém o discriminator Anchor e as contas derivadas:

```text
ProtocolConfig = PDA("config", deployment_id)
AnimalState    = PDA("animal", deployment_id, animal_id)
RfidBinding    = PDA("rfid", deployment_id, rfid_hash)
```

O builder mede o tamanho serializado e recusa uma transação acima de 1232 bytes. O navegador mede novamente a transação compilada e exige que o tamanho seja igual ao valor retornado pela API.

### 6.9 Validação e assinatura no navegador

`apps/web/src/solana/transaction.ts` valida todo o descritor antes de abrir o popup da carteira. Ele confere Program ID, quantidade de instruções, precompile, discriminator, evento, descriptor, assinatura Station, StationID, PDAs, papéis de contas, signer exigido e intenção da transição.

O navegador também compara o `eventHash`, `AnimalID`, deployment, ação e destinatário contra a intenção que o operador iniciou. Isso reduz a confiança necessária na API: mesmo que a resposta seja alterada antes da carteira, o cliente deve rejeitar a divergência.

Após a assinatura, o código compara `messageBytes` da transação sem assinatura com os bytes validados. Se a carteira modificar a mensagem, a operação falha. A chave privada não entra no código Lastro.

A operação assinada persiste no `localStorage` apenas metadados de recuperação: capture ID, event hash, assinatura, bytes wire Base64 e `lastValidBlockHeight`. Esses dados não são tratados como autoridade. Ao recarregar a página, a aplicação consulta novamente API e Solana.

### 6.10 Broadcast, expiração e finalização

Depois do broadcast, o navegador chama `submit`. A API consulta a Solana em commitment `confirmed` e verifica o envelope completo: assinatura, signer, cabeçalho, contas, ordem das instruções, Program IDs e dados exatos.

O campo `lastValidBlockHeight` permite ao navegador detectar uma transação assinada que expirou sem aparecer no histórico RPC. Antes de declarar a operação perdida, ele consulta `getBlockHeight` e `getSignatureStatuses`.

Se a assinatura ainda é válida, o navegador pode reenviar os mesmos bytes wire, sem abrir uma segunda solicitação de assinatura. Se o blockhash expirou e a assinatura não foi encontrada, ele limpa somente os metadados da transação, preserva o `eventHash` e a evidência física aceita e informa que a mesma ação pode ser autorizada com um blockhash novo.

A etapa `confirm` exige commitment `finalized`. A API verifica o estado final on-chain, aplica a projeção PostgreSQL em transação e só então marca o evento como `FINALIZED`.

A sequência de estados evita quatro confusões:

```text
Station assinou        != Solana aceitou
RPC recebeu             != transação confirmou
confirmed               != finalized
PostgreSQL atualizado   != autoridade canônica
```

## 7. Programa on-chain

O programa Anchor mantém quatro instruções públicas: `initialize`, `origin`, `transfer` e `reidentify`.

`initialize` cria um `ProtocolConfig` imutável com autoridade, deployment ID, chave P-256 da Station e bump. O script Python de inicialização agora espera o saldo do authority em commitment `finalized` antes de enviar a transação. Isso evita testar uma transação contra uma visão de banco ainda não financiada.

`origin` cria `AnimalState` e o primeiro `RfidBinding`. `transfer` altera apenas custodiante, sequência e hash do último evento. `reidentify` aposenta o binding antigo, cria o novo binding e altera RFID, revisão, sequência e hash.

O `AnimalState` guarda:

- `animal_id`;
- RFID atual;
- custodiante atual;
- revisão de identidade;
- sequência do evento;
- hash do último evento;
- bump da PDA.

O `RfidBinding` é um índice histórico por deployment. Ele nunca é fechado durante o fluxo normal. Um RFID que participou da história permanece associado ao animal e passa a `RETIRED` quando substituído. Isso impede reutilização silenciosa de um RFID histórico em outro animal.

Antes de qualquer mutação, os handlers verificam deployment, ação, semântica local, predecessor, sequência, revisão, StationID, contas esperadas, custodiante signer, binding e precompile.

A verificação Secp256r1 em `verify/secp256r1.rs` é particularmente importante. Ela exige:

- instrução atual do Lastro na posição 1;
- precompile oficial na posição 0;
- zero contas no precompile;
- descriptor de 113 bytes;
- uma assinatura;
- chave pública no offset 80;
- mensagem no offset 8 com tamanho 276;
- evento localizado de forma única na instrução Lastro;
- ausência de uma terceira instrução;
- chave pública igual à registrada no `ProtocolConfig`.

O programa não confia em um offset calculado no backend. Ele deriva e valida o offset a partir das instruções efetivamente presentes na transação.

## 8. Verificador independente

O verificador web trabalha em cinco camadas:

| Camada | Verificação |
|---|---|
| `RFID_EVIDENCE` | O RFID observado produz o `new_rfid_hash` do evento. |
| `STATION_SIGNATURE` | A chave deriva o StationID esperado e a assinatura P-256 é válida. |
| `IDENTITY_CONTINUITY` | AnimalID, deployment, sequência, predecessor, revisão e RFID formam uma cadeia linear. |
| `CUSTODY` | Cada evento parte do custodiante anterior e respeita as regras de transfer/reidentify. |
| `ON_CHAIN_STATE` | ProtocolConfig, AnimalState, bindings e transações finalizadas coincidem com o pacote. |

A autoridade do deployment é agora uma trust anchor configurada separadamente no navegador. O verificador compara o `deploymentId` do pacote e a autoridade armazenada no `ProtocolConfig` com essa configuração. Se a autoridade não estiver configurada, o resultado é `NOT_CHECKED`, nunca `VALID`.

A verificação de transações finalizadas não confere apenas a assinatura textual. Ela busca cada transação no RPC e verifica versão legacy, sucesso, assinatura única, signer, cabeçalho, conjunto e papel das contas, ordem das instruções, Program IDs e dados Base58 exatos.

Se o RPC estiver indisponível, retornar erro, JSON inválido ou exceder o orçamento de tempo, o layer vira `NOT_CHECKED`. Isso é uma distinção correta: indisponibilidade de rede não prova validade nem invalidade do pacote.

A validade global só é `true` quando todos os layers são `VALID`. Um pacote pode ter evidência física e histórico local válidos, mas continuar sem prova canônica se as assinaturas de transação estiverem ausentes ou o RPC não puder ser consultado.

## 9. Frontend e experiência de operação

O router separa narrativa, demo e verificação:

- `/`, `/problem` e `/future` explicam o produto;
- `/demo` é o console do operador;
- `/verify/:animalId?` é o verificador independente.

`DemoPage.vue` coordena o fluxo atual:

1. cria ou recupera o animal;
2. conecta e seleciona a carteira;
3. confere se a carteira corresponde ao custodiante esperado;
4. obtém e valida o challenge;
5. assina a intenção de captura;
6. reserva a captura;
7. aguarda o Agent e a Station;
8. mostra `EVIDENCE_ACCEPTED` quando a prova física chega;
9. valida o descritor de transação;
10. pede assinatura da transação;
11. persiste bytes assinados e block height;
12. tenta `submit` e rebroadcast se necessário;
13. aguarda `confirm` finalizado;
14. atualiza timeline e projeção apenas após finalização.

A interface também possui uma verificação explícita de custodiante antigo. Essa ação não cria captura, evidência ou transação. Ela demonstra que uma carteira desconectada do custodiante atual não deve consumir trabalho físico nem iniciar uma operação inválida.

O modo `Retry RFID scan` só é habilitado para uma captura `REIDENTIFY` com evidência aceita, sem assinatura de transação. Essa restrição evita que a UI use recaptura como atalho para substituir uma transição que já foi enviada à Solana.

## 10. Firmware e simulador

O firmware real roda no ESP32-C5 e usa USB Serial/JTAG nativa para o Agent. `app_main.c` inicializa o driver, mede tempo monotônico, alimenta o runtime e só consulta o adaptador RFID enquanto a Station está em `WAIT_RFID`.

`station.c` contém a máquina de estados. `event.c` implementa o layout binário. `signer.c` encapsula P-256 e a opção preparada para eFuse. `transport.c` implementa framing e CRC32C. `runtime.c` liga transporte, decodificação e máquina de estados.

A fronteira RFID continua deliberadamente incompleta. `rfid.c` não inventa frames de um fabricante não escolhido. O repositório ainda não declara modelo de leitor, pinagem, barramento, frame bruto, checksum ou regra de integridade. Portanto, o software da Station está preparado para receber um RFID canônico, mas a integração com um leitor real não está provada.

O simulador Python não é uma segunda API de produção. Ele reproduz o protocolo Station-Agent e oferece UI para inserir uma tag virtual. A ponte TCP/PTY permite que o Agent Rust real converse com uma Station simulada. A separação é útil porque o simulador testa integração de software sem esconder o fato de que o hardware físico permanece um gate separado.

## 11. Banco e estados de persistência

No PostgreSQL, a tabela `animals` é uma projeção. Ela pode saber que um animal foi registrado, mas só recebe RFID, custodiante, revisão, sequência e último evento depois da finalização canônica.

A tabela `captures` representa trabalho físico temporário. Ela pode estar `PENDING`, `DISPATCHED`, `EVIDENCE_ACCEPTED`, `EXPIRED` ou `CANCELLED`. A atualização de expiração só pode diminuir o prazo e não pode reabrir uma captura terminal.

A tabela `events` guarda o material criptográfico e seu ciclo de vida. A supersessão introduziu o estado `REJECTED`, mas não apagou bytes nem reescreveu a história. O índice parcial por animal e sequência garante que apenas um evento não rejeitado ocupe cada posição.

A tabela `capture_authorization_challenges` é consumível. O challenge tem intent, signer, mensagem, expiração e marca de uso. O bloqueio de linha e a atualização condicional impedem duas requisições concorrentes de consumir a mesma autorização.

No Agent, a outbox SQLite é o ponto de durabilidade entre a Station e a API. Os bytes são imutáveis. Somente estado, tentativas, erro e timestamps avançam.

A distinção operacional é:

```text
PostgreSQL = coordenação, evidência admitida e projeção
SQLite      = durabilidade local do transporte
Solana      = autoridade canônica da transição
EvidencePackage = material para verificação independente
```

## 12. Segurança: o que é tratado

O código e os testes cobrem vários ataques e falhas:

- assinatura Station sobre bytes diferentes;
- chave Station não autorizada;
- assinatura high-S;
- RFID observado incompatível com o hash assinado;
- evento com action, sequence ou revision incorretos;
- predecessor incorreto;
- salto ou repetição de sequência;
- custodiante antigo tentando transferir;
- transfer para o mesmo custodiante;
- reidentificação alterando custodiante;
- RFID antigo após reidentificação;
- reutilização de RFID histórico;
- descriptor Secp256r1 com offset ou mensagem incorretos;
- transação com programa, conta ou instrução extra;
- replay de ACK de outro capture/event;
- duplicação de evidência após retry;
- edição de bytes no SQLite ou PostgreSQL;
- desafio de autorização expirado ou consumido duas vezes;
- excesso de challenges por signer, animal ou globalmente;
- pacote de evidências maior que o limite do navegador;
- RPC lento ou indisponível;
- transação assinada expirada;
- captura física presa em `WAIT_RFID`;
- erro terminal retentado indefinidamente pelo Agent.

O token do Agent exige tamanho mínimo e é comparado por hash em comparação constante. A configuração de API e Agent falha fechado se faltar endpoint, token, Program ID, deployment, chave Station ou timeout válido. O frontend valida configuração pública e não deve receber segredo em `VITE_*`.

A inicialização HTTP só faz bind depois de validar configuração, aplicar migrations, criar estado compartilhado e preparar o cliente RPC. Isso evita aceitar requisições quando o processo está parcialmente pronto.

## 13. Limites que o projeto não resolve

A assinatura P-256 demonstra integridade e origem criptográfica da Station, não verdade biológica. O sistema não prova que o animal é o animal “correto” em sentido legal ou científico.

O sistema não prova que uma tag não foi removida fisicamente, clonada antes da leitura ou apresentada em um contexto fraudulento. Ele prova o que a Station observou, não a intenção humana por trás da observação.

O leitor RFID real ainda não foi integrado. Sem hardware selecionado e frames documentados, não existe base técnica para afirmar que o caminho físico está completo.

A configuração de eFuse é uma capacidade preparada. O código não pode provar sozinho que uma placa específica possui secure boot, bloco correto, leitura protegida e configuração ECDSA irreversível. Essa validação exige procedimento físico e teste após reboot.

O `StationEvent` v1 não contém nonce assinado nem timestamp. A sequência e o predecessor impedem a aplicação de uma história já finalizada em posição incompatível, mas não provam quando a observação ocorreu. Freshness temporal exigiria uma versão de protocolo com challenge ou nonce on-chain.

A carteira real do usuário final não foi validada neste ambiente. Os testes Wallet Standard usam atores determinísticos. Isso prova o contrato da aplicação, mas não substitui teste com Phantom ou outra extensão no cluster pretendido.

A supersessão ainda não revoga uma transação Solana já assinada. Uma transação antiga pode chegar à rede depois de o backend preparar uma captura substituta. A regra segura atual é não oferecer substituição irrestrita depois do broadcast; para garantia completa, é necessário estado de intent ou cancelamento on-chain.

Uma transação `confirmed` pode falhar antes de `finalized`. O sistema mantém a assinatura submetida e exige reconciliação. O navegador consegue tratar a expiração de uma transação ainda não registrada, mas não pode provar sozinho que uma transação confirmada não será finalizada.

CORS usa uma política ampla adequada ao hackathon, mas insuficiente para produção pública. Ainda seriam necessários origem permitida, TLS, rate limiting de borda, observabilidade, segregação de rede, gestão de secrets e autenticação operacional.

## 14. Como executar e validar

### 14.1 Dependências

O ambiente completo espera Python 3.13.x, Rust 1.98.1, Node 24.21.0, npm 11.19.0, Anchor 1.2.0, Solana CLI 4.2.0, Docker e ESP-IDF para firmware.

Os gates básicos são:

```bash
make doctor
python3 -m pip install -r requirements-dev.txt
npm ci
python3 scripts/spec_check.py
python3 scripts/check_vectors.py --verify-only
python3 -m pytest -q tests/repository
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace
npm --workspace @lastro/web run format:check
npm --workspace @lastro/web run typecheck
npm --workspace @lastro/web run test
npm --workspace @lastro/web run build
```

### 14.2 Ambiente local isolado

O caminho recomendado é:

```bash
make local-demo-doctor
make local-demo-init
make local-demo-up
make local-demo-test
```

`local_dev.py` mantém identidades e estado em `.lastro-local/`. Ele cria Program ID descartável, Wallet A/B/C, token do Agent, ledger, logs e artefatos. Também impede que um RPC ou Program ID exportado do ambiente externo escape para o Compose local.

O fluxo sobe um validator Solana real local, compila e implanta o programa, inicializa `ProtocolConfig`, sobe PostgreSQL/API/web/simulador e executa os testes de sistema e navegador.

| Serviço | Endereço local |
|---|---|
| Web | `http://127.0.0.1:8088` |
| API | `http://127.0.0.1:8080` |
| Hardware Simulator | `http://127.0.0.1:8090` |
| Solana RPC | `http://127.0.0.1:8899` |

### 14.3 CI

A CI separa os gates de Rust/especificação, web e auditoria de dependências. O workflow E2E prepara um validator, gera carteiras determinísticas, compila o programa, implanta, inicializa a configuração, sobe API, executa testes G3/G5, executa estabilidade em três rodadas e roda Playwright.

O E2E completo exige Linux, Docker, Solana CLI, Anchor, PostgreSQL e dependências web. Não é equivalente a rodar apenas testes unitários no Windows.

## 15. Validação observada neste checkout

O estado atual é um working tree com muitas alterações locais sobre `b8546f3`. Não é um checkout limpo nem uma nova revisão commitada. Portanto, o relatório descreve os arquivos presentes na árvore de trabalho atual.

Foi observado:

- `git diff --check`: sem erro de whitespace;
- `scripts/generate_test_index.py --check`: **passou**;
- o índice de testes está atualizado em relação às declarações encontradas;
- na primeira rodada, a suíte `tests/repository` executou **93 casos**, com **90 aprovados e 3 falhas**; uma falha era o relatório solto na raiz e duas eram relacionadas ao manifesto;
- depois que o relatório foi movido para `docs/ANALISE_PROJETO.md`, os testes direcionados de estrutura e manifesto executaram **53 casos**, com **51 aprovados e 2 falhas**;
- `scripts/spec_check.py` agora passa a política estrutural e o índice de testes, mas termina com `MANIFEST.sha256 is stale`;
- as falhas restantes são de manifesto/árvore do checkout, não de uma asserção de regra de domínio;
- o relatório e o arquivo auxiliar `.analysis_diff_names.txt` foram tratados como artefatos da análise, e o auxiliar foi removido;
- os testes Rust, web, on-chain, firmware e E2E completo não foram considerados aprovados nesta atualização.

A falha de manifesto não deve ser escondida. O próximo passo de integração é executar o gerador oficial do manifesto no checkout final, revisar o diff e só então repetir `tests/repository` e `scripts/spec_check.py`.

Também não se deve transformar a existência de testes em prova de execução. Os arquivos de teste mostram a intenção de cobertura, mas apenas uma execução completa no toolchain declarado comprova o comportamento naquela revisão.

## 16. Pontos fortes atuais

O projeto tem uma fronteira de protocolo excepcionalmente clara para um protótipo. O evento possui tamanho fixo, offsets explícitos, vetores cruzados e validação em Rust, C, Python, TypeScript e Solana.

A separação entre evidência, autoridade e projeção é consistente. PostgreSQL não avança o animal por conta própria. O frontend não trata uma resposta `valid` do backend como prova. O programa on-chain valida o precompile e as contas. O Agent grava antes de confirmar o transporte.

O hardening recente melhora justamente os pontos que costumam quebrar demos reais: wallet rejeitada, reload depois do broadcast, timeout físico, retry de HTTP, challenge repetido, concorrência de API e RPC indisponível.

A inclusão de uma trust anchor independente para o deployment também é importante. Sem ela, um verificador poderia consultar uma conta em um deployment controlado pelo próprio sistema e chamar isso de prova independente.

A documentação é honesta sobre a diferença entre software simulado, hardware preparado e hardware fisicamente validado. Essa honestidade é uma qualidade de engenharia, não uma fraqueza de apresentação.

## 17. Prioridades recomendadas

A primeira prioridade é fechar o estado de entrega: regenerar `MANIFEST.sha256`, adicionar ou remover conscientemente os arquivos novos, executar `spec_check`, repository tests e o índice derivado no checkout final.

A segunda é executar os gates no ambiente correto. O Windows local não substitui a validação Linux da CI para Anchor, Solana validator, Docker e Playwright. O resultado deve registrar separadamente testes unitários, integração, E2E, Devnet e hardware.

A terceira é selecionar um leitor RFID real. A decisão precisa registrar modelo, protocolo elétrico, pinagem, frames brutos, checksum, sequência de inicialização, tratamento de erro e mapeamento para os oito bytes canônicos.

A quarta é validar a Station física. O procedimento deve cobrir firmware assinado, secure boot, eFuse, chave pública derivada, reboot, falha de energia, timeout, leitura duplicada e ACK perdido.

A quinta é decidir o protocolo de frescor e cancelamento. Se uma recaptura puder concorrer com uma transação antiga, o protocolo precisa de nonce, intent on-chain ou mecanismo equivalente antes de alegar supersessão incondicionalmente segura.

A sexta é preparar a borda de produção. CORS, TLS, rate limiting, autenticação operacional, logs sem segredo, métricas de fila, alerta de quarentena e política de retenção precisam ser definidos antes de exposição pública.

## 18. Ordem recomendada para estudar o código

Para entender o projeto, comece por `README.md`, `docs/ARCHITECTURE.md`, `docs/PROTOCOL.md`, `docs/API.md` e `docs/HARDENING_STATUS.md`.

Depois leia `crates/lastro-protocol/src/event.rs`, `crypto.rs`, `rfid.rs` e `evidence.rs`. Esses arquivos definem o contrato que as outras camadas precisam respeitar.

Em seguida, leia `firmware/station/components/lastro_station/station.c` e `event.c`. Observe que o backend fornece contexto, mas o RFID novo só vem da Station.

Depois leia `services/agent/src/worker.rs`, `spool/mod.rs`, `spool/model.rs` e as migrations SQLite. Essa é a melhor forma de entender durabilidade, ACK, retry e quarentena.

Na API, leia nesta ordem: `model.rs`, `routes/captures.rs`, `repository/capture_authorizations.rs`, `domain/evidence.rs`, `routes/agent.rs`, `routes/events.rs` e `solana/transaction_builder.rs`.

No programa Solana, leia `verify/event.rs`, `verify/secp256r1.rs`, `instructions/origin.rs`, `transfer.rs`, `reidentify.rs` e os estados `AnimalState`, `ProtocolConfig` e `RfidBinding`.

No frontend, leia `solana/wallet.ts`, `demo/pendingOperation.ts`, `DemoPage.vue`, `solana/transaction.ts`, `verifyEvidencePackage.ts` e `verifyChain.ts`.

Finalmente, leia `scripts/local_dev.py`, `tests/system`, `apps/web/e2e` e os workflows CI/E2E para entender o que é executável, o que é simulado e o que ainda depende de infraestrutura externa.

## 19. Diagnóstico final

O Lastro não é apenas um CRUD de animais, um firmware de RFID, uma API ou uma tela Vue. Ele é um conjunto de contratos que tenta conectar um fato físico a um estado digital sem permitir que uma única camada declare o resultado sozinha.

A atualização recente melhora substancialmente a segurança operacional. A carteira agora autoriza a intenção antes do consumo da Station. A recaptura é explícita. O Agent não fica preso indefinidamente em uma captura. O navegador consegue recuperar uma transação expirada sem perder a evidência física. O verificador não aceita deployment sem autoridade confiável. Os budgets reduzem abuso previsível.

O software está organizado de modo coerente e tem uma boa separação entre protocolo, transporte, coordenação, projeção e autoridade canônica. A principal ressalva não é uma falha conceitual do código: é que o projeto ainda não provou hardware RFID real, eFuse/secure boot, carteira real, Devnet e operação produtiva.

A afirmação tecnicamente correta neste momento é: **o projeto possui uma implementação de software abrangente e fortemente testável para evidência física assinada e custódia verificável, mas ainda depende dos gates de infraestrutura e hardware para ser considerado um sistema físico de produção**.

## Referências internas

[1]: ./ARCHITECTURE.md "Arquitetura do projeto Lastro"
[2]: ./PROTOCOL.md "Protocolo canônico Lastro"
[3]: ./API.md "Contrato e fluxo da API Lastro"
[4]: ./SECURITY.md "Fronteiras e ameaças de segurança"
[5]: ./HARDENING_STATUS.md "Status das medidas de hardening"
[6]: ./LOCAL_DEVELOPMENT.md "Desenvolvimento local e execução do stack"
[7]: ./TESTING.md "Estratégia e gates de testes"
[8]: ../schemas/openapi.yaml "Contrato OpenAPI da API"
[9]: ../schemas/evidence-package.schema.json "Schema do EvidencePackage"
[10]: ../scripts/local_dev.py "Orquestração do ambiente local"
[11]: ../apps/web/src/verify/verifyChain.ts "Verificação independente do estado Solana"
[12]: ../chain/programs/lastro/src/verify/secp256r1.rs "Vinculação on-chain do precompile Secp256r1"
[13]: ../services/api/src/routes/captures.rs "Autorização e criação de capturas"
[14]: ../services/agent/src/worker.rs "Worker e outbox do Agent"
[15]: ../firmware/station/components/lastro_station/station.c "Máquina de estados da Station"

**Autor:** Manus AI

---

## Nota de atualização

Esta versão substitui a análise anterior e incorpora as mudanças presentes na árvore de trabalho atual, especialmente os arquivos adicionados em `services/api/migrations/0004`–`0007`, `services/api/src/repository/capture_authorizations.rs`, `services/agent/migrations/0003_outbox_quarantine.sql`, a autorização Wallet Standard, a recuperação de transações Solana, o deadline da Station, os limites do verificador e os novos gates de CI.

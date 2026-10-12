# SPEC v2.1 — Viabilidade operacional e comercial

**Status:** proposta para revisão
**Data:** 12/10/2026
**Base:** protocolo Lastro v2 (`chain/programs/lastro-v2`, `crates/lastro-protocol/src/v2`)
**Origem:** análise de investidor e técnica dos problemas comerciais do Lastro

Esta spec transforma os problemas comerciais identificados em frentes de engenharia com escopo,
limites, critérios de aceite e testes. Ela **não** muda a tese do produto nem autoriza features
fora dos limites declarados aqui.

---

## 0. Princípios e limites globais

### 0.1 Invariantes que nenhuma frente pode quebrar

| ID | Invariante |
|---|---|
| I1 | O AssetID sobrevive à troca de RFID; um RFID nunca identifica dois AssetIDs (`RfidBinding` nunca é fechado). |
| I2 | Existe um único estado canônico por AssetID; transições de identidade e custódia exigem `state_version` e `last_event_hash` exatos. |
| I3 | Toda evidência física é assinada pela Station (P-256) sobre bytes canônicos; nada é aceito sem assinatura válida. |
| I4 | Um terceiro verifica a história inteira com o EvidencePackage e um RPC Solana, sem confiar no veredito da API. |
| I5 | PostgreSQL e SQLite são projeção e fila; nunca decidem estado canônico. |
| I6 | Nenhum gate não executado é reportado como aprovado; nenhuma dependência externa ausente vira sucesso simulado. |
| I7 | A chave que paga taxas nunca é autoridade de custódia, de identidade ou de configuração. |

### 0.2 Fora do escopo desta spec (vale para todas as frentes)

- Token, RWA, crédito, seguro ou qualquer produto financeiro.
- Declaração de conformidade (EUDR, SISBOV, PNIB). O sistema produz **evidência**, não veredito legal.
- Alegação de identidade biológica do animal.
- GPS individual em todos os animais.
- Integração por API com sistemas internos de grandes frigoríficos.
- Integração por API com GTA/SISBOV/PNIB (só anexo de documento; ver S6).
- Deploy em mainnet. O teto desta spec é Devnet + piloto controlado.
- Blockchain própria, troca de rede ou ZK Compression (reavaliar após medição de piloto).
- App mobile nativo.

### 0.3 Limites numéricos globais

| Limite | Valor | Observação |
|---|---|---|
| Histórico por AssetID no EvidencePackage | 1.024 eventos | Já existente; mantido |
| Ativos por lote admitido | 512 | Já existente (`MAX_OPERATIONS_JSON_BODY_BYTES`) |
| Folhas por lote de observações (S4) | 1.024 | Igual ao limite de histórico |
| Duração de uma sessão de manejo (S3) | ≤ 12 h | Configurável para menos por deployment |
| Eventos por sessão (S3) | ≤ 2.000 | |
| Idade máxima de evento identitário | 24 h | `MAX_EVENT_AGE_SECONDS`, inalterado |
| Idade máxima de observação em lote (S8) | ≤ 7 dias | Nova constante, só para lotes |
| Vértices por polígono de fazenda (S5) | ≤ 10.000 | |

---

## 1. Mapa de problemas e frentes

| Problema comercial | Frente | Fase |
|---|---|---|
| Usuário precisa de SOL e paga taxas | **S1** Patrocínio de taxas e rent | A |
| Usuário precisa de carteira cripto | **S2** Contas de organização sem carteira | C |
| Uma assinatura por animal no curral | **S3** Sessão de manejo | B |
| Rent de US$ 3–5 por animal | **S4** Redução de custo on-chain | A/B |
| Origem e geolocalização inexistentes | **S5** Fazenda, polígono e GNSS | C |
| Comprador não recebe o que precisa | **S6** Triagem pública e Relatório de Origem | D |
| Leitor RFID não existe | **S7** Station como ponte de leitores de mercado | paralelo |
| Internet no curral | **S8** Modo offline | B |
| Sem evidência de qualidade para enterprise | **S9** CI, Devnet e auditoria | A |

---

## 2. Linha de base de custo (para medir S1/S4)

Rent isento calculado como `(SPACE + 128) × 6.960 lamports`.

| Conta | SPACE | Rent (SOL) | Criada por | Fechada? |
|---|---|---|---|---|
| `AssetState` | 341 | ≈ 0,00326 | `register_asset` | Não |
| `RfidBinding` | 74 | ≈ 0,00141 | bind/replace | Nunca (I1) |
| `EventAnchor` | 259 | ≈ 0,00269 | todo evento de Station | Não |
| `IntentState` | 172 | ≈ 0,00209 | proposta de custódia | Não |

**Animal de referência:** 1 BIND, 8 OBSERVE, 1 REPLACE e 2 transferências de custódia.

| Cenário | Capital travado (rent) | Quem paga hoje |
|---|---|---|
| Atual | ≈ 0,0372 SOL | Carteira do usuário |
| Meta v2.1 | ≤ 0,0095 SOL (≈ −75%) | Plataforma (S1) |

O custo **consumido** (taxas) por evento é de ≈ 0,00001–0,000015 SOL e não é o problema.
O problema é o capital travado em contas que nunca são fechadas. Toda medição de S4 deve
reportar as duas grandezas separadamente.

---

## S1 — Patrocínio de taxas e rent

**Problema:** todas as instruções usam `payer = authority` (18 ocorrências de `payer = ` no
programa). O custodiante precisa ter SOL e paga o rent.

**Objetivo:** uma carteira com saldo zero completa todo o fluxo.

### Escopo
1. Programa: em toda instrução com `init`, adicionar `payer: Signer<'info>` separado de
   `authority`/`actor`/`custodian` e trocar `payer = authority` por `payer = payer`.
   A autorização continua exigindo a assinatura de `authority` (sem mudança de regra).
2. Fechamento com reembolso: `IntentState` passa a ser fechado em `accept_custody_transfer`,
   `cancel_intent` e `expire_intent`, com `close = payer_original`. Para isso, `IntentState`
   ganha o campo `rent_payer: Pubkey`.
3. API — rota nova `POST /api/v2/sponsor/submit`:
   - recebe a transação parcialmente assinada pelo usuário;
   - revalida a transação com a mesma derivação independente usada no navegador
     (programa, ordem das instruções, contas, bytes de dados);
   - aplica a política de patrocínio (abaixo), co-assina como fee payer e transmite.
4. `transaction-data` passa a montar a transação com `feePayer = sponsor`.
5. Verificador e `confirm`: aceitar fee payer diferente da autoridade, exigindo que o
   signatário autorizador continue sendo o esperado.

### Política de patrocínio (limites)
- Só patrocina transações cujas instruções sejam: Lastro v2, Secp256r1 e ComputeBudget
  (no máximo 2, regra atual). Qualquer outra instrução → recusa.
- A chave sponsor não pode aparecer como `authority`, custodiante, owner de facility nem
  autoridade de config (I7). O programa rejeita `payer == authority` quando
  `authority == sponsor` não fizer sentido (verificação na API e teste de regressão).
- Teto por transação: taxa + rent ≤ 0,01 SOL.
- Teto por organização por dia e global por dia: configuráveis
  (`LASTRO_SPONSOR_DAILY_LAMPORTS_PER_PARTY`, `LASTRO_SPONSOR_DAILY_LAMPORTS_TOTAL`).
- Alerta quando o saldo da sponsor ficar abaixo de `LASTRO_SPONSOR_MIN_BALANCE_LAMPORTS`.
- A chave sponsor fica fora do repositório, em arquivo ou KMS indicado por variável de
  ambiente; a configuração falha fechada se ela estiver ausente (decisão 18).

### Fora do escopo
- Relayer de terceiros como dependência obrigatória (Kora ou similar pode ser avaliado depois).
- Cobrança automática do cliente pelo consumo (a medição existe; o faturamento não).

### Critérios de aceite
- G3 local passa com as Wallets B e C com **0 lamports**.
- A rota recusa: instrução estranha, sponsor como autoridade, teto excedido e transação
  diferente da derivada.
- Rent de `IntentState` volta à sponsor após aceite, cancelamento ou expiração.

### Testes
- LiteSVM: cada instrução com `init` executando com payer ≠ authority; `IntentState` fechado.
- API: testes de política (5 casos de recusa acima) e idempotência do submit.
- Web (Vitest): validação de transação com fee payer externo.
- Sistema: G3 com carteiras sem saldo.

---

## S2 — Contas de organização sem carteira

**Problema:** o login exige Phantom/MetaMask (`apps/web/src/auth/loginWallets.ts`).

**Objetivo:** o operador de curral entra com e-mail e código e opera sem saber que existe
blockchain. Carteira própria continua possível para usuários avançados.

### Escopo
1. O custodiante on-chain é a **carteira da organização** (`PartyRecord.wallet`), não a pessoa.
2. A interface `SigningProvider` na API tem duas implementações:
   - `WalletStandardProvider`: o fluxo atual, em que o navegador assina;
   - `ManagedWalletProvider`: provedor de carteira gerenciada que assina por política.
     O provedor é definido na decisão D1.
3. Usuários e papéis ficam off-chain, com RBAC: `ORG_ADMIN`, `OPERADOR_CURRAL`,
   `AUDITOR`, `LEITURA`. O login é por e-mail com OTP.
4. Política do provedor, aplicada no provedor e não só na API:
   - assina só transações do programa Lastro v2;
   - assina só para ativos e sessões da própria organização;
   - `OPERADOR_CURRAL` só abre e fecha sessão (S3); transferir custódia exige `ORG_ADMIN`.

### Limites
- A API nunca guarda a chave privada da organização em texto claro.
- A chave da organização precisa ser exportável pelo `ORG_ADMIN`, para não prender o cliente
  ao provedor.
- Uma organização tem uma única carteira custodiante por deployment na v2.1.

### Fora do escopo
- Login gov.br, SSO corporativo, recuperação de conta além da oferecida pelo provedor.

### Critérios de aceite
- Um operador sem nenhuma extensão de carteira abre uma sessão e registra observações.
- O provedor recusa uma transação para outro programa e uma transferência pedida por operador.
- O fluxo Wallet Standard atual continua passando no G3.

### Testes
- API: RBAC por rota; política do provedor com um provedor de teste determinístico.
- E2E (Playwright): login por OTP com servidor de e-mail de teste.

---

## S3 — Sessão de manejo

**Problema:** hoje cada captura exige um challenge assinado pela carteira, mais uma transação
assinada, mais a finalização. Um lote de 500 animais dá cerca de 1.000 assinaturas.

**Objetivo:** **uma** autorização por lote de manejo. A Station lê de forma contínua e o custo
de autorização fica constante em relação ao número de animais.

### Escopo on-chain
Conta nova `SessionGrant`, PDA `[b"session", deployment_id, session_id]`:

```text
session_id      [u8;32]
deployment_id   [u8;32]
custodian       Pubkey     carteira que autoriza (custodiante dos ativos)
station_id      [u8;32]    exatamente uma Station
facility_id     [u8;32]    exatamente uma facility; precisa bater com a Station (S5)
allowed_actions u16        bitmask: OBSERVE=1, BIND=2
max_events      u32        ≤ 2.000
events_used     u32
valid_from      i64
expires_at      i64        expires_at − valid_from ≤ 12 h
status          u8         OPEN | CLOSED
rent_payer      Pubkey
bump            u8
SPACE ≈ 228 bytes → rent ≈ 0,0025 SOL, devolvido no fechamento
```

Instruções:
- `open_session`: assinada pelo custodiante; payer = sponsor (S1).
- `close_session`: pelo custodiante a qualquer momento, ou por qualquer conta depois de
  `expires_at`; fecha a conta e devolve o rent.
- `bind_identifier` aceita `SessionGrant` como autorização alternativa à assinatura do
  custodiante quando `allowed_actions` contém BIND, a Station e a facility batem e
  `asset.custodian == session.custodian`.
- Observações com sessão são ancoradas em lote (S4a), nunca uma a uma.

### Limites de segurança
- `REPLACE_IDENTIFIER` e transferência de custódia **nunca** acontecem sob sessão; exigem
  assinatura explícita do custodiante (proteção contra troca fraudulenta de brinco).
- Um evento só é aceito se `valid_from ≤ observed_at ≤ expires_at`, `status == OPEN` e
  `events_used < max_events`.
- Sessão fechada ou expirada: todo evento posterior é rejeitado, inclusive os que já estavam
  na fila offline.
- No máximo uma sessão OPEN por Station.

### Escopo off-chain
1. API: `POST /api/v2/sessions` (prepara `open_session`), `GET /api/v2/sessions/{id}`,
   `POST /api/v2/sessions/{id}/close`.
2. **Manifesto de sessão**: ao abrir, a API entrega ao Agent a lista
   `rfid_hash → (asset_id, state_version, last_event_hash)` dos ativos do custodiante
   (até 2.000 entradas). Assim a Station monta o envelope sem consultar a rede.
3. Agent em **modo contínuo**: sem COMMAND por captura; cada leitura vira um envelope
   assinado e entra no outbox.
4. **Deduplicação:** no máximo uma observação por AssetID por sessão; leituras repetidas são
   descartadas no Agent e registradas só como métrica.
5. Tag desconhecida no manifesto: o evento fica em `UNMATCHED`, sem assinatura de transição;
   pode virar BIND se a sessão permitir e existir AssetID pré-registrado livre.
6. O envelope de 220 bytes **não muda**. `session_id` entra no payload, com o hash coberto
   por `payload_hash` (layout em S5).

### Fora do escopo
- Leitura de vários animais ao mesmo tempo (anticolisão de portal), pesagem automática obrigatória.

### Critérios de aceite
- 500 leituras simuladas numa sessão geram no máximo 3 assinaturas de carteira
  (abrir, eventual BIND autorizado em lote, fechar) e no máximo 3 transações on-chain de
  lote (S4a).
- Vazão sustentada ≥ 1 leitura/s por Station no simulador de hardware, sem perda.
- REPLACE sob sessão é rejeitado on-chain.
- Evento depois de `close_session` é rejeitado, mesmo vindo da fila offline.

### Testes
- LiteSVM: abrir, usar, exceder `max_events`, expirar, fechar, ação não permitida,
  Station ou facility divergentes.
- Agent: deduplicação, manifesto, `UNMATCHED`.
- Sistema: sessão de 500 animais no `hardware-simulator`.

---

## S4 — Redução de custo on-chain

**Problema:** `record_observation` (`instructions/events.rs`) cria uma `EventAnchor` de 259 bytes
por evento, que nunca é fechada; `IntentState` também nunca é fechado.

### S4a — Observações em lote (fase B)

Conta nova `BatchAnchor`, PDA `[b"batch", deployment_id, batch_id]`:

```text
batch_id, session_id, station_id, facility_id   [u8;32] cada
merkle_root                                     [u8;32]
leaf_count                                      u32   (1..=1.024)
first_observed_at, last_observed_at             i64
bump                                            u8
SPACE ≈ 189 bytes → rent ≈ 0,0022 SOL por lote (≈ 0,000004 SOL por observação com 500 folhas)
```

- Folha: `SHA-256("LASTRO_V2_BATCH_LEAF\0" ‖ envelope[220] ‖ station_signature[64])`.
  Árvore com separação de domínio de nó, igual à linhagem (`HASH_DOMAIN_LINEAGE_NODE` como
  referência).
- Instrução `anchor_observation_batch`: exige `SessionGrant` OPEN com OBSERVE, a mesma Station,
  `leaf_count ≤ max_events − events_used`, e incrementa `events_used`.
- **Observações em lote não avançam** `AssetState.state_version` nem `last_event_hash`.
  Elas são atestações que referenciam o `state_version` vigente na abertura da sessão.
  A cadeia de hashes do ativo passa a conter só eventos de identidade e custódia.
- O `record_observation` individual continua existindo para presença avulsa, com o custo atual.

**Limite declarado (troca consciente):** a assinatura P-256 de cada folha **não** é verificada
pelo programa. Ela é verificada:
1. pela API na admissão (como hoje);
2. pelo verificador independente, que recebe envelope, assinatura e prova Merkle no
   EvidencePackage.

Eventos de identidade e custódia continuam com verificação Secp256r1 on-chain.
O README e o `docs/PROTOCOL.md` precisam dizer isso explicitamente.

### S4b — EventAnchor reduzida (fase A)

A `EventAnchor` guarda campos que já estão cobertos por `event_hash`. Nova versão:

```text
event_hash     [u8;32]
subject_id     [u8;32]
state_version  u64
event_type     u16
bump           u8
SPACE = 83 bytes → rent ≈ 0,00147 SOL (−45%)
```

O verificador recalcula `event_hash` a partir dos 220 bytes do EvidencePackage e compara com a
conta. Nenhuma propriedade de verificação se perde, porque `event_hash` compromete todos os
campos removidos.

**Limite:** a mudança de layout vale só para deployments novos (Devnet). Não há migração de
contas em produção porque não existe produção.

### S4c — Fechamento de contas terminais (decisão D3, fora da v2.1)
Fechar `AssetState` e `EventAnchor` de ativos terminais exige um checkpoint verificável antes,
senão o verificador perde o estado canônico (quebra I4). Fica fora da v2.1 e precisa de
decisão sobre a janela de retenção.

### Metas (animal de referência da seção 2)

| Métrica | Atual | Meta |
|---|---|---|
| Capital travado por animal | ≈ 0,0372 SOL | ≤ 0,0095 SOL |
| Custo consumido por observação em lote | ≈ 0,000015 SOL | ≤ 0,00001 SOL |
| Contas criadas por observação | 1 | 0 (amortizada no lote) |

### Testes
- LiteSVM: lote válido, lote acima de 1.024, lote sem sessão, sessão de outra Station,
  `events_used` excedido.
- Protocolo: vetores de teste de folha e raiz Merkle em `test-vectors/` e
  `scripts/check_vectors.py`.
- Verificador: prova Merkle válida, folha alterada → `INVALID`, folha fora do lote → `INVALID`.
- Script de medição de rent (`scripts/`) reportando capital travado e custo consumido do
  animal de referência no G3.

---

## S5 — Fazenda, polígono e GNSS

**Problema:** não existe geolocalização real; o mapa da demo é simulado
(`apps/web/src/demo/chainHistory.ts`).

**Objetivo:** cada evento físico diz **onde** foi lido, ligado a uma fazenda com polígono
verificável.

### Escopo on-chain
- `FacilityRecord` ganha `geo_commitment: [u8;32]` =
  `SHA-256("LASTRO_V2_GEO\0" ‖ car_code_utf8 ‖ 0x00 ‖ geojson_canonico)`.
  O polígono e o código CAR ficam off-chain; on-chain fica só o compromisso.
- `StationRecord` ganha `facility_id`. Uma Station pertence a uma facility ACTIVE.
  `open_session` exige `session.facility_id == station.facility_id`.

### Payload v2.1 (coberto por `payload_hash`)

```text
offset tamanho campo
0      2      payload_version = 1
2      32     session_id (zero se não houver sessão)
34     4      lat_e7   i32 (WGS84)
38     4      lon_e7   i32
42     4      accuracy_cm u32
46     8      gnss_time_unix u64 (0 se não houver fix)
54     1      fix_type u8 (0 sem fix, 2 = 2D, 3 = 3D)
55     4      weight_grams u32 (0 se não houver balança)
59     ---    total 59 bytes
```

O layout é fixo, com codec manual, igual ao serial (decisão 19). Vetores de teste são
obrigatórios.

### Verificação geográfica (off-chain)
- A API calcula `DENTRO | FORA | INCONCLUSIVO` (ponto no polígono, com buffer igual a
  `accuracy_cm`). Sem fix ou com precisão > 50 m → `INCONCLUSIVO`.
- O EvidencePackage inclui o polígono, o código CAR e o payload; o verificador recalcula
  `geo_commitment` e o resultado.
- O resultado é rotulado como **cálculo**, não como prova da Station.

### Limites
- ≤ 10.000 vértices por polígono; só WGS84.
- Spoofing de GNSS é limitação declarada; a v2.1 não tenta detectá-lo.
- Sem GPS por animal. Os animais sentinela (GPS em 1–5% do lote) ficam para depois.

### Critérios de aceite
- Uma facility registrada com polígono e uma Station vinculada geram eventos com
  coordenada; o verificador mostra `DENTRO` e falha se um byte do polígono mudar.
- Uma sessão com facility diferente da Station é rejeitada on-chain.

---

## S6 — Triagem com dados públicos e Relatório de Origem por Lote

**Objetivo:** entregar o artefato que o frigorífico e o exportador compram: todas as fazendas
(diretas e indiretas) por onde cada animal do lote passou, com triagem e evidência.

### Fontes (snapshots importados, nunca "prova da Station")
| Fonte | Uso |
|---|---|
| SICAR | Situação do CAR da facility |
| IBAMA (áreas embargadas) | Sobreposição com o polígono |
| Cadastro de empregadores (trabalho análogo ao escravo) | Correspondência por CPF/CNPJ do responsável |
| PRODES | Desmatamento no polígono depois de 31/12/2020 |

Cada snapshot registra fonte, URL, data de download e SHA-256 do arquivo. Cada resultado de
triagem por facility é `CORRESPONDE | NAO_CORRESPONDE | NAO_VERIFICADO`, com o snapshot usado.

### Movimentação
A transferência de custódia aceita um anexo de GTA (número + SHA-256 do PDF) no `payload_hash`
da intent. É anexo declarado, não integração oficial.

### Relatório
- Entrada: lote ou embarque (≤ 512 ativos).
- Saída: JSON (canônico, assinado pela chave de relatório da API), PDF e CSV.
- Conteúdo por animal: AssetID, RFIDs, trajetória de facilities com datas, resultado
  geográfico (S5), resultado de triagem de cada facility, completude da cadeia.
- Indicadores do lote: % de animais com cadeia completa, % com origem `DENTRO`, facilities com
  `CORRESPONDE` em alguma fonte, idade do snapshot mais antigo.
- Texto obrigatório: *"Este relatório reúne evidências verificáveis e não constitui declaração
  de conformidade legal."*

### Fora do escopo
- APIs oficiais (GTA, SISBOV, PNIB), monitoramento contínuo por satélite, score de risco.

### Critérios de aceite
- Um lote de 512 animais em 3 fazendas (1 indireta) gera o relatório em até 60 s.
- Alterar um snapshot invalida o hash citado no relatório.

---

## S7 — Station como ponte de leitores de mercado

A Station está sendo construída pela equipe de hardware; esta frente define só o **contrato**.

### Escopo
- O adaptador RFID (`firmware/station`, hoje stub em `rfid.c`) recebe leituras de um leitor
  ISO 11784/11785 de mercado (bastão ou painel) por serial ou Bluetooth e as normaliza no RFID
  canônico de 8 bytes big-endian (decisão 15).
- Entrada opcional de peso de indicador de balança.
- Módulo GNSS com hora do satélite alimentando o payload (S5).
- Build de release com `#error` se `CONFIG_LASTRO_STATION_DEV_PRIVATE_KEY_HEX` ou o modo
  test-vector estiverem habilitados.

### Limites
- O modelo do leitor é a decisão D4. Nenhum frame é inventado sem o manual e capturas reais
  (`docs/HARDWARE.md`).
- Sem projeto de antena própria na v2.1.

### Critério de aceite
- Uma tag real lida por um leitor de mercado gera um envelope assinado, admitido pela API e
  ancorado em lote na Devnet.

---

## S8 — Modo offline

### Escopo
- A sessão é aberta online (no escritório ou com sinal) e baixa o manifesto (S3).
- Durante a sessão, Station e Agent operam sem rede; o outbox SQLite guarda ≥ 10.000 eventos.
- Na reconexão, o Agent envia, a API agrega em lotes (S4a) e ancora.
- Constante nova `MAX_BATCH_EVENT_AGE_SECONDS = 604.800` (7 dias), aplicada só a
  `anchor_observation_batch`. Eventos identitários mantêm 24 h.
- Com GNSS (S5), `gnss_time_unix` é a referência de tempo exibida; `observed_at` do host continua
  sendo registrado.

### Limites
- BIND offline só para AssetIDs que já estão no manifesto.
- Sessão expirada antes do envio: eventos ficam `EXPIRED_UNANCHORED`, visíveis e nunca
  descartados em silêncio.

### Critério de aceite
- Uma sessão de 500 leituras com a rede cortada do início ao fim é ancorada inteira depois da
  reconexão, sem perda e sem duplicação.

---

## S9 — CI, Devnet e auditoria

### Escopo
- Runner self-hosted ou billing que faça os workflows existentes realmente executarem
  (hoje os runners não iniciam).
- Todos os gates rodando: LiteSVM v1/v2, API com PostgreSQL, Vitest, Playwright, G3, G5.
- Gate G4 (Devnet) com sponsor (S1) e sessão (S3).
- Auditoria externa do programa v2.1 **contratada** antes de qualquer mainnet. A execução da
  auditoria fica fora desta spec.

### Critério de aceite
- CI verde no branch principal com todos os jobs listados, com logs arquivados.

---

## 3. Fases e dependências

```text
Fase A  S9 (CI) ─┬─ S1 (sponsor) ─┬─ S4b (EventAnchor reduzida)
                 │                │
Fase B           └────────────────┴─ S3 (sessão) ── S4a (lotes) ── S8 (offline)
Fase C                               S2 (contas de organização)    S5 (geo)
Fase D                                                             S6 (relatório)
Paralelo        S7 (Station), conduzida pela equipe de hardware, entra quando D4 for decidida
```

| Fase | Frentes | Estimativa | Saída verificável |
|---|---|---|---|
| A | S9, S1, S4b | 2–3 semanas | G3 com carteiras sem saldo; rent −45% por evento |
| B | S3, S4a, S8 | 4–6 semanas | Sessão de 500 animais offline em ≤ 3 transações |
| C | S2, S5 | 4–5 semanas | Operador sem carteira; eventos com coordenada verificada |
| D | S6 | 3–4 semanas | Relatório de Origem de um lote de 512 animais |

As estimativas consideram 2 desenvolvedores e precisam ser revistas ao fim da fase A.

## 4. Decisões pendentes

| ID | Decisão | Bloqueia |
|---|---|---|
| D1 | Provedor de carteira gerenciada (critérios: SDK sem React, política no provedor, exportação de chave, custo por carteira) | S2 |
| D2 | Tetos de patrocínio e quem absorve o rent no modelo comercial | S1 (valores) |
| D3 | Janela de retenção e checkpoint para fechar contas terminais | S4c |
| D4 | Modelo do leitor RFID e do indicador de balança | S7 |
| D5 | Uma sessão pode cobrir vários custodiantes (ex.: confinamento)? Na v2.1, não. | S3 |

## 5. Riscos desta spec

| Risco | Mitigação |
|---|---|
| A sponsor vira alvo (drenagem de saldo) | Política estrita de instruções, tetos diários, alerta de saldo |
| Lotes reduzem a verificação on-chain das observações | Declarado no protocolo; identidade e custódia continuam on-chain; verificador independente checa todas as folhas |
| A carteira gerenciada centraliza a confiança | Chave por organização, exportável, política no provedor |
| O manifesto de sessão vaza a relação RFID → ativo da fazenda | Manifesto só com hashes, cifrado em repouso no Agent, apagado ao fechar a sessão |
| Mudanças de layout (S4b/S5) quebram vetores existentes | Novos vetores v2.1 versionados; os vetores v2 continuam válidos para o programa v2 |

## 6. Fora de engenharia (registrado para não virar feature)

- Resposta ao "por que blockchain?": vender "registro neutro de evidência que nenhuma parte
  controla"; blockchain é detalhe de infraestrutura.
- Integração com grandes frigoríficos: posicionar como fornecedor de evidência para os
  programas de monitoramento de indiretos que eles já têm, depois de pilotos com certificadoras
  e exportadores médios.
- Validação de preço por cabeça e de quem paga: 15–20 entrevistas antes da fase D.

---

## 7. Estado de implementação (12/10/2026)

### Fase A — entregue
- **S1:** `payer` separado em `register_asset`, `record_observation`, `bind_identifier`,
  `replace_identifier`, `create_intent`; `IntentState` fechada em toda transição terminal com
  reembolso ao `rent_payer`; rota `POST /api/v2/sponsor/submit` com política, tetos e livro-razão;
  validador do navegador aceita só o sponsor fixado em `VITE_LASTRO_SPONSOR`.
- **S4b:** `EventAnchor` de 83 bytes; API, verificador e teste de sistema atualizados.
- **Refinamento:** instruções administrativas (inicialização, Station, fazenda, parte) e de
  transformação **não** são patrocinadas — quem assina já é a plataforma ou o frigorífico.

### Fase B — entregue (B1)
- **On-chain:** `SessionGrant` (`open_session`, `revoke_session`, `close_session`) e
  `BatchAnchor` (`anchor_observation_batch`), com 5 testes LiteSVM.
- **Protocolo:** `session.rs` (id de captura derivado da sessão, folha e árvore Merkle do lote),
  espelhado em TypeScript.
- **API:** ciclo da sessão, manifesto congelado, ingestão de observações (append-only),
  preparação/confirmação de lotes, `batchedObservations` no EvidencePackage, propósitos
  `SESSION_*` no patrocínio.
- **Verificador:** cada folha é checada localmente (assinatura, vínculo com a sessão, cabeça do
  histórico, tag ativa, prova Merkle) e contra a `BatchAnchor` finalizada.

**Refinamentos em relação ao texto original da S3/S4a/S8:**
1. **Sem mudança de firmware para assinar.** Em vez de um payload novo com `session_id`, a
   observação de sessão é uma captura OBSERVE comum cujo `capture_id` é derivado de
   `(session_id, asset_id)`. A Station atual assina sem alteração; o vínculo com a sessão e a
   regra "uma prova por animal por sessão" vêm do `event_id`.
2. **Offline × expiração.** Uma observação precisa ter sido feita **dentro** da janela da
   sessão (ou antes da revogação); o lote pode ser ancorado até 7 dias depois da última
   observação, mesmo com a sessão já expirada. A revogação corta tudo que vier depois dela.
3. **BIND sob sessão adiado para a fase C.** Exige relay automático de transações por evento,
   que pertence às contas de organização (S2). Na v2.1 a sessão só autoriza OBSERVE.
4. **`facility_id` na sessão adiado para S5**, junto com o vínculo Station → fazenda.
5. **`BatchAnchor` copia custodiante e Station**, para que a `SessionGrant` possa ser fechada
   (rent devolvido) sem impedir a verificação.

**Custo do animal de referência (seção 2):** `AssetState` 0,00326 + 2 `RfidBinding` 0,00282 +
2 `EventAnchor` reduzidas 0,00294 + 8 observações em lote ≈ 0,00003 ⇒ **≈ 0,0091 SOL** de
capital travado (−76%, meta ≤ 0,0095 atingida), pago pela plataforma.

### Fase C — entregue (API, programa e verificador)
- **S5:** `FacilityRecord.geo_commitment` (155 bytes), `StationRecord.facility_id` (187 bytes) e
  `set_station_facility`; `open_session` exige Station instalada numa fazenda ACTIVE e válida
  na janela; `SessionGrant` e `BatchAnchor` copiam o `facility_id`. Rota de polígono imutável
  (CAR + anéis), compromisso geográfico, pacote com `facilities` e camada `FACILITY_ORIGIN` no
  verificador.
- **S2:** organizações com carteira custodiante própria, login por e-mail + código, papéis
  `ORG_ADMIN`/`OPERADOR_CURRAL`/`AUDITOR`/`LEITURA`, assinatura gerenciada em
  `POST /api/v2/org/sign` (com ou sem patrocínio) e exportação da chave pelo `ORG_ADMIN`.

**Refinamentos em relação ao texto original da S2/S5:**
1. **Compromisso geográfico sobre codificação binária fixa**, não sobre GeoJSON canônico:
   canonicalizar JSON com ponto flutuante diverge entre linguagens; inteiros `e7` não.
2. **Cofre local no lugar do provedor (D1 continua aberta).** A chave é gerada pela API e
   guardada só cifrada (AES-256-GCM, chave mestra fora do banco, AAD = organização + carteira).
   A política é aplicada pela própria construção: o cofre só assina a transação que a API
   re-deriva para um propósito nomeado, então não existe caminho para assinar outro programa ou
   bytes arbitrários. Um provedor externo entra atrás das mesmas chamadas
   (`generate`/`open`/`sign`) quando D1 for decidida; até lá, a chave mestra é o ponto único de
   confiança e precisa ficar num cofre de segredos do ambiente.
3. **Entrega do código por log (desenvolvimento).** Não há provedor de e-mail/SMS; com
   `LASTRO_OTP_DELIVERY` vazio o login por código fica desligado.
4. **Coordenada por evento (payload GNSS de 59 bytes) fica para o hardware.** A fase C liga
   cada lote à fazenda pela Station instalada; o ponto-no-polígono por leitura entra com o GNSS.

**Pendente da fase C:** telas de login por código e de organização no front-end (o fluxo atual
com carteira continua funcionando) e o E2E de OTP com servidor de e-mail de teste.

### Revisão técnica (12/10/2026) — correções aplicadas
Revisão completa (on-chain, API, protocolo/Agent/firmware, web, infra) com correção em quatro
blocos:
1. **CI e API:** `maintenance-sync` só verifica (sem token de escrita, sem push, sem regenerar
   locks); deploy só de push no `main` do próprio repositório e só com chain/e2e/firmware sem
   falha; sessões exigem operador (custodiante conhecido) ou a própria organização; o
   patrocinador só paga para autoridade, partes ACTIVE e carteiras de organização; exportação da
   chave com OTP novo, uma vez, encerrando a assinatura gerenciada; OTP por log recusado em
   deploy público; logout, desativação de usuário, 403, timeout e limite de concorrência.
2. **On-chain:** transformação na ordem da cadeia (entradas reservadas → consumidas → saídas →
   finalização; abortar/expirar só antes de consumir); `session_id` derivado do custodiante e
   dos termos; Station suspensa/revogada não ancora lote; intent consumida vira tombstone;
   EXPIRED só sai por extensão; limites na configuração; ativo reservado não aceita evento;
   `SCHEMA_VERSION = 2`.
3. **Firmware/Agent:** build de produção exige Secure Boot v2, flash criptografada (release) e
   JTAG desligado; parser serial não trava com rajadas; ECDSA determinístico na chave de
   desenvolvimento; Agent não cai por um comando ruim, libera a Station em BUSY e exige https
   fora do loopback.
4. **Infra:** cabeçalhos de segurança (CSP, HSTS, frame-ancestors), cache correto, API sem root
   e containers com `no-new-privileges`/capabilities mínimas/rotação de log, backups cifrados e
   atômicos com restauração, rollback que desfaz migração, teste OpenAPI ↔ router.

**Pendentes conscientes:** `PartyRecord` como autorização on-chain (M6), `asset_type` na folha
de linhagem (M5), fazenda ativa exigida também no evento avulso (M8), entrega de OTP por
e-mail/SMS, chave mestra em KMS (D1), contador monotônico assinado pela Station (horário),
documentação OpenAPI das 23 rotas operacionais (lista congelada no teste) e validação do build
ESP-IDF de produção em hardware provisionado.

### Fase B2 — pendente (equipe de hardware)
Leitura contínua no curral. Contrato serial proposto:
1. A Station emite, sem comando, `TAG_SEEN(rfid canônico de 8 bytes)` a cada leitura.
2. O Agent procura o `rfid_hash` no manifesto (guardado no SQLite para funcionar offline) e envia
   o `CaptureCommand` já existente montado por `SessionAssetContext::command`.
3. A Station confirma que leu a mesma tag (mantém a última leitura por alguns segundos), assina
   e responde `EVENT_READY` como hoje.
4. O Agent grava no outbox e entrega em `POST /api/v2/sessions/{id}/observations` quando houver
   sinal.

Tags fora do manifesto ficam registradas como métrica (`UNMATCHED`) e não são assinadas.

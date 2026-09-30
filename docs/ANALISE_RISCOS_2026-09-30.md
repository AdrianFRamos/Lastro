# Análise de riscos do Lastro v2 — 30/09/2026

Revisão da ideia e da implementação depois da migração para o v2. Cada item diz o que foi
encontrado, o impacto e o que foi feito.

## Defeitos corrigidos

| # | Problema | Impacto | Correção |
|---|---|---|---|
| 1 | A API tratava capturas já finalizadas como "abertas" | Depois da primeira captura, nenhuma outra transição do ativo era aceita (409) | `find_open_for_asset` ignora capturas cujo evento está FINALIZED/REJECTED; regressão em `services/api/tests/capture_v2.rs` |
| 2 | Evento assinado e transmitido pela carteira, mas nunca reportado à API (aba fechada, rede) | Uma captura nova o marcava REJECTED embora finalizado on-chain; o pacote de evidência perdia o evento e o verificador acusaria INVALID para sempre | A API recupera a assinatura que criou o `EventAnchor` (`getSignaturesForAddress`) e finaliza o evento; não substitui evidência que ainda pode entrar na chain (até `expires_at`); o reconciliador liquida evidência vencida (recupera se ancorada, rejeita se não) |
| 3 | `lastro-protocol` exigia rustc 1.98.1 | O programa não compilava para SBF (platform-tools usa rustc 1.95); CI da chain quebrado | `rust-version = "1.85"` no crate de protocolo |
| 4 | `p256` com `std`/`pem` no crate de protocolo | `getrandom` entrava no build SBF e falhava | `p256` sem `std` (apenas `ecdsa`) nas dependências normais |
| 5 | Testes LiteSVM procuravam `target/deploy/lastro-v2.so` | O Anchor gera `lastro_v2.so`; a suíte on-chain nunca rodaria | Caminho corrigido |
| 6 | Registro da Station vale 1 ano e não havia renovação | Com chave em eFuse (não rotacionável) a Station ficaria inutilizável | Instrução `extend_station_validity` (só autoridade, só para frente, nunca para Station revogada) + `initialize_protocol_config.py --extend-station-days` |
| 7 | API aceitava capturas que a chain recusaria (Station inativa/expirada) | Station e carteira assinavam algo impossível de ancorar | Pré-checagem do `StationRecord` na captura; `/api/health` fica `degraded` |
| 8 | Toda prova de presença exigia a chave da autoridade da implantação | Chave de maior privilégio usada no dia a dia (chave "quente") | O programa aceita o custodiante atual (ou a autoridade); a API e o console usam o custodiante |
| 9 | Destinatário da custódia não conseguia aceitar em outro dispositivo | O ID da transferência só existia na aba de quem propôs | Campo compartilhável no console; a carteira conectada aceita para si |
| 10 | Verificador web fazia RPC em série com orçamento de 30 s | Históricos com dezenas de eventos ficavam NOT_CHECKED | RPC paralelo (8 simultâneas), orçamento de 120 s |
| 11 | Limite de 256 eventos por pacote contando eventos rejeitados | Animal com provas de presença frequentes estouraria em meses | Limite de 1024 contando só eventos FINALIZED |
| 12 | Testes de retry do Agent com timeout HTTP de 1 s | Falhas intermitentes sob carga | Timeout de 10 s nos testes que não testam timeout |
| 13 | `register_station_v2` validava o ponto P-256 em software | Estourava o limite de compute units: nenhuma Station podia ser registrada | Validação removida (o precompile Secp256r1 valida o ponto em cada evento; registro é só da autoridade) |
| 14 | API gravava `observed_at` com a hora do servidor | O relógio do cluster (sobretudo em `finalized`) fica atrás: toda captura falhava com `EventOutsideValidityWindow` | Janela abre 300 s antes da captura (`observed_at` passa a ser limite inferior) |
| 15 | Inicializador enviava `initialize_v2` logo após o deploy | Simulação em `finalized` não encontrava o programa ou o via "not deployed" | Espera o programa finalizar e repete só nesse erro |
| 16 | Teste de transformação reservava e consumia insumo a insumo | O programa passa a FINALIZING no primeiro consumo; o teste nunca tinha rodado | Regra documentada (todos os insumos reservados antes do consumo) e teste corrigido |
| 17 | Harness PTY fechava descritores com a thread de leitura ativa | Descritor reaproveitado por outro arquivo era lido pela thread | Espera a thread antes de fechar |

## Riscos de projeto que continuam abertos

| Risco | Por que importa | Recomendação |
|---|---|---|
| Tag vinculada ao animal errado | O RFID nunca pode ser reutilizado; a tag errada fica RETIRED apontando para o animal errado e não pode ir para o animal certo | Processo operacional de dupla leitura antes de assinar; para o piloto, uma instrução de "correção" pela autoridade com trilha auditável |
| Troca física de tags entre animais | O sistema prova que a tag foi lida, não qual animal a carrega | Evidência complementar (foto, marcação visual, pesagem) anexada ao evento no piloto |
| `observed_at` vem do host | A Station atesta a leitura, não o horário | Relógio assinado (RTC/GNSS) no hardware do piloto |
| Uma única chave de Station na API/Agent | Não escala para várias fazendas/Stations | Suportar lista de Stations e roteamento de capturas por Station |
| Cada evento cria uma conta on-chain (rent permanente) | Custo cresce com provas de presença | Medir o custo por animal/ano no piloto; limitar a frequência de presença |
| Pacote de evidência com até 1024 eventos | Históricos muito longos exigirão outro formato | Pacote paginado com checkpoints (hash do histórico + verificação por faixas) |
| Token de operador único compartilhado | Qualquer portador cria partes, fazendas e propostas (a custódia em si continua exigindo carteiras) | Autenticação por usuário e autorização por papel |

## Verificação

Executado em container com a toolchain do CI (Ubuntu 24.04, Rust 1.98.1, Solana 4.2.0,
Anchor 1.2.0): suíte LiteSVM completa (25 testes) e G3 (`tests/system/test_full_local.py`, fluxo
real com validador, programa, API, PostgreSQL, Agent e Station por PTY). Em Docker, o
`solana-test-validator` 4.2 precisa de `--security-opt seccomp=unconfined` (usa `io_uring`).

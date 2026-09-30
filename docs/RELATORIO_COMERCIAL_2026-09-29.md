# Lastro — Relatório Comercial

**Data:** 29/09/2026
**Perspectiva:** produto, mercado e estratégia de go-to-market
**Complementa:** `RELATORIO_TECNICO_2026-09-29.md`

> Os números de mercado citados aqui são ordens de grandeza de fontes públicas (IBGE, ABIEC, MAPA, Comissão Europeia) usadas como referência. **Valide-os na fonte primária antes de usá-los em pitch ou documento de investimento.**

---

## 1. Sumário executivo

O Lastro vende **confiança verificável sobre ativos físicos**. Começa pelo boi e responde três perguntas: *qual é este animal, quem teve a custódia e se a história pode ser verificada sem confiar em quem a produziu*. A combinação que o diferencia de "rastreabilidade em planilha" e de "blockchain que guarda hash" é:

1. **Identidade que sobrevive à troca de brinco** (AnimalID estável, RFID substituível).
2. **Evidência física assinada por hardware** (Station com chave P-256), **verificada dentro do contrato Solana**.
3. **Verificador independente**: um comprador, auditor ou banco confere a história no próprio navegador.

**Estado comercial:** protótipo de hackathon tecnicamente forte, **sem hardware RFID real, sem piloto e sem cliente**. A tese é boa e o momento regulatório favorece (EUDR, PNIB, exigências de importadores), mas o produto precisa sair de "primitiva criptográfica" para "resolve uma dor de compliance de frigorífico ou exportador" antes de qualquer tese financeira (crédito, seguro, RWA).

**Recomendação central:** posicionar como **camada de evidência para compliance socioambiental e sanitário da cadeia bovina**, vendida para quem carrega o risco (frigorífico, exportador, certificadora), com piloto pago em 1 a 2 fazendas fornecedoras de um frigorífico exportador. Deixar crédito e RWA como narrativa de longo prazo, não como produto inicial.

---

## 2. O problema e por que agora

### 2.1 A dor

- A cadeia bovina brasileira é longa e fragmentada: cria → recria → engorda → frigorífico, frequentemente com várias fazendas e transportadoras por animal.
- **Fornecedores indiretos** (fazendas de cria e recria) são o ponto cego clássico: o frigorífico conhece a fazenda que vendeu, mas não as anteriores. É justamente ali que se concentram os riscos de desmatamento e irregularidade.
- **Brincos se perdem ou são trocados**, e cada troca quebra a história em sistemas que tratam o brinco como identidade.
- Hoje a informação é **declaratória** (GTA, planilhas, sistemas privados), reconciliada manualmente e dependente da confiança em quem opera o sistema.

### 2.2 Por que agora (vetores regulatórios e de mercado)

| Vetor | Impacto no Lastro |
|---|---|
| **EUDR** (Regulamento Europeu Anti-Desmatamento): gado e carne estão no escopo; exige due diligence com geolocalização da origem. O cronograma de aplicação já foi adiado mais de uma vez. | Demanda de evidência de origem por lote exportado para a UE |
| **PNIB** (Plano Nacional de Identificação Individual de Bovinos e Búfalos, MAPA, lançado em 2024 com implantação gradual até cerca de 2032) | O brinco individual vira regra e cria a base física sobre a qual o Lastro opera |
| **Compromissos de frigoríficos** (desmatamento zero, monitoramento de indiretos) e pressão de varejistas e investidores ESG | Comprador corporativo com orçamento e urgência |
| **Importadores** (China, UE, Oriente Médio) exigindo protocolos sanitários e de origem | A exportação valoriza a prova verificável |

**Ordem de grandeza do Brasil:** rebanho de cerca de 230 a 240 milhões de cabeças, abate na casa de 35 a 40 milhões de cabeças por ano, e posição de maior exportador mundial de carne bovina.

---

## 3. Proposta de valor por segmento

| Segmento | Dor | O que o Lastro entrega | Disposição a pagar |
|---|---|---|---|
| **Frigorífico exportador** | Risco reputacional e regulatório com indiretos; custo de auditoria | Cadeia de custódia verificável por animal e por lote; pacote de evidência por embarque | **Alta**: é o comprador natural |
| **Exportador / trading** | Due diligence EUDR e protocolos de importadores | Evidence package exportável, verificável pelo cliente final | Alta |
| **Certificadoras e protocolos privados** (ex.: carne rastreada, orgânica, bem-estar) | Auditoria cara e amostral | Trilha contínua e à prova de adulteração, o que reduz custo de auditoria | Média/alta |
| **Produtor** | Burocracia; quer prêmio de preço e acesso a mercado | Acesso a compradores premium; histórico que valoriza o animal | **Baixa** (paga só se ganhar prêmio) |
| **Bancos, seguradoras, fintechs agro** | Assimetria de informação sobre garantia (rebanho) | Prova de existência, identidade e custódia do colateral | Alta, **mas depende de escala e de validação jurídica** |

**Insight de produto:** quem opera o sistema (o produtor) não é quem paga. O modelo precisa subsidiar o produtor e cobrar de quem captura o valor do risco reduzido. O próprio `PRODUCT_VISION.md` já aponta nessa direção, e está certo.

---

## 4. Diferenciação competitiva

### 4.1 Paisagem

Exemplos de players (a lista não é exaustiva e deve ser validada):

- **Plataformas de monitoramento socioambiental** (geomonitoramento de fazendas fornecedoras, cruzamento com listas públicas): atuam na *fazenda*, não no *animal*.
- **Rastreabilidade com blockchain agro** (startups brasileiras de rastreio bovino com QR code e registro em blockchain): em geral ancoram registros declarados, sem assinatura física verificada on-chain.
- **Sistemas próprios dos grandes frigoríficos** (plataformas de transparência de fornecedores): controlados por uma parte interessada, o que é justamente a dependência de confiança que o Lastro remove.
- **SISBOV / certificadoras oficiais** e o futuro sistema do PNIB: fonte oficial, que o Lastro deve **integrar e não substituir** (o projeto já assume esse princípio).
- **Software de gestão de fazenda** (pesagem, sanidade, manejo): dono do dado operacional do produtor e potencial canal de distribuição.

### 4.2 Onde o Lastro é genuinamente diferente

| Atributo | Mercado típico | Lastro |
|---|---|---|
| Identidade separada do brinco | Raro | **Nativo (REIDENTIFY)** |
| Evidência física assinada por dispositivo | Raro | **Sim (Station P-256)** |
| Verificação da assinatura *dentro* do contrato | Praticamente inexistente | **Sim (precompile Secp256r1)** |
| Verificação independente pelo terceiro | Raro (exige confiar no portal) | **Sim (verificador no navegador)** |
| Integração oficial (GTA, SISBOV, PNIB) | Comum nos incumbentes | **Não existe ainda** |
| Geomonitoramento socioambiental | Forte nos incumbentes | **Não existe** |
| Hardware de campo pronto | Variável | **Não existe (driver RFID pendente)** |

**Moat potencial:** o protocolo aberto de evidência (formato, vetores e verificador) pode virar **padrão**, e a rede de Stations certificadas mais a base histórica criam efeito de rede. **Moat real hoje:** know-how técnico e qualidade de engenharia, que são defensáveis mas copiáveis em 12 a 18 meses por um incumbente bem financiado.

**Posicionamento recomendado:** complementar ao geomonitoramento, não concorrente. "Eles provam que a fazenda é limpa; o Lastro prova que *este animal* passou por *estas* fazendas." Parcerias com plataformas de monitoramento são um atalho de distribuição.

---

## 5. Modelo de negócio

### 5.1 Fontes de receita sugeridas

| Linha | Cobrança | Observação |
|---|---|---|
| **SaaS de rastreabilidade** (frigorífico, exportador) | Por cabeça abatida ou rastreada, ou assinatura por planta | Receita principal |
| **Station como serviço** | Locação ou comodato do dispositivo mais mensalidade | Subsidiar o produtor; cobrar do frigorífico |
| **Evidence package / API de verificação** | Por consulta ou por embarque exportado | Monetiza compradores internacionais e auditores |
| **Integrações e implantação** | Setup e projeto | Relevante no início (enterprise) |
| **Dados para crédito e seguro** (futuro) | Por análise ou participação | Só após escala e validação jurídica |

### 5.2 Unit economics (ilustrativo, a validar)

- **Custo on-chain v1 por animal:** cerca de 0,0034 SOL no ORIGIN, cerca de 0,00001 SOL por transferência e cerca de 0,0014 SOL por troca de brinco. Com o SOL a US$ 150, isso dá **≈ US$ 0,50 a 0,80 por animal na vida inteira**. É viável, mas relevante frente a um preço-alvo de alguns reais por cabeça.
- **Alerta v2:** cada observação trava cerca de 0,0027 SOL (≈ US$ 0,40). Com 20 observações por animal, só o rent chega a ≈ US$ 8 por animal, **o que inviabiliza o modelo sem compressão ou fechamento de contas**. É prioridade técnica com impacto direto na margem.
- **Station:** o BOM depende do leitor (ESP32-C5 mais leitor FDX-B de baixa frequência, gabinete IP65, energia e conectividade). Estimo algumas centenas de dólares por unidade de campo; o custo precisa ser validado com o hardware escolhido.
- **Exposição ao preço do SOL:** o custo por animal oscila com o token. Mitigação: repassar ao cliente por evento, manter reserva em SOL ou migrar para contas comprimidas.

**Hipótese de preço para testar no piloto:** R$ 2 a 10 por cabeça rastreada até o abate, pagos pelo frigorífico, com prêmio para o produtor via bonificação por arroba. **Validar em entrevistas com compradores.**

---

## 6. Estágio atual e riscos comerciais

### 6.1 O que é vendável hoje
- **Demo convincente** do fluxo completo (origem → transferência → troca de brinco → rejeição de fraude → verificação pública), rodando localmente com validador Solana real.
- **Narrativa forte e honesta**: "o brinco mudou, o animal não, e a história não pôde ser reescrita".
- **Documentação e engenharia** que resistem a due diligence técnica de investidor ou banca.

### 6.2 O que ainda não é vendável
- **Nenhum leitor RFID real**: sem campo, não há produto.
- **Uma chave de Station por deployment**, sem rotação: não escala para uma rede de dispositivos.
- **Autorização operacional por token único**: não há multi-empresa, papéis nem assinatura das partes.
- **O v2 (lotes, carcaças, cortes, recall) não funciona on-chain** por um bug de alocação e não fecha ponta a ponta na API.
- **Nenhuma integração** com GTA, SISBOV ou PNIB, nem geolocalização. Para a EUDR, a geolocalização da origem é central.
- **CI nunca executado**: não há evidência formal de qualidade para um cliente enterprise.

### 6.3 Matriz de riscos

| Risco | Prob. | Impacto | Mitigação |
|---|---|---|---|
| Adoção no campo (produtor não quer mais um dispositivo nem mais trabalho) | Alta | Alto | Station embarcada no tronco ou balança já usada; zero trabalho extra; prêmio por arroba |
| Brinco/RFID clonável (a prova física tem teto) | Média | Alto | Comunicar o limite com honestidade; combinar com pesagem, foto e geolocalização; olhar tags criptográficas no futuro |
| Incumbente (frigorífico ou plataforma) replica | Média | Alto | Protocolo aberto como padrão; parceria em vez de competição; neutralidade como argumento |
| Regulação muda ou atrasa (adiamentos da EUDR, cronograma do PNIB) | Alta | Médio | Não depender de um único driver regulatório; vender também redução de fraude e auditoria |
| Percepção negativa de "cripto" no agro e no setor bancário | Média | Médio | Vender "evidência verificável"; blockchain como detalhe de infraestrutura |
| LGPD e dado sensível (localização de fazenda, dados de produtor) em rede pública | Média | Alto | Só hashes e compromissos on-chain (o design já segue isso); política de dados formal |
| Custo on-chain e volatilidade do SOL | Média | Médio | Compressão, fechamento de contas, repasse por evento |
| Custódia ≠ propriedade (uso como garantia) | Alta, para produtos financeiros | Alto | Parecer jurídico antes de qualquer produto de crédito ou RWA; integração com registro de penhor |

---

## 7. Go-to-market recomendado

### Fase 1: piloto de evidência (0 a 6 meses)
- **Cliente-âncora:** 1 frigorífico exportador de médio porte ou uma certificadora, com 1 a 3 fazendas fornecedoras.
- **Escopo:** v1 (origem, transferência, troca de brinco) com leitor RFID real, Station no curral ou tronco, e evidence package por lote abatido.
- **Métrica de sucesso:** percentual de animais com cadeia completa verificável, tempo de auditoria economizado, NPS do comprador, custo por cabeça.
- **Pré-requisitos técnicos:** driver RFID, CI rodando, registry de Stations, correção do v2.

### Fase 2: compliance de exportação (6 a 18 meses)
- Geolocalização da origem com proveniência, integração com GTA/SISBOV/PNIB e relatório de due diligence (EUDR) gerado a partir do evidence package.
- Multi-empresa com papéis e assinaturas por parte; custódia com aceite do recebedor.
- Linhagem pós-abate (carcaça → cortes → embarque) com o v2 funcionando.

### Fase 3: plataforma e capital (18 meses ou mais)
- API de verificação para importadores e varejo; selo verificável no produto final.
- Parcerias com bancos e seguradoras para crédito com garantia em rebanho, **com estrutura jurídica validada** (CPR, penhor pecuário, registradoras).
- RWA e tokenização só com direito real subjacente e enforceability. O próprio projeto já assume essa ordem, e é a ordem certa.

### Canais
1. Venda direta enterprise (frigoríficos e exportadores).
2. Parceria com certificadoras e protocolos de qualidade (cada certificadora traz seus produtores).
3. Parceria com software de gestão de fazenda e fabricantes de balança e tronco (Station embarcada).
4. Ecossistema Solana e grants para financiar P&D inicial.

---

## 8. Pitch sugerido (uma frase por bloco)

- **Problema:** a cadeia do boi depende de declarações, e o brinco, que é a única ligação física, se perde.
- **Solução:** identidade que sobrevive à troca do brinco, com cada evento assinado por hardware e validado por um contrato público.
- **Prova:** qualquer comprador verifica a história inteira no navegador, sem confiar em nós.
- **Mercado:** dezenas de milhões de abates por ano no Brasil, pressão regulatória crescente da UE e de importadores.
- **Modelo:** SaaS por cabeça pago por quem carrega o risco; produtor subsidiado.
- **Visão:** o que se pode provar pode ser financiado; identidade → custódia → história → capital.

---

## 9. Avaliação para investimento / banca

| Critério | Avaliação |
|---|---|
| Qualidade técnica do time | **Muito alta**: rigor raro (vetores cruzados, verificação on-chain, honestidade sobre limites) |
| Clareza da tese | **Alta** |
| Timing de mercado | **Favorável** (EUDR, PNIB, ESG) |
| Tração | **Nenhuma** (sem piloto, sem LOI) |
| Risco de execução em hardware e campo | **Alto** |
| Defensibilidade | Média; depende de virar padrão e de construir rede |
| Pontos que um investidor vai questionar | Quem paga? Por que blockchain? Qual o CAC num setor conservador? O que acontece se o frigorífico construir sozinho? |

**Próximos passos com maior retorno comercial:**
1. **Conseguir 1 LOI ou piloto** com frigorífico ou certificadora, antes de qualquer nova feature.
2. **Fechar o hardware** (leitor RFID escolhido, Station funcionando em curral real), que é a prova que falta à narrativa.
3. **Corrigir e comprovar a engenharia** (CI verde, bug do v2 corrigido, Devnet), que é o que resiste a due diligence técnica.
4. **Adicionar geolocalização com proveniência**, que desbloqueia a narrativa EUDR.
5. **Entrevistar 15 a 20 compradores** (frigorífico, exportador, certificadora, banco) para validar preço e disposição a pagar.

---

## 10. Conclusão

O Lastro tem **uma das bases técnicas mais sólidas que se pode esperar de um projeto nesse estágio** e uma tese alinhada a uma pressão regulatória real. O risco não está na ideia nem no código central: está em **atravessar para o mundo físico e comercial**, com leitor RFID, rede de dispositivos governada, integração com fontes oficiais e, acima de tudo, um primeiro cliente pagando. A recomendação é concentrar os próximos 6 meses num piloto pago de rastreabilidade para exportação, usando o núcleo v1 que já é robusto, e tratar lote, linhagem e capital como expansões condicionadas à validação desse piloto.

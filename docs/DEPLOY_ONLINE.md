# Deploy online do Lastro

Este documento descreve o primeiro deploy controlado do Lastro em uma VM Linux. A arquitetura recomendada separa o frontend do backend quando possível, mas o Compose incluído no repositório também permite executar os quatro serviços na mesma VM durante o piloto.

## Arquitetura recomendada

Use `Cloudflare Pages` para o frontend Vue/Vite e a VM apenas para a API Rust, PostgreSQL, proxy HTTPS e worker de reconciliação. Se o frontend for executado na própria VM, o serviço `proxy` publica tanto o site quanto a API.

```text
seudominio.com      -> Cloudflare Pages ou proxy/web:80
api.seudominio.com  -> proxy -> api:8080
api:8080            -> PostgreSQL interno
api:8080            -> RPC Solana configurado
```

O PostgreSQL não deve receber porta pública. A única entrada pública da VM deve ser `80/tcp` e `443/tcp`; o SSH deve ser restringido ao IP administrativo sempre que possível.

## Pré-requisitos da VM

Use Ubuntu 24.04 LTS com Docker Engine e Docker Compose Plugin. Reserve pelo menos 2 vCPU, 4 GB de RAM e 50 GB de disco para o piloto. Em uma VM ARM64, confirme previamente que todas as imagens e dependências do projeto possuem variante ARM64.

Instale Docker pelo procedimento oficial da distribuição. Depois, confirme:

```bash
docker --version
docker compose version
git --version
```

Configure o firewall antes de publicar o proxy:

```bash
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow from SEU_IP_ADMINISTRATIVO to any port 22 proto tcp
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw enable
```

Se o IP administrativo mudar, atualize a regra SSH antes de remover a regra antiga.

## DNS e domínio

Crie no provedor DNS os registros abaixo:

```text
A      @       IP_PUBLICO_DA_VM
A      api     IP_PUBLICO_DA_VM
```

Se o frontend for publicado no Cloudflare Pages, a raiz do domínio deve apontar para o projeto Pages e somente `api.seudominio.com` deve apontar para a VM. Para domínio raiz no Pages, o domínio precisa ser uma zona administrada pelo Cloudflare; para subdomínio, é possível usar CNAME conforme a configuração do provedor [4].

O Caddy solicitará certificados automaticamente quando o DNS estiver resolvendo para a VM e as portas 80 e 443 estiverem acessíveis. Não coloque certificados privados no Git.

## Preparar o checkout

Na VM:

```bash
git clone https://github.com/AdrianFRamos/Lastro.git
cd Lastro
cp infra/production.env.example .env.production
chmod 600 .env.production
```

Edite `.env.production` e substitua todos os valores vazios. Gere os segredos localmente na VM:

```bash
openssl rand -hex 32
openssl rand -base64 48
```

`LASTRO_AGENT_TOKEN` e `LASTRO_OPERATOR_TOKEN` devem ser segredos diferentes, cada um com pelo menos 32 caracteres. O `POSTGRES_PASSWORD` deve ser URL-safe; usar uma sequência hexadecimal evita caracteres que precisam de escaping na URL do PostgreSQL.

O `LASTRO_PROGRAM_ID`, `LASTRO_DEPLOYMENT_ID_HEX`, `LASTRO_STATION_PUBKEY_HEX`, `VITE_LASTRO_PROGRAM_ID`, `VITE_LASTRO_DEPLOYMENT_ID_HEX` e `VITE_LASTRO_AUTHORITY` precisam corresponder ao mesmo deployment. Não misture valores de localnet, devnet e mainnet.

## Subir o sistema

Valide a configuração interpolada sem exibir o arquivo de segredos:

```bash
docker compose --env-file .env.production \
  -f infra/compose.production.yml config >/tmp/lastro-compose-resolved.yml
```

Construa e inicie:

```bash
docker compose --env-file .env.production \
  -f infra/compose.production.yml build

docker compose --env-file .env.production \
  -f infra/compose.production.yml up -d
```

Acompanhe a inicialização e as migrations:

```bash
docker compose --env-file .env.production \
  -f infra/compose.production.yml ps

docker compose --env-file .env.production \
  -f infra/compose.production.yml logs -f api
```

Teste localmente na VM:

```bash
curl -fsS https://SEU_DOMINIO/api/health
curl -fsS https://api.SEU_DOMINIO/api/health
```

O endpoint de saúde deve ser validado por HTTPS externo, não somente pelo endereço interno do container.

A interface operacional fica em `https://SEU_DOMINIO/operations`. Ela não recebe o token por `VITE_*`: o operador informa o token na sessão do navegador e ele permanece apenas em memória. Antes de expor essa tela a terceiros, substitua o token bootstrap por autenticação de wallet/sessão com capabilities por party e facility.

## Backup e restauração

Crie o primeiro backup depois que o banco estiver saudável:

```bash
chmod +x scripts/backup_production.sh
LASTRO_PRODUCTION_ENV="$PWD/.env.production" \
  scripts/backup_production.sh
```

Agende o script com `systemd timer` ou cron. O backup deve ser copiado para armazenamento fora da VM. O arquivo local é uma segunda camada, não o único backup.

Teste restauração em uma instância PostgreSQL separada antes de considerar o backup válido:

```bash
zcat backups/lastro-postgres-YYYYMMDDTHHMMSSZ.sql.gz \
  | docker exec -i lastro-postgres-restore psql -U lastro -d lastro
```

Adapte o nome do container ao ambiente de restauração e nunca restaure diretamente sobre o banco de produção sem uma janela operacional e uma cópia anterior.

## Atualização e rollback

Faça uma atualização somente depois de confirmar que existe um backup recente:

```bash
LASTRO_PRODUCTION_ENV="$PWD/.env.production" \
  scripts/backup_production.sh

git fetch origin
git checkout MAIN_COMMIT_VALIDADO

docker compose --env-file .env.production \
  -f infra/compose.production.yml build --pull
docker compose --env-file .env.production \
  -f infra/compose.production.yml up -d
```

A API aplica migrations no startup. Migrations destrutivas não devem ser incluídas em uma atualização sem um plano de rollback de banco. Para voltar a uma versão anterior, use um commit conhecido e restaure o backup correspondente se o schema já tiver avançado de forma incompatível.

## Checklist de staging obrigatório

Antes do uso com dados reais, execute o fluxo completo em uma rede de testes. O cenário precisa incluir PostgreSQL real, API, Agent, simulador ou Station, wallet, RPC Solana e uma confirmação final. Verifique também a recuperação do worker depois de uma interrupção, a expiração de leases, a quarentena de conflitos e a restauração de backup.

O reconciliador automatiza o fluxo `StationEvent` v1 e o primeiro fluxo v2 `ObservationRecorded` ancorado por `record_observation`. O adapter v2 exige a transação exata em estado finalizado, o `EventAnchor` e o `AssetState` correspondentes antes de atualizar PostgreSQL. O corte operacional de abate, cortes, subprodutos, perdas, expedição e recall já existe como projeção durável e exige transformação finalizada/linhagem ou snapshot bounded; ele ainda não equivale a um adapter on-chain específico para cada instrução industrial. Não declare esses registros como prova Solana até executar a transação canônica correspondente em staging.

Depois de obter um deployment v2 inicializado e assets reais finalizados, execute o runner sem segredos versionados:

```bash
LASTRO_E2E_BASE_URL=https://api.SEU_DOMINIO \
LASTRO_E2E_OPERATOR_TOKEN="..." \
LASTRO_E2E_DEPLOYMENT_ID_HEX="..." \
LASTRO_E2E_ASSET_ID="..." \
LASTRO_E2E_TRANSFORMATION_ID="..." \
LASTRO_E2E_CARCASS_ASSET_ID="..." \
  scripts/e2e_v2_staging.sh
```

Sem `LASTRO_E2E_ASSET_ID`, o runner valida apenas health e registro de parties/facilities e encerra sem fabricar um estado canônico. Com `LASTRO_E2E_TRANSFORMATION_ID`, também é obrigatório fornecer um asset de saída distinto.

## Variáveis que não podem ser publicadas

Nunca versionar:

- `.env.production`;
- `LASTRO_AGENT_TOKEN`;
- `LASTRO_OPERATOR_TOKEN`;
- senha PostgreSQL;
- chave privada de wallet;
- chave privada da Station;
- arquivos de backup;
- logs com tokens ou dados pessoais.

O frontend recebe apenas variáveis `VITE_*`, que são públicas por definição. Nenhum segredo deve ser colocado em `VITE_*`.

## Rollback operacional rápido

Se a API apresentar falha depois de uma atualização:

```bash
git checkout COMMIT_ANTERIOR_VALIDADO
docker compose --env-file .env.production \
  -f infra/compose.production.yml up -d --build
```

Se o erro for de migration ou de dados, pare antes de executar qualquer reparo manual. Preserve logs, o hash do commit, o estado das migrations e o backup mais recente. Depois restaure em um ambiente separado e valide o procedimento antes de atuar no banco de produção.

## Referências

[1]: https://docs.oracle.com/iaas/Content/FreeTier/freetier_topic-Always_Free_Resources.htm "Oracle Cloud Always Free resources"
[2]: https://docs.docker.com/compose/ "Docker Compose documentation"
[3]: https://caddyserver.com/docs/automatic-https "Caddy automatic HTTPS"
[4]: https://developers.cloudflare.com/pages/configuration/custom-domains/ "Cloudflare Pages custom domains"

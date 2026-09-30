# Deploy contínuo no AWS EC2 com GitHub Actions

Cada push na `main` que passa no `ci` é publicado automaticamente no servidor.

```text
push main → ci (testes) ✔ → deploy.yml
  build: imagens lastro-api, lastro-web, lastro-deploy → GHCR (tag = SHA do commit)
  deploy: GitHub OIDC → role AWS temporária → SSM Run Command no EC2
          → deploy_production.sh <SHA>: backup → pull → up → health → (rollback se falhar)
```

- **Sem porta SSH aberta** e **sem chave AWS no GitHub**: o GitHub troca um token OIDC por uma
  sessão de 1 h numa role que só pode mandar comando SSM para esta instância.
- O EC2 **não compila nada**: só baixa imagens prontas. Uma instância pequena basta.
- Cada release é identificada pelo SHA do commit; voltar para qualquer versão é um clique.

O que roda no EC2: API (com o reconciliador), PostgreSQL, frontend e Caddy (HTTPS automático).
O **Agent não roda no servidor**: ele fica no computador ligado à Station na fazenda e aponta
para `https://api.SEU_DOMINIO`.

## 1. Criar a instância

| Item | Valor recomendado |
|---|---|
| AMI | Ubuntu Server 24.04 LTS (x86_64) |
| Tipo | `t3.small` (2 vCPU, 2 GB) para o piloto; `t3.medium` se houver muitos usuários |
| Disco | 30 GB gp3 |
| IP | Elastic IP (o DNS aponta para ele) |
| Security group | entrada **80** e **443** de `0.0.0.0/0`; **nenhuma regra para 22** |
| IAM instance profile | role com a política gerenciada `AmazonSSMManagedInstanceCore` |

O agente SSM já vem instalado na AMI Ubuntu. Confira em *Systems Manager → Fleet Manager* que a
instância aparece como *Online*. Para entrar no servidor use *Session Manager* (botão *Connect* no
console EC2), não SSH.

## 2. Preparar o servidor (uma vez)

Pelo Session Manager:

```bash
sudo -i
curl -fsSL https://raw.githubusercontent.com/AdrianFRamos/Lastro/main/scripts/ec2_bootstrap.sh -o /tmp/ec2_bootstrap.sh
bash /tmp/ec2_bootstrap.sh
```

Se o repositório for privado, copie o conteúdo de `scripts/ec2_bootstrap.sh` para o servidor em
vez de usar `curl`. O script instala Docker, ativa atualizações de segurança automáticas, cria
2 GB de swap, a pasta `/opt/lastro` e um backup diário do PostgreSQL às 03:30 UTC.

Crie a configuração de produção (os segredos ficam **só** no servidor):

```bash
cd /opt/lastro
nano .env.production        # use infra/production.env.example como modelo
chmod 600 .env.production
```

Preencha em especial:

- `LASTRO_REGISTRY=ghcr.io/<seu-usuario-github-em-minusculas>`
- `SITE_DOMAIN`, `API_DOMAIN`, `LASTRO_CORS_ALLOWED_ORIGINS=https://SITE_DOMAIN`
- `POSTGRES_PASSWORD`, `LASTRO_AGENT_TOKEN`, `LASTRO_OPERATOR_TOKEN`: gere cada um com
  `openssl rand -hex 32` (valores diferentes)
- `LASTRO_SOLANA_RPC_URL`: para Devnet, prefira um RPC dedicado (Helius, QuickNode, Triton);
  o público `api.devnet.solana.com` tem limite de requisições
- `LASTRO_PROGRAM_ID`, `LASTRO_DEPLOYMENT_ID_HEX`, `LASTRO_STATION_PUBKEY_HEX` do seu deployment

Se o repositório (e portanto os pacotes no GHCR) for privado, autentique o Docker do servidor
com um token de acesso pessoal que tenha **apenas** `read:packages`:

```bash
docker login ghcr.io -u SEU_USUARIO_GITHUB
```

## 3. DNS

```text
A   SITE_DOMAIN   ELASTIC_IP
A   API_DOMAIN    ELASTIC_IP
```

O Caddy emite os certificados HTTPS sozinho quando o DNS resolver para o Elastic IP.

## 4. Permitir que o GitHub faça o deploy (IAM)

**4.1 Provedor OIDC** (uma vez por conta): *IAM → Identity providers → Add provider* →
OpenID Connect, URL `https://token.actions.githubusercontent.com`, audience `sts.amazonaws.com`.

**4.2 Role de deploy** (`lastro-github-deploy`), com esta *trust policy* (troque `ACCOUNT_ID`):

```json
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Principal": { "Federated": "arn:aws:iam::ACCOUNT_ID:oidc-provider/token.actions.githubusercontent.com" },
    "Action": "sts:AssumeRoleWithWebIdentity",
    "Condition": {
      "StringEquals": {
        "token.actions.githubusercontent.com:aud": "sts.amazonaws.com",
        "token.actions.githubusercontent.com:sub": "repo:AdrianFRamos/Lastro:environment:production"
      }
    }
  }]
}
```

O `sub` restringe a role ao environment `production` deste repositório: forks e outros branches
não conseguem assumi-la. Permissões da role (troque `REGION`, `ACCOUNT_ID`, `INSTANCE_ID`):

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": "ssm:SendCommand",
      "Resource": [
        "arn:aws:ssm:REGION::document/AWS-RunShellScript",
        "arn:aws:ec2:REGION:ACCOUNT_ID:instance/INSTANCE_ID"
      ]
    },
    { "Effect": "Allow", "Action": "ssm:GetCommandInvocation", "Resource": "*" }
  ]
}
```

## 5. Configurar o GitHub

Em *Settings → Environments*, crie `production`. Recomendado: *Deployment branches* só `main`
e, se quiser aprovar cada deploy, *Required reviewers*.

Em *Settings → Secrets and variables → Actions → Variables* (são valores públicos; **nenhum
segredo vai para o GitHub**):

| Variável | Exemplo |
|---|---|
| `AWS_REGION` | `sa-east-1` |
| `AWS_DEPLOY_ROLE_ARN` | `arn:aws:iam::123456789012:role/lastro-github-deploy` |
| `EC2_INSTANCE_ID` | `i-0abc...` |
| `VITE_API_BASE_URL` | `https://api.SEU_DOMINIO` |
| `VITE_SOLANA_RPC_URL` | URL pública de RPC para o navegador |
| `VITE_SOLANA_CHAIN` | `solana:devnet` |
| `VITE_LASTRO_PROGRAM_ID` | igual a `LASTRO_PROGRAM_ID` |
| `VITE_LASTRO_DEPLOYMENT_ID_HEX` | igual a `LASTRO_DEPLOYMENT_ID_HEX` |
| `VITE_LASTRO_AUTHORITY` | endereço da autoridade do deployment (âncora de confiança do verificador) |

## 6. Primeiro deploy

*Actions → deploy → Run workflow* (tag vazia). O job `build` publica as três imagens e o job
`deploy` roda no servidor. Depois:

```bash
curl -fsS https://API_DOMAIN/api/health
```

Deve retornar `{"status":"ok",...}`. `degraded` com `rpc: error` significa que o
`ProtocolConfigV2` ou a Station não estão legíveis no RPC configurado (programa não inicializado,
Station não registrada ou fora da validade: veja `docs/DEPLOY_ONLINE.md`).

A partir daí, **todo push na `main` com o `ci` verde é publicado sozinho**.

## Operação

- **Voltar uma versão:** *Actions → deploy → Run workflow* com a tag (SHA) desejada.
- **Deploy que falha na verificação de saúde** volta sozinho para a versão anterior.
- **Migrations são só para frente.** Se uma release com migration falhar e for revertida,
  restaure o backup feito antes dela:

  ```bash
  cd /opt/lastro
  ls -t backups/ | head
  gunzip -c backups/lastro-postgres-AAAAMMDDTHHMMSSZ.sql.gz | \
    docker compose --env-file .env.production -f compose.deploy.yml -p lastro \
    exec -T postgres sh -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB"'
  ```

- **Logs:** `docker compose --env-file .env.production -f compose.deploy.yml -p lastro logs -f api`
- **Backups:** diários em `/opt/lastro/backups` (14 dias). Copie-os para fora da instância
  (S3 com versionamento) para sobreviverem à perda do EC2.
- **Renovar a Station** antes de `stationValidUntil`:
  `initialize_protocol_config.py --extend-station-days 365` com a chave da autoridade, fora do
  servidor.

## O que continua manual (de propósito)

- **Deploy e upgrade do programa Solana.** A chave de upgrade do programa nunca deve estar no
  GitHub nem no servidor.
- **Inicialização do deployment** e registro/renovação de Stations (chave da autoridade).
- Rotação de segredos do `.env.production`.

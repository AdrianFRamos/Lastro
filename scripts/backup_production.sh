#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${LASTRO_PRODUCTION_ENV:-$ROOT/.env.production}"
COMPOSE_FILE="${LASTRO_COMPOSE_FILE:-$ROOT/infra/compose.production.yml}"
PROJECT_ARGS=()
[[ -n "${LASTRO_COMPOSE_PROJECT:-}" ]] && PROJECT_ARGS=(-p "$LASTRO_COMPOSE_PROJECT")
BACKUP_DIR="${LASTRO_BACKUP_DIR:-$ROOT/backups}"
RETENTION_DAYS="${LASTRO_BACKUP_RETENTION_DAYS:-14}"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "production env file not found: $ENV_FILE" >&2
  exit 1
fi
mkdir -p "$BACKUP_DIR"
umask 077
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
out="$BACKUP_DIR/lastro-postgres-$stamp.sql.gz"

docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" "${PROJECT_ARGS[@]}" exec -T postgres \
  sh -c 'pg_dump --no-owner --no-privileges --clean --if-exists \
  -U "$POSTGRES_USER" -d "$POSTGRES_DB"' \
  | gzip -9 > "$out"

find "$BACKUP_DIR" -type f -name 'lastro-postgres-*.sql.gz' -mtime "+$RETENTION_DAYS" -delete
printf 'backup=%s\n' "$out"

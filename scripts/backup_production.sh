#!/usr/bin/env bash
# PostgreSQL backup of a Lastro deployment.
#
# - Written atomically: a failed pg_dump never leaves a truncated file behind.
# - Encrypted with age when LASTRO_BACKUP_AGE_RECIPIENT is set (recommended; the database holds
#   sealed organization keys). Without it the dump stays plaintext and a warning is printed.
# - Copied off the host when LASTRO_BACKUP_S3_URI is set (aws s3 cp, SSE-KMS through the
#   instance role). Backups never contain the key files in /opt/lastro/secrets.
# Prints `backup=<path>` on success.
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
# Read one non-secret setting from the env file without sourcing it (it also holds secrets).
setting() { sed -n "s/^$1=//p" "$ENV_FILE" | tail -n 1; }
RECIPIENT="${LASTRO_BACKUP_AGE_RECIPIENT:-$(setting LASTRO_BACKUP_AGE_RECIPIENT)}"
S3_URI="${LASTRO_BACKUP_S3_URI:-$(setting LASTRO_BACKUP_S3_URI)}"
NAME="${LASTRO_BACKUP_NAME:-lastro-postgres}"
# LASTRO_BACKUP_PLAINTEXT=1: local, unencrypted, never copied off the host. Used only for the
# short-lived pre-release dump the automated rollback restores from (the age identity is
# offline, so an encrypted dump could not be restored unattended).
if [[ "${LASTRO_BACKUP_PLAINTEXT:-0}" == 1 ]]; then
  RECIPIENT=""
  S3_URI=""
fi
umask 077
mkdir -p "$BACKUP_DIR"
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
out="$BACKUP_DIR/$NAME-$stamp.sql.gz"
if [[ -n "$RECIPIENT" ]]; then
  command -v age >/dev/null || { echo "age is required for encrypted backups" >&2; exit 1; }
  out="$out.age"
elif [[ "${LASTRO_BACKUP_PLAINTEXT:-0}" != 1 ]]; then
  echo "WARNING: LASTRO_BACKUP_AGE_RECIPIENT is not set; this backup is not encrypted" >&2
fi
tmp="$out.partial"
trap 'rm -f "$tmp"' EXIT

dump() {
  docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" "${PROJECT_ARGS[@]}" exec -T postgres \
    sh -c 'pg_dump --no-owner --no-privileges --clean --if-exists \
    -U "$POSTGRES_USER" -d "$POSTGRES_DB"' \
    | gzip -9
}
if [[ -n "$RECIPIENT" ]]; then
  dump | age -r "$RECIPIENT" >"$tmp"
else
  dump >"$tmp"
fi
[[ -s "$tmp" ]] || { echo "backup is empty" >&2; exit 1; }
mv "$tmp" "$out"
trap - EXIT

if [[ -n "$S3_URI" ]]; then
  command -v aws >/dev/null || { echo "aws CLI is required for LASTRO_BACKUP_S3_URI" >&2; exit 1; }
  aws s3 cp --only-show-errors --sse aws:kms "$out" "${S3_URI%/}/$(basename "$out")"
fi

find "$BACKUP_DIR" -type f \( -name 'lastro-postgres-*.sql.gz' -o -name 'lastro-postgres-*.sql.gz.age' \
  -o -name 'pre-release-*.sql.gz' \) \
  -mtime "+$RETENTION_DAYS" -delete
printf 'backup=%s\n' "$out"

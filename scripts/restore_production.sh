#!/usr/bin/env bash
# Restores a backup written by backup_production.sh into the deployment's PostgreSQL.
#
#   restore_production.sh <backup-file>
#
# Stops the API (so nothing writes during the restore), recreates the public schema (objects a
# newer migration added are not in the dump, so replaying it alone would not undo them), replays
# the dump in a single transaction and starts the API again. An `.age` backup needs the offline
# identity in LASTRO_BACKUP_AGE_IDENTITY. deploy_production.sh calls it with
# LASTRO_RESTORE_NO_START=1 and then starts the previous release itself: restarting the existing
# container would run the failed release, which migrates again.
set -euo pipefail

BACKUP="${1:?usage: restore_production.sh <backup-file>}"
HOME_DIR="${LASTRO_HOME:-/opt/lastro}"
ENV_FILE="${LASTRO_PRODUCTION_ENV:-$HOME_DIR/.env.production}"
COMPOSE=(docker compose --env-file "$ENV_FILE" -f "${LASTRO_COMPOSE_FILE:-$HOME_DIR/compose.deploy.yml}" -p "${LASTRO_COMPOSE_PROJECT:-lastro}")

[[ -s "$BACKUP" ]] || { echo "backup not found or empty: $BACKUP" >&2; exit 1; }
decode() {
  case "$BACKUP" in
    *.age)
      [[ -n "${LASTRO_BACKUP_AGE_IDENTITY:-}" ]] || { echo "LASTRO_BACKUP_AGE_IDENTITY is required for $BACKUP" >&2; exit 1; }
      age -d -i "$LASTRO_BACKUP_AGE_IDENTITY" "$BACKUP" | gunzip
      ;;
    *.sql.gz) gunzip -c "$BACKUP" ;;
    *) echo "unknown backup format: $BACKUP" >&2; exit 1 ;;
  esac
}

"${COMPOSE[@]}" rm -sf api
{
  printf 'DROP SCHEMA public CASCADE;\nCREATE SCHEMA public;\n'
  decode
} | "${COMPOSE[@]}" exec -T postgres \
  sh -c 'psql --set ON_ERROR_STOP=1 --single-transaction --quiet -U "$POSTGRES_USER" -d "$POSTGRES_DB"' >/dev/null
if [[ "${LASTRO_RESTORE_NO_START:-0}" != 1 ]]; then
  "${COMPOSE[@]}" up -d api
fi
echo "restored $BACKUP"

#!/usr/bin/env bash
# Server-side release step, run by .github/workflows/deploy.yml through AWS SSM.
#
#   deploy_production.sh <image-tag>
#
# Backs up PostgreSQL, pulls the release images, restarts the stack, waits for a healthy API and
# rolls back to the previous tag on failure. Migrations are forward-only: when the failed release
# had already migrated the schema, the pre-release backup is restored before the previous images
# start again. Secrets stay in $LASTRO_HOME/.env.production, which this script never prints.
set -euo pipefail

TAG="${1:?usage: deploy_production.sh <image-tag>}"
HOME_DIR="${LASTRO_HOME:-/opt/lastro}"
ENV_FILE="$HOME_DIR/.env.production"
COMPOSE=(docker compose --env-file "$ENV_FILE" -f "$HOME_DIR/compose.deploy.yml" -p lastro)
STATE="$HOME_DIR/.deployed-tag"

[[ "$TAG" =~ ^[A-Za-z0-9_.-]{1,128}$ ]] || { echo "invalid image tag" >&2; exit 2; }

# One release at a time, even if GitHub cancels a job whose SSM command is still running.
exec 9>"$HOME_DIR/.deploy.lock"
flock -w 1800 9 || { echo "another deployment is still running" >&2; exit 1; }
[[ -f "$ENV_FILE" ]] || { echo "missing $ENV_FILE" >&2; exit 1; }
grep -q '^LASTRO_REGISTRY=' "$ENV_FILE" || { echo "LASTRO_REGISTRY is not set in $ENV_FILE" >&2; exit 1; }

set_tag() {
  if grep -q '^LASTRO_IMAGE_TAG=' "$ENV_FILE"; then
    sed -i "s/^LASTRO_IMAGE_TAG=.*/LASTRO_IMAGE_TAG=$1/" "$ENV_FILE"
  else
    printf 'LASTRO_IMAGE_TAG=%s\n' "$1" >>"$ENV_FILE"
  fi
}

healthy() {
  for _ in $(seq 1 60); do
    if "${COMPOSE[@]}" exec -T proxy wget -qO- http://api:8080/api/health 2>/dev/null | grep -q '"status":"ok"'; then
      return 0
    fi
    sleep 5
  done
  return 1
}

# Latest applied migration (0 before the first deploy).
migration_version() {
  "${COMPOSE[@]}" exec -T postgres sh -c \
    'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -tAc "SELECT COALESCE(max(version),0) FROM _sqlx_migrations" 2>/dev/null || echo 0' \
    | tr -d '[:space:]'
}

previous="$(cat "$STATE" 2>/dev/null || true)"
backup=""
schema_before="0"

run_backup() {
  LASTRO_PRODUCTION_ENV="$ENV_FILE" LASTRO_BACKUP_DIR="$HOME_DIR/backups" \
    LASTRO_COMPOSE_FILE="$HOME_DIR/compose.deploy.yml" LASTRO_COMPOSE_PROJECT=lastro \
    bash "$HOME_DIR/backup_production.sh" "$@"
}

# 1. Back up before any schema migration (the API migrates on start). First deploy has no DB yet.
#    The archived backup is encrypted when configured; a second, local plaintext dump (0600) is
#    what an unattended rollback can restore, and it is deleted once the release is healthy.
if [[ -n "$previous" ]]; then
  run_backup >/dev/null || { echo "backup failed; deployment aborted" >&2; exit 1; }
  backup="$(LASTRO_BACKUP_PLAINTEXT=1 LASTRO_BACKUP_NAME=pre-release run_backup | sed -n 's/^backup=//p')" \
    || { echo "pre-release dump failed; deployment aborted" >&2; exit 1; }
  [[ -n "$backup" ]] || { echo "pre-release dump produced no file; deployment aborted" >&2; exit 1; }
  schema_before="$(migration_version)"
fi

# 2. Pull first, so a failed pull leaves the running release and its tag untouched; only then
#    record the new tag and start it.
LASTRO_IMAGE_TAG="$TAG" "${COMPOSE[@]}" pull --quiet \
  || { echo "pull of $TAG failed; the running release is unchanged" >&2; exit 1; }
set_tag "$TAG"
"${COMPOSE[@]}" up -d --remove-orphans || echo "start of $TAG failed" >&2

# 3. Healthy = database reachable and the v2 ProtocolConfig/Station readable on-chain.
if healthy; then
  printf '%s\n' "$TAG" >"$STATE"
  [[ -n "$backup" ]] && rm -f "$backup"
  # Keep the current and the previous release images; drop older lastro tags and dangling layers.
  docker images --format '{{.Repository}}:{{.Tag}}' | grep '/lastro-' \
    | grep -v -e ":$TAG\$" ${previous:+-e ":$previous\$"} | xargs -r docker rmi >/dev/null 2>&1 || true
  docker image prune -f >/dev/null
  echo "deployed $TAG"
  exit 0
fi

echo "release $TAG is not healthy; recent API logs:" >&2
"${COMPOSE[@]}" logs --tail 80 api >&2 || true
if [[ -n "$previous" ]]; then
  echo "rolling back to $previous" >&2
  set_tag "$previous"
  schema_after="$(migration_version)"
  if [[ "$schema_after" != "$schema_before" ]]; then
    # The failed release migrated the schema; the previous binary must not run on it.
    echo "schema moved from $schema_before to $schema_after; restoring $backup" >&2
    LASTRO_RESTORE_NO_START=1 LASTRO_HOME="$HOME_DIR" LASTRO_PRODUCTION_ENV="$ENV_FILE" \
      bash "$HOME_DIR/restore_production.sh" "$backup" \
      || { echo "RESTORE FAILED: API left stopped; restore $backup by hand" >&2; exit 1; }
  fi
  "${COMPOSE[@]}" up -d --remove-orphans
  if healthy; then
    echo "rolled back to $previous" >&2
  else
    echo "ROLLBACK UNHEALTHY: $previous does not answer /api/health" >&2
  fi
fi
exit 1

#!/usr/bin/env bash
# Server-side release step, run by .github/workflows/deploy.yml through AWS SSM.
#
#   deploy_production.sh <image-tag>
#
# Backs up PostgreSQL, pulls the release images, restarts the stack, waits for a healthy API and
# rolls back to the previous tag on failure. Secrets stay in $LASTRO_HOME/.env.production, which
# this script never prints.
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

previous="$(cat "$STATE" 2>/dev/null || true)"

# 1. Back up before any schema migration (the API migrates on start). First deploy has no DB yet.
if [[ -n "$previous" ]]; then
  LASTRO_PRODUCTION_ENV="$ENV_FILE" LASTRO_BACKUP_DIR="$HOME_DIR/backups" \
    LASTRO_COMPOSE_FILE="$HOME_DIR/compose.deploy.yml" LASTRO_COMPOSE_PROJECT=lastro \
    bash "$HOME_DIR/backup_production.sh" \
    || { echo "backup failed; deployment aborted" >&2; exit 1; }
fi

# 2. Pull and start the release.
set_tag "$TAG"
"${COMPOSE[@]}" pull --quiet
"${COMPOSE[@]}" up -d --remove-orphans

# 3. Healthy = database reachable and the v2 ProtocolConfig/Station readable on-chain.
if healthy; then
  printf '%s\n' "$TAG" >"$STATE"
  docker image prune -f >/dev/null
  echo "deployed $TAG"
  exit 0
fi

echo "release $TAG is not healthy; recent API logs:" >&2
"${COMPOSE[@]}" logs --tail 80 api >&2 || true
if [[ -n "$previous" ]]; then
  echo "rolling back to $previous" >&2
  set_tag "$previous"
  "${COMPOSE[@]}" up -d --remove-orphans
  healthy && echo "rolled back to $previous" >&2
  # Migrations are forward-only: if the failed release migrated the schema, restore the backup
  # taken in step 1.
fi
exit 1

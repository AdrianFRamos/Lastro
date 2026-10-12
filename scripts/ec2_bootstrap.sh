#!/usr/bin/env bash
# One-time preparation of an Ubuntu 24.04 EC2 instance for .github/workflows/deploy.yml.
# Run as root once (e.g. `sudo bash ec2_bootstrap.sh` through SSM Session Manager).
# Idempotent: re-running only fills in what is missing.
set -euo pipefail

[[ "$(id -u)" == 0 ]] || { echo "run as root" >&2; exit 1; }
. /etc/os-release
[[ "$ID" == ubuntu ]] || { echo "expected Ubuntu, found $ID" >&2; exit 1; }

# Docker Engine + Compose plugin from Docker's official apt repository.
if ! command -v docker >/dev/null; then
  apt-get update
  apt-get install -y ca-certificates curl
  install -m 0755 -d /etc/apt/keyrings
  curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc
  # Docker's published release-signing key fingerprint; refuse anything else.
  gpg --show-keys --with-colons /etc/apt/keyrings/docker.asc \
    | grep -q '^fpr:::::::::9DC858229FC7DD38854AE2D88D81803C0EBFCD88:$' \
    || { echo "unexpected Docker apt key fingerprint" >&2; exit 1; }
  chmod a+r /etc/apt/keyrings/docker.asc
  echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/ubuntu ${VERSION_CODENAME} stable" \
    >/etc/apt/sources.list.d/docker.list
  apt-get update
  apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin
fi
systemctl enable --now docker

# Security updates without manual intervention; containers restart on their own policy.
# age encrypts backups (LASTRO_BACKUP_AGE_RECIPIENT). For LASTRO_BACKUP_S3_URI install the AWS
# CLI v2 separately (Ubuntu 24.04 no longer ships it as an apt package).
apt-get install -y unattended-upgrades age
dpkg-reconfigure -f noninteractive unattended-upgrades

# 2 GB swap so small instances survive PostgreSQL + API + image pulls.
if ! swapon --show | grep -q /swapfile; then
  fallocate -l 2G /swapfile
  chmod 600 /swapfile
  mkswap /swapfile
  swapon /swapfile
  grep -q '^/swapfile' /etc/fstab || echo '/swapfile none swap sw 0 0' >>/etc/fstab
fi

install -d -m 0750 /opt/lastro /opt/lastro/backups
# Key files (fee sponsor keypair, organization master key) are mounted read-only into the API,
# which runs as uid 10001: chown 10001:10001 and chmod 0400 every file placed here.
install -d -m 0700 -o 10001 -g 10001 /opt/lastro/secrets

# Daily PostgreSQL backup at 03:30 UTC (the deploy also backs up before every release).
cat >/etc/cron.d/lastro-backup <<'CRON'
30 3 * * * root [ -f /opt/lastro/.deployed-tag ] && flock -w 1800 /opt/lastro/.deploy.lock env LASTRO_PRODUCTION_ENV=/opt/lastro/.env.production LASTRO_BACKUP_DIR=/opt/lastro/backups LASTRO_COMPOSE_FILE=/opt/lastro/compose.deploy.yml LASTRO_COMPOSE_PROJECT=lastro bash /opt/lastro/backup_production.sh >>/var/log/lastro-backup.log 2>&1
CRON

echo "bootstrap done. Next: create /opt/lastro/.env.production (chmod 600) and, for a private"
echo "repository, run: docker login ghcr.io -u <github-user>  (token with read:packages only)."

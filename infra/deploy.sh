#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INFRA_DIR="${ROOT_DIR}/infra"
COMPOSE_FILE="${INFRA_DIR}/compose.yml"
ENV_FILE="${ENV_FILE:-${INFRA_DIR}/.env}"
SECRETS_DIR="${SECRETS_DIR:-${INFRA_DIR}/secrets}"
NGINX_DIR="${INFRA_DIR}/nginx"

DEPLOY_HOST="${DEPLOY_HOST:-}"
DEPLOY_USER="${DEPLOY_USER:-$USER}"
REMOTE_DIR="${REMOTE_DIR:-/opt/reauth}"
SSH_OPTS="${SSH_OPTS:-}"

API_IMAGE="reauth-api:latest"
UI_IMAGE="reauth-ui:latest"

usage() {
  cat <<EOF
Usage: DEPLOY_HOST=example.com [DEPLOY_USER=ubuntu] [REMOTE_DIR=/opt/reauth] \
[ENV_FILE=infra/.env] [SECRETS_DIR=infra/secrets] [SSH_OPTS="-p 2222"] ./infra/deploy.sh
EOF
}

require() {
  command -v "$1" >/dev/null 2>&1 || { echo "Missing required command: $1" >&2; exit 1; }
}

[ -z "$DEPLOY_HOST" ] && { echo "DEPLOY_HOST is required"; usage; exit 1; }

require docker
require ssh
require rsync

[ -f "$COMPOSE_FILE" ] || { echo "Compose file not found at $COMPOSE_FILE" >&2; exit 1; }
[ -f "$ENV_FILE" ] || { echo "Env file not found at $ENV_FILE" >&2; exit 1; }
[ -f "${SECRETS_DIR}/jwt_secret" ] || { echo "Secret file ${SECRETS_DIR}/jwt_secret is required" >&2; exit 1; }

echo "Building images..."
DOCKER_BUILDKIT=1 bash "${ROOT_DIR}/build-images.sh"

IMAGES_DIR="$(mktemp -d "${INFRA_DIR}/images.XXXX")"
trap 'rm -rf "$IMAGES_DIR"' EXIT

docker save "$API_IMAGE" > "${IMAGES_DIR}/reauth-api.tar"
docker save "$UI_IMAGE" > "${IMAGES_DIR}/reauth-ui.tar"

echo "Syncing artifacts to ${DEPLOY_USER}@${DEPLOY_HOST}:${REMOTE_DIR}"
ssh $SSH_OPTS "${DEPLOY_USER}@${DEPLOY_HOST}" "mkdir -p ${REMOTE_DIR}/images ${REMOTE_DIR}/nginx ${REMOTE_DIR}/secrets" || true

rsync -az -e "ssh ${SSH_OPTS}" "$COMPOSE_FILE" "${DEPLOY_USER}@${DEPLOY_HOST}:${REMOTE_DIR}/compose.yml"
rsync -az -e "ssh ${SSH_OPTS}" "$ENV_FILE" "${DEPLOY_USER}@${DEPLOY_HOST}:${REMOTE_DIR}/.env"
rsync -az -e "ssh ${SSH_OPTS}" "$NGINX_DIR/" "${DEPLOY_USER}@${DEPLOY_HOST}:${REMOTE_DIR}/nginx/"
rsync -az -e "ssh ${SSH_OPTS}" "${INFRA_DIR}/certbot-check.sh" "${DEPLOY_USER}@${DEPLOY_HOST}:${REMOTE_DIR}/certbot-check.sh"
rsync -az -e "ssh ${SSH_OPTS}" "$SECRETS_DIR/" "${DEPLOY_USER}@${DEPLOY_HOST}:${REMOTE_DIR}/secrets/"
rsync -az -e "ssh ${SSH_OPTS}" "$IMAGES_DIR/" "${DEPLOY_USER}@${DEPLOY_HOST}:${REMOTE_DIR}/images/"

echo "Deploying on remote host..."
ssh $SSH_OPTS "${DEPLOY_USER}@${DEPLOY_HOST}" <<EOF
set -euo pipefail
cd "${REMOTE_DIR}"
chmod +x certbot-check.sh
docker load -i images/reauth-api.tar
docker load -i images/reauth-ui.tar

# Try docker compose (v2) first, fall back to docker-compose (v1)
if docker compose version >/dev/null 2>&1; then
  docker compose -f compose.yml --env-file .env up -d
else
  docker-compose -f compose.yml --env-file .env up -d
fi
EOF

echo "Deployment finished."

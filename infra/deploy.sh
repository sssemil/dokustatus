#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INFRA_DIR="${ROOT_DIR}/infra"
COMPOSE_FILE="${INFRA_DIR}/compose.yml"
ENV_FILE="${ENV_FILE:-${INFRA_DIR}/.env}"
SECRETS_DIR="${SECRETS_DIR:-${INFRA_DIR}/secrets}"
CADDY_DIR="${INFRA_DIR}/caddy"

DEPLOY_HOST="${DEPLOY_HOST:-}"
DEPLOY_USER="${DEPLOY_USER:-$USER}"
REMOTE_DIR="${REMOTE_DIR:-/opt/reauth}"
SSH_OPTS="${SSH_OPTS:-}"

API_IMAGE="reauth-api:latest"
UI_IMAGE="reauth-ui:latest"
DEMO_API_IMAGE="demo-api:latest"
DEMO_UI_IMAGE="demo-ui:latest"

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
DOCKER_BUILDKIT=1 BUILD_ARGS="${BUILD_ARGS:-}" bash "${ROOT_DIR}/build-images.sh"

IMAGES_DIR="$(mktemp -d "${INFRA_DIR}/images.XXXX")"
trap 'rm -rf "$IMAGES_DIR"' EXIT

docker save "$API_IMAGE" > "${IMAGES_DIR}/reauth-api.tar"
docker save "$UI_IMAGE" > "${IMAGES_DIR}/reauth-ui.tar"
docker save "$DEMO_API_IMAGE" > "${IMAGES_DIR}/demo-api.tar"
docker save "$DEMO_UI_IMAGE" > "${IMAGES_DIR}/demo-ui.tar"

echo "Syncing artifacts to ${DEPLOY_USER}@${DEPLOY_HOST}:${REMOTE_DIR}"
ssh $SSH_OPTS "${DEPLOY_USER}@${DEPLOY_HOST}" "sudo mkdir -p ${REMOTE_DIR}/images ${REMOTE_DIR}/caddy/ingress ${REMOTE_DIR}/secrets && sudo chown -R ${DEPLOY_USER}:${DEPLOY_USER} ${REMOTE_DIR}" || true

rsync -az -e "ssh ${SSH_OPTS}" "$COMPOSE_FILE" "${DEPLOY_USER}@${DEPLOY_HOST}:${REMOTE_DIR}/compose.yml"
rsync -az -e "ssh ${SSH_OPTS}" "$ENV_FILE" "${DEPLOY_USER}@${DEPLOY_HOST}:${REMOTE_DIR}/.env"

# Sync caddy files and capture if anything changed
CADDY_CHANGED=$(rsync -az --itemize-changes -e "ssh ${SSH_OPTS}" "$CADDY_DIR/" "${DEPLOY_USER}@${DEPLOY_HOST}:${REMOTE_DIR}/caddy/" | grep -c '^>f' || true)

rsync -az -e "ssh ${SSH_OPTS}" "$SECRETS_DIR/" "${DEPLOY_USER}@${DEPLOY_HOST}:${REMOTE_DIR}/secrets/"
rsync -az -e "ssh ${SSH_OPTS}" "$IMAGES_DIR/" "${DEPLOY_USER}@${DEPLOY_HOST}:${REMOTE_DIR}/images/"

echo "Deploying on remote host..."
ssh $SSH_OPTS "${DEPLOY_USER}@${DEPLOY_HOST}" bash -s -- "$CADDY_CHANGED" <<'EOF'
set -euo pipefail
CADDY_CHANGED="$1"
cd "${REMOTE_DIR:-/opt/reauth}"
docker load -i images/reauth-api.tar
docker load -i images/reauth-ui.tar
docker load -i images/demo-api.tar
docker load -i images/demo-ui.tar

# Helper function for docker compose
dc() {
  if docker compose version >/dev/null 2>&1; then
    docker compose -f compose.yml --env-file .env "$@"
  else
    docker-compose -f compose.yml --env-file .env "$@"
  fi
}

dc up -d --remove-orphans

# Restart caddy if its config files changed
if [ "$CADDY_CHANGED" -gt 0 ]; then
  echo "Caddy config changed, restarting caddy..."
  dc restart caddy
fi

# Clean up old containers and dangling images from our app
echo "Cleaning up old containers and images..."
# Remove stopped containers from this compose project
docker container prune -f --filter "label=com.docker.compose.project=reauth" 2>/dev/null || true
# Remove dangling images (old untagged images after loading new ones with same tag)
docker image prune -f 2>/dev/null || true
EOF

echo "Deployment finished."

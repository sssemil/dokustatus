#!/usr/bin/env bash
set -euo pipefail

# Frontend-only deployment script
# Only builds and deploys reauth-ui, much faster than full deploy

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INFRA_DIR="${ROOT_DIR}/infra"
COMPOSE_FILE="${INFRA_DIR}/compose.yml"
ENV_FILE="${ENV_FILE:-${INFRA_DIR}/.env}"

DEPLOY_HOST="${DEPLOY_HOST:-63.178.106.82}"
DEPLOY_USER="${DEPLOY_USER:-ubuntu}"
REMOTE_DIR="${REMOTE_DIR:-/opt/reauth}"
SSH_OPTS="${SSH_OPTS:-}"
BUILD_ARGS="${BUILD_ARGS:---network=host}"

UI_IMAGE="reauth-ui:latest"

echo "=== Frontend-only deployment ==="

echo "Building UI image..."
docker build ${BUILD_ARGS} -f apps/ui/Dockerfile -t "$UI_IMAGE" "$ROOT_DIR"

IMAGES_DIR="$(mktemp -d "${INFRA_DIR}/images.XXXX")"
trap 'rm -rf "$IMAGES_DIR"' EXIT

echo "Saving UI image..."
docker save "$UI_IMAGE" > "${IMAGES_DIR}/reauth-ui.tar"

echo "Syncing UI image to ${DEPLOY_USER}@${DEPLOY_HOST}:${REMOTE_DIR}"
ssh $SSH_OPTS "${DEPLOY_USER}@${DEPLOY_HOST}" "mkdir -p ${REMOTE_DIR}/images" || true
rsync -az --progress -e "ssh ${SSH_OPTS}" "${IMAGES_DIR}/reauth-ui.tar" "${DEPLOY_USER}@${DEPLOY_HOST}:${REMOTE_DIR}/images/"

echo "Loading and restarting UI on remote host..."
ssh $SSH_OPTS "${DEPLOY_USER}@${DEPLOY_HOST}" bash -s <<'EOF'
set -euo pipefail
cd "${REMOTE_DIR:-/opt/reauth}"
docker load -i images/reauth-ui.tar

# Helper function for docker compose
dc() {
  if docker compose version >/dev/null 2>&1; then
    docker compose -f compose.yml --env-file .env "$@"
  else
    docker-compose -f compose.yml --env-file .env "$@"
  fi
}

# Restart only the UI service
dc up -d --no-deps ui

# Clean up dangling images
docker image prune -f 2>/dev/null || true
EOF

echo "=== UI deployment finished ==="

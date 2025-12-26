#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_ARGS="${BUILD_ARGS:-}"

# Build main reauth images
docker build ${BUILD_ARGS} -f apps/api/Dockerfile -t reauth-api "$ROOT_DIR"
docker build ${BUILD_ARGS} -f apps/ui/Dockerfile -t reauth-ui "$ROOT_DIR"

# Build demo app images
docker build ${BUILD_ARGS} -f apps/demo_api/Dockerfile -t demo-api "$ROOT_DIR"
docker build ${BUILD_ARGS} -f apps/demo_ui/Dockerfile -t demo-ui "$ROOT_DIR"

echo "Built images: reauth-api, reauth-ui, demo-api, demo-ui"

#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

docker build -f apps/api/Dockerfile -t dokustatus-api "$ROOT_DIR"
docker build -f apps/ui/Dockerfile -t dokustatus-ui "$ROOT_DIR"

echo "Built images: dokustatus-api and dokustatus-ui"

#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "==> Tearing down DinD cluster..."
docker compose -f "$SCRIPT_DIR/docker-compose.yml" down -v

echo "==> Done."

#!/usr/bin/env bash
# Run one development node agent in the foreground against its DinD node:
#   ./dev/agent.sh manager | worker-1 | worker-2
# Requires `pnpm dev:cluster` and `pnpm dev:pki`.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NODE="${1:-}"

case "$NODE" in
  manager) DOCKER_PORT=2375 RPC_PORT=50052 ;;
  worker-1) DOCKER_PORT=2376 RPC_PORT=50053 ;;
  worker-2) DOCKER_PORT=2378 RPC_PORT=50054 ;;
  *) echo "usage: $0 manager|worker-1|worker-2" >&2; exit 64 ;;
esac

export KONTAINER_ENV=development
export KONTAINER_DOCKER_HOST="tcp://localhost:$DOCKER_PORT"
export KONTAINER_NODE_ID="$NODE"
export KONTAINER_CLUSTER_ID=development
export KONTAINER_NODE_RPC_BINDING="127.0.0.1:$RPC_PORT"
export KONTAINER_NODE_RPC_TLS_CERTIFICATE_PATH="$ROOT/dev/pki/node-$NODE.pem"
export KONTAINER_NODE_RPC_TLS_PRIVATE_KEY_PATH="$ROOT/dev/pki/node-$NODE-key.pem"
export KONTAINER_NODE_RPC_TLS_CLIENT_CA_PATH="$ROOT/dev/pki/agent-ca.pem"
export KONTAINER_CONTROLLER_RPC_ENDPOINT="${KONTAINER_CONTROLLER_RPC_ENDPOINT:-https://127.0.0.1:50051}"
export KONTAINER_CONTROLLER_SERVER_NAME="${KONTAINER_CONTROLLER_SERVER_NAME:-controller.kontainer.internal}"
export RUST_LOG="${RUST_LOG:-info}"

cargo build --quiet --manifest-path "$ROOT/server/Cargo.toml" -p kontainer-node-agent
exec "$ROOT/server/target/debug/kontainer-node-agent"

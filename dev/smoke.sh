#!/usr/bin/env bash
# Bring the development stack up (cluster, PKI, PostgreSQL, controller, three
# agents), then check that the controller is ready and every agent answers an
# mTLS handshake. Tears nothing down; `pnpm dev:agents:stop` and
# `kill "$(cat dev/logs/controller.pid)"` stop the processes it started.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOG_DIR="$ROOT/dev/logs"
mkdir -p "$LOG_DIR"

if ! docker exec kontainer-cluster-manager-1 docker info >/dev/null 2>&1; then
  "$ROOT/dev/setup.sh"
fi
"$ROOT/dev/pki.sh"
docker compose -f "$ROOT/server/compose.postgres.yaml" up -d --wait postgres

if ! curl -fsS http://127.0.0.1:5150/health/live >/dev/null 2>&1; then
  echo "==> Starting controller (log dev/logs/controller.log)"
  cargo build --quiet --manifest-path "$ROOT/server/Cargo.toml" --bin kontainer-controller
  # exec so the backgrounded subshell becomes the controller itself: the pid
  # file names the real process and nothing keeps this script's stdout open.
  (cd "$ROOT/server" && KONTAINER_DOCKER_HOST=tcp://localhost:2375 \
    exec "$ROOT/server/target/debug/kontainer-controller" start \
    >"$LOG_DIR/controller.log" 2>&1 </dev/null) &
  echo $! >"$LOG_DIR/controller.pid"
fi
"$ROOT/dev/agents.sh" start

echo "==> Waiting for /health/ready"
for _ in $(seq 1 180); do
  if [[ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:5150/health/ready)" == 200 ]]; then
    echo "    controller ready"
    break
  fi
  sleep 1
done
[[ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:5150/health/ready)" == 200 ]] ||
  { echo "controller never became ready; see dev/logs/controller.log" >&2; exit 1; }

failures=0
for entry in manager:50052 worker-1:50053 worker-2:50054; do
  node="${entry%%:*}" port="${entry##*:}"
  for _ in $(seq 1 120); do
    if openssl s_client -connect "127.0.0.1:$port" \
      -servername "$node.agent.kontainer.internal" \
      -CAfile "$ROOT/dev/pki/agent-ca.pem" \
      -cert "$ROOT/dev/pki/controller.pem" -key "$ROOT/dev/pki/controller-key.pem" \
      -verify_return_error -brief </dev/null >/dev/null 2>&1; then
      echo "    agent $node answered an mTLS handshake on :$port"
      continue 2
    fi
    sleep 1
  done
  echo "agent $node did not answer on :$port; see dev/logs/agent-$node.log" >&2
  failures=$((failures + 1))
done
exit "$failures"

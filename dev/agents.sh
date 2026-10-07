#!/usr/bin/env bash
# Start or stop the three development node agents in the background.
# Logs and pid files live in dev/logs/ (gitignored).
#   ./dev/agents.sh start | stop | status
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
LOG_DIR="$SCRIPT_DIR/logs"
NODES=(manager worker-1 worker-2)

running() {
  local pid_file="$LOG_DIR/agent-$1.pid"
  [[ -f "$pid_file" ]] && kill -0 "$(cat "$pid_file")" 2>/dev/null
}

start() {
  mkdir -p "$LOG_DIR"
  cargo build --quiet --manifest-path "$SCRIPT_DIR/../server/Cargo.toml" -p kontainer-node-agent
  for node in "${NODES[@]}"; do
    if running "$node"; then
      echo "    agent $node already running (pid $(cat "$LOG_DIR/agent-$node.pid"))"
      continue
    fi
    nohup "$SCRIPT_DIR/agent.sh" "$node" >"$LOG_DIR/agent-$node.log" 2>&1 </dev/null &
    echo $! >"$LOG_DIR/agent-$node.pid"
    echo "==> agent $node started (pid $!, log dev/logs/agent-$node.log)"
  done
}

stop() {
  for node in "${NODES[@]}"; do
    if running "$node"; then
      kill "$(cat "$LOG_DIR/agent-$node.pid")"
      echo "==> agent $node stopped"
    fi
    rm -f "$LOG_DIR/agent-$node.pid"
  done
}

status() {
  for node in "${NODES[@]}"; do
    if running "$node"; then echo "$node running"; else echo "$node stopped"; fi
  done
}

case "${1:-}" in
  start) start ;;
  stop) stop ;;
  status) status ;;
  *) echo "usage: $0 start|stop|status" >&2; exit 64 ;;
esac

#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "==> Starting DinD cluster..."
docker compose -f "$SCRIPT_DIR/docker-compose.yml" up -d

echo "==> Waiting for manager Docker daemon..."
until docker exec kontainer-cluster-manager-1 docker info >/dev/null 2>&1; do
  sleep 1
done

echo "==> Waiting for worker-1 Docker daemon..."
until docker exec kontainer-cluster-worker-1-1 docker info >/dev/null 2>&1; do
  sleep 1
done

echo "==> Waiting for worker-2 Docker daemon..."
until docker exec kontainer-cluster-worker-2-1 docker info >/dev/null 2>&1; do
  sleep 1
done

echo "==> Initializing Swarm on manager..."
MANAGER_IP=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' kontainer-cluster-manager-1)
docker exec kontainer-cluster-manager-1 docker swarm init --advertise-addr "$MANAGER_IP" 2>/dev/null || echo "  (already initialized)"

echo "==> Getting join token..."
JOIN_TOKEN=$(docker exec kontainer-cluster-manager-1 docker swarm join-token -q worker)

echo "==> Joining worker-1..."
docker exec kontainer-cluster-worker-1-1 docker swarm join --token "$JOIN_TOKEN" "$MANAGER_IP:2377" 2>/dev/null || echo "  (already joined)"

echo "==> Joining worker-2..."
docker exec kontainer-cluster-worker-2-1 docker swarm join --token "$JOIN_TOKEN" "$MANAGER_IP:2377" 2>/dev/null || echo "  (already joined)"

echo "==> Cluster nodes:"
docker exec kontainer-cluster-manager-1 docker node ls

echo ""
echo "==> Deploying seed services..."
"$SCRIPT_DIR/seed-services.sh"

echo ""
echo "==> DinD cluster is ready!"
echo "    Manager: tcp://localhost:2375"
echo "    Worker-1: tcp://localhost:2376"
echo "    Worker-2: tcp://localhost:2378"

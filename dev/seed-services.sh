#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DOCKER="docker exec devoper-cluster-manager-1 docker"

echo "  Deploying webapp stack..."
docker cp "$SCRIPT_DIR/webapp.yml" devoper-cluster-manager-1:/webapp.yml
$DOCKER stack deploy -c /webapp.yml webapp

echo "  Deploying monitoring stack..."
docker cp "$SCRIPT_DIR/monitoring.yml" devoper-cluster-manager-1:/monitoring.yml
$DOCKER stack deploy -c /monitoring.yml monitoring

echo "  Waiting for services to converge..."
sleep 5

echo "  Services:"
$DOCKER service ls

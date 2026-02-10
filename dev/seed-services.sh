#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DOCKER="docker exec kontainer-cluster-manager-1 docker"

echo "  Deploying webapp stack..."
docker cp "$SCRIPT_DIR/webapp.yml" kontainer-cluster-manager-1:/webapp.yml
$DOCKER stack deploy -c /webapp.yml webapp

echo "  Deploying monitoring stack..."
docker cp "$SCRIPT_DIR/monitoring.yml" kontainer-cluster-manager-1:/monitoring.yml
$DOCKER stack deploy -c /monitoring.yml monitoring

echo "  Deploying toolbox stack..."
docker cp "$SCRIPT_DIR/toolbox.yml" kontainer-cluster-manager-1:/toolbox.yml
$DOCKER stack deploy -c /toolbox.yml toolbox

echo "  Waiting for services to converge..."
sleep 5

echo "  Services:"
$DOCKER service ls

# 0004. Dev-only TCP Docker host override for agents and controller

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/02-dev-environment.md

## Context

Production agents talk to the local Unix Docker socket only, and the decision register rejects
remote-daemon connections as a product feature. The development cluster is three docker-in-docker
nodes that expose their daemons on `localhost:2375`, `2376`, and `2378`. Running the agent inside each
DinD container means shipping a Linux binary into an Alpine image on every change; the old Go setup
instead ran three agent processes on the host with `DOCKER_HOST` set.

## Decision

- The node agent and the controller read an optional `KONTAINER_DOCKER_HOST`. When unset they use
  `/var/run/docker.sock`. When set to `tcp://host:port` they connect over plaintext TCP.
- The override is accepted only when the process runs in the `development` or `test` environment;
  the production config rejects it at startup with a clear error.
- The engine crate gains one constructor for TCP next to the socket constructor. Bollard's `http`
  feature is already enabled, so no dependency changes.

## Consequences

- `dev/Procfile` runs one controller and three agents on the host against the DinD nodes.
- Real-engine tests can target any DinD node by environment variable.
- The product surface is unchanged; the override is a development affordance, not a feature.

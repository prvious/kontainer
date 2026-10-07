# 0042. Run the controller on a Swarm manager with its Docker socket

- Status: accepted
- Date: 2026-10-04
- Spec: docs/build/10-networks.md
- Amends: ADR 0020

## Context

ADR 0020 gave the controller its own manager engine for Swarm-scoped work: overlay networks,
services, stacks, service logs, and Swarm node state. ADR 0031 builds that engine from
`settings.docker.host` and checks the Swarm role on each call. Neither ADR states what this means
for deployment, so an operator could put the controller on a worker, or anywhere without Docker,
and only find out from `not_a_manager` errors.

## Decision

Deploy the controller with these four facts in mind:

- **It runs on a Swarm manager.** `settings.docker.host` (`KONTAINER_DOCKER_HOST`, default
  `unix:///var/run/docker.sock`) must reach a manager's Docker. On a worker, or outside a Swarm,
  every Swarm-scoped feature answers 503 `not_a_manager`.
- **It gets the Docker socket.** The default is the host's Unix socket, mounted into the
  controller's container. Plain TCP is refused unless it's loopback, because Docker's TCP API has
  no authentication.
- **That socket is root on the manager.** Anyone who controls the controller process controls
  every container on that host, and through Swarm, the cluster. Run it only on a trusted manager.
  Keep its HTTP edge behind the owner's session and TLS (ADRs 0001 and 0014).
- **That manager is a single point of failure for Swarm features.** If it goes down, services,
  stacks, overlay networks and service logs are unavailable until the controller runs on another
  manager. Node-local features don't depend on it, because they go through each node's agent:
  containers, images, volumes, bridge networks, exec and metrics.

The path to high availability is to route manager calls through an agent on any reachable manager
instead of the controller's own socket. That is deferred until there is a demonstrated need.

## Consequences

- The README's deployment notes say the same thing.
- A future change that routes manager calls through agents removes the socket requirement and
  amends this ADR.

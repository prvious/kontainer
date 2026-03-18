# Kontainer Deployment

This guide covers the current deployment model for the Kontainer agent.

The desktop app connects to a leader agent over HTTP/WebSocket. In multi-node Swarm setups, the leader also talks to follower agents over gRPC for node-local work.

## Deployment Modes

### Single Node

Run one `leader` agent on a machine that has access to the Docker socket.

Use this when:

- you are developing locally
- you are managing a single-node Swarm
- you do not need cross-node follower discovery

### Multi Node

Run:

- one `leader` agent for the public API and scheduled work
- one or more `follower` agents for node-local operations

Use this when you want the leader to coordinate work across multiple Swarm nodes.

## Required Runtime Access

Every agent needs:

- access to the Docker socket for the node it is managing
- network reachability for its HTTP port
- for multi-node setups, network reachability from the leader to follower gRPC ports

## Core Environment Variables

These are the main deployment variables used by the current server config.

| Variable | Default | Purpose |
| --- | --- | --- |
| `AGENT_ROLE` | `leader` | Chooses `leader` or `follower` mode |
| `AGENT_URL` | `http://localhost:8080` | Public base URL used by auth/setup flows |
| `APP_HOST` | `127.0.0.1` | HTTP bind host |
| `APP_PORT` | `3000` | HTTP bind port |
| `GRPC_PORT` | `9000` | Internal gRPC port used for follower communication |
| `DB_DATABASE` | `storage/kontainer.db` | SQLite database path |
| `CLUSTER_SECRET` | empty | Shared secret for leader/follower internal auth |
| `CLUSTER_FOLLOWER_SERVICE` | `kontainer-agent` | Swarm service name used for follower discovery |
| `CLUSTER_DEV_FOLLOWERS` | empty | Comma-separated follower addresses for development-only discovery override |

## Secrets via `_FILE`

At startup, the agent resolves `_FILE` variants for these environment variables:

- `CLUSTER_SECRET_FILE`
- `JWT_SECRET_FILE`
- `DB_DATABASE_FILE`

If set, the file contents are read and copied into the base environment variable.

## Single-Node Example

This example runs only a leader agent.

```bash
docker run -d \
  --name kontainer-agent \
  -p 3000:3000 \
  -p 9000:9000 \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v kontainer-data:/data \
  -e AGENT_ROLE=leader \
  -e AGENT_URL=http://localhost:3000 \
  -e APP_HOST=0.0.0.0 \
  -e APP_PORT=3000 \
  -e GRPC_PORT=9000 \
  -e DB_DATABASE=/data/kontainer.db \
  ghcr.io/prvious/kontainer-agent:latest
```

Notes:

- mount the Docker socket read/write as required by the Docker operations you want the agent to perform
- persist the SQLite database if you want settings and setup state to survive restarts

## Multi-Node Example

In a Swarm cluster, the leader and followers share the same `CLUSTER_SECRET`.

Create a shared overlay network first:

```bash
docker network create --driver overlay --attachable kontainer-network
```

### Leader

```bash
docker service create \
  --name kontainer-leader \
  --constraint 'node.role==manager' \
  --network kontainer-network \
  --publish published=3000,target=3000 \
  --mount type=bind,source=/var/run/docker.sock,target=/var/run/docker.sock \
  --mount type=volume,source=kontainer-data,target=/data \
  -e AGENT_ROLE=leader \
  -e AGENT_URL=https://agent.example.com \
  -e APP_HOST=0.0.0.0 \
  -e APP_PORT=3000 \
  -e GRPC_PORT=9000 \
  -e DB_DATABASE=/data/kontainer.db \
  -e CLUSTER_SECRET=replace-me \
  -e CLUSTER_FOLLOWER_SERVICE=kontainer-follower \
  ghcr.io/prvious/kontainer-agent:latest
```

### Followers

```bash
docker service create \
  --name kontainer-follower \
  --mode global \
  --network kontainer-network \
  --mount type=bind,source=/var/run/docker.sock,target=/var/run/docker.sock \
  -e AGENT_ROLE=follower \
  -e APP_HOST=0.0.0.0 \
  -e APP_PORT=3000 \
  -e GRPC_PORT=9000 \
  -e CLUSTER_SECRET=replace-me \
  ghcr.io/prvious/kontainer-agent:latest
```

How discovery works:

- if `CLUSTER_DEV_FOLLOWERS` is set, the leader uses that explicit address list instead of Swarm discovery
- otherwise, the leader discovers followers from Swarm tasks using `CLUSTER_FOLLOWER_SERVICE`

## First Boot Setup

The current auth setup is bootstrapped through the setup flow rather than by shipping long-lived GitHub OAuth credentials directly in environment variables.

At a high level:

- start the leader agent
- open the setup flow
- complete the GitHub App manifest exchange
- let the agent persist the resulting GitHub credentials and generated JWT secret in its SQLite database

Relevant routes live under:

- `/setup/*`
- `/auth/*`

Persist the database if you want this setup state to survive container replacement.

## Health and Networking

- all agents expose `GET /health` and `GET /ping`
- leaders also expose public auth, setup, API, and WebSocket routes
- followers do not serve the public API; they only serve health endpoints plus internal gRPC

## Local Development

The repo includes a local Docker-in-Docker Swarm environment under `dev/`.

- `pnpm dev` starts the local cluster, leader, followers, and desktop app
- `pnpm dev:cluster` starts only the local Swarm cluster
- the local leader config uses `CLUSTER_DEV_FOLLOWERS` to point at follower addresses directly

## Source of Truth

If this document and the code ever disagree, prefer the current server config and bootstrap code:

- `server/config/cluster.go`
- `server/config/http.go`
- `server/config/grpc.go`
- `server/config/database.go`
- `server/bootstrap/app.go`

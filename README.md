# Kontainer

Kontainer is a desktop app for managing Docker Swarm infrastructure through a dedicated controller and node agents.

The greenfield Rust rewrite is now the active implementation. The old Go/Goravel server has been removed; there are no compatibility or data-migration requirements.

**Website:** [kontainer.dev](https://kontainer.dev)

## Current Focus

- Docker Swarm is the primary target.
- Docker Compose and Kubernetes are planned, not first-class today.

## What It Does

- Browse cluster resources such as services, tasks, nodes, stacks, images, networks, volumes, secrets, and configs.
- Operate services with actions like scale, restart, rollback, and selected configuration updates.
- Stream service and task logs over WebSocket.
- Open terminal sessions into running containers from the desktop app.
- View cluster and node metrics reported by every node agent.
- Connect to multiple clusters from one desktop app.
- Sign in with an owner email and password; GitHub login arrives later as an integration.

## Architecture

Kontainer has two main areas:

- `app/` - Tauri desktop app built with Rust, React, Shadcn UI, Tailwind CSS, TanStack Router, and TanStack Query.
- `server/` - Rust workspace with a Loco/SeaORM controller, a Tokio/Tonic node agent, shared protobuf contracts, and a typed Docker Engine adapter.

The server workspace ships two processes:

- `kontainer-controller` - owns `/api/v1`, PostgreSQL, auth, operations, and cluster coordination.
- `kontainer-node-agent` - runs on each Swarm node, owns local Docker access, and never connects to PostgreSQL.

Frontend requests flow through the desktop boundary like this:

`React -> Tauri invoke -> Rust client -> controller -> node agent -> local Docker Engine`

## Repository Layout

- `app/` - desktop UI and Tauri commands
- `server/` - controller, node agent, protobuf, migrations, Docker adapter, and tests
- `dev/` - local multi-node Docker-in-Docker development environment
- `docs/` - planning, reference, and design artifacts

## Architecture Decisions

- `docs/v2/approved-greenfield-reset.md` - authoritative rewrite decisions
- `docs/adr/` - decisions made after the reset, one file each
- `docs/v2/decision-register.md` - decision history and superseded research

## Build Plan

- `docs/build/00-goal.md` - standing instructions for the implementing agent and the `/goal` condition
- `docs/build/NN-*.md` - one spec per feature, in build order
- `docs/build/PROGRESS.md` - the ledger; `docs/build/design-gaps.md` - screens the design does not draw yet
- `docs/design.pen` - the product design; read it through the Pencil MCP

## Local Development

### Prerequisites

- `pnpm`
- Rust 1.98
- Docker
- PostgreSQL

### Bring the stack up

Run from the repository root, in this order:

```bash
pnpm dev:cluster       # three docker:dind nodes as a Swarm (tcp://localhost:2375, 2376, 2378) + seed stacks
pnpm dev:pki           # dev mTLS CA, controller and node certificates in dev/pki/ (idempotent; --force regenerates)
docker compose -f server/compose.postgres.yaml up -d --wait postgres
pnpm dev:controller    # controller on http://127.0.0.1:5150, agent RPC on 127.0.0.1:50051
pnpm dev:agents        # manager, worker-1, worker-2 agents on 127.0.0.1:50052-50054, logs in dev/logs/
pnpm dev:app           # the Tauri desktop app
```

`pnpm dev:smoke` runs the same sequence (skipping what is already up), waits for
`/health/ready`, and checks that every agent answers an mTLS handshake. `dev/Procfile`
lists the same processes for a process manager; `./dev/agent.sh <node>` runs one
agent in the foreground.

The dev agents and the controller reach the DinD daemons over TCP. That override
(`KONTAINER_DOCKER_HOST=tcp://…` for agents, `settings.docker.host` for the
controller) is accepted only when `KONTAINER_ENV` / the Loco environment is
`development` or `test` (ADR 0004); production requires the Unix socket.

To reset: `pnpm dev:agents:stop`, stop the controller, then `pnpm reset` (removes the
cluster and its volumes). `pnpm dev:cluster:seed` redeploys the seed stacks.
PostgreSQL keeps its data; see below.

### Owner account and password reset

The first sign-in on a fresh controller creates the owner (email, display name,
password of 12–128 characters); registration then closes for good. While no owner
exists, the controller prints a setup code to its standard output when it starts
(`Kontainer setup code: …`); the Create owner screen asks for it, so nobody else
who reaches the controller first can claim it. A restart prints a new code.

Ten failed sign-ins for one email within fifteen minutes lock that email for
fifteen minutes. There is no self-service reset. On the controller host, set a new
password; it is read from stdin, so `read -s` keeps it out of the terminal and the
shell history, and every session is signed out and the lock is cleared:

```bash
cd server
read -rs PW && printf '%s\n' "$PW" | cargo run --bin kontainer-controller -- task auth:reset-password email:owner@example.com; unset PW
```

### Common Commands

```bash
pnpm build:server     # Build both server binaries
pnpm test:server      # Test the Rust server workspace
pnpm lint:server      # Run Clippy with warnings denied
pnpm lint             # Lint the frontend
pnpm fmt              # Format the frontend
pnpm types            # Run frontend type checking
```

The controller development configuration expects PostgreSQL at
`postgres://kontainer:kontainer@127.0.0.1:55466/kontainer_development` unless
`DATABASE_URL` overrides it. Private controller-agent RPC is enabled in the
development config and reads its certificates from `dev/pki/`, so run
`pnpm dev:pki` before the first start.

Use Docker exclusively for local PostgreSQL; do not install or start a host
PostgreSQL service. The development Compose service uses `postgres:17-alpine`,
publishes only on loopback port `55466`, and supports `KONTAINER_POSTGRES_PORT`
to avoid conflicts with other local development tools (also override the client
database URLs when changing ports). Its project-scoped named volume preserves
data across container restarts. The checked-in credentials are local-development
credentials, not deployment secrets. Do not delete or reuse another application's
data.

Controller-owned `settings`, `agent_rpc`, and nested TLS settings reject unknown
keys. Configured cluster/node IDs use the same canonical segment rules as
certificate identities: 1–128 ASCII letters, digits, dots, underscores, or
hyphens, excluding `.` and `..`. When private RPC is enabled, its TLS files and
key/certificate pair are validated before the controller starts admitting HTTP
requests; setup failure closes both bound listeners. When `agent_rpc.client`
is configured, the static agent endpoint, expected DNS server name, and node
ID are validated with the same TLS bundle before admission: the endpoint must
be an `https` origin with no routing path, and the peer must present the exact
configured node-agent workload identity plus a matching `Describe` node ID.
An unreachable agent does not fail startup; it surfaces only as a typed call
failure. This is not certificate enrollment.
The node agent's `--version` still requires no runtime configuration.

The [canonical controller probes](docs/v2/approved-greenfield-reset.md#controller-probe-paths)
are unauthenticated and return only `{"ok": true}` or
`{"ok": false}`, with `Cache-Control: no-store`:

| Probe | Meaning |
| --- | --- |
| `/health/live` | HTTP is serving; does not check PostgreSQL or Docker. |
| `/health/startup` | Startup completed: HTTP admission begins only after static configuration and enabled RPC TLS preparation succeed. Startup failure leaves this endpoint unreachable. |
| `/health/ready` | Not draining, and PostgreSQL answered a ping within one second; otherwise HTTP 503. This is not a cluster-wide Docker availability check. |

Shutdown immediately makes readiness false and rejects newly admitted application
requests with HTTP 503, while requests already admitted may finish. Probes remain
meaningful on any connection still being served during drain; the listeners stop
accepting connections as part of shutdown. Both binaries share a 20-second
default drain budget. The controller reads `settings.shutdown_grace` (overridable
with `SHUTDOWN_GRACE` in development and production); the node agent reads
`KONTAINER_NODE_SHUTDOWN_GRACE`. Values must be positive integer durations with
`ms`, `s`, or `m` units, such as `750ms` or `30s`. The first shutdown cause fixes
one deadline. At that deadline, unfinished connections and requests are cancelled,
their tasks are joined, and the process exits with an error. The one-second database
probe deadline is an operational default, not an
availability SLO. `/api/v1/status` reports build/liveness information only. The
framework defaults `/_ping`, `/_health`, and `/_readiness` are not exposed.

Operators with a valid access session can request `/api/v1/status/deep` for a
redacted dependency diagnostic. It returns the controller build version and
`migration` (`current` or `unknown`), `database` (`reachable`, `unreachable`, or
`timeout`), and `agentRpc` (`up`, `down`, or `timeout`) states. Dependency
failures appear in an HTTP 200 response; the request still requires a live
session and is rejected during drain. The diagnostic checks dependencies for
up to four seconds and is separate from `/health/ready`, whose one-second
PostgreSQL check remains the admission signal. An `up` agent RPC state means
its authenticated `Describe` call succeeded; it does not mean Docker is
connected.

### Public HTTP transport

Production serves the public API (including the probes above) over TLS and
fails closed before admitting any handler when the identity is missing,
malformed, or mismatched:

```yaml
settings:
  public_http:
    mode: tls
    tls:
      certificate_path: <%= get_env(name="PUBLIC_HTTP_TLS_CERTIFICATE_PATH") %>
      private_key_path: <%= get_env(name="PUBLIC_HTTP_TLS_PRIVATE_KEY_PATH") %>
```

Development and Test may use `mode: plaintext` only when the bind address
and the advertised `server.host` URL are both loopback; production rejects
plaintext even on loopback. There is no plaintext fallback, redirect
listener, ACME, rotation, or hot reload.

## Testing

For real PostgreSQL integration tests, reuse the project's persistent Compose
service. The start command is safe to repeat:

```bash
docker compose -f server/compose.postgres.yaml up -d --wait postgres
KONTAINER_REQUIRE_POSTGRES_TESTS=1 \
KONTAINER_TEST_DATABASE_URL=postgres://kontainer:kontainer@127.0.0.1:55466/postgres \
  cargo test --manifest-path server/Cargo.toml --workspace
```

Tests create and clean up their own scratch databases; the service and development
database remain intact. Leave PostgreSQL running between test runs. To pause it
without deleting data, use `docker compose -f server/compose.postgres.yaml stop postgres`.
Do not run `down --volumes` or delete the named volume unless intentionally
resetting project data with explicit approval. Host PostgreSQL is never needed.

```bash
pnpm --filter app test
pnpm test:server
pnpm lint:server
cargo test --manifest-path app/src-tauri/Cargo.toml
```

## Notes

- The desktop app talks to the controller API; neither public APIs nor protobuf messages proxy the Docker socket.
- Both Linux AMD64 and ARM64 are first-class release targets and require native smoke evidence.
- Keep long-form planning and framework-specific guidance in `docs/` and skills rather than expanding this README.

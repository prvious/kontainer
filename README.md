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

The controller must run on a Swarm manager with that manager's Docker socket mounted
(`KONTAINER_DOCKER_HOST`, default `unix:///var/run/docker.sock`; TCP only on loopback). On a
worker, services, stacks, overlay networks and service logs answer `not_a_manager`. The socket is
root-equivalent access to the host, so run the controller only on a trusted manager. If that
manager goes down, those Swarm features are unavailable until the controller runs on another
manager; node-local features keep working through the agents. See ADR 0042.

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

See [`docs/build/README.md`](docs/build/README.md) for the specs, the ledger, and the design source.

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
requests; setup failure closes both bound listeners. Development reads them
from `dev/pki/`; without configured paths the controller and agents issue
their own from `KONTAINER_SECRET` (ADR 0045). The controller dials each agent
at the address the agent's check-in recorded and requires that node's exact
workload identity plus a matching `Describe` node ID. An unreachable agent does
not fail startup; it surfaces only as a typed call failure.
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

The controller serves plain HTTP and expects a reverse proxy (Traefik,
HAProxy, nginx) to terminate TLS in front of it, as Portainer does (ADR 0046).
The desktop app only connects to remote controllers over `https://`, so it
always goes through that proxy. Setting `PUBLIC_HTTP_TLS_CERTIFICATE_PATH` and
`PUBLIC_HTTP_TLS_PRIVATE_KEY_PATH` makes the controller serve that certificate
itself instead; a missing, malformed or mismatched identity fails startup
before any handler is admitted. There is no ACME, rotation, or hot reload.

## Run on a Swarm

Images are published to `ghcr.io/prvious/kontainer/controller` and
`ghcr.io/prvious/kontainer/agent` (`latest` follows `main`; releases are
tagged by version). The packages are private: log in to `ghcr.io` with a token
that can read them and deploy with `docker stack deploy --with-registry-auth`.
A stack is the controller on a manager, an agent on every node, and
PostgreSQL:

```yaml
services:
  controller:
    image: ghcr.io/prvious/kontainer/controller:latest
    environment:
      DATABASE_URL: postgres://kontainer:${DB_PASSWORD}@postgres/kontainer
      KONTAINER_SECRET: ${KONTAINER_SECRET}
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
    networks: [kontainer, proxy]
    deploy:
      placement:
        constraints: [node.role == manager]
      # Route your proxy to port 5150, for example with Traefik labels.

  agent:
    image: ghcr.io/prvious/kontainer/agent:latest
    environment:
      KONTAINER_SECRET: ${KONTAINER_SECRET}
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
    networks: [kontainer]
    deploy:
      mode: global

  postgres:
    image: postgres:17-alpine
    environment:
      POSTGRES_USER: kontainer
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: kontainer
    volumes:
      - database:/var/lib/postgresql/data
    networks: [kontainer]
    deploy:
      placement:
        constraints: [node.role == manager]

networks:
  kontainer:
    driver: overlay
  proxy:
    external: true

volumes:
  database:
```

- `KONTAINER_SECRET` is any string of at least 32 characters, the same on the
  controller and every agent (`openssl rand -hex 32`). Each process derives the
  cluster CA from it and issues its own certificate, so there is nothing to
  generate (ADR 0045). `KONTAINER_SECRET_FILE` reads it from a Swarm secret
  instead.
- The controller migrates its database on start. Agents find their node id
  (the Swarm node id) and their address on the `kontainer` network themselves;
  `KONTAINER_NODE_ID` and `KONTAINER_NODE_ADVERTISE_ADDRESS` override them.
- Publish the controller only through your proxy; don't publish 5150 or the
  agent ports on the host.
- PostgreSQL's volume lives on whichever node runs it. With more than one
  manager, pin it to one node with a label (`node.labels.kontainer-db == true`)
  and back that volume up, or Swarm may reschedule it onto an empty volume.
- On first start the controller prints a setup code
  (`docker service logs <stack>_controller`); enter it on the desktop app's
  Create owner screen.

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

End-to-end tests drive the built desktop app with WebdriverIO's embedded WebDriver. This works on
macOS, Windows and Linux and needs no external driver. The `e2e` build has its own identifier and
keychain service, so it never touches your real saved connections (ADR 0044):

```bash
pnpm --filter app e2e:build -- --locked
pnpm --filter app e2e
```

The `Desktop` workflow (`.github/workflows/desktop.yml`) runs unit tests and this
E2E suite in Linux, Windows and macOS matrix jobs. Linux wraps the runner in
Xvfb and a D-Bus session with an unlocked Secret Service keyring. `Server`
combines workspace tests, stream evidence and native release smoke checks;
`Docker` keeps pinned real-daemon qualification and offers an explicit manual
`backend=fake` API-contract fallback. See [the testing plan](docs/testing.md).

## Notes

- The desktop app talks to the controller API; neither public APIs nor protobuf messages proxy the Docker socket.
- Both Linux AMD64 and ARM64 are first-class release targets and require native smoke evidence.
- Keep long-form planning and framework-specific guidance in `docs/` and skills rather than expanding this README.

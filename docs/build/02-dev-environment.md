# 02. Dev environment

## Goal

One documented sequence brings up PostgreSQL, the three-node docker-in-docker Swarm, a controller,
and three node agents with valid mTLS certificates, so every later spec can be tested against a real
cluster. Today the controller config points at certificate files that don't exist, the Procfile
launches the deleted Go agent, and the agent only knows the local Unix socket.

Decisions: ADR 0004 (dev TCP override).

## Existing pieces to keep

- `dev/docker-compose.yml` (three `docker:dind` nodes on `localhost:2375`, `2376`, `2378`),
  `dev/setup.sh`, `dev/teardown.sh`, `dev/seed-services.sh`, and the seed stacks `webapp.yml`,
  `monitoring.yml`, `toolbox.yml`. Root scripts `dev:cluster`, `dev:cluster:seed`, `reset`.
- `server/compose.postgres.yaml` for PostgreSQL. Never install host PostgreSQL.
- Test PKI helpers in `server/tests/support/pki.rs` (rcgen) as the reference for what certificates
  must contain.

## Deliverables

### Docker host override (server)

- `kontainer-engine`: add a constructor for a TCP endpoint next to the Unix-socket constructor
  (`bollard::Docker::connect_with_http`). No new features in `Cargo.toml`; `http` is already on.
- Node agent config: optional `KONTAINER_DOCKER_HOST`. Unset means `/var/run/docker.sock`.
  `tcp://host:port` selects the TCP constructor. The agent refuses the override unless
  `KONTAINER_ENV` is `development` or `test` (add that variable, default `production`).
- Controller config: `settings.docker.host`, default `unix:///var/run/docker.sock`, same rule via
  Loco's environment (`development.yaml` and `test.yaml` may set `tcp://`, `production.yaml`
  rejects it at startup). The controller's manager-scoped engine (spec 11) reads this.
- Real-engine tests: the existing socket-path variable also accepts a `tcp://` value.

### Dev PKI script

- `dev/pki.sh` (bash + `openssl`, no Cargo changes) writes to `dev/pki/` (add to `.gitignore`):
  - `agent-ca.pem` / `agent-ca-key.pem`
  - `controller.pem` / `controller-key.pem` with URI SAN
    `spiffe://kontainer.internal/swarm/development/controller/dev`
  - `node-manager.pem`, `node-worker-1.pem`, `node-worker-2.pem` and keys with URI SAN
    `spiffe://kontainer.internal/swarm/development/node/<node-id>/agent`
  - one year validity, ECDSA P-256, idempotent (skips existing files unless `--force`).
- Read `server/crates/rpc-auth` to confirm the exact identity format and that exactly one URI SAN is
  required; adjust the script until `tests/mtls_handshake.rs` style checks pass against the files.
- `server/config/development.yaml` defaults for the three TLS paths point at `../dev/pki/...`
  relative to `server/`, and `agent_rpc.enabled` defaults to `true` in development.

### Node ids

- `KONTAINER_NODE_ID` is an operator-chosen stable id: `manager`, `worker-1`, `worker-2` in dev.
  It must equal the node id in the certificate.
- The agent learns the Swarm node id (`docker info` → `Swarm.NodeID`) and reports it in check-in
  (spec 05) so the controller can join agent rows to the manager's node list.

### Process manifest

- Rewrite `dev/Procfile`:
  ```
  controller: cd server && KONTAINER_DOCKER_HOST=tcp://localhost:2375 cargo run --bin kontainer-controller -- start
  agent-manager: cd server && KONTAINER_ENV=development KONTAINER_DOCKER_HOST=tcp://localhost:2375 KONTAINER_NODE_ID=manager KONTAINER_CLUSTER_ID=development KONTAINER_NODE_RPC_BINDING=127.0.0.1:50052 <tls paths> cargo run --bin kontainer-node-agent
  agent-worker-1: … tcp://localhost:2376 … KONTAINER_NODE_ID=worker-1 … 127.0.0.1:50053
  agent-worker-2: … tcp://localhost:2378 … KONTAINER_NODE_ID=worker-2 … 127.0.0.1:50054
  app: pnpm --filter app tauri dev
  ```
  Use the real variable names from `crates/node-agent/src/config.rs`. Put the long environment in
  `dev/agent.env.example` style files if the Procfile gets unreadable; the owner runs processes with
  Solo, so keep it a plain Procfile.
- Root `package.json`: `dev:controller` becomes `cd server && cargo run --bin kontainer-controller -- start`
  (no `loco` CLI dependency); add `dev:pki` → `./dev/pki.sh`, `dev:agents` → a small script that
  starts the three agents in the background with logs under `dev/logs/` (gitignored), and
  `dev:agents:stop`.
- `README.md` Local Development section: the exact order `pnpm dev:cluster`, `pnpm dev:pki`,
  `docker compose -f server/compose.postgres.yaml up -d --wait postgres`, `pnpm dev:controller`,
  `pnpm dev:agents`, `pnpm dev:app`, and how to reset. Remove the stale statements about `air` and
  leader/follower.

### Controller ↔ agent in dev before membership exists

Spec 05 replaces the single static agent client with membership. Until then, keep the existing
`AGENT_CLIENT_*` settings working: `development.yaml` defaults them to the manager agent at
`https://127.0.0.1:50052` with server name matching the certificate and node id `manager`, so
`/api/v1/status/deep` reports the agent up.

## Tests

- Unit: engine TCP constructor parses `tcp://host:port` and rejects other schemes; the agent config
  rejects the override outside development/test.
- Integration (behind `real-engine-tests`, needs the DinD cluster): `Describe` against
  `tcp://localhost:2375` reports a Swarm manager; against `2376` a worker.
- A shell smoke test `dev/smoke.sh`: brings the stack up, curls `/health/ready` until 200, calls
  `/api/v1/status/deep` with a dev session (spec 04 adds password login; until then the deep status
  route is protected, so the smoke script checks only `/health/ready` and the agent's TLS port
  answering a handshake with `openssl s_client`), and tears nothing down.

## Done when

- From a clean checkout: `pnpm dev:cluster && pnpm dev:pki && pnpm dev:controller` starts the
  controller with agent RPC enabled and no missing-file errors.
- `pnpm dev:agents` starts three agents that stay up and log a successful Docker ping each.
- `curl -s localhost:5150/health/ready` returns 200 and the manager agent shows `up` in
  `/api/v1/status/deep` once spec 04 provides a session (record the curl output in `PROGRESS.md`
  when you get there).
- `README.md` documents the sequence; `dev/Procfile` has no Go or `air` references.

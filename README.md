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
- View cluster and node metrics collected by the leader agent.
- Connect to multiple clusters from one desktop app.
- Authenticate through a browser-based GitHub OAuth flow.

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
- `docs/v2/decision-register.md` - decision history and superseded research

## Local Development

### Prerequisites

- `pnpm`
- Rust 1.98
- Docker
- PostgreSQL

### Common Commands

Run from the repository root:

```bash
pnpm dev:app          # Start the Tauri desktop app
pnpm dev:cluster      # Start the local Docker-in-Docker Swarm cluster
pnpm dev:controller   # Start the controller (requires PostgreSQL)
pnpm build:server     # Build both server binaries
pnpm test:server      # Test the Rust server workspace
pnpm lint:server      # Run Clippy with warnings denied
pnpm lint          # Lint the frontend
pnpm fmt           # Format the frontend
pnpm types         # Run frontend type checking
pnpm down          # Tear down the local dev cluster
pnpm reset         # Tear down the local dev cluster and volumes
```

The controller development configuration expects PostgreSQL at
`postgres://kontainer:kontainer@127.0.0.1:5432/kontainer_development` unless
`DATABASE_URL` overrides it. Private controller-agent RPC is disabled in the
development config until an mTLS certificate bundle is supplied.

## Testing

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

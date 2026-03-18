# Kontainer

Kontainer is a desktop app for managing Docker Swarm infrastructure through a dedicated agent API.

It pairs a Tauri desktop client with a Go agent built on Goravel so you can inspect clusters, operate services, stream logs, and open exec sessions without exposing the Docker socket directly to the UI.

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

Kontainer has two main parts:

- `app/` - Tauri desktop app built with Rust, React, Shadcn UI, Tailwind CSS, TanStack Router, and TanStack Query.
- `server/` - Go agent built with Goravel, gRPC, WebSocket support, Docker SDK integrations, and SQLite-backed persistence where needed.

The agent runs in one of two modes:

- `leader` - serves the public API, handles auth, runs scheduled work, and coordinates cluster-wide operations.
- `follower` - exposes internal gRPC and health endpoints for node-local work in multi-node setups.

Frontend requests flow through the desktop boundary like this:

`React -> Tauri invoke -> Rust command -> Go agent API`

## Repository Layout

- `app/` - desktop UI and Tauri commands
- `server/` - Goravel agent, routes, actions, services, and tests
- `dev/` - local multi-node Docker-in-Docker development environment
- `docs/` - planning, reference, and design artifacts

## Deployment Docs

- `docs/deployment.md` - deployment modes, environment variables, Docker examples, and setup notes
- `docs/agent.md` - current agent architecture, responsibilities, and discovery model

## Local Development

### Prerequisites

- `pnpm`
- Go
- Rust
- Docker

### Common Commands

Run from the repository root:

```bash
pnpm dev           # Start the dev cluster, leader, followers, and desktop app
pnpm dev:cluster   # Start only the local Docker-in-Docker Swarm cluster
pnpm dev:app       # Start only the Tauri desktop app
pnpm dev:agent     # Start only the Goravel leader agent (defaults to http://localhost:3000)
pnpm artisan list  # Run Goravel artisan commands from the server workspace
pnpm build         # Build app and agent
pnpm test:agent    # Run Go agent tests
pnpm vet:agent     # Run Go agent vet checks
pnpm lint          # Lint the frontend
pnpm fmt           # Format the frontend
pnpm types         # Run frontend type checking
pnpm down          # Tear down the local dev cluster
pnpm reset         # Tear down the cluster and clear local agent databases
```

`pnpm dev` bootstraps a local 3-node Docker-in-Docker Swarm cluster for development.

The checked-in local server defaults in `server/.env.example` use:

- leader HTTP on `http://localhost:3000`
- follower HTTP on `http://localhost:3001` and `http://localhost:3002` via `dev/Procfile`
- follower discovery through `CLUSTER_DEV_FOLLOWERS`

## Testing

```bash
pnpm --filter app test
pnpm test:agent
pnpm vet:agent
cargo test --manifest-path app/src-tauri/Cargo.toml
```

## Notes

- The desktop app talks to the agent API; the agent is not a raw Docker socket proxy.
- In multi-node setups, the leader discovers and coordinates follower agents.
- Keep long-form planning and framework-specific guidance in `docs/` and skills rather than expanding this README.

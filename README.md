# Kontainer

A desktop app for managing Docker Swarm clusters. Self-hosted, lightweight, multi-node.

**Website:** [kontainer.dev](https://kontainer.dev)

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                   Desktop App                       │
│              Tauri (Rust) + React                   │
│                                                     │
│  OAuth login ─► Session store ─► Tauri IPC bridge   │
└──────────────────────┬──────────────────────────────┘
                       │ HTTPS + WebSocket
          ┌────────────┼────────────────┐
          ▼            ▼                ▼
   ┌─────────┐  ┌─────────┐     ┌─────────┐
   │ Leader  │  │Follower │     │Follower │
   │  Agent  │◄─┤  Agent  │ ... │  Agent  │
   │ (Go)    │  │ (Go)    │     │ (Go)    │
   └────┬────┘  └────┬────┘     └────┬────┘
        │             │               │
   Docker socket Docker socket  Docker socket
```

**Leader agent** -- Full REST + WebSocket API. Handles auth, discovers followers via Swarm tasks, collects cluster-wide metrics, and maintains an in-memory registry of all agents. Healthchecks followers every 10s. Stores time-series node metrics in embedded SQLite with automatic retention-based pruning.

**Follower agents** -- Lightweight. Expose `/internal/health`, `/internal/info`, `/internal/metrics`, and `/internal/exec/{containerID}` endpoints behind a shared secret (`X-Kontainer-Secret`). Auto-discovered by the leader in production, or configured statically in dev via `KONTAINER_DEV_FOLLOWERS`.

**Desktop app** -- Cross-platform (macOS, Linux, Windows). Connects to one or more leader agents, authenticates via OAuth through the system browser, communicates over Tauri IPC to Rust to HTTP/WebSocket to the agent. Supports multiple cluster profiles with instant switching.

## Features

### Infrastructure Dashboard
- Cluster-wide stats: node count, CPU usage, memory usage per node
- CPU and memory time-series charts with per-node filtering
- Live container table with CPU/memory metrics, sortable and filterable
- Service and stack overview cards
- Auto-refreshing (5s polling) with live indicator

### Service Management
- List, inspect, scale, restart, rollback services
- Inline environment variable editor (add, edit, remove vars with immediate deploy)
- Inline image tag editor (deploy new versions by changing the image)
- Published ports, mounts, constraints, placement preferences
- Update status tracking with convergence monitoring and error reporting
- Tabbed detail view: overview, tasks, logs, configuration

### Container Exec (Terminal)
- Full interactive terminal via WebSocket into any running container
- Multi-session tabbed interface -- open terminals to multiple containers simultaneously
- Shell auto-detection: tries `/bin/sh`, `/bin/bash`, `/bin/ash` with fallback chain
- Custom shell and user parameters per session
- Resizable panel with drag handle, minimize, and fullscreen modes
- Per-session connection status indicators (connecting, connected, error, exited)
- Reconnect on disconnect
- Cross-node exec: leader proxies WebSocket connections to follower agents for containers on remote nodes
- xterm.js-based terminal with Tokyo Night theme, auto-fit on resize

### Real-Time Log Streaming
- WebSocket streaming for both services and individual tasks
- stdout/stderr separation with toggles
- Timestamp display toggle
- Pause/resume, tail line selection (100, 500, 1000, all)
- Auto-scroll with manual scroll override
- Clear logs, reconnect on disconnect
- Auto-reconnect with exponential backoff (up to 16s)

### Task Inspection
- Per-service task listing with state, desired state, exit codes, error messages
- Live CPU and memory metrics per task (from cluster metrics)
- Full task detail sheet: container ID, PID, image, env vars, mounts, networks
- Per-task log streaming
- One-click terminal access from task rows

### Node Management
- Swarm node overview: hostname, role, status, availability, IP, engine version, OS
- Promote/demote (worker to manager and back)
- Drain, pause, activate nodes
- Per-node CPU and memory breakdown in dashboard stats cards

### Multi-Cluster Support
- Connect to multiple clusters from a single app instance
- Cluster switcher in the sidebar with provider detection (AWS, GCP, Azure, DigitalOcean, local, self-hosted)
- Add cluster dialog: validates agent URL with ping, triggers OAuth, saves profile
- Remove and switch between clusters instantly; query cache clears on switch

### Cluster Metrics
- Leader collects node and container metrics from all agents on a periodic interval
- Per-node metrics: CPU %, memory used/total/%, disk used/total/%
- Per-container metrics: CPU %, memory used/limit/%
- Time-series history stored in embedded SQLite with automatic pruning
- Cluster and per-node metrics API endpoints

### Stack Management
- List stacks with service and replica counts
- Remove entire stacks

### Networks, Volumes, Secrets & Configs
- Browse overlay networks with attached services
- Volume listing with size, driver, mountpoint
- Create and delete secrets with content input
- Create and delete configs with content input

### Authentication
- GitHub OAuth 2.0 (extensible provider system via the `providers` package)
- Optional access control -- restrict login by GitHub username (`GITHUB_ALLOWED_USERS`) and/or organization membership (`GITHUB_ALLOWED_ORGS`)
- WebSocket-based auth bridge between desktop app and agent
- System browser opens for OAuth, deep-link callback returns JWT to the app
- Short-lived JWTs (HS256), session persistence via `tauri-plugin-store`
- Auto-logout on token expiry (30s check interval)
- Multi-profile session storage: each cluster connection is an independent profile

### Multi-Node Cluster Architecture
- Leader/follower agent model controlled by `KONTAINER_ROLE` env var
- Auto-discovery via Docker Swarm service tasks (`KONTAINER_FOLLOWER_SERVICE`)
- Static follower list for development (`KONTAINER_DEV_FOLLOWERS`)
- Internal healthchecks with shared secret authentication
- Thread-safe in-memory agent registry with cluster-wide visibility
- Agent listing API endpoint for the desktop app

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop | Tauri 2 (Rust) + React 19 + Shadcn UI + Tailwind CSS 4 |
| Agent | Go (Chi, Docker SDK, gorilla/websocket) |
| Auth | OAuth 2.0 + JWT (HS256) |
| Transport | HTTPS + WebSocket |
| Storage | Agent: in-memory + embedded SQLite (metrics history, leader only). App: tauri-plugin-store |
| Terminal | xterm.js + FitAddon |
| Charts | Recharts |
| Routing | TanStack Router (file-based) |
| Data Fetching | TanStack Query (5s polling intervals) |

## Deployment

The leader agent runs as a Swarm service with the Docker socket mounted:

```yaml
services:
  kontainer-agent:
    image: prvious/kontainer-agent
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
    environment:
      - KONTAINER_ROLE=leader
      - KONTAINER_FOLLOWER_SERVICE=kontainer-follower
```

Follower agents deploy as a global service across worker nodes:

```yaml
services:
  kontainer-follower:
    image: prvious/kontainer-agent
    deploy:
      mode: global
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
    environment:
      - KONTAINER_ROLE=follower
```

## Access Control

Login can optionally be restricted by GitHub username and/or organization membership:

```yaml
environment:
  - GITHUB_ALLOWED_USERS=alice,bob
  - GITHUB_ALLOWED_ORGS=my-org,my-other-org
```

Both variables are **comma-separated** lists and matching is **case-insensitive**.

| `GITHUB_ALLOWED_USERS` | `GITHUB_ALLOWED_ORGS` | Who can log in |
|---|---|---|
| unset | unset | All GitHub users (default) |
| set | unset | Only listed usernames |
| unset | set | Only members of listed orgs |
| set | set | Only listed usernames **who are also** members of listed orgs |

- `GITHUB_ALLOWED_ORGS` requires the `read:org` OAuth scope (added automatically when configured)

Non-matching users receive a `403 Forbidden` response during the OAuth callback.

## Development

```bash
pnpm dev          # Start DinD cluster + leader + 2 followers + app
pnpm dev:app      # Tauri desktop app only
pnpm dev:agent    # Go agent only
pnpm build        # Build everything
pnpm build:app    # Build desktop app
pnpm build:agent  # Build Go agent
pnpm lint         # Lint frontend
pnpm fmt          # Format frontend (oxfmt)
```

Dev environment spins up a 3-node Docker-in-Docker Swarm cluster (1 manager + 2 workers) with seed stacks.

### Running Tests

```bash
pnpm --filter app test         # Vitest (frontend)
go test -C agent ./...         # Go agent tests
cargo test --manifest-path app/src-tauri/Cargo.toml  # Rust tests
```

## Supported Orchestrators

- Docker Swarm (active)
- Docker Compose (planned)
- Kubernetes (planned)

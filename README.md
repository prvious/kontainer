# Devoper

A desktop app for managing Docker Swarm clusters. Self-hosted, lightweight, multi-node.

**Website:** [devoper.dev](https://devoper.dev)

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

**Leader agent** — Full REST + WebSocket API. Handles auth, discovers followers via Swarm tasks, maintains an in-memory registry of all cluster agents. Healthchecks followers every 10s.

**Follower agents** — Lightweight. Expose `/internal/health` and `/internal/info` endpoints behind a shared secret. Auto-discovered by the leader in production, or configured statically in dev.

**Desktop app** — Cross-platform (macOS, Linux, Windows). Connects to the leader, authenticates via OAuth through the system browser, communicates over Tauri IPC → Rust → HTTP to the agent.

## Features

### Cluster Dashboard
- Composite health score across services, nodes, and tasks
- Running/failed task counts, node status breakdown
- Stack health overview, recent activity feed
- Auto-refreshing (5s polling)

### Service Management
- List, inspect, scale, restart, rollback services
- Inline environment variable editor
- Inline image tag editor (deploy new versions)
- Published ports, mounts, constraints
- Update status tracking with error reporting

### Real-Time Logs
- WebSocket streaming for both services and individual tasks
- stdout/stderr separation, timestamp toggle
- Pause/resume, tail line selection, auto-reconnect with backoff

### Task Inspection
- Per-service task listing with state, exit codes, errors
- Full task detail: container ID, PID, env vars, mounts, networks
- Per-task log streaming

### Node Management
- Swarm node overview: role, status, availability, IP, engine version
- Promote/demote (worker ↔ manager)
- Drain, pause, activate nodes

### Multi-Node Cluster
- Leader/follower agent architecture
- Auto-discovery via Docker Swarm service tasks
- Internal healthchecks with shared secret auth
- Agent registry with cluster-wide visibility

### Stack Management
- List stacks with service and replica counts
- Remove entire stacks

### Networks, Volumes, Secrets & Configs
- Browse overlay networks with attached services
- Volume listing with size, driver, mountpoint
- Create and delete secrets and configs

### Authentication
- GitHub OAuth 2.0 (extensible provider system)
- WebSocket-based auth bridge between desktop app and agent
- Short-lived JWTs (24h), session persistence, auto-logout on expiry

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop | Tauri 2 (Rust) + React 19 + Shadcn UI + Tailwind CSS 4 |
| Agent | Go (Chi, Docker SDK, gorilla/websocket) |
| Auth | OAuth 2.0 + JWT (HS256) |
| Transport | HTTPS + WebSocket |
| Storage | Agent: stateless (in-memory). App: tauri-plugin-store |

## Deployment

The leader agent runs as a Swarm service with the Docker socket mounted:

```yaml
services:
  devoper-agent:
    image: prvious/devoper-agent
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
    environment:
      - DEVOPER_ROLE=leader
      - DEVOPER_FOLLOWER_SERVICE=devoper-follower
```

Follower agents deploy as a global service across worker nodes:

```yaml
services:
  devoper-follower:
    image: prvious/devoper-agent
    deploy:
      mode: global
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
    environment:
      - DEVOPER_ROLE=follower
```

## Development

```bash
pnpm dev          # Start DinD cluster + leader + 2 followers + app
pnpm dev:app      # Tauri desktop app only
pnpm dev:agent    # Go agent only
pnpm build        # Build everything
```

Dev environment spins up a 3-node Docker-in-Docker Swarm cluster (1 manager + 2 workers) with seed stacks.

## Supported Orchestrators

- Docker Swarm
- Docker Compose _(planned)_
- Kubernetes _(planned)_

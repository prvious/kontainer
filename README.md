# Devoper

A lightweight desktop app for managing containerized infrastructure. A simpler, self-hosted alternative to Portainer — built for small teams and solo developers who don't need the bloat.

**Website:** [devoper.dev](https://devoper.dev)

## Architecture

The project has two components:

### 1. Agent (Go)

A small, stateless Go binary deployed alongside your infrastructure. It exposes a clean REST + WebSocket API that talks to the Docker socket internally. Handles authentication via OAuth (GitHub, Google, etc.) using a Laravel Socialite-inspired flow, and issues short-lived JWTs for subsequent requests.

### 2. Desktop App (Tauri + React)

A cross-platform desktop app built with Tauri (Rust) and React. Connects to one or more agents, handles the OAuth login flow via the system browser, and stores credentials securely in the OS keychain.

## Core Features

- **Services & Containers** — List, inspect, scale, restart, and rollback
- **Stacks** — Deploy and manage stacks from Compose files
- **Logs** — Real-time log streaming via WebSocket
- **Nodes** — View cluster node status
- **Auth** — Third-party OAuth with org/domain-level access control

## Supported Orchestrators

- Docker Swarm
- Docker Compose _(planned)_
- Kubernetes _(planned)_

## Tech Stack

| Component     | Technology                 |
| ------------- | -------------------------- |
| Desktop App   | Tauri + React              |
| Agent         | Go (stdlib)                |
| Auth          | OAuth 2.0 + JWT            |
| Transport     | HTTPS + WebSocket          |
| App Storage   | OS Keychain + SQLite       |
| Agent Storage | Stateless (in-memory only) |

## Deployment

The agent runs as a single container alongside your infrastructure, mounted with the Docker socket (read-only) and exposed via any reverse proxy over HTTPS.

```yaml
services:
    devoper-agent:
        image: prvious/devoper-agent
        volumes:
            - /var/run/docker.sock:/var/run/docker.sock:ro
```

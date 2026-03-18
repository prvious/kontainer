# Kontainer Agent

This document describes the current Kontainer agent at a high level.

For deployment examples, Docker service commands, and environment-variable oriented setup, see `docs/deployment.md`.

## Purpose

The Kontainer agent is the server-side runtime that talks to Docker, exposes the Kontainer API, and coordinates cluster-wide operations for the desktop app.

It is intentionally not a raw Docker socket proxy. It exposes a narrower application API shaped around what the frontend needs.

## Runtime Modes

The agent runs in one of two modes, controlled by `AGENT_ROLE`:

- `leader` - serves the public API, auth routes, setup routes, WebSocket routes, and scheduled cluster work
- `follower` - serves health endpoints and internal gRPC only

The current bootstrap wiring lives in `server/bootstrap/app.go`.

## Responsibilities

### Leader

- expose public HTTP routes for auth, setup, and `/api/*`
- run scheduled work such as follower discovery and metrics collection
- coordinate cluster-wide operations
- maintain the in-memory registry of known agents
- serve WebSocket-backed flows such as logs and exec

### Follower

- expose node-local health endpoints
- expose internal gRPC for work initiated by the leader
- participate in multi-node operations without serving the public API directly

## Request Flow

The desktop app communicates through this path:

`React -> Tauri invoke -> Rust command -> leader agent`

In multi-node cases, the leader may then fan out to follower agents over gRPC.

## Public vs Internal Surface

### Public HTTP Surface

The leader serves:

- `GET /health`
- `GET /ping`
- `/auth/*`
- `/setup/*`
- `/api/*`

### Internal Surface

Followers do not expose the public API. They serve:

- `GET /health`
- `GET /ping`
- internal gRPC services used by the leader

## Discovery Model

Leader discovery is configured in `server/app/providers/cluster_service_provider.go`.

- if `CLUSTER_DEV_FOLLOWERS` is set, the leader uses that comma-separated address list directly
- otherwise, the leader discovers follower tasks through Swarm using `CLUSTER_FOLLOWER_SERVICE`

This makes `CLUSTER_DEV_FOLLOWERS` a development shortcut, not the normal production path.

## Persistence

The agent uses SQLite by default via `DB_DATABASE`.

Persist this database if you want to keep:

- setup completion state
- stored app config values
- generated JWT secret
- GitHub App credentials created during setup

Current database defaults live in `server/config/database.go`.

## Auth and Setup

The current flow is setup-driven.

- the setup routes bootstrap the GitHub App integration
- the agent stores the resulting credentials in the database
- the JWT secret is also loaded from persisted app config
- auth is considered incomplete until setup has been finished

Relevant code paths:

- `server/routes/setup.go`
- `server/routes/auth.go`
- `server/app/actions/setup/`
- `server/app/providers/auth_service_provider.go`

## Important Configuration

The most important runtime variables are:

- `AGENT_ROLE`
- `AGENT_URL`
- `APP_HOST`
- `APP_PORT`
- `GRPC_PORT`
- `DB_DATABASE`
- `CLUSTER_SECRET`
- `CLUSTER_FOLLOWER_SERVICE`
- `CLUSTER_DEV_FOLLOWERS`

The agent also resolves `_FILE` variants for selected secrets at startup, including:

- `CLUSTER_SECRET_FILE`
- `JWT_SECRET_FILE`
- `DB_DATABASE_FILE`

See `server/main.go` and `server/app/helpers/env_file.go`.

## Local Development

The repo ships with a local multi-node Docker-in-Docker environment under `dev/`.

- `pnpm dev` starts the cluster, agents, and desktop app
- `pnpm dev:cluster` starts only the local Swarm cluster
- the local leader reads `CLUSTER_DEV_FOLLOWERS` from `server/.env` (loaded by `air`)

## Source of Truth

If this document drifts, prefer the code:

- `server/bootstrap/app.go`
- `server/config/cluster.go`
- `server/config/http.go`
- `server/config/grpc.go`
- `server/config/database.go`
- `server/routes/api.go`
- `server/routes/auth.go`
- `server/routes/setup.go`

# Goravel Agent Migration Design

**Date:** 2026-03-11
**Status:** Approved
**Scope:** Full rewrite of the Go agent from Chi + raw SQL to Goravel framework

## Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Frontend during migration | Fix later (option C) | Agent-first, no backward-compat constraint |
| Project location | New `/server` dir (option B) | Old `/agent` stays as reference |
| Inter-agent communication | gRPC (option B) | Replacing HTTP + shared secret; avoids double rewrite |
| Database | SQLite + Goravel ORM, swappable (option C) | Zero-config deploy now, Postgres-ready later |
| Migration order | Core infra first (option A) | Foundation-first, then vertical features |
| Migration strategy | Layer-by-layer (approach A) | Small PRs, each layer builds on the last |

## 1. Project Structure

New Goravel project at `/server`:

```
server/
├── app/
│   ├── http/
│   │   ├── controllers/
│   │   │   ├── auth/            # OAuth flow, /auth/* routes
│   │   │   ├── services/        # Service CRUD, scale, restart, rollback
│   │   │   ├── tasks/           # Task listing, inspection
│   │   │   ├── nodes/           # Node listing, availability, role
│   │   │   ├── stacks/          # Stack listing, removal
│   │   │   ├── networks/        # Network listing
│   │   │   ├── volumes/         # Volume listing
│   │   │   ├── images/          # Image listing, prune, remove
│   │   │   ├── secrets/         # Secret CRUD
│   │   │   ├── configs/         # Config CRUD
│   │   │   ├── metrics/         # Cluster + node metrics
│   │   │   ├── settings/        # User preferences
│   │   │   ├── agents/          # Cluster agent listing
│   │   │   └── setup/           # Initial setup flow
│   │   └── middleware/
│   │       ├── jwt.go           # JWT auth (wraps Goravel's auth)
│   │       └── setup_token.go   # Setup mode guard
│   │
│   ├── grpc/
│   │   ├── controllers/
│   │   │   ├── health.go        # Replaces /internal/health
│   │   │   ├── metrics.go       # Replaces /internal/metrics
│   │   │   ├── images.go        # Replaces /internal/images
│   │   │   └── exec.go          # Container exec relay
│   │   └── interceptors/
│   │       └── secret_auth.go   # gRPC shared secret validation
│   │
│   ├── actions/                 # Business logic (one action per file)
│   │   ├── services/
│   │   ├── tasks/
│   │   ├── nodes/
│   │   ├── images/
│   │   ├── secrets/
│   │   ├── configs/
│   │   ├── metrics/
│   │   └── auth/
│   │
│   ├── models/
│   │   ├── app_config.go
│   │   ├── user.go
│   │   ├── node_metric.go
│   │   └── setting.go
│   │
│   ├── jobs/                    # Queue jobs (future)
│   ├── providers/
│   │   ├── docker_service_provider.go
│   │   ├── cluster_service_provider.go
│   │   └── auth_service_provider.go
│   │
│   └── services/
│       ├── docker/
│       │   ├── client.go        # Orchestrator interface
│       │   └── swarm/           # Swarm implementation
│       ├── cluster/             # Registry, discovery
│       └── websocket/           # Log streaming, exec handlers
│
├── bootstrap/
├── config/
├── database/migrations/
├── proto/agent/agent.proto
├── routes/
│   ├── api.go
│   ├── auth.go
│   ├── setup.go
│   └── grpc.go
├── storage/
└── tests/
```

### Mapping from old to new

| Old (`agent/`) | New (`server/`) |
|---|---|
| `services/*.go` (handlers) | `app/http/controllers/` |
| `services/*.go` (actions) | `app/actions/` |
| `auth/` | `app/http/controllers/auth/` + `app/actions/auth/` |
| `cluster/` | `app/services/cluster/` + `app/grpc/` |
| `metrics/` | `app/actions/metrics/` + `app/http/controllers/metrics/` |
| `storage/` | `app/models/` + `database/migrations/` |
| `config/` | `config/` + `.env` |
| `server/` | `routes/` + `bootstrap/` |

## 2. gRPC Internal API

Leader-follower communication moves from HTTP + `X-Kontainer-Secret` header to gRPC with interceptor-based auth.

```protobuf
syntax = "proto3";
package agent;

service AgentService {
  rpc Health(HealthRequest) returns (HealthResponse);
  rpc Info(InfoRequest) returns (InfoResponse);
  rpc Metrics(MetricsRequest) returns (MetricsResponse);
  rpc ListImages(ListImagesRequest) returns (ListImagesResponse);
  rpc RemoveImage(RemoveImageRequest) returns (RemoveImageResponse);
  rpc PruneImages(PruneImagesRequest) returns (PruneImagesResponse);
  rpc Exec(stream ExecMessage) returns (stream ExecMessage);
}
```

- Auth: Shared secret validated via gRPC unary/stream interceptor on metadata.
- Discovery: Leader discovers followers via Swarm task IPs, calls `AgentService.Health()` via gRPC instead of HTTP ping.
- Registry: Stores gRPC connection handles instead of HTTP addresses.
- Exec: Bidirectional gRPC stream replaces WebSocket proxying between leader and follower.

External API (`/api/*`, `/auth/*`, `/setup/*`) and browser-facing WebSocket endpoints remain HTTP/WS.

## 3. Configuration

YAML config (`kontainer.yml`) replaced by Goravel's `.env` + `config/*.go`:

```
APP_PORT=8080
AGENT_URL=https://...
AGENT_ROLE=leader
APP_DEBUG=false
CLUSTER_SECRET=...
CLUSTER_FOLLOWER_SERVICE=kontainer-agent
CLUSTER_DEV_FOLLOWERS=localhost:8081,localhost:8082
METRICS_INTERVAL=30
METRICS_RETENTION=1440
DB_DATABASE=kontainer.db
```

Config files: `app.go`, `auth.go`, `database.go`, `grpc.go`, `cluster.go` (custom), `metrics.go` (custom).

Key changes:
- Two SQLite DBs merged into one (`kontainer.db`).
- `file:` prefix secret resolution replaced by `*_FILE` env var convention for Docker Swarm secrets.
- `gopkg.in/yaml.v3` dependency removed.

## 4. Database & Migrations

Single `kontainer.db` via Goravel ORM (SQLite, swappable to Postgres via config).

### Models

**User:** `id`, `provider`, `provider_id`, `name`, `email`, `avatar_url` — created/updated on first OAuth login. Required by Goravel's `facades.Auth()` for `LoginUsingID()` and `User()` calls. The current agent embeds all user claims directly in the JWT with no persistence; Goravel's auth guard requires a backing model.

**AppConfig:** `key` (PK), `value` — stores setup credentials, JWT secret.

**NodeMetric:** `id`, `timestamp` (indexed), `node_id` (indexed), `hostname`, `cpus`, `cpu_pct`, `mem_used`, `mem_total`, `mem_pct`, `disk_used`, `disk_total`, `disk_pct` — time-series node metrics.

**Setting:** `key` (PK), `value` — user preferences.

### 4 migrations

1. `create_users_table`
2. `create_app_configs_table`
3. `create_node_metrics_table` (with indexes on timestamp, node_id)
4. `create_settings_table`

Retention pruning via scheduled task (every 5 minutes) instead of manual SQL in goroutine.

## 5. Authentication & Middleware

OAuth flow logic preserved, built on Goravel's JWT guard:

- **User model**: New `User` model (id, provider, provider_id, name, email, avatar_url). On OAuth callback, upsert user by provider + provider_id, then issue token via `facades.Auth(ctx).LoginUsingID(user.ID)`. This is the key architectural change — the current agent is stateless (no user table), but Goravel's auth facade requires a backing model to resolve users from tokens.
- **Token issuance**: `facades.Auth(ctx).LoginUsingID(userID)` replaces manual HS256 signing.
- **Token validation**: Goravel JWT middleware on `/api/*` group replaces custom `JWTMiddleware`.
- **User context**: `facades.Auth(ctx).User(&user)` replaces `ctx.Value(auth.ClaimsKey)`. Returns full User model with name, email, avatar_url.
- **`/api/me` endpoint**: Returns authenticated user's profile from `facades.Auth(ctx).User()`. Included in PR 3.
- **TTL**: 24h, configured in `config/jwt.go`.
- **OAuth state store**: Custom in-memory `SessionStore` that binds OAuth `state` parameter to AuthBridge session IDs for CSRF protection. Re-implemented as a service (not provided by Goravel). Flow: `AuthListenHandler` creates bridge session → `RedirectHandler` generates state token bound to session → `CallbackHandler` validates state to recover session ID.
- **AuthBridge**: Stays as custom WebSocket handler (Goravel has no WS-to-OAuth bridge).

### Setup mode

SwappableHandler replaced by global middleware checking `AppConfig.IsSetupComplete()`:
- Not complete: only setup + health routes pass, everything else returns `{"status": "setup_required"}`.
- Complete: middleware reads updated DB value, all routes accessible.
- No router swapping, no goroutine restart.

### Middleware stack

```
Global: Recovery, Logger, CORS (all Goravel built-in)
/api/*: JWT guard middleware
/setup/*: SetupToken middleware (custom)
/auth/*: No auth (public)
gRPC: SecretAuth interceptor
```

## 6. Docker Abstraction Layer

Orchestrator interface enables future Kubernetes support:

```go
type Orchestrator interface {
    ListServices(ctx, filters) ([]Service, error)
    InspectService(ctx, id) (*ServiceDetail, error)
    ScaleService(ctx, id, replicas) error
    RestartService(ctx, id) error
    RollbackService(ctx, id) error
    UpdateServiceEnv(ctx, id, env) error
    UpdateServiceImage(ctx, id, image) error
    ListTasks(ctx, filters) ([]Task, error)
    InspectTask(ctx, id) (*TaskDetail, error)
    TaskStats(ctx) ([]ContainerStats, error)
    ListNodes(ctx) ([]Node, error)
    UpdateNodeAvailability(ctx, id, avail) error
    UpdateNodeRole(ctx, id, role) error
    ListNetworks(ctx) ([]Network, error)
    ListVolumes(ctx) ([]Volume, error)
    ListSecrets(ctx) ([]Secret, error)
    CreateSecret(ctx, name, data) error
    RemoveSecret(ctx, id) error
    ListConfigs(ctx) ([]Config, error)
    CreateConfig(ctx, name, data) error
    RemoveConfig(ctx, id) error
    InspectConfig(ctx, id) (*ConfigDetail, error)
    ListStacks(ctx) ([]Stack, error)
    RemoveStack(ctx, name) error
    ListImages(ctx) ([]Image, error)
    RemoveImage(ctx, id, force) error
    PruneImages(ctx) error
    StreamServiceLogs(ctx, id, opts) (LogStream, error)
    StreamTaskLogs(ctx, id, opts) (LogStream, error)
    ExecContainer(ctx, id, opts) (ExecSession, error)
}
```

Implementations: `docker/swarm/` (now), `docker/kubernetes/` (future).

Registered via Goravel service provider as singleton. Actions depend on the interface, not the implementation.

## 7. Controllers, Actions & Routes

Thin controllers (parse request, call action, return response). Actions hold business logic with no HTTP awareness.

One controller per action, one file per controller. Goravel request validation replaces ad-hoc parsing.

### Route files

- `routes/api.go` — all `/api/*` routes, JWT middleware group
- `routes/auth.go` — public `/auth/*` routes
- `routes/setup.go` — `/setup/*` routes, setup token middleware
- `routes/grpc.go` — gRPC service registration

Full route list: 40+ endpoints matching current API surface.

## 8. WebSocket Endpoints

Three WebSocket endpoints stay as raw `net/http` handlers (Goravel has no native WS support):

- `/api/services/{serviceID}/logs` — Docker mux stream parsing, JSON frames to browser
- `/api/tasks/{taskID}/logs` — same as service logs
- `/api/exec/{containerID}` — bidirectional binary/text frames for terminal I/O

`gorilla/websocket` remains as a dependency.

Exec flow change: browser → WS → leader → **gRPC stream** → follower → Docker exec (replaces WS proxy between leader/follower).

## 9. Background Workers & Scheduling

Raw goroutines replaced by Goravel scheduled tasks:

| Task | Schedule | Replaces |
|---|---|---|
| `metrics:collect` | Every 30s | Metrics collector goroutine |
| `metrics:prune` | Every 5min | Manual prune every 10 ticks |
| `cluster:discover` | Every 10s (prod) / 5s (dev) | Discovery goroutine |

Benefits over goroutines:
- `SkipIfStillRunning()` prevents pile-up
- `./artisan schedule:list` for visibility
- No manual ticker/context management

Note: `OnOneServer()` is available for future multi-leader setups but requires a shared cache backend (Redis/Memcached). Not used in this migration since the architecture is single-leader. Can be added later when a cache driver is introduced.

Follower mode: gRPC server only, no scheduled tasks, no HTTP API routes.

**Startup sequencing**: Scheduled tasks (metrics collection, cluster discovery) are registered unconditionally at boot but are safe as no-ops before setup is complete — discovery finds no followers without cluster config, and metrics collection runs locally regardless. No special `onComplete` callback is needed; the setup middleware controls HTTP access while background tasks run harmlessly.

**Container metrics**: The metrics collector gathers both node-level metrics (persisted to `node_metrics` table) and container-level metrics (CPU/memory per container via Docker stats API). Container metrics are ephemeral — collected in real-time during each fan-out cycle and included in the `/api/metrics` response alongside stored node history, but not persisted to the database. This matches the current behavior.

Future queue jobs (infrastructure ready, not built in this migration):
- `ScheduledDeployJob`, `SendAlertJob`, `EvaluateScalingPolicyJob`, `PruneImagesJob`

## 10. PR Breakdown

15 PRs, layer-by-layer:

| PR | Name | Scope |
|---|---|---|
| 1 | Scaffold Goravel project | `goravel new server`, `.env`, `config/*.go`, `_FILE` env helper, SQLite driver |
| 2 | Database models & migrations | 4 migrations (users, app_configs, node_metrics, settings), 4 models |
| 3 | Auth system | JWT guard, User model, OAuth provider, GitHub provider, OAuth state store, AuthBridge WS, `/auth/*` routes, `/api/me`, JWT middleware |
| 4 | Setup flow | Setup mode middleware, setup controllers, GitHub App manifest, `/setup/*` routes |
| 5 | Docker orchestrator interface | `Orchestrator` interface, domain types, Swarm implementation, Docker service provider |
| 6 | gRPC internal API | Proto, codegen, follower gRPC controllers, secret auth interceptor, follower bootstrap |
| 7 | Cluster management | Registry, Swarm discovery (scheduled), dev discovery, agent listing, gRPC client pool |
| 8 | Service & Task controllers | Services CRUD, tasks index/show/stats, validation |
| 9 | Node, Stack, Network, Volume controllers | Nodes, stacks, networks, volumes controllers |
| 10 | Secrets & Configs controllers | Secrets CRUD, Configs CRUD, validation |
| 11 | WebSocket endpoints | Log streaming, container exec (WS → gRPC via cluster client pool), gorilla/websocket |
| 12 | Metrics system | Collector action, gRPC fan-out, scheduled tasks, `/api/metrics` controllers |
| 13 | Images & Settings | Cluster image aggregation, prune/remove, settings CRUD |
| 14 | Health, Ping & cleanup | Health/ping endpoints, role-based bootstrap, remove `/agent` |
| 15 | Frontend adaptation | Update Tauri commands, TypeScript types, end-to-end testing |

### Dependency graph

```
PR1 (scaffold)
├── PR2 (database) → PR3 (auth) → PR4 (setup)
├── PR5 (orchestrator)
│   ├── PR6 (gRPC) → PR7 (cluster) → PR12 (metrics)
│   │                              → PR13 (images/settings)
│   ├── PR8 (services/tasks)
│   ├── PR9 (nodes/stacks/networks/volumes)
│   └── PR10 (secrets/configs)
├── PR11 (websocket) ← PR7, PR8
└── PR14 (cleanup) ← all
    └── PR15 (frontend) ← PR14
```

PRs 5-10 can be worked in parallel after PR1 lands.

## Dependencies

### Kept
- `github.com/docker/docker` — Docker SDK
- `github.com/gorilla/websocket` — WebSocket (browser-facing)
- `github.com/shirou/gopsutil/v4` — System metrics
- `golang.org/x/oauth2` — OAuth 2.0

### Removed
- `github.com/go-chi/chi/v5` — replaced by Goravel routing
- `github.com/golang-jwt/jwt/v5` — replaced by Goravel auth
- `gopkg.in/yaml.v3` — replaced by Goravel config
- `modernc.org/sqlite` — replaced by Goravel ORM (which uses its own SQLite driver)

### Added
- `github.com/goravel/framework` — Goravel core
- `google.golang.org/grpc` + `google.golang.org/protobuf` — gRPC (via Goravel's gRPC support)

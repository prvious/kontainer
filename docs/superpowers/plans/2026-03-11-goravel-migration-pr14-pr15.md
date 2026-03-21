# Goravel Agent Migration: PR 14 (Health, Ping & Cleanup) + PR 15 (Frontend Adaptation)

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add health/ping endpoints, implement role-based bootstrap (leader runs HTTP + schedules, follower runs gRPC server only), remove the old `/agent` directory, and adapt the frontend (Tauri commands + TypeScript types) to work with the new `/server` agent.

**Architecture:** Role-based bootstrap uses Goravel's Runner interface: the LeaderRunner starts the HTTP server + scheduled tasks, the FollowerRunner starts only the gRPC server. The `main.go` resolves the runner from config. Health and ping are public endpoints (no JWT). The frontend changes are minimal since the API surface is preserved — mainly updating Tauri Rust commands if any request/response shapes changed, and end-to-end testing.

**Tech Stack:** Goravel framework (Runner interface, bootstrap), Tauri (Rust), React + TypeScript

**Spec:** `docs/superpowers/specs/2026-03-11-goravel-agent-migration-design.md`

**Prerequisites:** All PRs 1-13

---

## Chunk 1: PR 14 — Health, Ping & Role-Based Bootstrap

### File Structure

```
server/app/http/controllers/
├── health_controller.go       # GET /health
└── ping_controller.go         # GET /ping

server/bootstrap/
├── app.go                     # Main bootstrap (existing from PR 1)
└── runners.go                 # Leader + Follower runner implementations

server/routes/
├── api.go                     # (update) remove setup-only restrictions
└── grpc.go                    # (already exists from PR 6)
```

### Task 1: Health Controller

Simple health check endpoint. Returns `{"status": "ok"}`. Public, no auth.

**Files:**
- Create: `server/app/http/controllers/health_controller.go`

- [ ] **Step 1: Create health controller**

```go
package controllers

import (
	"github.com/goravel/framework/contracts/http"
)

type HealthController struct{}

func NewHealthController() *HealthController {
	return &HealthController{}
}

func (c *HealthController) Handle(ctx http.Context) http.Response {
	return ctx.Response().Json(200, map[string]string{"status": "ok"})
}
```

- [ ] **Step 2: Register route**

In `server/routes/api.go` or a dedicated public routes file, add outside any middleware group:

```go
router.Get("/health", healthCtrl.Handle)
```

- [ ] **Step 3: Commit**

```bash
cd server && git add app/http/controllers/health_controller.go
git commit -m "feat: add health check endpoint"
```

### Task 2: Ping Controller

Application-specific ping. Returns `{"app": "dev.kontainer.app", "status": "ok"}`. Public, no auth.

**Files:**
- Create: `server/app/http/controllers/ping_controller.go`

- [ ] **Step 1: Create ping controller**

```go
package controllers

import (
	"github.com/goravel/framework/contracts/http"
)

type PingController struct{}

func NewPingController() *PingController {
	return &PingController{}
}

func (c *PingController) Handle(ctx http.Context) http.Response {
	return ctx.Response().Json(200, map[string]string{
		"app":    "dev.kontainer.app",
		"status": "ok",
	})
}
```

- [ ] **Step 2: Register route**

```go
router.Get("/ping", pingCtrl.Handle)
```

- [ ] **Step 3: Commit**

```bash
cd server && git add app/http/controllers/ping_controller.go
git commit -m "feat: add ping endpoint"
```

### Task 3: Role-Based Bootstrap

Implements the core bootstrap logic that determines what servers to start based on `AGENT_ROLE` config.

**Leader mode:** HTTP server + gRPC server + scheduled tasks (discovery, metrics).
**Follower mode:** gRPC server only (no HTTP, no scheduled tasks, no JWT).

This uses Goravel's idiomatic Runner interface. Servers (HTTP, gRPC, Schedule) are started via Runners registered in service providers. Each Runner's `ShouldRun()` method controls whether it starts based on role. The `main.go` only calls `bootstrap.Boot()` — Goravel handles the rest.

**Files:**
- Modify: `server/bootstrap/app.go`
- Create: `server/bootstrap/runners.go`
- Modify: `server/main.go`

- [ ] **Step 1: Update bootstrap/app.go with routing and gRPC interceptors**

```go
// In bootstrap/app.go
package bootstrap

import (
	"github.com/goravel/framework/foundation"
	contractsfoundation "github.com/goravel/framework/contracts/foundation"
	"google.golang.org/grpc"

	"github.com/prvious/kontainer/server/config"
	"github.com/prvious/kontainer/server/routes"
	"github.com/prvious/kontainer/server/app/grpc/interceptors"
)

func Boot() contractsfoundation.Application {
	return foundation.Setup().
		WithProviders(Providers).
		WithConfig(config.Boot).
		WithRouting(func() {
			routes.Auth()
			routes.Api()
			routes.Setup()
			routes.Grpc() // registers gRPC service on facades.Grpc().Server()
		}).
		WithGrpcServerInterceptors(func() []grpc.UnaryServerInterceptor {
			return []grpc.UnaryServerInterceptor{
				interceptors.SharedSecret,
			}
		}).
		Create()
}
```

- [ ] **Step 2: Create runner implementations**

The Runner approach uses `ShouldRun()` to conditionally start services based on role. Goravel's built-in HTTP RouteRunner and gRPC runner are configured via these custom runners.

- The **gRPC runner** always runs (both leader and follower need it).
- The **HTTP RouteRunner** only runs on leader (controlled via `ShouldRun()`).
- The **Schedule runner** only runs on leader.

```go
package bootstrap

import (
	"os"

	"github.com/goravel/framework/facades"
)

// HttpRunner conditionally starts the HTTP server (leader only).
// Note: Uses os.Getenv("AGENT_ROLE") because ShouldRun() may be called
// before facades are bootstrapped. At runtime, this maps to config key
// "cluster.role" (from config/cluster.go).
type HttpRunner struct{}

func (r *HttpRunner) ShouldRun() bool {
	role := os.Getenv("AGENT_ROLE")
	if role == "" {
		role = "leader"
	}
	return role == "leader"
}

func (r *HttpRunner) Run() error {
	facades.Log().Infof("Starting HTTP server on :%s", facades.Config().GetString("app.port", "8080"))
	return facades.Route().Run()
}

func (r *HttpRunner) Shutdown() error {
	facades.Log().Info("HTTP server shutting down")

	// Close gRPC client pool (leader uses this for follower communication)
	pool := facades.App().Make("cluster.pool")
	if pool != nil {
		pool.(*cluster.GrpcPool).CloseAll()
	}

	return nil
}

// ScheduleRunner conditionally starts scheduled tasks (leader only).
type ScheduleRunner struct{}

func (r *ScheduleRunner) ShouldRun() bool {
	role := os.Getenv("AGENT_ROLE")
	if role == "" {
		role = "leader"
	}
	return role == "leader"
}

func (r *ScheduleRunner) Run() error {
	facades.Log().Info("Starting scheduler")
	go facades.Schedule().Run()
	return nil
}

func (r *ScheduleRunner) Shutdown() error {
	return nil
}
```

**Note:** The gRPC server runner is already provided by the `GrpcServiceProvider` (from PR 6) and always runs for both roles. No custom runner is needed for gRPC — it is started automatically when `routes.Grpc()` is registered via `WithRouting`.

- [ ] **Step 3: Update main.go to be minimal**

With the Runner pattern, `main.go` does not contain any conditional startup logic. Goravel handles everything via the runners registered in service providers.

```go
package main

import "github.com/prvious/kontainer/server/bootstrap"

func main() {
	bootstrap.Boot()
}
```

- [ ] **Step 4: Commit**

```bash
cd server && git add main.go bootstrap/app.go bootstrap/runners.go
git commit -m "feat: implement role-based bootstrap using Goravel Runner pattern"
```

### Task 4: Conditional Provider Registration

Followers don't need auth, API, or scheduling providers. Optimize by conditionally registering providers based on role.

> **Warning:** `facades.Config()` is NOT available during `init()` or in functions called at init time (e.g., provider registration), because facades are not bootstrapped yet. Use `os.Getenv()` for any role checks during provider registration. Alternatively, register all providers unconditionally and let each provider's `Runner.ShouldRun()` handle the conditional logic (preferred approach).

**Files:**
- Modify: `server/config/app.go`

- [ ] **Step 1: Update provider list**

```go
import "os"

func providers() []foundation.ServiceProvider {
    // Use os.Getenv() for role check during provider registration.
    // facades.Config() is NOT available during init().
    role := os.Getenv("AGENT_ROLE")
    if role == "" {
        role = "leader"
    }

    // Common providers (both leader and follower)
    common := []foundation.ServiceProvider{
        &providers.DockerServiceProvider{},
        &providers.GrpcServiceProvider{}, // gRPC server for followers, client for leaders
    }

    if role == "leader" {
        return append(common,
            &providers.AuthServiceProvider{},
            &providers.ClusterServiceProvider{},
            &providers.MetricsServiceProvider{},
            &providers.ApiServiceProvider{},
        )
    }

    // Follower: minimal providers
    return common
}
```

> **Alternative (preferred):** Register all providers unconditionally, and let each provider's `Runner.ShouldRun()` decide whether to actually start. This avoids the `os.Getenv()` workaround entirely and keeps provider registration simple.

- [ ] **Step 2: Commit**

```bash
cd server && git add config/app.go
git commit -m "feat: conditionally register providers based on agent role"
```

### Task 5: Verification (Before Removing Old Agent)

Verify the new `/server` agent has full feature parity with the old `/agent` before deleting anything.

- [ ] **Step 1: Verify server builds**

```bash
cd server && go build ./...
```

- [ ] **Step 2: Verify no dangling imports**

```bash
cd server && go vet ./...
```

- [ ] **Step 3: Run server tests**

```bash
cd server && go test ./...
```

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "chore: verify new server builds and passes all checks"
```

### Task 6: Remove Old Agent Directory

**Important:** Only remove `/agent` AFTER all end-to-end verification steps in Task 5 pass. The verification must confirm feature parity before the old code is deleted.

**Files:**
- Delete: `/agent/` (entire directory)

- [ ] **Step 1: Remove the old agent**

```bash
rm -rf agent/
```

- [ ] **Step 2: Update root `go.work` or module references if any**

Check if there's a `go.work` file or any root-level references to `agent/`:

```bash
grep -r "agent/" go.work pnpm-workspace.yaml package.json 2>/dev/null
```

Update any build scripts in `package.json`:

```json
{
  "scripts": {
    "dev:agent": "cd server && go run .",
    "build:agent": "cd server && go build -o bin/agent ."
  }
}
```

- [ ] **Step 3: Update CLAUDE.md**

Update the repository structure section and development commands to reference `/server` instead of `/agent`.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: complete PR 14 — health, ping, role-based bootstrap, cleanup"
```

---

## Chunk 2: PR 15 — Frontend Adaptation

### Overview

The frontend (Tauri + React) communicates with the agent via Tauri IPC commands that make HTTP requests to the agent. Since the new Goravel agent preserves the same API surface (same routes, same JSON shapes), most frontend code should work unchanged. This PR focuses on:

1. Verifying all Tauri commands still work
2. Updating any TypeScript types that changed
3. Updating Tauri Rust commands if response shapes differ
4. End-to-end testing

### Task 7: Audit API Response Compatibility

Compare old and new API response shapes.

- [ ] **Step 1: List all Tauri commands and their expected response types**

```bash
grep -r "invoke(" app/src/ --include="*.ts" --include="*.tsx" | grep -oP '"[a-z_]+"' | sort -u
```

- [ ] **Step 2: Compare response types**

For each command, verify the Rust action's expected response shape matches the new Goravel controller's output. Key areas to check:

| Command | Endpoint | Potential Change |
|---|---|---|
| `list_services` | GET /api/services | Verify `ServiceItem` fields match |
| `inspect_service` | GET /api/services/{id} | Verify `ServiceDetail` fields match |
| `list_nodes` | GET /api/nodes | Verify `NodeItem` fields match |
| `get_metrics` | GET /api/metrics | Verify `ClusterMetricsResponse` matches |
| `list_images` | GET /api/images | Verify `ClusterImagesResponse` matches |

The domain types defined in the Orchestrator interface (PR 5) should produce the same JSON as the old handlers. Any differences need to be resolved here.

- [ ] **Step 3: Document any breaking changes**

If any field names, types, or structures differ, document them for the Tauri command updates.

### Task 8: Update Tauri Commands (if needed)

If any API response shapes changed, update the corresponding Rust structs in `app/src-tauri/src/actions/`.

**Files:**
- Modify: `app/src-tauri/src/actions/*/` (as needed)
- Modify: `app/src/types/*.ts` (as needed)

- [ ] **Step 1: Update Rust response structs**

For each changed response shape, update the `#[derive(Deserialize)]` struct:

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceItem {
    pub id: String,
    pub name: String,
    pub image: String,
    pub stack: String,
    pub replicas_running: i32,
    pub replicas_desired: i32,
    pub updated_at: String,
}
```

Ensure `serde(rename_all = "camelCase")` matches the Go JSON tags.

- [ ] **Step 2: Update TypeScript types**

Match any struct changes in `app/src/types/*.ts`.

- [ ] **Step 3: Commit**

```bash
git add app/src-tauri/src/actions/ app/src/types/
git commit -m "fix: update Tauri types for new agent response shapes"
```

### Task 9: Update Build Configuration

Update the root `package.json` and any build scripts to use `/server` instead of `/agent`.

**Files:**
- Modify: `package.json` (root)

- [ ] **Step 1: Update scripts**

```json
{
  "scripts": {
    "dev": "concurrently \"pnpm dev:app\" \"pnpm dev:agent\"",
    "dev:agent": "cd server && go run .",
    "build:agent": "cd server && go build -o bin/agent .",
    "dev:app": "pnpm --filter app dev"
  }
}
```

- [ ] **Step 2: Commit**

```bash
git add package.json
git commit -m "chore: update build scripts for /server directory"
```

### Task 10: End-to-End Smoke Test

- [ ] **Step 1: Start the new agent**

```bash
cd server && go run .
```

Verify startup logs show:
- "Starting kontainer agent in leader mode"
- HTTP server listening
- Scheduler running

- [ ] **Step 2: Test health and ping**

```bash
curl http://localhost:8080/health
# Expected: {"status":"ok"}

curl http://localhost:8080/ping
# Expected: {"app":"dev.kontainer.app","status":"ok"}
```

- [ ] **Step 3: Test auth flow**

Navigate to `http://localhost:8080/auth/providers` and verify it returns available OAuth providers.

- [ ] **Step 4: Start Tauri app and verify connection**

```bash
pnpm dev:app
```

Connect to the agent and verify:
- Service listing works
- Node listing works
- Metrics display correctly
- Log streaming works
- Settings save correctly

- [ ] **Step 5: Final commit**

```bash
git add -A && git commit -m "feat: complete PR 15 — frontend adaptation and verification"
```

---

## Summary

### PR 14 Changes

| Component | Action |
|---|---|
| Health endpoint | New: `GET /health → {"status":"ok"}` |
| Ping endpoint | New: `GET /ping → {"app":"dev.kontainer.app","status":"ok"}` |
| Role-based bootstrap | Leader: HTTP + gRPC + scheduler. Follower: gRPC only |
| Conditional providers | Auth, API, Cluster, Metrics only on leader |
| Remove `/agent` | Delete entire old agent directory |
| Update build scripts | Point to `/server` instead of `/agent` |
| Update CLAUDE.md | Reflect new directory structure |

### PR 15 Changes

| Component | Action |
|---|---|
| Tauri commands | Verify/update response types if shapes changed |
| TypeScript types | Match any response shape changes |
| Build config | Update `package.json` scripts |
| E2E testing | Smoke test all major features |

### Migration Complete Checklist

After PR 15, verify:
- [ ] All 40+ API endpoints respond correctly
- [ ] Auth flow (OAuth → JWT) works end-to-end
- [ ] Setup flow creates GitHub App and stores credentials
- [ ] WebSocket log streaming works for services and tasks
- [ ] Container exec works for local and remote containers
- [ ] Metrics collection runs on schedule with pruning
- [ ] Cluster discovery finds followers (Swarm or dev mode)
- [ ] Image aggregation shows images from all nodes
- [ ] Settings persist across restarts
- [ ] Follower mode starts gRPC server only
- [ ] `/agent` directory is removed
- [ ] CLAUDE.md is updated
- [ ] All existing Vitest tests pass

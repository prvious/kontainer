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

**Leader mode:** HTTP server + gRPC client pool + scheduled tasks (discovery, metrics).
**Follower mode:** gRPC server only (no HTTP, no scheduled tasks, no JWT).

This uses Goravel's Runner interface (from service providers) and the scheduler.

**Files:**
- Create or modify: `server/main.go`
- Create: `server/bootstrap/runners.go`

- [ ] **Step 1: Create runner implementations**

```go
package bootstrap

import (
	"github.com/goravel/framework/facades"
)

// LeaderRunner starts the HTTP server and scheduled tasks.
type LeaderRunner struct{}

func (r *LeaderRunner) ShouldRun() bool {
	return facades.Config().GetString("agent.role", "leader") == "leader"
}

func (r *LeaderRunner) Run() error {
	// Start the scheduler in background
	go facades.Schedule().Run()

	facades.Log().Infof("Starting leader: HTTP on :%s", facades.Config().GetString("app.port", "8080"))

	// HTTP server is started by Goravel's default HTTP runner.
	// This runner is for additional leader-specific initialization.
	return nil
}

func (r *LeaderRunner) Shutdown() error {
	facades.Log().Info("Leader shutting down")

	// Close gRPC pool
	pool := facades.App().Make("cluster.pool")
	if pool != nil {
		pool.(*cluster.GrpcPool).CloseAll()
	}

	return nil
}

// FollowerRunner starts only the gRPC server.
type FollowerRunner struct{}

func (r *FollowerRunner) ShouldRun() bool {
	return facades.Config().GetString("agent.role") == "follower"
}

func (r *FollowerRunner) Run() error {
	facades.Log().Infof("Starting follower: gRPC on :%s", facades.Config().GetString("grpc.port", "9001"))
	// gRPC server is started by Goravel's gRPC runner via the GrpcServiceProvider.
	return nil
}

func (r *FollowerRunner) Shutdown() error {
	facades.Log().Info("Follower shutting down")
	return nil
}
```

**Note:** The actual runner pattern depends on how Goravel's bootstrap works. The key insight is:

1. Goravel's HTTP server runner is already configured by default.
2. For followers, we need to **skip** the HTTP server and only run gRPC.
3. The `GrpcServiceProvider` (from PR 6) registers the gRPC server as a Runner via `Runners()`.
4. The main bootstrap conditionally registers providers based on role.

A simpler approach for role-based bootstrap in `main.go`:

```go
func main() {
    app := bootstrap.Boot()

    role := facades.Config().GetString("agent.role", "leader")

    if role == "leader" {
        // Start scheduler
        go facades.Schedule().Run()
        // Start HTTP server (Goravel default)
        facades.Route().Run()
    } else {
        // Follower: only gRPC
        facades.Grpc().Run()
    }
}
```

- [ ] **Step 2: Update main.go with role-based startup**

```go
package main

import (
	"os"
	"os/signal"
	"syscall"

	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/bootstrap"
)

func main() {
	bootstrap.Boot()

	role := facades.Config().GetString("agent.role", "leader")
	facades.Log().Infof("Starting kontainer agent in %s mode", role)

	if role == "leader" {
		go facades.Schedule().Run()

		// Run HTTP server (blocks)
		if err := facades.Route().Run(); err != nil {
			facades.Log().Errorf("HTTP server error: %v", err)
			os.Exit(1)
		}
	} else {
		// Follower: gRPC only
		if err := facades.Grpc().Run(); err != nil {
			facades.Log().Errorf("gRPC server error: %v", err)
			os.Exit(1)
		}
	}

	// Graceful shutdown
	quit := make(chan os.Signal, 1)
	signal.Notify(quit, syscall.SIGINT, syscall.SIGTERM)
	<-quit

	facades.Log().Info("Shutting down...")
}
```

**Note:** The actual shutdown and server blocking pattern depends on Goravel's HTTP/gRPC server API. `facades.Route().Run()` likely blocks. If both HTTP and gRPC need to run on the leader, start gRPC in a goroutine:

```go
if role == "leader" {
    go facades.Grpc().Run() // gRPC for internal communication
    go facades.Schedule().Run()
    facades.Route().Run() // HTTP blocks
}
```

- [ ] **Step 3: Commit**

```bash
cd server && git add main.go bootstrap/runners.go
git commit -m "feat: implement role-based bootstrap (leader vs follower)"
```

### Task 4: Conditional Provider Registration

Followers don't need auth, API, or scheduling providers. Optimize by conditionally registering providers based on role.

**Files:**
- Modify: `server/config/app.go`

- [ ] **Step 1: Update provider list**

```go
func providers() []foundation.ServiceProvider {
    role := facades.Config().GetString("agent.role", "leader")

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

- [ ] **Step 2: Commit**

```bash
cd server && git add config/app.go
git commit -m "feat: conditionally register providers based on agent role"
```

### Task 5: Remove Old Agent Directory

Now that the new `/server` directory is complete, remove the old `/agent` directory.

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
git add -A && git commit -m "chore: remove old agent directory, update build scripts"
```

### Task 6: Final Verification

- [ ] **Step 1: Verify server builds**

```bash
cd server && go build ./...
```

- [ ] **Step 2: Verify no dangling imports**

```bash
cd server && go vet ./...
```

- [ ] **Step 3: Final commit**

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

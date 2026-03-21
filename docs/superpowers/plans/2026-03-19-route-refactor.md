# Route Refactor — Move Dependency Resolution to Service Providers

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move all `facades.App().Make()` calls out of route files into service providers, so route files only instantiate controllers with pre-resolved dependencies — matching Goravel conventions.

**Architecture:** Service providers register controller factory singletons (or pre-wired dependency bundles) during `Register()`/`Boot()`. Route files resolve these and call `New*Controller()` with zero container interaction beyond a single top-level resolve. Also bundles quick related fixes: merge duplicate route groups, extract inline 503 handlers, move setup token log, remove duplicate `os.Hostname()`.

**Tech Stack:** Go, Goravel framework, service container pattern

**Relevant skills:** `goravel-foundations`, `goravel-http`

---

## File Structure

### Modified Files
- `server/app/providers/docker_service_provider.go` — Add `docker.hostname` singleton
- `server/app/providers/cluster_service_provider.go` — No changes needed (already registers what routes need)
- `server/app/providers/metrics_service_provider.go` — No changes needed
- `server/app/providers/auth_service_provider.go` — Register setup token and log it in Boot()
- `server/routes/api.go` — Simplify: resolve singletons at top, remove builder functions, merge route groups, extract 503 handler
- `server/routes/auth.go` — Already clean pattern, just uses Make() at top (acceptable for now)
- `server/routes/grpc.go` — Already clean pattern, just uses Make() at top (acceptable for now)
- `server/routes/setup.go` — Remove log call (moved to provider)

---

### Task 1: Add `docker.hostname` singleton to DockerServiceProvider

**Files:**
- Modify: `server/app/providers/docker_service_provider.go`

- [ ] **Step 1: Add hostname singleton to Register()**

In `docker_service_provider.go`, add a new singleton inside `Register()`:

```go
app.Singleton("docker.hostname", func(app foundation.Application) (any, error) {
    hostname, err := os.Hostname()
    if err != nil {
        return "", fmt.Errorf("failed to get hostname: %w", err)
    }
    return hostname, nil
})
```

This eliminates the three separate `os.Hostname()` calls across providers and routes.

- [ ] **Step 2: Update orchestrator factory to use the hostname singleton**

In the `docker.orchestrator` factory, replace:
```go
hostname, _ := os.Hostname()
```
with:
```go
hostnameInstance, err := app.Make("docker.hostname")
if err != nil {
    return nil, fmt.Errorf("failed to resolve docker.hostname: %w", err)
}
hostname, ok := hostnameInstance.(string)
if !ok {
    return nil, fmt.Errorf("docker.hostname is not a string")
}
```

- [ ] **Step 3: Run `go vet` to verify**

Run: `go vet -C server ./...`
Expected: Clean (no output)

- [ ] **Step 4: Commit**

```
git add server/app/providers/docker_service_provider.go
git commit -m "refactor: add docker.hostname singleton, remove duplicate os.Hostname() calls"
```

---

### Task 2: Remove `os.Hostname()` from routes/api.go and use docker.hostname

**Files:**
- Modify: `server/routes/api.go`

- [ ] **Step 1: In `buildImageControllers()`, replace `os.Hostname()` with container resolve**

Replace lines 355-358:
```go
hostname, err := os.Hostname()
if err != nil {
    facades.Log().Warningf(...)
}
```
with:
```go
hostnameInstance, err := facades.App().Make("docker.hostname")
if err != nil {
    facades.Log().Warningf("image routes: failed to resolve hostname: %v", err)
    return nil, nil, nil
}
hostname, ok := hostnameInstance.(string)
if !ok {
    facades.Log().Warningf("image routes: docker.hostname is not a string")
    return nil, nil, nil
}
```

Remove the `"os"` import from api.go.

- [ ] **Step 2: Run `go vet`, commit**

```
git add server/routes/api.go
git commit -m "refactor: use docker.hostname singleton instead of os.Hostname() in api routes"
```

---

### Task 3: Extract shared unavailable handler in api.go

**Files:**
- Modify: `server/routes/api.go`

- [ ] **Step 1: Add a package-level helper function**

At the top of api.go (after imports), add:
```go
func unavailableHandler(message string) func(ctx http.Context) http.Response {
    return func(ctx http.Context) http.Response {
        return ctx.Response().Json(503, map[string]string{"error": message})
    }
}
```

- [ ] **Step 2: Replace all inline 503 closures**

Replace the multiple inline `unavailable := func(ctx http.Context) http.Response { ... }` blocks with calls to `unavailableHandler("docker orchestrator is not available on this agent")`, `unavailableHandler("metrics unavailable")`, `unavailableHandler("image management unavailable")`.

- [ ] **Step 3: Run `go vet`, commit**

```
git add server/routes/api.go
git commit -m "refactor: extract shared unavailableHandler in api routes"
```

---

### Task 4: Merge duplicate API route groups

**Files:**
- Modify: `server/routes/api.go`

- [ ] **Step 1: Move settings routes into the main JWT group**

Move the settings controller instantiation and route registration (lines 203-212) into the existing `facades.Route().Prefix("api").Middleware(middleware.JWT()).Group(...)` block. Delete the second group.

- [ ] **Step 2: Run `go vet`, commit**

```
git add server/routes/api.go
git commit -m "refactor: merge duplicate API route groups"
```

---

### Task 5: Move setup token log from routes to provider

**Files:**
- Modify: `server/routes/setup.go` — remove the log line
- Modify: `server/app/providers/auth_service_provider.go` — add setup token logging to Boot()

- [ ] **Step 1: In `routes/setup.go`, remove line 20:**

```go
facades.Log().Infof("Setup token: %s", setupToken)
```

- [ ] **Step 2: In `auth_service_provider.go` Boot(), add setup token logging:**

After the existing `r.registerOAuthProviders(app)` call, add:
```go
setupToken := helpers.GetSetupToken()
facades.Log().Infof("Setup token: %s", setupToken)
```

Import `helpers` package.

- [ ] **Step 3: Run `go vet`, commit**

```
git add server/routes/setup.go server/app/providers/auth_service_provider.go
git commit -m "refactor: move setup token log from route file to AuthServiceProvider.Boot()"
```

---

### Task 6: Replace net/http imports with contractshttp in middleware

**Files:**
- Modify: `server/app/http/middleware/jwt.go`
- Modify: `server/app/http/middleware/setup_token.go`
- Modify: `server/app/http/middleware/setup_guard.go`

- [ ] **Step 1: In each middleware file, replace `nethttp "net/http"` usage**

Replace `nethttp.StatusUnauthorized` → `contractshttp.StatusUnauthorized`, `nethttp.StatusServiceUnavailable` → `contractshttp.StatusServiceUnavailable`, etc. Remove the `nethttp "net/http"` import.

Note: Check that `contractshttp` (the already-imported `github.com/goravel/framework/contracts/http`) actually exports these constants. If not, keep `net/http`.

- [ ] **Step 2: Run `go vet`, commit**

```
git add server/app/http/middleware/
git commit -m "refactor: use Goravel contract status constants in middleware"
```

---

### Task 7: Add Commands() stub to console Kernel

**Files:**
- Modify: `server/app/console/kernel.go`

- [ ] **Step 1: Add the Commands method**

After the import block, add:
```go
func (kernel *Kernel) Commands() []console.Command {
    return []console.Command{}
}
```

Add `"github.com/goravel/framework/contracts/console"` to imports.

- [ ] **Step 2: Run `go vet`, commit**

```
git add server/app/console/kernel.go
git commit -m "refactor: add Commands() stub to console Kernel per Goravel convention"
```

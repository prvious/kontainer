# Goravel Agent Migration: PR 1 (Scaffold) + PR 2 (Database)

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Scaffold a new Goravel project at `/server` with custom config files for cluster/metrics settings, SQLite database driver, and 4 ORM migrations + models (users, app_configs, node_metrics, settings).

**Architecture:** Standard Goravel scaffold with custom config files (`cluster.go`, `metrics.go`) extending the default config set. SQLite as the database driver (CGO-free via `github.com/goravel/sqlite`). Goravel ORM models replace the current raw SQL in `agent/storage/`. A boot-time `_FILE` env var helper reads Docker Swarm secrets. Migrations are auto-registered in `bootstrap/migrations.go` when generated via `make:migration`.

**Tech Stack:** Go 1.23+, Goravel framework, SQLite (via `github.com/goravel/sqlite`), Goravel ORM

**Spec:** `docs/superpowers/specs/2026-03-11-goravel-agent-migration-design.md`

---

## Chunk 1: PR 1 — Scaffold Goravel Project

### File Structure

```
server/                           # New Goravel project
├── main.go                       # Entry point (generated)
├── go.mod                        # Go module: github.com/prvious/kontainer/server
├── go.sum
├── .env                          # Environment config (from .env.example)
├── .env.example                  # Template for env vars
├── bootstrap/
│   └── app.go                    # Goravel bootstrap (generated, we modify)
├── config/
│   ├── app.go                    # Standard Goravel config (generated)
│   ├── auth.go                   # Standard Goravel auth config (generated)
│   ├── database.go               # Database config — SQLite (modify generated)
│   ├── grpc.go                   # Standard Goravel gRPC config (generated)
│   ├── queue.go                  # Standard Goravel queue config (generated)
│   ├── cluster.go                # CUSTOM — role, secret, follower service, dev followers
│   └── metrics.go                # CUSTOM — collection interval, retention
├── app/
│   ├── providers/                # (generated, we add ours later)
│   └── helpers/
│       └── env_file.go           # _FILE env var resolver
├── database/
│   └── migrations/               # (empty, populated in PR 2)
├── routes/                       # (empty, populated in PR 3+)
├── storage/                      # Runtime storage (SQLite DB lives here)
└── tests/
    └── helpers/
        └── env_file_test.go      # Tests for _FILE env var resolver
```

---

### Task 1: Scaffold the Goravel project

**Files:**
- Create: `server/` (entire scaffold)

- [ ] **Step 1: Install the Goravel CLI installer**

```bash
go install github.com/goravel/installer/goravel@latest
```

- [ ] **Step 2: Scaffold the project**

Run from repo root:
```bash
cd /Users/clovismuneza/Apps/kontainer
goravel new server
```

This creates `server/` with the default Goravel structure.

- [ ] **Step 3: Initialize dependencies**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go mod tidy
```

- [ ] **Step 4: Generate app key**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan key:generate
```

Expected: `.env` file updated with `APP_KEY=base64:...`

- [ ] **Step 5: Generate JWT secret**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan jwt:secret
```

Expected: `.env` file updated with `JWT_SECRET=...`

- [ ] **Step 6: Verify the scaffold runs**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run .
```

Expected: Server starts on default port. Stop it with Ctrl+C.

- [ ] **Step 7: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/
git commit -m "chore: scaffold Goravel project at /server"
```

---

### Task 2: Configure `.env` and `.env.example`

**Files:**
- Modify: `server/.env`
- Modify: `server/.env.example`

- [ ] **Step 1: Update `.env.example` with all Kontainer env vars**

Replace the contents of `server/.env.example` with:

```env
APP_NAME=kontainer
APP_ENV=local
APP_KEY=
APP_DEBUG=true
APP_URL=http://localhost
APP_PORT=8080

JWT_SECRET=

DB_CONNECTION=sqlite
DB_DATABASE=storage/kontainer.db

AGENT_URL=http://localhost:8080
AGENT_ROLE=leader

CLUSTER_SECRET=
CLUSTER_FOLLOWER_SERVICE=kontainer-agent
CLUSTER_DEV_FOLLOWERS=

METRICS_INTERVAL=30
METRICS_RETENTION=1440

GRPC_PORT=9000
```

- [ ] **Step 2: Update `.env` with dev defaults**

Copy the same content to `.env`, filling in dev values:

```env
APP_NAME=kontainer
APP_ENV=local
APP_KEY=<keep generated value>
APP_DEBUG=true
APP_URL=http://localhost
APP_PORT=8080

JWT_SECRET=<keep generated value>

DB_CONNECTION=sqlite
DB_DATABASE=storage/kontainer.db

AGENT_URL=http://localhost:8080
AGENT_ROLE=leader

CLUSTER_SECRET=dev-secret
CLUSTER_FOLLOWER_SERVICE=kontainer-agent
CLUSTER_DEV_FOLLOWERS=

METRICS_INTERVAL=30
METRICS_RETENTION=1440

GRPC_PORT=9000
```

- [ ] **Step 3: Ensure `.env` is in `.gitignore`**

Check `server/.gitignore` contains `.env`. Goravel's scaffold should include this. If not, add it.

- [ ] **Step 4: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/.env.example server/.gitignore
git commit -m "chore: configure env vars for Kontainer agent"
```

---

### Task 3: Install SQLite driver and configure database

**Files:**
- Modify: `server/config/database.go`
- Modify: `server/config/app.go` (add SQLite service provider)

- [ ] **Step 1: Install the Goravel SQLite driver**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go get github.com/goravel/sqlite
```

- [ ] **Step 2: Verify the SQLite package structure**

After `go get`, inspect the installed package to confirm the import paths:

```bash
ls /Users/clovismuneza/Apps/kontainer/server/vendor/github.com/goravel/sqlite/ 2>/dev/null || go doc github.com/goravel/sqlite
```

Look for the `ServiceProvider` type and a `facades` sub-package. The import paths in Steps 2-3 below are based on the expected package structure — adjust if the actual package differs.

- [ ] **Step 3: Register the SQLite service provider**

In `server/config/app.go`, add the SQLite provider to the `providers` slice:

```go
import "github.com/goravel/sqlite"
```

Add to providers:
```go
&sqlite.ServiceProvider{},
```

- [ ] **Step 4: Configure the SQLite connection in `config/database.go`**

Find the `"connections"` map in `config/database.go`. Add or replace the sqlite entry:

```go
import (
    "github.com/goravel/framework/contracts/database/driver"
    sqlitefacades "github.com/goravel/sqlite/facades"
)
```

In the connections map:
```go
"sqlite": map[string]any{
    "driver":   "sqlite",
    "database": config.Env("DB_DATABASE", "storage/kontainer.db"),
    "prefix":   "",
    "via": func() (driver.Driver, error) {
        return sqlitefacades.Sqlite("sqlite")
    },
},
```

Set the default connection:
```go
"default": config.Env("DB_CONNECTION", "sqlite"),
```

Also configure the migration driver:
```go
"migrations": map[string]any{
    "driver": "sql",
    "table":  "migrations",
},
```

- [ ] **Step 5: Create the storage directory**

```bash
mkdir -p /Users/clovismuneza/Apps/kontainer/server/storage
touch /Users/clovismuneza/Apps/kontainer/server/storage/.gitkeep
```

- [ ] **Step 6: Verify database connectivity**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan about
```

Expected: Shows app info including database connection details.

- [ ] **Step 7: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/config/database.go server/config/app.go server/go.mod server/go.sum server/storage/.gitkeep
git commit -m "feat: configure SQLite database driver"
```

---

### Task 4: Create custom config files (cluster + metrics)

**Files:**
- Create: `server/config/cluster.go`
- Create: `server/config/metrics.go`

**Important:** Goravel's generated config files (e.g. `config/app.go`) use `init()` with `facades.Config()`. This works because the framework's config service provider bootstraps the config facade before other `init()` functions in the `config/` package are called. Custom config files in `config/` follow the same pattern — just ensure they're in the `config` package so the scaffold's blank import in `bootstrap/app.go` loads them.

- [ ] **Step 1: Inspect an existing generated config file for the pattern**

Read `server/config/app.go` to see how the scaffold uses `init()` and `facades.Config()`. Your custom files must follow the exact same pattern.

- [ ] **Step 2: Create `config/cluster.go`**

Follow the same pattern as the generated config files:

```go
package config

import (
    "github.com/goravel/framework/facades"
)

func init() {
    config := facades.Config()
    config.Add("cluster", map[string]any{
        "role":             config.Env("AGENT_ROLE", "leader"),
        "secret":           config.Env("CLUSTER_SECRET", ""),
        "follower_service": config.Env("CLUSTER_FOLLOWER_SERVICE", "kontainer-agent"),
        "dev_followers":    config.Env("CLUSTER_DEV_FOLLOWERS", ""),
        "agent_url":        config.Env("AGENT_URL", "http://localhost:8080"),
    })
}
```

- [ ] **Step 3: Create `config/metrics.go`**

```go
package config

import (
    "github.com/goravel/framework/facades"
)

func init() {
    config := facades.Config()
    config.Add("metrics", map[string]any{
        "interval":  config.Env("METRICS_INTERVAL", 30),
        "retention": config.Env("METRICS_RETENTION", 1440),
    })
}
```

- [ ] **Step 4: Verify config loads**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan about
```

Expected: No errors. App starts and shows config. If it panics with nil pointer on `facades.Config()`, check that `bootstrap/app.go` imports the `config` package.

- [ ] **Step 5: Verify custom config values are accessible**

Add a temporary test in main.go or use artisan tinker if available:

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan about
```

Alternatively, add a quick `fmt.Println(facades.Config().GetString("cluster.role"))` in `main.go` after boot, run, verify it prints "leader", then remove it.

- [ ] **Step 6: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/config/cluster.go server/config/metrics.go
git commit -m "feat: add custom cluster and metrics config files"
```

---

### Task 5: Create `_FILE` env var helper

**Files:**
- Create: `server/app/helpers/env_file.go`
- Create: `server/tests/helpers/env_file_test.go`

This helper reads `*_FILE` env vars (e.g. `CLUSTER_SECRET_FILE=/run/secrets/secret`) and sets the base env var (`CLUSTER_SECRET`) to the file contents. Used for Docker Swarm secrets.

- [ ] **Step 1: Write the failing test**

Create `server/tests/helpers/env_file_test.go`:

```go
package helpers_test

import (
    "os"
    "path/filepath"
    "testing"

    "github.com/prvious/kontainer/server/app/helpers"
)

func TestResolveFileEnvVars(t *testing.T) {
    // Create a temp file with a secret value
    dir := t.TempDir()
    secretFile := filepath.Join(dir, "secret.txt")
    if err := os.WriteFile(secretFile, []byte("  my-secret-value  \n"), 0644); err != nil {
        t.Fatal(err)
    }

    // Set the _FILE env var
    t.Setenv("TEST_SECRET_FILE", secretFile)
    t.Setenv("TEST_SECRET", "")

    helpers.ResolveFileEnvVars("TEST_SECRET")

    got := os.Getenv("TEST_SECRET")
    if got != "my-secret-value" {
        t.Errorf("expected 'my-secret-value', got '%s'", got)
    }
}

func TestResolveFileEnvVars_NoFileVar(t *testing.T) {
    // When no _FILE var is set, the original value is preserved
    t.Setenv("TEST_VAR", "original")

    helpers.ResolveFileEnvVars("TEST_VAR")

    got := os.Getenv("TEST_VAR")
    if got != "original" {
        t.Errorf("expected 'original', got '%s'", got)
    }
}

func TestResolveFileEnvVars_FileNotFound(t *testing.T) {
    t.Setenv("TEST_MISSING_FILE", "/nonexistent/path")
    t.Setenv("TEST_MISSING", "fallback")

    helpers.ResolveFileEnvVars("TEST_MISSING")

    // Should keep original value when file doesn't exist
    got := os.Getenv("TEST_MISSING")
    if got != "fallback" {
        t.Errorf("expected 'fallback', got '%s'", got)
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go test ./tests/helpers/ -v
```

Expected: FAIL — `helpers` package not found.

- [ ] **Step 3: Write the implementation**

Create `server/app/helpers/env_file.go`:

```go
package helpers

import (
    "os"
    "strings"
)

// ResolveFileEnvVars checks if a _FILE suffixed env var exists for each
// given key. If found, reads the file and sets the base env var to its
// trimmed contents. Used for Docker Swarm secrets mounted as files.
func ResolveFileEnvVars(keys ...string) {
    for _, key := range keys {
        fileKey := key + "_FILE"
        filePath := os.Getenv(fileKey)
        if filePath == "" {
            continue
        }

        data, err := os.ReadFile(filePath)
        if err != nil {
            continue
        }

        os.Setenv(key, strings.TrimSpace(string(data)))
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go test ./tests/helpers/ -v
```

Expected: 3 tests PASS.

- [ ] **Step 5: Wire up the helper in `main.go`**

At the top of `server/main.go`, before Goravel boots, add:

```go
import "github.com/prvious/kontainer/server/app/helpers"

func main() {
    // Resolve Docker Swarm secrets mounted as files
    helpers.ResolveFileEnvVars(
        "CLUSTER_SECRET",
        "JWT_SECRET",
        "DB_DATABASE",
    )

    // ... existing Goravel boot code
}
```

- [ ] **Step 6: Run full app to verify no regressions**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run .
```

Expected: Starts without errors. Stop with Ctrl+C.

- [ ] **Step 7: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/helpers/env_file.go server/tests/helpers/env_file_test.go server/main.go
git commit -m "feat: add _FILE env var resolver for Docker Swarm secrets"
```

---

### Task 6: Clean up scaffold boilerplate

**Files:**
- Modify: Various generated files (remove unused boilerplate routes, controllers, etc.)

- [ ] **Step 1: Remove default example routes**

Clear any example routes in `server/routes/` (e.g. `web.go` welcome route). Keep the file structure but with empty route definitions.

- [ ] **Step 2: Remove default example controllers**

Remove any example/welcome controller in `server/app/http/controllers/`.

- [ ] **Step 3: Verify app still boots clean**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run .
```

Expected: Starts on port 8080, no routes registered (or just health).

- [ ] **Step 4: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/
git commit -m "chore: clean up Goravel scaffold boilerplate"
```

---

### Task 7: Update root pnpm workspace and dev scripts

**Files:**
- Modify: `pnpm-workspace.yaml` (if applicable)
- Modify: `package.json` (root — update dev:agent script)

- [ ] **Step 1: Update root `package.json` dev:agent script**

The current scripts use `air` for hot-reload:

```json
"dev:agent": "cd agent && air",
"build:agent": "cd agent && go build -o bin/agent ."
```

Update to point to new server directory, keeping `air` for hot-reload:

```json
"dev:agent": "cd server && air",
"build:agent": "cd server && go build -o bin/agent ."
```

- [ ] **Step 2: Copy and adapt `.air.toml` for the new server**

Copy the `air` config from `agent/` to `server/`:

```bash
cp /Users/clovismuneza/Apps/kontainer/agent/.air.toml /Users/clovismuneza/Apps/kontainer/server/.air.toml
```

Update paths in the copied `.air.toml` if any are agent-specific (e.g. binary output dir).

- [ ] **Step 3: Verify `pnpm dev:agent` works**

```bash
cd /Users/clovismuneza/Apps/kontainer
pnpm dev:agent
```

Expected: Goravel server starts on port 8080 with hot-reload.

- [ ] **Step 4: Update reset script path**

In `package.json`, update the `reset` script to reference the new server directory for DB cleanup:

```json
"reset": "./dev/teardown.sh && rm -f server/storage/*.db server/storage/*.db-wal server/storage/*.db-shm"
```

- [ ] **Step 5: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add package.json server/.air.toml
git commit -m "chore: update dev scripts to use /server (Goravel)"
```

---

**PR 1 Complete.** Create PR:

```bash
git push -u origin feat/goravel-scaffold
gh pr create --title "feat: scaffold Goravel project at /server" --body "$(cat <<'EOF'
## Summary
- Scaffold new Goravel project at `/server` (old `/agent` stays as reference)
- Configure SQLite database driver (CGO-free)
- Add custom config files for cluster and metrics settings
- Add `_FILE` env var resolver for Docker Swarm secrets
- Clean up boilerplate, update dev scripts

## Test plan
- [ ] `go run . artisan about` shows correct config
- [ ] `go test ./tests/helpers/ -v` passes (3 tests)
- [ ] `pnpm dev:agent` starts Goravel server on port 8080
EOF
)"
```

---

## Chunk 2: PR 2 — Database Models & Migrations

**Critical:** Every migration generated via `go run . artisan make:migration` is auto-registered in `bootstrap/migrations.go`. The generated file adds the migration struct to the `Migrations()` function which is called by `WithMigrations` in `bootstrap/app.go`. **Always commit `bootstrap/migrations.go` alongside new migration files.** If you create a migration manually without the artisan command, you must register it yourself in `bootstrap/migrations.go`.

### File Structure

```
server/
├── app/models/
│   ├── user.go                   # User model (OAuth profiles)
│   ├── app_config.go             # Key-value app configuration
│   ├── node_metric.go            # Time-series node metrics
│   └── setting.go                # Key-value user settings
├── bootstrap/
│   └── migrations.go             # Auto-updated by make:migration (register all migrations)
└── database/migrations/
    ├── <timestamp>_create_users_table.go
    ├── <timestamp>_create_app_configs_table.go
    ├── <timestamp>_create_node_metrics_table.go
    └── <timestamp>_create_settings_table.go
```

---

### Task 8: Create User model and migration

**Files:**
- Create: `server/app/models/user.go`
- Create: `server/database/migrations/<timestamp>_create_users_table.go` (generated)
- Modify: `server/bootstrap/migrations.go` (auto-updated by `make:migration`)

**Important:** Goravel migrations use a struct with a `Signature()` method. The `make:migration` command generates the file AND auto-registers it in `bootstrap/migrations.go`. The struct name follows `M<timestamp>CreateUsersTable`.

- [ ] **Step 1: Generate the migration**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan make:migration create_users_table
```

Expected: Creates a file in `database/migrations/` AND updates `bootstrap/migrations.go` with the new migration.

- [ ] **Step 2: Define the migration schema**

Edit the generated migration file. The struct will look like `M<timestamp>CreateUsersTable`. Update the `Up` method:

```go
func (r *M<timestamp>CreateUsersTable) Up() error {
    if !facades.Schema().HasTable("users") {
        return facades.Schema().Create("users", func(table schema.Blueprint) {
            table.ID()
            table.String("provider")
            table.String("provider_id")
            table.String("name")
            table.String("email").Nullable()
            table.String("avatar_url").Nullable()
            table.Timestamps()

            table.Unique("provider", "provider_id")
        })
    }
    return nil
}
```

Update the `Down` method:

```go
func (r *M<timestamp>CreateUsersTable) Down() error {
    return facades.Schema().DropIfExists("users")
}
```

Note: Replace `<timestamp>` with the actual timestamp generated by the CLI. The `HasTable` guard prevents errors if run against an existing database. The unique index is defined inline in the `Create` callback.

- [ ] **Step 3: Create the User model**

Create `server/app/models/user.go`:

```go
package models

import "github.com/goravel/framework/database/orm"

type User struct {
    orm.Model
    Provider   string `gorm:"not null"`
    ProviderID string `gorm:"column:provider_id;not null"`
    Name       string `gorm:"not null"`
    Email      string
    AvatarURL  string `gorm:"column:avatar_url"`
}
```

Note: `orm.Model` already provides `ID`, `CreatedAt`, `UpdatedAt` fields. Do NOT also embed `orm.Timestamps` — that would cause duplicate fields and a compile error.

- [ ] **Step 4: Run the migration**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan migrate
```

Expected: `create_users_table` migration runs successfully.

- [ ] **Step 5: Verify the table**

```bash
sqlite3 storage/kontainer.db ".schema users"
```

Expected: Shows `id`, `provider`, `provider_id`, `name`, `email`, `avatar_url`, `created_at`, `updated_at` columns with unique index.

- [ ] **Step 6: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/models/user.go server/database/migrations/ server/bootstrap/migrations.go
git commit -m "feat: add User model and migration"
```

---

### Task 9: Create AppConfig model and migration

**Files:**
- Create: `server/app/models/app_config.go`
- Create: `server/database/migrations/<timestamp>_create_app_configs_table.go` (generated)
- Modify: `server/bootstrap/migrations.go` (auto-updated)

- [ ] **Step 1: Generate the migration**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan make:migration create_app_configs_table
```

- [ ] **Step 2: Define the migration schema**

Update the generated struct's `Up` method:

```go
func (r *M<timestamp>CreateAppConfigsTable) Up() error {
    if !facades.Schema().HasTable("app_configs") {
        return facades.Schema().Create("app_configs", func(table schema.Blueprint) {
            table.String("key")
            table.Text("value")
            // Note: table.String("key").Primary() may not compile —
            // Goravel's ColumnDefinition may not expose .Primary().
            // Use table.Primary("key") as a separate call instead.
            table.Primary("key")
        })
    }
    return nil
}
```

`Down`:

```go
func (r *M<timestamp>CreateAppConfigsTable) Down() error {
    return facades.Schema().DropIfExists("app_configs")
}
```

- [ ] **Step 3: Create the AppConfig model**

Create `server/app/models/app_config.go`:

```go
package models

type AppConfig struct {
    Key   string `gorm:"primaryKey"`
    Value string `gorm:"type:text;not null"`
}

func (r *AppConfig) TableName() string {
    return "app_configs"
}
```

No `orm.Model` embed — simple key-value table without id/timestamps.

- [ ] **Step 4: Run the migration**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan migrate
```

Expected: `create_app_configs_table` runs successfully.

- [ ] **Step 5: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/models/app_config.go server/database/migrations/ server/bootstrap/migrations.go
git commit -m "feat: add AppConfig model and migration"
```

---

### Task 10: Create NodeMetric model and migration

**Files:**
- Create: `server/app/models/node_metric.go`
- Create: `server/database/migrations/<timestamp>_create_node_metrics_table.go` (generated)
- Modify: `server/bootstrap/migrations.go` (auto-updated)

- [ ] **Step 1: Generate the migration**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan make:migration create_node_metrics_table
```

- [ ] **Step 2: Define the migration schema**

Update the generated struct's `Up` method:

```go
func (r *M<timestamp>CreateNodeMetricsTable) Up() error {
    if !facades.Schema().HasTable("node_metrics") {
        return facades.Schema().Create("node_metrics", func(table schema.Blueprint) {
            table.ID()
            table.Timestamp("timestamp")
            table.String("node_id")
            table.String("hostname")
            table.Integer("cpus")
            table.Float("cpu_pct", 2)
            table.BigInteger("mem_used")
            table.BigInteger("mem_total")
            table.Float("mem_pct", 2)
            table.BigInteger("disk_used")
            table.BigInteger("disk_total")
            table.Float("disk_pct", 2)

            table.Index("timestamp")
            table.Index("node_id")
        })
    }
    return nil
}
```

`Down`:

```go
func (r *M<timestamp>CreateNodeMetricsTable) Down() error {
    return facades.Schema().DropIfExists("node_metrics")
}
```

- [ ] **Step 3: Create the NodeMetric model**

Create `server/app/models/node_metric.go`:

```go
package models

import "time"

type NodeMetric struct {
    ID          uint      `gorm:"primaryKey"`
    Timestamp   time.Time `gorm:"not null"`
    NodeID      string    `gorm:"column:node_id;not null"`
    Hostname    string    `gorm:"not null"`
    CPUs        int       `gorm:"column:cpus"`
    CPUPercent  float64   `gorm:"column:cpu_pct"`
    MemUsed     int64     `gorm:"column:mem_used"`
    MemTotal    int64     `gorm:"column:mem_total"`
    MemPercent  float64   `gorm:"column:mem_pct"`
    DiskUsed    int64     `gorm:"column:disk_used"`
    DiskTotal   int64     `gorm:"column:disk_total"`
    DiskPercent float64   `gorm:"column:disk_pct"`
}

func (r *NodeMetric) TableName() string {
    return "node_metrics"
}
```

No `orm.Model` embed — custom `ID` and `Timestamp` fields match existing schema. No `created_at`/`updated_at` for time-series data.

- [ ] **Step 4: Run the migration**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan migrate
```

Expected: `create_node_metrics_table` runs with indexes.

- [ ] **Step 5: Verify indexes**

```bash
sqlite3 storage/kontainer.db ".indexes node_metrics"
```

Expected: Shows indexes on `timestamp` and `node_id`.

- [ ] **Step 6: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/models/node_metric.go server/database/migrations/ server/bootstrap/migrations.go
git commit -m "feat: add NodeMetric model and migration with indexes"
```

---

### Task 11: Create Setting model and migration

**Files:**
- Create: `server/app/models/setting.go`
- Create: `server/database/migrations/<timestamp>_create_settings_table.go` (generated)
- Modify: `server/bootstrap/migrations.go` (auto-updated)

- [ ] **Step 1: Generate the migration**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan make:migration create_settings_table
```

- [ ] **Step 2: Define the migration schema**

Update the generated struct's `Up` method:

```go
func (r *M<timestamp>CreateSettingsTable) Up() error {
    if !facades.Schema().HasTable("settings") {
        return facades.Schema().Create("settings", func(table schema.Blueprint) {
            table.String("key")
            table.Text("value")
            // Note: table.String("key").Primary() may not compile —
            // Goravel's ColumnDefinition may not expose .Primary().
            // Use table.Primary("key") as a separate call instead.
            table.Primary("key")
        })
    }
    return nil
}
```

`Down`:

```go
func (r *M<timestamp>CreateSettingsTable) Down() error {
    return facades.Schema().DropIfExists("settings")
}
```

- [ ] **Step 3: Create the Setting model**

Create `server/app/models/setting.go`:

```go
package models

type Setting struct {
    Key   string `gorm:"primaryKey"`
    Value string `gorm:"type:text;not null"`
}

func (r *Setting) TableName() string {
    return "settings"
}
```

- [ ] **Step 4: Run the migration**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan migrate
```

Expected: `create_settings_table` runs successfully.

- [ ] **Step 5: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/models/setting.go server/database/migrations/ server/bootstrap/migrations.go
git commit -m "feat: add Setting model and migration"
```

---

### Task 12: Verify all migrations and models end-to-end

**Files:**
- No new files

- [ ] **Step 1: Reset and re-run all migrations from scratch**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
rm -f storage/kontainer.db
go run . artisan migrate
```

Expected: All 4 migrations run in order:
1. `create_users_table`
2. `create_app_configs_table`
3. `create_node_metrics_table`
4. `create_settings_table`

- [ ] **Step 2: Verify tables exist in SQLite**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
sqlite3 storage/kontainer.db ".tables"
```

Expected output includes: `app_configs`, `migrations`, `node_metrics`, `settings`, `users`

- [ ] **Step 3: Verify table schemas**

```bash
sqlite3 storage/kontainer.db ".schema users"
sqlite3 storage/kontainer.db ".schema app_configs"
sqlite3 storage/kontainer.db ".schema node_metrics"
sqlite3 storage/kontainer.db ".schema settings"
```

Expected: Columns match model definitions. `users` has unique index on (provider, provider_id). `node_metrics` has indexes on timestamp and node_id.

- [ ] **Step 4: Test migration rollback**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan migrate:rollback
```

Expected: Last migration batch rolls back cleanly.

- [ ] **Step 5: Re-run migrations to restore state**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan migrate
```

Expected: Migrations re-apply successfully.

- [ ] **Step 6: Final commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add -A server/
git commit -m "chore: verify all migrations run cleanly"
```

---

**PR 2 Complete.** Create PR:

```bash
git push -u origin feat/goravel-database
gh pr create --title "feat: add database models and migrations" --body "$(cat <<'EOF'
## Summary
- 4 ORM models: User, AppConfig, NodeMetric, Setting
- 4 migrations with proper indexes (users unique composite, node_metrics timestamp + node_id)
- User model added for Goravel auth facade (OAuth profile persistence)
- Key-value tables (AppConfig, Setting) use string primary keys matching current schema

## Test plan
- [ ] `go run . artisan migrate` runs all 4 migrations
- [ ] `sqlite3 storage/kontainer.db ".tables"` shows all tables
- [ ] `go run . artisan migrate:rollback` rolls back cleanly
- [ ] `go run . artisan migrate` re-applies successfully
EOF
)"
```

---

## Subsequent Plans

The remaining PRs will be planned in separate documents:
- `2026-03-11-goravel-migration-pr3-pr4.md` — Auth system + Setup flow
- `2026-03-11-goravel-migration-pr5-pr6.md` — Docker orchestrator + gRPC internal API
- `2026-03-11-goravel-migration-pr7.md` — Cluster management
- `2026-03-11-goravel-migration-pr8-pr10.md` — Controllers (services, tasks, nodes, stacks, networks, volumes, secrets, configs)
- `2026-03-11-goravel-migration-pr11.md` — WebSocket endpoints
- `2026-03-11-goravel-migration-pr12-pr13.md` — Metrics + Images + Settings
- `2026-03-11-goravel-migration-pr14-pr15.md` — Cleanup + Frontend adaptation

# GitHub App Integration & Setup Wizard

## Overview

Replace the current GitHub OAuth App integration with a GitHub App using the manifest flow. Introduces a setup mode for first-time agent configuration, eliminating the need for manual GitHub credentials in `kontainer.yml`.

## Goals

- Remove `github.*`, `jwt_secret`, `allowed_users`, `allowed_orgs` from YAML config
- Auto-configure GitHub App via manifest flow during first-time setup
- Store all auth credentials in SQLite (auto-generated, not user-configured)
- Secure setup with a one-time setup token
- Seamless transition from setup to first login

## Agent State Machine

The agent operates in one of two modes, determined at boot by querying SQLite:

```
SETUP MODE                          NORMAL MODE
─────────────                       ───────────
Only /setup/* routes active         Full API routes active
Protected by setup token            Protected by JWT
/ping → setup_required              /ping → ok
All other routes → 403              /setup/* → 404
```

The transition happens once — when setup completes, the agent flips the flag in SQLite and hot-swaps the router. No restart needed.

## SQLite Schema

New table `app_config` in the existing SQLite database (same DB as metrics/settings):

```sql
CREATE TABLE IF NOT EXISTS app_config (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

Keys stored after setup:

| Key | Value |
|-----|-------|
| `setup_complete` | `"true"` |
| `github_app_id` | `"123456"` |
| `github_client_id` | `"Iv1.abc123..."` |
| `github_client_secret` | `"secret..."` |
| `github_private_key` | `"-----BEGIN RSA..."` (PEM) |
| `github_webhook_secret` | `"whsec_..."` |
| `github_installation_id` | `"78901234"` |
| `github_org` | `"myorg"` |
| `jwt_secret` | `"random-256-bit-hex"` |
| `setup_owner_github_id` | `"12345"` (user who completed setup) |

## Slimmed-Down kontainer.yml

```yaml
port: "8080"
agent_url: http://localhost:8080
role: leader

cluster:
  internal_secret: file:/run/secrets/internal_secret
  follower_service: kontainer-agent
  dev_followers: []

metrics:
  interval: 30s
  retention: 24h
  db_path: kontainer-metrics.db

dev: false
```

Removed: `jwt_secret`, entire `github:` block.

## Setup Token

- Format: `ktsetup_` + 12 random hex chars (e.g., `ktsetup_a8f3b2c1d4e5`)
- Generated fresh on every boot (only in setup mode)
- Env var override: `KONTAINER_SETUP_TOKEN` — if set, use that value instead of auto-generating

Boot banner:

```
🚀 Kontainer Agent starting...
⚡ Setup required!
🔑 Setup token: ktsetup_a8f3b2c1d4e5
📱 Open your Kontainer app and add this agent
   URL: https://agent.kontainer.dev
   Token: ktsetup_a8f3b2c1d4e5
```

Sent by desktop app as `X-Kontainer-Setup-Token` header on all `/setup/*` requests.

## Setup Routes

All behind `SetupTokenMiddleware` (except `/ping`):

```
GET  /ping                              → { "status": "setup_required" }  (no token needed)
POST /setup/github/manifest             → returns { "setupUrl": "https://agent/setup/github/start?token=..." }
GET  /setup/github/start?token=...      → serves HTML form that auto-submits manifest to GitHub
GET  /setup/github/callback?code=...    → exchanges code for app credentials, stores in SQLite
POST /setup/complete                    → validates setup, flips flag, hot-swaps router
GET  /setup/status                      → { "step": "pending|github_connected|complete" }
```

### Route Details

**`GET /ping` (modified)**

```json
// Setup mode:
{ "app": "dev.kontainer.app", "status": "setup_required" }

// Normal mode:
{ "app": "dev.kontainer.app", "status": "ok" }
```

**`POST /setup/github/manifest`**

Desktop app calls this with setup token header. Agent builds manifest JSON from its own URL:

```json
{
  "name": "Kontainer (agent.example.com)",
  "url": "https://agent.example.com",
  "callback_urls": ["https://agent.example.com/setup/github/callback"],
  "redirect_url": "https://agent.example.com/setup/github/callback",
  "setup_url": "https://agent.example.com/setup/github/callback",
  "public": false,
  "default_permissions": {
    "members": "read"
  },
  "default_events": []
}
```

Generates a random `state` token, stores it. Returns `{ "setupUrl": "https://agent/setup/github/start?token=ktsetup_..." }`.

**`GET /setup/github/start?token=...`**

Opened in system browser. Verifies setup token query param. Serves HTML page that auto-submits the manifest form to `https://github.com/settings/apps/new?state=STATE`.

**`GET /setup/github/callback?code=...&state=...`**

1. Validates `state` matches stored value
2. Calls `POST https://api.github.com/app-manifests/{code}/conversions`
3. GitHub returns: `id`, `client_id`, `client_secret`, `pem`, `webhook_secret`, `owner.login`
4. Stores all values in `app_config` table
5. Generates random JWT secret, stores it
6. Returns HTML success page: "GitHub App connected! Return to the Kontainer app."

**`POST /setup/complete`**

1. Verifies GitHub App credentials are stored
2. Sets `setup_complete = true` in `app_config`
3. Builds normal-mode router with credentials from SQLite
4. Hot-swaps the HTTP handler
5. Returns `{ "status": "ok" }`

**`GET /setup/status`**

```json
{ "step": "pending" }           // nothing done yet
{ "step": "github_connected" }  // GitHub App created, not yet completed
{ "step": "complete" }          // setup done
```

## Desktop App Setup Wizard

### Detection

`ping_agent` already calls `GET /ping`. When response contains `"status": "setup_required"`, the app shows the setup wizard instead of the login screen.

### Wizard Steps

```
Step 1: Enter Setup Token
  "This agent needs to be set up for the first time."
  [Agent URL: https://agent.example.com]  (pre-filled, read-only)
  [Setup Token: _______________]
  [Verify →]
  Calls: GET /setup/status with X-Kontainer-Setup-Token header
  → 401 = wrong token → show error
  → 200 = valid → proceed

Step 2: Connect GitHub
  "Connect a GitHub organization to manage access."
  [Connect GitHub →]
  Calls: POST /setup/github/manifest (with setup token header)
  → Opens setupUrl in system browser
  → Polls GET /setup/status every 2s
  → When step = "github_connected" → proceed

Step 3: Confirmation
  "GitHub App installed on [org-name]."
  "All members of [org-name] will be able to log in."
  [Complete Setup →]
  Calls: POST /setup/complete (with setup token header)
  → Agent hot-swaps router
  → Desktop app triggers normal OAuth login flow
  → User authorizes → JWT → dashboard
```

### Seamless First Login

After `POST /setup/complete` returns success:

1. Calls `GET /auth/providers` (now available)
2. Auto-triggers `login(agentUrl, label, "github")`
3. User sees GitHub OAuth consent → authorizes → deep link → JWT → dashboard

### Interrupted Setup Recovery

If the user returns after closing the app mid-setup:

1. `GET /ping` still returns `setup_required`
2. User enters setup token again
3. `GET /setup/status` returns current step — wizard resumes from there

### New Tauri Commands

Following action-per-file pattern in `src-tauri/src/actions/`:

- `setup_verify_token(url, token)` — `GET /setup/status` with token header
- `setup_github_connect(url, token)` — `POST /setup/github/manifest` with token header
- `setup_complete(url, token)` — `POST /setup/complete` with token header

## Agent-Side OAuth Changes

### GitHub App OAuth (replaces OAuth App)

Credentials come from SQLite instead of YAML. The OAuth flow is unchanged — redirect, code exchange, user token, org check, JWT.

Key differences:

1. **Credentials source** — SQLite `app_config` table instead of `config.GitHub.*`
2. **Access control** — org membership via installation token: `GET /orgs/{org}/members/{username}` (204 = member, 404 = not). Replaces `allowed_users` / `allowed_orgs` allowlists.
3. **Provider initialization** — built lazily from SQLite values, not from config at startup

### Router Hot-Swap

```go
type Server struct {
    mu      sync.RWMutex
    handler http.Handler
}

func (s *Server) ServeHTTP(w http.ResponseWriter, r *http.Request) {
    s.mu.RLock()
    h := s.handler
    s.mu.RUnlock()
    h.ServeHTTP(w, r)
}

func (s *Server) SwapHandler(h http.Handler) {
    s.mu.Lock()
    s.handler = h
    s.mu.Unlock()
}
```

### Boot Sequence

```go
func main() {
    cfg := config.Load(configPath)
    db  := openSQLite(cfg.Metrics.DBPath)
    appConfig := storage.NewAppConfigStore(db)

    if appConfig.IsSetupComplete() {
        // NORMAL MODE
        ghCreds := appConfig.GetGitHubCredentials()
        jwtSecret := appConfig.Get("jwt_secret")
        provider := auth.NewGitHubAppProvider(ghCreds)
        router := server.NewNormalRouter(provider, jwtSecret, ...)
        srv.SetHandler(router)
    } else {
        // SETUP MODE
        token := generateSetupToken()  // or from KONTAINER_SETUP_TOKEN
        printSetupBanner(token, cfg.AgentURL)
        router := server.NewSetupRouter(token, appConfig, cfg)
        srv.SetHandler(router)
    }

    http.ListenAndServe(":"+cfg.Port, srv)
}
```

### Code Removed

- `config.GitHub` struct and all fields
- `config.JWTSecret` field
- Secret resolution for `github.*` and `jwt_secret`
- `allowed_users` / `allowed_orgs` checking logic
- `/user/orgs` pagination loop

### Code Added

- `storage.AppConfigStore` — same pattern as `SettingsStore`, different table
- `server.NewSetupRouter()` — setup-mode route builder
- `auth.NewGitHubAppProvider()` — builds provider from DB credentials
- Setup handlers (manifest, start, callback, complete, status)
- `Server` wrapper struct for hot-swap
- Setup token middleware

## Error Handling

### Setup Token Errors

| Scenario | Response | Desktop App |
|----------|----------|-------------|
| Wrong token | 401 | "Invalid token" inline error |
| No token header | 401 | Should never happen |
| Setup already complete | 404 (routes gone) | `/ping` returns `"ok"`, shows normal login |

### Manifest Flow Errors

| Scenario | Behavior |
|----------|----------|
| User cancels on GitHub | No redirect. Desktop polls status — stays "pending". User retries. |
| GitHub error on callback | Agent shows HTML error page. Status stays "pending". |
| Code exchange network failure | Same — error page, status stays "pending". |
| App created on personal account | Works as single-user mode. Org check becomes "is this the same user". |

### Race Conditions

| Scenario | Mitigation |
|----------|-----------|
| Two people hit `/setup/github/start` | Only one `state` stored. First callback wins. |
| `POST /setup/complete` called twice | Idempotent — returns `{"status": "ok"}`. |
| Request during hot-swap | `sync.RWMutex` — in-flight requests complete, new ones wait (microseconds). |

### Agent Restart

| Scenario | Behavior |
|----------|----------|
| Before setup complete | Boots setup mode. New token. Wizard resumes via `/setup/status`. |
| After GitHub connected | Same. Credentials in SQLite. Status returns `"github_connected"`. |
| After setup complete | Boots normal mode. Setup routes not registered. |

### Database Loss

Agent boots into setup mode. User redoes setup. Same risk as losing metrics DB — mitigated by volume mounts.

### Follower Nodes

Unaffected. Followers don't run auth — only `cluster.internal_secret` (still in YAML).

## Testing Strategy

### Agent (Go) — Unit Tests

- `AppConfigStore` — CRUD, `IsSetupComplete()`, `GetGitHubCredentials()`
- Setup token generation — format validation, env var override
- Setup token middleware — valid/wrong/missing token
- Manifest builder — correct JSON given `agent_url`
- GitHub code exchange — mock GitHub API, verify storage
- Router hot-swap — `ServeHTTP` delegates correctly, swap is atomic
- `/ping` response — `setup_required` vs `ok`

### Agent (Go) — Integration Tests

- Full setup flow: manifest → callback → complete → normal mode
- Setup mode: `/api/*` returns 403
- Normal mode: `/setup/*` returns 404
- Interrupted setup recovery

### Desktop App (Vitest)

- Setup detection — `ping_agent` returns `setup_required`, wizard renders
- Wizard step transitions — token → GitHub → confirmation
- Polling — mock `/setup/status`, verify wizard advances
- Invalid token — error displayed
- Seamless transition — after complete, OAuth login triggered
- Tauri commands — `setup_verify_token`, `setup_github_connect`, `setup_complete`

## Full Lifecycle

```
Day 1: Deploy
  docker stack deploy -c kontainer.yml kontainer
  → grab setup token from logs

Day 1: Setup (5 minutes)
  Open desktop app → Add connection + setup token
  → Connect GitHub (one click) → Set default role → Done

Day 2+: Normal use
  → Team members log in via GitHub org membership
  → Everything just works
```

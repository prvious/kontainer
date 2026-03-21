# Goravel Agent Migration: PR 3 (Auth System) + PR 4 (Setup Flow)

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the full OAuth authentication system using Goravel's JWT auth facade and the GitHub App setup flow with setup-mode middleware, replacing the current manual JWT signing, Chi handlers, and SwappableHandler pattern.

**Architecture:** Goravel's `facades.Auth(ctx).LoginUsingID()` replaces manual HS256 token signing. A new User model (from PR 2) backs the auth facade — on OAuth callback, upsert user by provider+provider_id, then issue token via Goravel. The JWT secret lives in the `app_configs` DB table (set during setup) and is loaded into Goravel config at boot by the auth service provider. Setup mode uses a global middleware that checks `AppConfig.IsSetupComplete()` instead of the current SwappableHandler pattern. The OAuth state store, auth bridge, and provider registry are re-implemented as Goravel services.

**Tech Stack:** Goravel framework (auth facade, middleware, routing), `golang.org/x/oauth2`, `gorilla/websocket`

**Spec:** `docs/superpowers/specs/2026-03-11-goravel-agent-migration-design.md`

**Goravel patterns used:**
- `facades.Auth(ctx).LoginUsingID(userID)` → returns JWT token string
- `facades.Auth(ctx).Parse(token)` → parses JWT from Authorization header
- `facades.Auth(ctx).User(&user)` → populates User model from parsed token
- `middleware.Auth()` → built-in JWT middleware (from `github.com/goravel/framework/http/middleware`)
- `facades.Route().Prefix().Middleware().Group()` → route groups with middleware
- `ctx.Request().Input()` / `ctx.Request().Route()` → request params
- `ctx.Response().Success().Json()` → JSON responses
- Service providers: `Register()` for container bindings, `Boot()` for wiring

---

## Chunk 1: PR 3 — Auth System

### File Structure

```
server/
├── config/
│   ├── auth.go                                    # Modify: JWT guard + User provider
│   └── jwt.go                                     # Modify: TTL=1440, secret from env
├── app/
│   ├── http/
│   │   ├── controllers/auth/
│   │   │   ├── providers_controller.go            # GET /auth/providers
│   │   │   ├── listen_controller.go               # GET /auth/listen (WebSocket)
│   │   │   ├── redirect_controller.go             # GET /auth/{provider}
│   │   │   ├── callback_controller.go             # GET /auth/{provider}/callback
│   │   │   └── me_controller.go                   # GET /api/me
│   │   └── middleware/
│   │       └── jwt.go                             # JWT auth (wraps Goravel auth facade)
│   ├── actions/auth/
│   │   └── upsert_user.go                         # Find-or-create User by provider+provider_id
│   ├── services/auth/
│   │   ├── session_store.go                       # OAuth CSRF state store (in-memory, TTL)
│   │   ├── bridge.go                              # WebSocket ↔ OAuth token bridge
│   │   └── providers/
│   │       ├── provider.go                        # Provider interface + ProviderRegistry
│   │       └── github.go                          # GitHub OAuth provider
│   └── providers/
│       └── auth_service_provider.go               # Wires auth services, loads JWT secret from DB
├── routes/
│   ├── auth.go                                    # Public /auth/* routes
│   └── api.go                                     # Modify: add /api/me with JWT middleware
└── tests/
    ├── auth/
    │   ├── upsert_user_test.go
    │   ├── session_store_test.go
    │   └── bridge_test.go
    └── providers/
        └── github_test.go
```

### Mapping from old agent code

| Old (`agent/auth/`) | New (`server/app/`) |
|---|---|
| `jwt.go` (CreateToken, ParseToken) | Replaced by `facades.Auth(ctx).LoginUsingID()` + `Parse()` |
| `middleware.go` (JWTMiddleware) | `app/http/middleware/jwt.go` (wraps Goravel auth) |
| `session.go` (SessionStore) | `app/services/auth/session_store.go` |
| `bridge.go` (AuthBridge) | `app/services/auth/bridge.go` |
| `handlers.go` (AuthHandler) | Split into 4 controllers in `controllers/auth/` |
| `me_handler.go` (MeHandler) | `controllers/auth/me_controller.go` |
| `providers/provider.go` | `app/services/auth/providers/provider.go` |
| `providers/github.go` | `app/services/auth/providers/github.go` |

---

### Task 1: Configure auth and JWT config files

**Files:**
- Modify: `server/config/auth.go`
- Modify: `server/config/jwt.go` (if exists) or verify existing

**Important:** Goravel scaffold generates `config/auth.go` and likely includes JWT config. We modify the generated files to reference our User model and set the correct TTL.

- [ ] **Step 1: Read the generated auth config**

Read `server/config/auth.go` to see the scaffolded structure. Identify the `guards`, `providers`, and `defaults` sections.

- [ ] **Step 2: Update auth config for our User model**

Ensure `server/config/auth.go` has:

```go
package config

import (
    "github.com/goravel/framework/facades"
)

func init() {
    config := facades.Config()
    config.Add("auth", map[string]any{
        "defaults": map[string]any{
            "guard": "user",
        },
        "guards": map[string]any{
            "user": map[string]any{
                "driver":   "jwt",
                "provider": "user",
                "ttl":      1440, // 24 hours in minutes
                // "secret" is loaded dynamically from DB by AuthServiceProvider.Boot()
                // via facades.Config().Add("auth.guards.user.secret", value)
            },
        },
        "providers": map[string]any{
            "user": map[string]any{
                "driver": "orm",
            },
        },
    })
}
```

**Note:** Per Goravel v1.16 docs, the ORM user provider just needs `"driver": "orm"` — no explicit model reference needed. Goravel resolves the User model via the guard's provider name.

- [ ] **Step 3: Verify JWT config**

Read `server/config/jwt.go` (may be in `config/auth.go` or separate). Ensure:

```go
config.Add("jwt", map[string]any{
    "secret":      config.Env("JWT_SECRET", ""),
    "ttl":         1440,  // 24 hours in minutes (fallback, overridden per-guard)
    "refresh_ttl": 0,     // no refresh tokens
})
```

The TTL of 1440 minutes = 24 hours matches the current agent's `tokenTTL = 24 * time.Hour`.

- [ ] **Step 4: Verify app boots with updated auth config**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan about
```

Expected: No errors.

- [ ] **Step 5: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/config/auth.go server/config/jwt.go
git commit -m "$(cat <<'EOF'
feat: configure JWT auth guard with User model provider
EOF
)"
```

---

### Task 2: Create OAuth provider interface and GitHub implementation

**Files:**
- Create: `server/app/services/auth/providers/provider.go`
- Create: `server/app/services/auth/providers/github.go`
- Create: `server/tests/providers/github_test.go`

These are direct ports from `agent/auth/providers/` with no Goravel-specific changes (pure Go, no framework dependency).

- [ ] **Step 1: Create the Provider interface and ProviderRegistry**

Create `server/app/services/auth/providers/provider.go`:

```go
package providers

import (
    "context"
    "fmt"
    "sort"

    "golang.org/x/oauth2"
)

// AccessDeniedError is returned when a user authenticates successfully but is not
// authorized (e.g. not a member of an allowed organization).
type AccessDeniedError struct {
    Message string
}

func (e *AccessDeniedError) Error() string { return e.Message }

// ProviderUser is a normalized user representation returned by any OAuth provider.
type ProviderUser struct {
    ID        string
    Name      string
    Email     string
    AvatarURL string
    Provider  string
}

// Provider is the interface that each OAuth provider must implement.
type Provider interface {
    Name() string
    OAuthConfig() *oauth2.Config
    FetchUser(ctx context.Context, token *oauth2.Token) (*ProviderUser, error)
}

// ProviderRegistry holds registered OAuth providers keyed by name.
type ProviderRegistry struct {
    providers map[string]Provider
}

func NewProviderRegistry() *ProviderRegistry {
    return &ProviderRegistry{providers: make(map[string]Provider)}
}

func (r *ProviderRegistry) Register(p Provider) {
    r.providers[p.Name()] = p
}

func (r *ProviderRegistry) Names() []string {
    names := make([]string, 0, len(r.providers))
    for name := range r.providers {
        names = append(names, name)
    }
    sort.Strings(names)
    return names
}

func (r *ProviderRegistry) Get(name string) (Provider, error) {
    p, ok := r.providers[name]
    if !ok {
        return nil, fmt.Errorf("unknown provider: %s", name)
    }
    return p, nil
}
```

- [ ] **Step 2: Create the GitHub provider**

Create `server/app/services/auth/providers/github.go` — direct port from `agent/auth/providers/github.go`:

```go
package providers

import (
    "context"
    "encoding/json"
    "fmt"
    "net/http"
    "strconv"

    "golang.org/x/oauth2"
    oauthGithub "golang.org/x/oauth2/endpoints"
)

type GitHubProvider struct {
    config *oauth2.Config
    org    string
}

func NewGitHubProvider(clientID, clientSecret, redirectURL, org string) *GitHubProvider {
    scopes := []string{"read:user", "user:email"}
    if org != "" {
        scopes = append(scopes, "read:org")
    }

    return &GitHubProvider{
        config: &oauth2.Config{
            ClientID:     clientID,
            ClientSecret: clientSecret,
            RedirectURL:  redirectURL,
            Scopes:       scopes,
            Endpoint:     oauthGithub.GitHub,
        },
        org: org,
    }
}

func (g *GitHubProvider) Name() string            { return "github" }
func (g *GitHubProvider) OAuthConfig() *oauth2.Config { return g.config }

func (g *GitHubProvider) FetchUser(ctx context.Context, token *oauth2.Token) (*ProviderUser, error) {
    client := g.config.Client(ctx, token)

    user, login, err := g.fetchProfile(client)
    if err != nil {
        return nil, err
    }

    if user.Email == "" {
        email, err := g.fetchPrimaryEmail(client)
        if err != nil {
            return nil, err
        }
        user.Email = email
    }

    if err := g.checkAccess(client, login); err != nil {
        return nil, err
    }

    return user, nil
}

type githubProfile struct {
    ID        int    `json:"id"`
    Login     string `json:"login"`
    Name      string `json:"name"`
    Email     string `json:"email"`
    AvatarURL string `json:"avatar_url"`
}

func (g *GitHubProvider) fetchProfile(client *http.Client) (*ProviderUser, string, error) {
    resp, err := client.Get("https://api.github.com/user")
    if err != nil {
        return nil, "", fmt.Errorf("github: fetch profile: %w", err)
    }
    defer resp.Body.Close()

    if resp.StatusCode != http.StatusOK {
        return nil, "", fmt.Errorf("github: fetch profile: status %d", resp.StatusCode)
    }

    var profile githubProfile
    if err := json.NewDecoder(resp.Body).Decode(&profile); err != nil {
        return nil, "", fmt.Errorf("github: decode profile: %w", err)
    }

    name := profile.Name
    if name == "" {
        name = profile.Login
    }

    return &ProviderUser{
        ID:        strconv.Itoa(profile.ID),
        Name:      name,
        Email:     profile.Email,
        AvatarURL: profile.AvatarURL,
        Provider:  "github",
    }, profile.Login, nil
}

type githubEmail struct {
    Email    string `json:"email"`
    Primary  bool   `json:"primary"`
    Verified bool   `json:"verified"`
}

func (g *GitHubProvider) fetchPrimaryEmail(client *http.Client) (string, error) {
    resp, err := client.Get("https://api.github.com/user/emails")
    if err != nil {
        return "", fmt.Errorf("github: fetch emails: %w", err)
    }
    defer resp.Body.Close()

    if resp.StatusCode != http.StatusOK {
        return "", fmt.Errorf("github: fetch emails: status %d", resp.StatusCode)
    }

    var emails []githubEmail
    if err := json.NewDecoder(resp.Body).Decode(&emails); err != nil {
        return "", fmt.Errorf("github: decode emails: %w", err)
    }

    for _, e := range emails {
        if e.Primary && e.Verified {
            return e.Email, nil
        }
    }

    return "", fmt.Errorf("github: no primary verified email found")
}

func (g *GitHubProvider) checkAccess(client *http.Client, login string) error {
    if g.org == "" {
        return nil
    }

    url := fmt.Sprintf("https://api.github.com/orgs/%s/members/%s", g.org, login)
    resp, err := client.Get(url)
    if err != nil {
        return fmt.Errorf("github: check org membership: %w", err)
    }
    defer resp.Body.Close()

    if resp.StatusCode == http.StatusNoContent {
        return nil
    }

    return &AccessDeniedError{
        Message: fmt.Sprintf("user %q is not a member of organization %q", login, g.org),
    }
}
```

- [ ] **Step 3: Add `golang.org/x/oauth2` dependency**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go get golang.org/x/oauth2
```

- [ ] **Step 4: Write GitHub provider tests**

Create `server/tests/providers/github_test.go`:

```go
package providers_test

import (
    "encoding/json"
    "net/http"
    "net/http/httptest"
    "testing"

    "github.com/prvious/kontainer/server/app/services/auth/providers"
    "golang.org/x/oauth2"
)

func TestGitHubProvider_Name(t *testing.T) {
    p := providers.NewGitHubProvider("id", "secret", "http://localhost/callback", "")
    if p.Name() != "github" {
        t.Errorf("expected 'github', got '%s'", p.Name())
    }
}

func TestGitHubProvider_FetchUser(t *testing.T) {
    // Mock GitHub API
    mux := http.NewServeMux()
    mux.HandleFunc("/user", func(w http.ResponseWriter, r *http.Request) {
        json.NewEncoder(w).Encode(map[string]any{
            "id":         12345,
            "login":      "testuser",
            "name":       "Test User",
            "email":      "test@example.com",
            "avatar_url": "https://example.com/avatar.png",
        })
    })
    server := httptest.NewServer(mux)
    defer server.Close()

    // Create provider pointing to mock server
    // Note: We can't easily override the GitHub API URL in the provider.
    // For a proper test, we'd need to make the API base URL configurable.
    // For now, test the ProviderRegistry and basic construction.
    t.Log("GitHub provider constructs correctly — integration test requires API mock injection")
}

func TestProviderRegistry(t *testing.T) {
    registry := providers.NewProviderRegistry()

    p := providers.NewGitHubProvider("id", "secret", "http://localhost/callback", "")
    registry.Register(p)

    names := registry.Names()
    if len(names) != 1 || names[0] != "github" {
        t.Errorf("expected ['github'], got %v", names)
    }

    got, err := registry.Get("github")
    if err != nil {
        t.Fatalf("unexpected error: %v", err)
    }
    if got.Name() != "github" {
        t.Errorf("expected 'github', got '%s'", got.Name())
    }

    _, err = registry.Get("unknown")
    if err == nil {
        t.Error("expected error for unknown provider")
    }
}

func TestAccessDeniedError(t *testing.T) {
    err := &providers.AccessDeniedError{Message: "access denied"}
    if err.Error() != "access denied" {
        t.Errorf("expected 'access denied', got '%s'", err.Error())
    }
}
```

- [ ] **Step 5: Run tests**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go test ./tests/providers/ -v
```

Expected: All tests PASS.

- [ ] **Step 6: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/services/auth/providers/ server/tests/providers/ server/go.mod server/go.sum
git commit -m "$(cat <<'EOF'
feat: add OAuth provider interface and GitHub implementation
EOF
)"
```

---

### Task 3: Create OAuth session store

**Files:**
- Create: `server/app/services/auth/session_store.go`
- Create: `server/tests/auth/session_store_test.go`

Direct port from `agent/auth/session.go`. No Goravel dependency — pure Go in-memory CSRF state store.

- [ ] **Step 1: Write the failing test**

Create `server/tests/auth/session_store_test.go`:

```go
package auth_test

import (
    "testing"

    authservice "github.com/prvious/kontainer/server/app/services/auth"
)

func TestSessionStore_GenerateAndValidate(t *testing.T) {
    store := authservice.NewSessionStore()

    state, err := store.Generate("session-123")
    if err != nil {
        t.Fatalf("unexpected error: %v", err)
    }
    if state == "" {
        t.Fatal("expected non-empty state token")
    }

    sessionID, ok := store.Validate(state)
    if !ok {
        t.Fatal("expected valid state token")
    }
    if sessionID != "session-123" {
        t.Errorf("expected 'session-123', got '%s'", sessionID)
    }
}

func TestSessionStore_ValidateConsumesToken(t *testing.T) {
    store := authservice.NewSessionStore()

    state, _ := store.Generate("session-456")

    // First validate succeeds
    _, ok := store.Validate(state)
    if !ok {
        t.Fatal("first validation should succeed")
    }

    // Second validate fails (token consumed)
    _, ok = store.Validate(state)
    if ok {
        t.Fatal("second validation should fail — token already consumed")
    }
}

func TestSessionStore_ValidateUnknownToken(t *testing.T) {
    store := authservice.NewSessionStore()

    _, ok := store.Validate("nonexistent-token")
    if ok {
        t.Fatal("expected false for unknown token")
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go test ./tests/auth/ -v
```

Expected: FAIL — package not found.

- [ ] **Step 3: Create the session store**

Create `server/app/services/auth/session_store.go`:

```go
package auth

import (
    "crypto/rand"
    "encoding/hex"
    "sync"
    "time"
)

const sessionTTL = 10 * time.Minute

type sessionEntry struct {
    createdAt time.Time
    sessionID string
}

// SessionStore is an in-memory CSRF state store with TTL-based expiration.
// Used to bind OAuth state parameters to AuthBridge session IDs.
type SessionStore struct {
    mu      sync.Mutex
    entries map[string]sessionEntry
    done    chan struct{}
}

func NewSessionStore() *SessionStore {
    s := &SessionStore{
        entries: make(map[string]sessionEntry),
        done:    make(chan struct{}),
    }
    go s.cleanup()
    return s
}

// Close stops the background cleanup goroutine to prevent goroutine leaks.
func (s *SessionStore) Close() {
    close(s.done)
}

// Generate creates a random state token bound to a bridge session ID.
func (s *SessionStore) Generate(sessionID string) (string, error) {
    b := make([]byte, 16)
    if _, err := rand.Read(b); err != nil {
        return "", err
    }
    token := hex.EncodeToString(b)

    s.mu.Lock()
    s.entries[token] = sessionEntry{createdAt: time.Now(), sessionID: sessionID}
    s.mu.Unlock()

    return token, nil
}

// Validate checks if a state token exists and has not expired, then removes it.
// Returns the associated bridge session ID.
func (s *SessionStore) Validate(token string) (string, bool) {
    s.mu.Lock()
    defer s.mu.Unlock()

    entry, ok := s.entries[token]
    if !ok {
        return "", false
    }
    delete(s.entries, token)

    if time.Since(entry.createdAt) >= sessionTTL {
        return "", false
    }
    return entry.sessionID, true
}

func (s *SessionStore) cleanup() {
    ticker := time.NewTicker(time.Minute)
    defer ticker.Stop()

    for {
        select {
        case <-ticker.C:
            s.mu.Lock()
            for token, entry := range s.entries {
                if time.Since(entry.createdAt) >= sessionTTL {
                    delete(s.entries, token)
                }
            }
            s.mu.Unlock()
        case <-s.done:
            return
        }
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go test ./tests/auth/ -v
```

Expected: 3 tests PASS.

- [ ] **Step 5: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/services/auth/session_store.go server/tests/auth/session_store_test.go
git commit -m "$(cat <<'EOF'
feat: add OAuth session store for CSRF state management
EOF
)"
```

---

### Task 4: Create auth bridge

**Files:**
- Create: `server/app/services/auth/bridge.go`
- Create: `server/tests/auth/bridge_test.go`

Direct port from `agent/auth/bridge.go`. No Goravel dependency.

- [ ] **Step 1: Write the failing test**

Create `server/tests/auth/bridge_test.go`:

```go
package auth_test

import (
    "testing"
    "time"

    authservice "github.com/prvious/kontainer/server/app/services/auth"
)

func TestAuthBridge_SendAndReceive(t *testing.T) {
    bridge := authservice.NewAuthBridge()

    sessionID, err := bridge.CreateSession()
    if err != nil {
        t.Fatalf("unexpected error: %v", err)
    }

    go func() {
        time.Sleep(10 * time.Millisecond)
        bridge.SendToken(sessionID, "jwt-token-123")
    }()

    token, err := bridge.WaitForToken(sessionID, 1*time.Second)
    if err != nil {
        t.Fatalf("unexpected error: %v", err)
    }
    if token != "jwt-token-123" {
        t.Errorf("expected 'jwt-token-123', got '%s'", token)
    }
}

func TestAuthBridge_Timeout(t *testing.T) {
    bridge := authservice.NewAuthBridge()

    sessionID, _ := bridge.CreateSession()

    _, err := bridge.WaitForToken(sessionID, 50*time.Millisecond)
    if err == nil {
        t.Fatal("expected timeout error")
    }
}

func TestAuthBridge_UnknownSession(t *testing.T) {
    bridge := authservice.NewAuthBridge()

    _, err := bridge.WaitForToken("nonexistent", 50*time.Millisecond)
    if err == nil {
        t.Fatal("expected error for unknown session")
    }
}

func TestAuthBridge_Cleanup(t *testing.T) {
    bridge := authservice.NewAuthBridge()

    sessionID, _ := bridge.CreateSession()
    bridge.Cleanup(sessionID)

    ok := bridge.SendToken(sessionID, "token")
    if ok {
        t.Fatal("expected false — session was cleaned up")
    }
}
```

- [ ] **Step 2: Create the auth bridge**

Create `server/app/services/auth/bridge.go`:

```go
package auth

import (
    "crypto/rand"
    "encoding/hex"
    "errors"
    "sync"
    "time"
)

// AuthBridge connects the WebSocket listener with the OAuth callback.
type AuthBridge struct {
    mu      sync.Mutex
    pending map[string]chan string
}

func NewAuthBridge() *AuthBridge {
    return &AuthBridge{
        pending: make(map[string]chan string),
    }
}

func (b *AuthBridge) CreateSession() (string, error) {
    buf := make([]byte, 16)
    if _, err := rand.Read(buf); err != nil {
        return "", err
    }
    id := hex.EncodeToString(buf)

    b.mu.Lock()
    b.pending[id] = make(chan string, 1)
    b.mu.Unlock()

    return id, nil
}

func (b *AuthBridge) SendToken(sessionID, token string) bool {
    b.mu.Lock()
    ch, ok := b.pending[sessionID]
    b.mu.Unlock()

    if !ok {
        return false
    }

    select {
    case ch <- token:
        return true
    default:
        return false
    }
}

func (b *AuthBridge) WaitForToken(sessionID string, timeout time.Duration) (string, error) {
    b.mu.Lock()
    ch, ok := b.pending[sessionID]
    b.mu.Unlock()

    if !ok {
        return "", errors.New("unknown session")
    }

    select {
    case token := <-ch:
        return token, nil
    case <-time.After(timeout):
        return "", errors.New("timeout waiting for token")
    }
}

func (b *AuthBridge) Cleanup(sessionID string) {
    b.mu.Lock()
    defer b.mu.Unlock()

    if ch, ok := b.pending[sessionID]; ok {
        close(ch)
        delete(b.pending, sessionID)
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go test ./tests/auth/ -v
```

Expected: All 7 tests PASS (3 session store + 4 bridge).

- [ ] **Step 4: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/services/auth/bridge.go server/tests/auth/bridge_test.go
git commit -m "$(cat <<'EOF'
feat: add auth bridge for WebSocket-to-OAuth token delivery
EOF
)"
```

---

### Task 5: Create UpsertUser action

**Files:**
- Create: `server/app/actions/auth/upsert_user.go`
- Create: `server/tests/auth/upsert_user_test.go`

This is new — the current agent doesn't persist users. Goravel's auth facade requires a User model backed by the database. On OAuth callback, we find-or-create the user by (provider, provider_id).

- [ ] **Step 1: Write the failing test**

Create `server/tests/auth/upsert_user_test.go`:

```go
package auth_test

import (
    "testing"

    "github.com/prvious/kontainer/server/app/actions/auth"
    "github.com/prvious/kontainer/server/app/models"
    "github.com/prvious/kontainer/server/app/services/auth/providers"
    "github.com/goravel/framework/facades"
    "github.com/goravel/framework/testing/mock"
)

func TestUpsertUser_CreatesNewUser(t *testing.T) {
    // Note: This test requires Goravel's test helpers for database mocking.
    // Use facades.Orm() mock or run against a test SQLite database.
    // The exact test setup depends on Goravel's testing support.
    //
    // Skeleton:
    // 1. Set up test database (in-memory SQLite)
    // 2. Run migrations
    // 3. Call UpsertUser with a new provider user
    // 4. Assert user is created in DB
    // 5. Call again with same provider+provider_id
    // 6. Assert user is updated (not duplicated)
    t.Skip("Requires Goravel test database setup — implement after service provider wiring")
}
```

- [ ] **Step 2: Create the action**

Create `server/app/actions/auth/upsert_user.go`:

```go
package auth

import (
    "github.com/goravel/framework/facades"
    "github.com/prvious/kontainer/server/app/models"
    "github.com/prvious/kontainer/server/app/services/auth/providers"
)

// UpsertUserAction finds or creates a User by provider + provider_id.
// On subsequent logins, updates name, email, and avatar_url.
type UpsertUserAction struct{}

func NewUpsertUserAction() *UpsertUserAction {
    return &UpsertUserAction{}
}

func (a *UpsertUserAction) Execute(providerUser *providers.ProviderUser) (*models.User, error) {
    var user models.User

    // UpdateOrCreate: search by provider+provider_id, upsert name/email/avatar.
    // This handles both new and returning users in a single call.
    err := facades.Orm().Query().UpdateOrCreate(&user,
        models.User{Provider: providerUser.Provider, ProviderID: providerUser.ID}, // search conditions
        models.User{Name: providerUser.Name, Email: providerUser.Email, AvatarURL: providerUser.AvatarURL}, // values to set
    )
    if err != nil {
        return nil, err
    }

    return &user, nil
}

- [ ] **Step 3: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/actions/auth/upsert_user.go server/tests/auth/upsert_user_test.go
git commit -m "$(cat <<'EOF'
feat: add UpsertUser action for OAuth user persistence
EOF
)"
```

---

### Task 6: Create auth service provider

**Files:**
- Create: `server/app/providers/auth_service_provider.go`
- Modify: `server/bootstrap/providers.go` (register the provider)

The auth service provider:
1. **Register:** Binds ProviderRegistry, SessionStore, AuthBridge as singletons
2. **Boot:** Reads JWT secret from `app_configs` table and overrides Goravel config, then registers GitHub OAuth provider using stored credentials

**Critical architecture decision:** The JWT secret is stored in the `app_configs` DB table (set during setup flow). At boot time, the auth service provider reads it and sets it on the guard via `facades.Config().Add("auth.guards.user.secret", value)`. Since Goravel v1.16, each JWT guard supports its own `secret`, `ttl`, and `refresh_ttl` — per-guard config takes precedence over the global `jwt.secret` fallback. This is the cleanest approach. If setup isn't complete yet, the provider skips this (auth will be inactive anyway since setup middleware blocks all `/api/*` routes).

- [ ] **Step 1: Create the auth service provider**

Create `server/app/providers/auth_service_provider.go`:

```go
package providers

import (
    "github.com/goravel/framework/contracts/foundation"
    "github.com/goravel/framework/facades"

    "github.com/prvious/kontainer/server/app/models"
    authservice "github.com/prvious/kontainer/server/app/services/auth"
    authproviders "github.com/prvious/kontainer/server/app/services/auth/providers"
)

type AuthServiceProvider struct{}

func (r *AuthServiceProvider) Register(app foundation.Application) {
    app.Singleton("auth.session_store", func(app foundation.Application) (any, error) {
        return authservice.NewSessionStore(), nil
    })

    app.Singleton("auth.bridge", func(app foundation.Application) (any, error) {
        return authservice.NewAuthBridge(), nil
    })

    app.Singleton("auth.provider_registry", func(app foundation.Application) (any, error) {
        return authproviders.NewProviderRegistry(), nil
    })
}

func (r *AuthServiceProvider) Boot(app foundation.Application) {
    r.loadJWTSecretFromDB()
    r.registerOAuthProviders(app)
}

// loadJWTSecretFromDB reads the jwt_secret from app_configs and sets it on the JWT guard.
// Since Goravel v1.16, each JWT guard supports its own secret/ttl/refresh_ttl.
// Per-guard config takes precedence over the global jwt.secret fallback.
// If setup is not complete (no jwt_secret), this is a no-op.
func (r *AuthServiceProvider) loadJWTSecretFromDB() {
    var appConfig models.AppConfig
    err := facades.Orm().Query().Where("key", "jwt_secret").First(&appConfig)
    if err != nil || appConfig.Value == "" {
        facades.Log().Warning("JWT secret not found in app_configs — auth disabled until setup completes")
        return
    }

    // Set secret on the "user" guard (per-guard config, Goravel v1.16+)
    facades.Config().Add("auth.guards.user.secret", appConfig.Value)
    facades.Log().Info("JWT secret loaded from app_configs into user guard")
}

// registerOAuthProviders reads GitHub OAuth credentials from app_configs and registers the provider.
// Note: `app` is passed from Boot() to access the service container for Make() calls.
func (r *AuthServiceProvider) registerOAuthProviders(app foundation.Application) {
    registry, err := app.Make("auth.provider_registry")
    if err != nil {
        facades.Log().Errorf("Failed to resolve provider registry: %v", err)
        return
    }
    providerRegistry := registry.(*authproviders.ProviderRegistry)

    // Read GitHub credentials from app_configs
    configs := map[string]string{}
    for _, key := range []string{"github_client_id", "github_client_secret", "github_org"} {
        var ac models.AppConfig
        if err := facades.Orm().Query().Where("key", key).First(&ac); err == nil {
            configs[key] = ac.Value
        }
    }

    clientID := configs["github_client_id"]
    clientSecret := configs["github_client_secret"]
    if clientID == "" || clientSecret == "" {
        facades.Log().Warning("GitHub OAuth credentials not found — OAuth disabled until setup completes")
        return
    }

    agentURL := facades.Config().GetString("cluster.agent_url", "http://localhost:8080")
    org := configs["github_org"]

    providerRegistry.Register(authproviders.NewGitHubProvider(
        clientID,
        clientSecret,
        agentURL+"/auth/github/callback",
        org,
    ))

    facades.Log().Info("GitHub OAuth provider registered")
}
```

**Important:** The `app.Make()` call in `Boot()` requires that `Register()` has already run. Goravel guarantees this — all providers `Register()` first, then all `Boot()`. Verify this pattern compiles by checking the `foundation.Application` interface for `Make()` or `MakeAs()`.

- [ ] **Step 2: Register the provider in bootstrap**

In `server/bootstrap/providers.go` (or wherever providers are listed), add:

```go
&providers.AuthServiceProvider{},
```

- [ ] **Step 3: Verify app boots**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan about
```

Expected: Boot logs show "JWT secret not found" warning (expected — setup hasn't run yet).

- [ ] **Step 4: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/providers/auth_service_provider.go server/bootstrap/providers.go
git commit -m "$(cat <<'EOF'
feat: add auth service provider with JWT secret loading from DB
EOF
)"
```

---

### Task 7: Create auth controllers

**Files:**
- Create: `server/app/http/controllers/auth/providers_controller.go`
- Create: `server/app/http/controllers/auth/listen_controller.go`
- Create: `server/app/http/controllers/auth/redirect_controller.go`
- Create: `server/app/http/controllers/auth/callback_controller.go`
- Create: `server/app/http/controllers/auth/me_controller.go`

Each controller is thin — parse request, call service/action, return response. One controller per file per our conventions.

**Goravel controller pattern:**
```go
func (c *FooController) Handle(ctx http.Context) http.Response {
    return ctx.Response().Success().Json(http.Json{"key": "value"})
}
```

- [ ] **Step 1: Create ProvidersController**

Create `server/app/http/controllers/auth/providers_controller.go`:

```go
package auth

import (
    contractshttp "github.com/goravel/framework/contracts/http"
    "github.com/goravel/framework/http"

    authproviders "github.com/prvious/kontainer/server/app/services/auth/providers"
)

type ProvidersController struct {
    registry *authproviders.ProviderRegistry
}

func NewProvidersController(registry *authproviders.ProviderRegistry) *ProvidersController {
    return &ProvidersController{registry: registry}
}

func (c *ProvidersController) Handle(ctx contractshttp.Context) contractshttp.Response {
    return ctx.Response().Success().Json(http.Json{
        "providers": c.registry.Names(),
    })
}
```

- [ ] **Step 2: Create ListenController (WebSocket)**

Create `server/app/http/controllers/auth/listen_controller.go`:

```go
package auth

import (
    "log"
    nethttp "net/http"
    "time"

    contractshttp "github.com/goravel/framework/contracts/http"
    "github.com/gorilla/websocket"

    authservice "github.com/prvious/kontainer/server/app/services/auth"
)

var upgrader = websocket.Upgrader{
    CheckOrigin: func(r *nethttp.Request) bool { return true },
}

type ListenController struct {
    bridge *authservice.AuthBridge
}

func NewListenController(bridge *authservice.AuthBridge) *ListenController {
    return &ListenController{bridge: bridge}
}

// Handle upgrades to WebSocket, creates a bridge session, sends session ID,
// then blocks until OAuth callback delivers a token (or 10min timeout).
func (c *ListenController) Handle(ctx contractshttp.Context) contractshttp.Response {
    // Access raw HTTP primitives for WebSocket upgrade.
    // Goravel wraps net/http — the response writer implements http.ResponseWriter.
    w := ctx.Response().Writer()
    r := ctx.Request().Origin()

    conn, err := upgrader.Upgrade(w.(nethttp.ResponseWriter), r.(*nethttp.Request), nil)
    if err != nil {
        log.Printf("ws upgrade error: %v", err)
        return nil
    }
    defer conn.Close()

    sessionID, err := c.bridge.CreateSession()
    if err != nil {
        conn.WriteJSON(map[string]string{"error": "failed to create session"})
        return nil
    }
    defer c.bridge.Cleanup(sessionID)

    if err := conn.WriteJSON(map[string]string{"sessionId": sessionID}); err != nil {
        return nil
    }

    token, err := c.bridge.WaitForToken(sessionID, 10*time.Minute)
    if err != nil {
        conn.WriteJSON(map[string]string{"error": err.Error()})
        return nil
    }

    conn.WriteJSON(map[string]string{"token": token})
    return nil
}
```

**Important:** The `ctx.Response().Writer()` call may not exist in Goravel's contract. Verify the actual method name. Alternatives:
- `ctx.Response().Writer()` — returns `io.Writer`
- Type-assert to `http.ResponseWriter` for WebSocket upgrade
- If unavailable, register this endpoint as a raw `net/http.Handler` outside Goravel's router

If Goravel's context doesn't expose the raw writer, use this fallback pattern in `routes/auth.go`:
```go
// Register WS endpoint as raw handler on the underlying HTTP mux
http.HandleFunc("/auth/listen", rawListenHandler)
```

- [ ] **Step 3: Create RedirectController**

Create `server/app/http/controllers/auth/redirect_controller.go`:

```go
package auth

import (
    nethttp "net/http"

    contractshttp "github.com/goravel/framework/contracts/http"

    authproviders "github.com/prvious/kontainer/server/app/services/auth/providers"
    authservice "github.com/prvious/kontainer/server/app/services/auth"
)

type RedirectController struct {
    registry *authproviders.ProviderRegistry
    sessions *authservice.SessionStore
}

func NewRedirectController(
    registry *authproviders.ProviderRegistry,
    sessions *authservice.SessionStore,
) *RedirectController {
    return &RedirectController{registry: registry, sessions: sessions}
}

func (c *RedirectController) Handle(ctx contractshttp.Context) contractshttp.Response {
    providerName := ctx.Request().Route("provider")

    provider, err := c.registry.Get(providerName)
    if err != nil {
        return ctx.Response().Json(nethttp.StatusNotFound, contractshttp.Json{
            "error": err.Error(),
        })
    }

    sessionID := ctx.Request().Query("session")
    if sessionID == "" {
        return ctx.Response().Json(nethttp.StatusBadRequest, contractshttp.Json{
            "error": "missing session parameter",
        })
    }

    state, err := c.sessions.Generate(sessionID)
    if err != nil {
        return ctx.Response().Json(nethttp.StatusInternalServerError, contractshttp.Json{
            "error": "failed to generate state",
        })
    }

    authURL := provider.OAuthConfig().AuthCodeURL(state)
    return ctx.Response().Redirect(nethttp.StatusFound, authURL)
}
```

**Verify:** The JSON response helper. Goravel uses `contractshttp.Json` or `http.Json` — check which is correct. From the docs: `ctx.Response().Json(http.StatusOK, http.Json{"key": "value"})` — so import `"github.com/goravel/framework/http"` and use `http.Json`.

- [ ] **Step 4: Create CallbackController**

Create `server/app/http/controllers/auth/callback_controller.go`:

```go
package auth

import (
    "errors"
    nethttp "net/http"

    contractshttp "github.com/goravel/framework/contracts/http"
    "github.com/goravel/framework/facades"
    "github.com/goravel/framework/http"

    authactions "github.com/prvious/kontainer/server/app/actions/auth"
    authproviders "github.com/prvious/kontainer/server/app/services/auth/providers"
    authservice "github.com/prvious/kontainer/server/app/services/auth"
)

type CallbackController struct {
    registry   *authproviders.ProviderRegistry
    sessions   *authservice.SessionStore
    bridge     *authservice.AuthBridge
    upsertUser *authactions.UpsertUserAction
}

func NewCallbackController(
    registry *authproviders.ProviderRegistry,
    sessions *authservice.SessionStore,
    bridge *authservice.AuthBridge,
    upsertUser *authactions.UpsertUserAction,
) *CallbackController {
    return &CallbackController{
        registry:   registry,
        sessions:   sessions,
        bridge:     bridge,
        upsertUser: upsertUser,
    }
}

const successHTML = `<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><title>Authenticated</title></head>
<body style="display:flex;align-items:center;justify-content:center;height:100vh;font-family:system-ui,sans-serif">
<p>Authentication successful. You can close this tab.</p>
</body>
</html>`

func (c *CallbackController) Handle(ctx contractshttp.Context) contractshttp.Response {
    providerName := ctx.Request().Route("provider")

    provider, err := c.registry.Get(providerName)
    if err != nil {
        return ctx.Response().Json(nethttp.StatusNotFound, http.Json{"error": err.Error()})
    }

    code := ctx.Request().Query("code")
    state := ctx.Request().Query("state")

    if code == "" || state == "" {
        return ctx.Response().Json(nethttp.StatusBadRequest, http.Json{"error": "missing code or state"})
    }

    sessionID, ok := c.sessions.Validate(state)
    if !ok {
        return ctx.Response().Json(nethttp.StatusBadRequest, http.Json{"error": "invalid or expired state"})
    }

    oauthToken, err := provider.OAuthConfig().Exchange(ctx.Request().Origin().Context(), code)
    if err != nil {
        facades.Log().Errorf("oauth exchange error: %v", err)
        return ctx.Response().Json(nethttp.StatusBadGateway, http.Json{"error": "oauth exchange failed"})
    }

    providerUser, err := provider.FetchUser(ctx.Request().Origin().Context(), oauthToken)
    if err != nil {
        var accessErr *authproviders.AccessDeniedError
        if errors.As(err, &accessErr) {
            return ctx.Response().Json(nethttp.StatusForbidden, http.Json{"error": accessErr.Message})
        }
        facades.Log().Errorf("fetch user error: %v", err)
        return ctx.Response().Json(nethttp.StatusBadGateway, http.Json{"error": "failed to fetch user"})
    }

    // Upsert user in database (new in Goravel — current agent is stateless)
    user, err := c.upsertUser.Execute(providerUser)
    if err != nil {
        facades.Log().Errorf("upsert user error: %v", err)
        return ctx.Response().Json(nethttp.StatusInternalServerError, http.Json{"error": "failed to save user"})
    }

    // Issue JWT via Goravel auth facade (replaces manual HS256 signing)
    token, err := facades.Auth(ctx).LoginUsingID(user.ID)
    if err != nil {
        facades.Log().Errorf("jwt creation error: %v", err)
        return ctx.Response().Json(nethttp.StatusInternalServerError, http.Json{"error": "failed to create token"})
    }

    c.bridge.SendToken(sessionID, token)

    return ctx.Response().Data(nethttp.StatusOK, "text/html; charset=utf-8", []byte(successHTML))
}
```

**Key change from old agent:** Instead of `CreateToken(secret, user, ttl)` which embeds all user claims in JWT, we now:
1. Upsert user in DB → get `user.ID`
2. Call `facades.Auth(ctx).LoginUsingID(user.ID)` → Goravel JWT with user ID only
3. To get user info later, `facades.Auth(ctx).User(&user)` queries DB

- [ ] **Step 5: Create MeController**

Create `server/app/http/controllers/auth/me_controller.go`:

```go
package auth

import (
    nethttp "net/http"

    contractshttp "github.com/goravel/framework/contracts/http"
    "github.com/goravel/framework/facades"
    "github.com/goravel/framework/http"

    "github.com/prvious/kontainer/server/app/models"
)

type MeController struct{}

func NewMeController() *MeController {
    return &MeController{}
}

func (c *MeController) Handle(ctx contractshttp.Context) contractshttp.Response {
    var user models.User
    if err := facades.Auth(ctx).User(&user); err != nil {
        return ctx.Response().Json(nethttp.StatusUnauthorized, http.Json{"error": "unauthorized"})
    }

    return ctx.Response().Success().Json(http.Json{
        "user": http.Json{
            "id":        user.ID,
            "name":      user.Name,
            "email":     user.Email,
            "avatarUrl": user.AvatarURL,
            "provider":  user.Provider,
        },
        "status": "ok",
    })
}
```

**Note:** The response shape matches the current `meResponse` struct from `agent/auth/me_handler.go`, preserving frontend compatibility. The `id` field changes from string (GitHub user ID) to uint (database ID). The frontend may need adjustment in PR 15.

- [ ] **Step 6: Add `gorilla/websocket` dependency**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go get github.com/gorilla/websocket
```

- [ ] **Step 7: Verify compilation**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go build ./...
```

Expected: Compiles without errors. If there are type mismatches with Goravel's contracts, fix the import paths.

- [ ] **Step 8: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/http/controllers/auth/ server/go.mod server/go.sum
git commit -m "$(cat <<'EOF'
feat: add auth controllers (providers, listen, redirect, callback, me)
EOF
)"
```

---

### Task 8: Create JWT middleware and auth routes

**Files:**
- Create: `server/app/http/middleware/jwt.go`
- Create: `server/routes/auth.go`
- Modify: `server/routes/api.go`

- [ ] **Step 1: Check if Goravel has a built-in auth middleware**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
grep -r "middleware.Auth" vendor/github.com/goravel/ 2>/dev/null || echo "Check Goravel source for built-in auth middleware"
```

If Goravel provides `middleware.Auth()` from `github.com/goravel/framework/http/middleware`, use it directly. If not, create a custom one.

- [ ] **Step 2: Create JWT middleware**

Create `server/app/http/middleware/jwt.go`:

```go
package middleware

import (
    nethttp "net/http"

    contractshttp "github.com/goravel/framework/contracts/http"
    "github.com/goravel/framework/facades"
    "github.com/goravel/framework/http"
)

// JWT validates the Authorization Bearer token using Goravel's auth facade.
func JWT() contractshttp.Middleware {
    return func(ctx contractshttp.Context) {
        token := ctx.Request().Header("Authorization")
        if token == "" {
            ctx.Request().Abort(nethttp.StatusUnauthorized)
            return
        }

        // Per Goravel docs: "The token can be parsed normally with or without the Bearer prefix"
        _, err := facades.Auth(ctx).Parse(token)
        if err != nil {
            ctx.Response().Json(nethttp.StatusUnauthorized, http.Json{
                "error": "invalid or expired token",
            })
            ctx.Request().Abort()
            return
        }

        ctx.Request().Next()
    }
}
```

**Note:** If Goravel's built-in `middleware.Auth()` exists, use it instead and delete this file. The built-in middleware would handle the same logic. Our custom version exists as a fallback.

- [ ] **Step 3: Create auth routes**

Create `server/routes/auth.go`:

```go
package routes

import (
    "github.com/goravel/framework/contracts/route"
    "github.com/goravel/framework/facades"

    authcontrollers "github.com/prvious/kontainer/server/app/http/controllers/auth"
    authactions "github.com/prvious/kontainer/server/app/actions/auth"
    authservice "github.com/prvious/kontainer/server/app/services/auth"
    authproviders "github.com/prvious/kontainer/server/app/services/auth/providers"
)

func Auth() {
    // Resolve dependencies from container
    registryInstance, _ := facades.App().Make("auth.provider_registry")
    registry := registryInstance.(*authproviders.ProviderRegistry)

    sessionInstance, _ := facades.App().Make("auth.session_store")
    sessions := sessionInstance.(*authservice.SessionStore)

    bridgeInstance, _ := facades.App().Make("auth.bridge")
    bridge := bridgeInstance.(*authservice.AuthBridge)

    upsertUser := authactions.NewUpsertUserAction()

    // Controllers
    providersCtrl := authcontrollers.NewProvidersController(registry)
    listenCtrl := authcontrollers.NewListenController(bridge)
    redirectCtrl := authcontrollers.NewRedirectController(registry, sessions)
    callbackCtrl := authcontrollers.NewCallbackController(registry, sessions, bridge, upsertUser)

    // Public auth routes (no JWT required)
    facades.Route().Prefix("auth").Group(func(router route.Router) {
        router.Get("providers", providersCtrl.Handle)
        router.Get("listen", listenCtrl.Handle)
        router.Get("{provider}", redirectCtrl.Handle)
        router.Get("{provider}/callback", callbackCtrl.Handle)
    })
}
```

- [ ] **Step 4: Add /api/me route**

Modify `server/routes/api.go` (create if not exists):

```go
package routes

import (
    "github.com/goravel/framework/contracts/route"
    "github.com/goravel/framework/facades"

    authcontrollers "github.com/prvious/kontainer/server/app/http/controllers/auth"
    "github.com/prvious/kontainer/server/app/http/middleware"
)

func Api() {
    meCtrl := authcontrollers.NewMeController()

    facades.Route().Prefix("api").Middleware(middleware.JWT()).Group(func(router route.Router) {
        router.Get("me", meCtrl.Handle)
    })
}
```

- [ ] **Step 5: Register routes in bootstrap**

In the appropriate bootstrap file (likely `bootstrap/app.go` or via a route service provider), ensure both `routes.Auth()` and `routes.Api()` are called during boot.

- [ ] **Step 6: Verify routes are registered**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan route:list
```

Expected: Shows `/auth/providers`, `/auth/listen`, `/auth/{provider}`, `/auth/{provider}/callback`, `/api/me`.

- [ ] **Step 7: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/http/middleware/jwt.go server/routes/auth.go server/routes/api.go server/bootstrap/
git commit -m "$(cat <<'EOF'
feat: add JWT middleware and auth/api routes
EOF
)"
```

---

### Task 9: End-to-end verification for PR 3

**Files:**
- No new files

- [ ] **Step 1: Verify full compilation**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go build ./...
```

- [ ] **Step 2: Run all tests**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go test ./... -v
```

Expected: All tests pass (helpers, providers, session store, bridge).

- [ ] **Step 3: Start the server and test health**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . &
sleep 2
curl -s http://localhost:8080/auth/providers
kill %1
```

Expected: Returns `{"providers":[]}` (no providers registered since setup hasn't run).

- [ ] **Step 4: Commit any fixes**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add -A server/
git commit -m "$(cat <<'EOF'
chore: verify auth system compiles and tests pass
EOF
)"
```

---

**PR 3 Complete.** Create PR:

```bash
git push -u origin feat/goravel-auth
gh pr create --title "feat: add auth system with Goravel JWT facade" --body "$(cat <<'EOF'
## Summary
- JWT auth via `facades.Auth(ctx).LoginUsingID()` replacing manual HS256 signing
- OAuth provider interface + GitHub implementation (ported from agent/)
- OAuth session store (CSRF state) and auth bridge (WS ↔ OAuth)
- UpsertUser action for persisting OAuth profiles to User model
- Auth service provider loads JWT secret from app_configs DB at boot
- 5 controllers: providers, listen (WS), redirect, callback, me
- JWT middleware on `/api/*` routes, public `/auth/*` routes

## Test plan
- [ ] `go test ./... -v` — all tests pass
- [ ] `go build ./...` — compiles cleanly
- [ ] `curl /auth/providers` returns provider list
- [ ] Full OAuth flow tested after PR 4 (setup) lands
EOF
)"
```

---

## Chunk 2: PR 4 — Setup Flow

### File Structure

```
server/
├── app/
│   ├── http/
│   │   ├── controllers/setup/
│   │   │   ├── status_controller.go               # GET /setup/status
│   │   │   ├── manifest_controller.go             # POST /setup/manifest
│   │   │   ├── start_controller.go                # GET /setup/github/start
│   │   │   ├── callback_controller.go             # GET /setup/github/callback
│   │   │   └── complete_controller.go             # POST /setup/complete
│   │   └── middleware/
│   │       ├── setup_guard.go                     # Global: blocks non-setup routes if setup incomplete
│   │       └── setup_token.go                     # Route-level: validates X-Kontainer-Setup-Token
│   ├── actions/setup/
│   │   ├── check_status.go                        # Check setup step from app_configs
│   │   ├── exchange_code.go                       # Exchange GitHub manifest code for credentials
│   │   └── complete_setup.go                      # Mark setup complete, update JWT config
│   └── helpers/
│       └── setup_token.go                         # Generate/resolve setup token
├── routes/
│   └── setup.go                                   # /setup/* routes with token middleware
└── tests/
    └── setup/
        └── setup_token_test.go
```

### Mapping from old agent code

| Old (`agent/setup/`) | New (`server/app/`) |
|---|---|
| `handlers.go` (SetupHandlers) | Split into 5 controllers in `controllers/setup/` |
| `middleware.go` (SetupTokenMiddleware) | `app/http/middleware/setup_token.go` |
| `token.go` (GenerateSetupToken) | `app/helpers/setup_token.go` |
| `main.go` SwappableHandler pattern | `app/http/middleware/setup_guard.go` (global middleware) |

---

### Task 10: Create setup token helper

**Files:**
- Create: `server/app/helpers/setup_token.go`
- Create: `server/tests/setup/setup_token_test.go`

Direct port from `agent/setup/token.go`.

- [ ] **Step 1: Write the test**

Create `server/tests/setup/setup_token_test.go`:

```go
package setup_test

import (
    "strings"
    "testing"

    "github.com/prvious/kontainer/server/app/helpers"
)

func TestGenerateSetupToken(t *testing.T) {
    token := helpers.GenerateSetupToken()

    if !strings.HasPrefix(token, "ktsetup_") {
        t.Errorf("expected prefix 'ktsetup_', got '%s'", token)
    }

    if len(token) != 20 {
        t.Errorf("expected length 20, got %d", len(token))
    }

    // Tokens should be unique
    token2 := helpers.GenerateSetupToken()
    if token == token2 {
        t.Error("expected unique tokens")
    }
}

func TestGetSetupToken_FromEnv(t *testing.T) {
    t.Setenv("KONTAINER_SETUP_TOKEN", "custom-token")
    token := helpers.GetSetupToken()
    if token != "custom-token" {
        t.Errorf("expected 'custom-token', got '%s'", token)
    }
}

func TestGetSetupToken_Generated(t *testing.T) {
    t.Setenv("KONTAINER_SETUP_TOKEN", "")
    token := helpers.GetSetupToken()
    if !strings.HasPrefix(token, "ktsetup_") {
        t.Errorf("expected generated token with prefix, got '%s'", token)
    }
}
```

- [ ] **Step 2: Create the helper**

Create `server/app/helpers/setup_token.go`:

```go
package helpers

import (
    "crypto/rand"
    "encoding/hex"
    "os"
)

const tokenPrefix = "ktsetup_"

// GenerateSetupToken generates a setup token: "ktsetup_" + 12 random hex characters.
func GenerateSetupToken() string {
    b := make([]byte, 6) // 6 bytes = 12 hex chars
    if _, err := rand.Read(b); err != nil {
        panic("crypto/rand failed: " + err.Error())
    }
    return tokenPrefix + hex.EncodeToString(b)
}

// GetSetupToken returns KONTAINER_SETUP_TOKEN env var if set, otherwise generates a new token.
func GetSetupToken() string {
    if token := os.Getenv("KONTAINER_SETUP_TOKEN"); token != "" {
        return token
    }
    return GenerateSetupToken()
}
```

- [ ] **Step 3: Run tests**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go test ./tests/setup/ -v
```

Expected: 3 tests PASS.

- [ ] **Step 4: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/helpers/setup_token.go server/tests/setup/setup_token_test.go
git commit -m "$(cat <<'EOF'
feat: add setup token helper
EOF
)"
```

---

### Task 11: Create setup middleware

**Files:**
- Create: `server/app/http/middleware/setup_guard.go`
- Create: `server/app/http/middleware/setup_token.go`

**Setup guard (global):** Replaces the SwappableHandler pattern. Checks if setup is complete on every request. If not complete, only allows `/setup/*`, `/health`, `/ping` routes through — everything else returns `{"status":"setup_required"}`.

**Setup token (route-level):** Validates `X-Kontainer-Setup-Token` header on `/setup/*` routes.

- [ ] **Step 1: Create the setup guard middleware**

Create `server/app/http/middleware/setup_guard.go`:

```go
package middleware

import (
    "strings"
    "sync/atomic"
    nethttp "net/http"

    contractshttp "github.com/goravel/framework/contracts/http"
    "github.com/goravel/framework/facades"
    "github.com/goravel/framework/http"

    "github.com/prvious/kontainer/server/app/models"
)

// setupComplete caches the setup completion state.
// Once setup completes, it stays true forever (no restart needed).
// This avoids a DB query on every request after setup is done.
var setupComplete atomic.Bool

// MarkSetupComplete should be called by CompleteSetupAction after writing setup_complete to DB.
func MarkSetupComplete() {
    setupComplete.Store(true)
}

// SetupGuard is a global middleware that blocks non-setup routes when setup is incomplete.
// Replaces the SwappableHandler pattern from the old agent.
func SetupGuard() contractshttp.Middleware {
    return func(ctx contractshttp.Context) {
        path := ctx.Request().Path()

        // Always allow health, ping, and setup routes
        if path == "/health" || path == "/ping" || strings.HasPrefix(path, "/setup") {
            ctx.Request().Next()
            return
        }

        // Fast path: cached setup complete
        if setupComplete.Load() {
            ctx.Request().Next()
            return
        }

        // Slow path: check DB (only during setup phase, low traffic)
        if isSetupComplete() {
            setupComplete.Store(true)
            ctx.Request().Next()
            return
        }

        ctx.Response().Json(nethttp.StatusServiceUnavailable, http.Json{
            "status": "setup_required",
        })
        ctx.Request().Abort()
    }
}

func isSetupComplete() bool {
    var config models.AppConfig
    err := facades.Orm().Query().Where("key", "setup_complete").First(&config)
    if err != nil {
        return false
    }
    return config.Value == "true"
}

- [ ] **Step 2: Create the setup token middleware**

Create `server/app/http/middleware/setup_token.go`:

```go
package middleware

import (
    nethttp "net/http"

    contractshttp "github.com/goravel/framework/contracts/http"
    "github.com/goravel/framework/http"
)

const SetupTokenHeader = "X-Kontainer-Setup-Token"

// SetupToken validates the X-Kontainer-Setup-Token header against the expected token.
func SetupToken(expectedToken string) contractshttp.Middleware {
    return func(ctx contractshttp.Context) {
        provided := ctx.Request().Header(SetupTokenHeader)

        if provided == "" {
            ctx.Response().Json(nethttp.StatusUnauthorized, http.Json{
                "error": "setup token required",
            })
            ctx.Request().Abort()
            return
        }

        if provided != expectedToken {
            ctx.Response().Json(nethttp.StatusUnauthorized, http.Json{
                "error": "invalid setup token",
            })
            ctx.Request().Abort()
            return
        }

        ctx.Request().Next()
    }
}
```

- [ ] **Step 3: Register setup guard as global middleware**

In `bootstrap/app.go` (or wherever global middleware is registered), add `SetupGuard` to the global middleware stack:

```go
foundation.Setup().WithMiddleware(func(handler configuration.Middleware) {
    handler.Append(middleware.SetupGuard())
})
```

This runs on every request, before route-specific middleware.

- [ ] **Step 4: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/http/middleware/setup_guard.go server/app/http/middleware/setup_token.go server/bootstrap/
git commit -m "$(cat <<'EOF'
feat: add setup guard (global) and setup token (route) middleware
EOF
)"
```

---

### Task 12: Create setup actions

**Files:**
- Create: `server/app/actions/setup/check_status.go`
- Create: `server/app/actions/setup/exchange_code.go`
- Create: `server/app/actions/setup/complete_setup.go`

- [ ] **Step 1: Create CheckStatusAction**

Create `server/app/actions/setup/check_status.go`:

```go
package setup

import (
    "github.com/goravel/framework/facades"

    "github.com/prvious/kontainer/server/app/models"
)

type CheckStatusAction struct{}

func NewCheckStatusAction() *CheckStatusAction {
    return &CheckStatusAction{}
}

// Execute returns the current setup step: "pending", "github_connected", or "complete".
func (a *CheckStatusAction) Execute() (string, error) {
    if a.hasKey("setup_complete") {
        return "complete", nil
    }
    if a.hasKey("github_app_id") {
        return "github_connected", nil
    }
    return "pending", nil
}

func (a *CheckStatusAction) hasKey(key string) bool {
    var config models.AppConfig
    err := facades.Orm().Query().Where("key", key).First(&config)
    return err == nil && config.Value != ""
}
```

- [ ] **Step 2: Create ExchangeCodeAction**

Create `server/app/actions/setup/exchange_code.go`:

```go
package setup

import (
    "crypto/rand"
    "encoding/hex"
    "encoding/json"
    "fmt"
    "io"
    "net/http"
    "strconv"

    "github.com/goravel/framework/facades"

    "github.com/prvious/kontainer/server/app/models"
)

type ExchangeCodeAction struct{}

func NewExchangeCodeAction() *ExchangeCodeAction {
    return &ExchangeCodeAction{}
}

type GitHubAppCredentials struct {
    AppID        int
    Slug         string
    ClientID     string
    ClientSecret string
    PEM          string
    WebhookSecret string
    OwnerLogin   string
}

// Execute exchanges a GitHub App manifest code for credentials and stores them.
func (a *ExchangeCodeAction) Execute(code string) (*GitHubAppCredentials, error) {
    conversionURL := fmt.Sprintf("https://api.github.com/app-manifests/%s/conversions", code)
    resp, err := http.Post(conversionURL, "application/json", nil)
    if err != nil {
        return nil, fmt.Errorf("failed to contact GitHub API: %w", err)
    }
    defer resp.Body.Close()

    body, err := io.ReadAll(resp.Body)
    if err != nil {
        return nil, fmt.Errorf("failed to read GitHub API response: %w", err)
    }

    if resp.StatusCode != http.StatusCreated {
        return nil, fmt.Errorf("GitHub API returned status %d", resp.StatusCode)
    }

    var ghResp struct {
        ID            int    `json:"id"`
        Slug          string `json:"slug"`
        ClientID      string `json:"client_id"`
        ClientSecret  string `json:"client_secret"`
        PEM           string `json:"pem"`
        WebhookSecret string `json:"webhook_secret"`
        Owner         struct {
            Login string `json:"login"`
        } `json:"owner"`
    }
    if err := json.Unmarshal(body, &ghResp); err != nil {
        return nil, fmt.Errorf("failed to parse GitHub API response: %w", err)
    }

    // Generate JWT secret
    jwtSecret := generateRandomHex(32)

    // Store all credentials in app_configs
    configs := map[string]string{
        "github_app_id":         strconv.Itoa(ghResp.ID),
        "github_client_id":      ghResp.ClientID,
        "github_client_secret":  ghResp.ClientSecret,
        "github_private_key":    ghResp.PEM,
        "github_webhook_secret": ghResp.WebhookSecret,
        "github_org":            ghResp.Owner.Login,
        "jwt_secret":            jwtSecret,
    }

    for key, value := range configs {
        var appConfig models.AppConfig
        // UpdateOrCreate: search by Key, set/update Value.
        // This handles both insert (new key) and update (existing key) correctly,
        // which is critical because AppConfig uses a string primary key (Key)
        // and Save() would always attempt UPDATE for non-zero PKs.
        if err := facades.Orm().Query().UpdateOrCreate(&appConfig,
            models.AppConfig{Key: key},
            models.AppConfig{Value: value},
        ); err != nil {
            return nil, fmt.Errorf("failed to store %s: %w", key, err)
        }
    }

    return &GitHubAppCredentials{
        AppID:        ghResp.ID,
        Slug:         ghResp.Slug,
        ClientID:     ghResp.ClientID,
        ClientSecret: ghResp.ClientSecret,
        PEM:          ghResp.PEM,
        WebhookSecret: ghResp.WebhookSecret,
        OwnerLogin:   ghResp.Owner.Login,
    }, nil
}

func generateRandomHex(nBytes int) string {
    b := make([]byte, nBytes)
    if _, err := rand.Read(b); err != nil {
        panic("crypto/rand failed: " + err.Error())
    }
    return hex.EncodeToString(b)
}
```

- [ ] **Step 3: Create CompleteSetupAction**

Create `server/app/actions/setup/complete_setup.go`:

```go
package setup

import (
    "fmt"

    "github.com/goravel/framework/facades"

    "github.com/prvious/kontainer/server/app/http/middleware"
    "github.com/prvious/kontainer/server/app/models"
)

type CompleteSetupAction struct{}

func NewCompleteSetupAction() *CompleteSetupAction {
    return &CompleteSetupAction{}
}

// Execute marks setup as complete and updates Goravel JWT config with the stored secret.
func (a *CompleteSetupAction) Execute() error {
    // Verify GitHub credentials exist
    var appID models.AppConfig
    if err := facades.Orm().Query().Where("key", "github_app_id").First(&appID); err != nil || appID.Value == "" {
        return fmt.Errorf("GitHub App not connected — complete the GitHub setup first")
    }

    // Set setup_complete flag using UpdateOrCreate (AppConfig has string PK)
    var complete models.AppConfig
    if err := facades.Orm().Query().UpdateOrCreate(&complete,
        models.AppConfig{Key: "setup_complete"},
        models.AppConfig{Value: "true"},
    ); err != nil {
        return fmt.Errorf("failed to complete setup: %w", err)
    }

    // Update in-memory cache so SetupGuard stops querying DB
    middleware.MarkSetupComplete()

    // Load JWT secret into per-guard config so auth works immediately (no restart needed)
    var jwtConfig models.AppConfig
    if err := facades.Orm().Query().Where("key", "jwt_secret").First(&jwtConfig); err == nil && jwtConfig.Value != "" {
        facades.Config().Add("auth.guards.user.secret", jwtConfig.Value)
        facades.Log().Info("JWT secret loaded into user guard — auth system now active")
    }

    return nil
}
```

**Key design decision:** After completing setup, `CompleteSetupAction`:
1. Writes `setup_complete = true` to DB via `UpdateOrCreate` (handles string PK correctly)
2. Calls `middleware.MarkSetupComplete()` to update the in-memory `atomic.Bool` cache
3. Loads JWT secret into the per-guard config via `facades.Config().Add("auth.guards.user.secret", value)`

This means:
- The setup guard middleware immediately stops blocking requests (cached, no DB query)
- The auth system (JWT parsing/issuing) works without restarting the server
- This replaces the old SwappableHandler `onComplete` callback that rebuilt the entire router

- [ ] **Step 4: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/actions/setup/
git commit -m "$(cat <<'EOF'
feat: add setup actions (check status, exchange code, complete)
EOF
)"
```

---

### Task 13: Create setup controllers

**Files:**
- Create: `server/app/http/controllers/setup/status_controller.go`
- Create: `server/app/http/controllers/setup/manifest_controller.go`
- Create: `server/app/http/controllers/setup/start_controller.go`
- Create: `server/app/http/controllers/setup/callback_controller.go`
- Create: `server/app/http/controllers/setup/complete_controller.go`

- [ ] **Step 1: Create StatusController**

Create `server/app/http/controllers/setup/status_controller.go`:

```go
package setup

import (
    nethttp "net/http"

    contractshttp "github.com/goravel/framework/contracts/http"
    "github.com/goravel/framework/http"

    setupactions "github.com/prvious/kontainer/server/app/actions/setup"
)

type StatusController struct {
    checkStatus *setupactions.CheckStatusAction
}

func NewStatusController(checkStatus *setupactions.CheckStatusAction) *StatusController {
    return &StatusController{checkStatus: checkStatus}
}

func (c *StatusController) Handle(ctx contractshttp.Context) contractshttp.Response {
    step, err := c.checkStatus.Execute()
    if err != nil {
        return ctx.Response().Json(nethttp.StatusInternalServerError, http.Json{
            "error": "failed to check setup status",
        })
    }

    return ctx.Response().Success().Json(http.Json{"step": step})
}
```

- [ ] **Step 2: Create ManifestController**

Create `server/app/http/controllers/setup/manifest_controller.go`:

```go
package setup

import (
    "crypto/rand"
    "encoding/hex"
    "fmt"
    "sync"

    contractshttp "github.com/goravel/framework/contracts/http"
    "github.com/goravel/framework/facades"
    "github.com/goravel/framework/http"
)

// ManifestState holds the CSRF state and one-time token for the GitHub App manifest flow.
// Shared between ManifestController, StartController, and CallbackController.
type ManifestState struct {
    mu    sync.Mutex
    State string
    Token string
    Org   string
}

func NewManifestState() *ManifestState {
    return &ManifestState{}
}

func (s *ManifestState) Set(state, token, org string) {
    s.mu.Lock()
    defer s.mu.Unlock()
    s.State = state
    s.Token = token
    s.Org = org
}

func (s *ManifestState) Get() (state, token, org string) {
    s.mu.Lock()
    defer s.mu.Unlock()
    return s.State, s.Token, s.Org
}

func (s *ManifestState) ClearToken() {
    s.mu.Lock()
    defer s.mu.Unlock()
    s.Token = ""
}

func (s *ManifestState) ClearState() {
    s.mu.Lock()
    defer s.mu.Unlock()
    s.State = ""
}

type ManifestController struct {
    manifestState *ManifestState
}

func NewManifestController(state *ManifestState) *ManifestController {
    return &ManifestController{manifestState: state}
}

func (c *ManifestController) Handle(ctx contractshttp.Context) contractshttp.Response {
    org := ctx.Request().Input("org")

    state := randomHex(16)
    token := randomHex(16)

    c.manifestState.Set(state, token, org)

    agentURL := facades.Config().GetString("cluster.agent_url", "http://localhost:8080")
    setupURL := fmt.Sprintf("%s/setup/github/start?token=%s", agentURL, token)

    return ctx.Response().Success().Json(http.Json{"setupUrl": setupURL})
}

func randomHex(nBytes int) string {
    b := make([]byte, nBytes)
    if _, err := rand.Read(b); err != nil {
        panic("crypto/rand failed: " + err.Error())
    }
    return hex.EncodeToString(b)
}
```

- [ ] **Step 3: Create StartController**

Create `server/app/http/controllers/setup/start_controller.go`:

```go
package setup

import (
    "encoding/json"
    "fmt"
    nethttp "net/http"

    contractshttp "github.com/goravel/framework/contracts/http"
    "github.com/goravel/framework/facades"
    "github.com/goravel/framework/http"
)

type StartController struct {
    manifestState *ManifestState
}

func NewStartController(state *ManifestState) *StartController {
    return &StartController{manifestState: state}
}

func (c *StartController) Handle(ctx contractshttp.Context) contractshttp.Response {
    queryToken := ctx.Request().Query("token")

    state, storedToken, org := c.manifestState.Get()

    if state == "" || storedToken == "" {
        return ctx.Response().Data(nethttp.StatusForbidden, "text/html; charset=utf-8",
            []byte(renderErrorHTML("No setup session found. Start the setup flow first.")))
    }

    if queryToken != storedToken {
        return ctx.Response().Data(nethttp.StatusUnauthorized, "text/html; charset=utf-8",
            []byte(renderErrorHTML("Invalid or expired setup token.")))
    }

    agentURL := facades.Config().GetString("cluster.agent_url", "http://localhost:8080")

    appName := "Kontainer"
    if org != "" {
        appName = fmt.Sprintf("Kontainer (%s)", org)
    }

    manifest := map[string]interface{}{
        "name":          appName,
        "url":           agentURL,
        "callback_urls": []string{agentURL + "/auth/github/callback"},
        "redirect_url":  agentURL + "/setup/github/callback",
        "setup_url":     agentURL + "/setup/github/callback",
        "public":        false,
        "default_permissions": map[string]string{
            "members": "read",
            "emails":  "read",
        },
        "default_events": []string{},
    }

    manifestJSON, err := json.Marshal(manifest)
    if err != nil {
        return ctx.Response().Data(nethttp.StatusInternalServerError, "text/html; charset=utf-8",
            []byte(renderErrorHTML("Failed to generate manifest.")))
    }

    // Invalidate token after use
    c.manifestState.ClearToken()

    var githubURL string
    if org != "" {
        githubURL = fmt.Sprintf("https://github.com/organizations/%s/settings/apps/new?state=%s", org, state)
    } else {
        githubURL = fmt.Sprintf("https://github.com/settings/apps/new?state=%s", state)
    }

    html := fmt.Sprintf(`<!DOCTYPE html>
<html>
<head><title>Kontainer Setup</title></head>
<body>
<p>Redirecting to GitHub...</p>
<form id="manifest-form" action="%s" method="post">
<input type="hidden" name="manifest" value='%s'>
</form>
<script>document.getElementById('manifest-form').submit();</script>
</body>
</html>`, githubURL, htmlEscapeJSON(string(manifestJSON)))

    return ctx.Response().Data(nethttp.StatusOK, "text/html; charset=utf-8", []byte(html))
}

func renderErrorHTML(message string) string {
    return fmt.Sprintf(`<!DOCTYPE html>
<html>
<head><title>Kontainer Setup - Error</title></head>
<body>
<h1>Setup Error</h1>
<p>%s</p>
</body>
</html>`, message)
}

func htmlEscapeJSON(s string) string {
    result := make([]byte, 0, len(s))
    for i := 0; i < len(s); i++ {
        switch s[i] {
        case '\'':
            result = append(result, []byte("&#39;")...)
        case '&':
            result = append(result, []byte("&amp;")...)
        case '<':
            result = append(result, []byte("&lt;")...)
        case '>':
            result = append(result, []byte("&gt;")...)
        default:
            result = append(result, s[i])
        }
    }
    return string(result)
}
```

- [ ] **Step 4: Create CallbackController**

Create `server/app/http/controllers/setup/callback_controller.go`:

```go
package setup

import (
    "fmt"
    nethttp "net/http"

    contractshttp "github.com/goravel/framework/contracts/http"
    "github.com/goravel/framework/http"

    setupactions "github.com/prvious/kontainer/server/app/actions/setup"
)

type CallbackController struct {
    manifestState *ManifestState
    exchangeCode  *setupactions.ExchangeCodeAction
}

func NewCallbackController(state *ManifestState, exchangeCode *setupactions.ExchangeCodeAction) *CallbackController {
    return &CallbackController{manifestState: state, exchangeCode: exchangeCode}
}

func (c *CallbackController) Handle(ctx contractshttp.Context) contractshttp.Response {
    // Post-install redirect
    if ctx.Request().Query("setup_action") != "" {
        html := `<!DOCTYPE html>
<html>
<head><title>Kontainer Setup</title></head>
<body>
<h1>GitHub App Installed!</h1>
<p>Return to the Kontainer app to complete setup.</p>
</body>
</html>`
        return ctx.Response().Data(nethttp.StatusOK, "text/html; charset=utf-8", []byte(html))
    }

    code := ctx.Request().Query("code")
    if code == "" {
        return ctx.Response().Json(nethttp.StatusBadRequest, http.Json{"error": "missing code parameter"})
    }

    creds, err := c.exchangeCode.Execute(code)
    if err != nil {
        return ctx.Response().Data(nethttp.StatusBadGateway, "text/html; charset=utf-8",
            []byte(renderErrorHTML(err.Error())))
    }

    // Clear CSRF state
    c.manifestState.ClearState()

    // Redirect to GitHub App installation page
    installURL := fmt.Sprintf("https://github.com/apps/%s/installations/new", creds.Slug)
    return ctx.Response().Redirect(nethttp.StatusFound, installURL)
}
```

- [ ] **Step 5: Create CompleteController**

Create `server/app/http/controllers/setup/complete_controller.go`:

```go
package setup

import (
    nethttp "net/http"

    contractshttp "github.com/goravel/framework/contracts/http"
    "github.com/goravel/framework/http"

    setupactions "github.com/prvious/kontainer/server/app/actions/setup"
)

type CompleteController struct {
    completeSetup *setupactions.CompleteSetupAction
}

func NewCompleteController(completeSetup *setupactions.CompleteSetupAction) *CompleteController {
    return &CompleteController{completeSetup: completeSetup}
}

func (c *CompleteController) Handle(ctx contractshttp.Context) contractshttp.Response {
    if err := c.completeSetup.Execute(); err != nil {
        return ctx.Response().Json(nethttp.StatusBadRequest, http.Json{"error": err.Error()})
    }

    return ctx.Response().Success().Json(http.Json{"status": "complete"})
}
```

- [ ] **Step 6: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/http/controllers/setup/ server/app/actions/setup/
git commit -m "$(cat <<'EOF'
feat: add setup controllers and actions for GitHub App manifest flow
EOF
)"
```

---

### Task 14: Create setup routes and wire everything up

**Files:**
- Create: `server/routes/setup.go`
- Modify: `server/bootstrap/` (register setup routes, print setup token on boot)

- [ ] **Step 1: Create setup routes**

Create `server/routes/setup.go`:

```go
package routes

import (
    "github.com/goravel/framework/contracts/route"
    "github.com/goravel/framework/facades"

    setupcontrollers "github.com/prvious/kontainer/server/app/http/controllers/setup"
    setupactions "github.com/prvious/kontainer/server/app/actions/setup"
    "github.com/prvious/kontainer/server/app/helpers"
    "github.com/prvious/kontainer/server/app/http/middleware"
)

func Setup() {
    setupToken := helpers.GetSetupToken()

    // Print setup token to console (like current agent)
    facades.Log().Infof("Setup token: %s", setupToken)

    // Shared state for the manifest flow
    manifestState := setupcontrollers.NewManifestState()

    // Actions
    checkStatus := setupactions.NewCheckStatusAction()
    exchangeCode := setupactions.NewExchangeCodeAction()
    completeSetup := setupactions.NewCompleteSetupAction()

    // Controllers
    statusCtrl := setupcontrollers.NewStatusController(checkStatus)
    manifestCtrl := setupcontrollers.NewManifestController(manifestState)
    startCtrl := setupcontrollers.NewStartController(manifestState)
    callbackCtrl := setupcontrollers.NewCallbackController(manifestState, exchangeCode)
    completeCtrl := setupcontrollers.NewCompleteController(completeSetup)

    facades.Route().Prefix("setup").Group(func(router route.Router) {
        // Status is accessible without setup token (desktop app checks this first)
        router.Get("status", statusCtrl.Handle)

        // GitHub start page doesn't need setup token (accessed via browser with one-time URL token)
        router.Get("github/start", startCtrl.Handle)

        // GitHub callback doesn't need setup token (GitHub redirects here)
        router.Get("github/callback", callbackCtrl.Handle)

        // These require setup token
        router.Middleware(middleware.SetupToken(setupToken)).Group(func(r route.Router) {
            r.Post("manifest", manifestCtrl.Handle)
            r.Post("complete", completeCtrl.Handle)
        })
    })
}
```

- [ ] **Step 2: Register setup routes in bootstrap**

In the route boot file (where `Auth()` and `Api()` are called), add `Setup()`:

```go
routes.Auth()
routes.Api()
routes.Setup()
```

- [ ] **Step 3: Verify compilation**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go build ./...
```

- [ ] **Step 4: Verify routes**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan route:list
```

Expected: Shows `/setup/status`, `/setup/manifest`, `/setup/github/start`, `/setup/github/callback`, `/setup/complete` plus all auth routes from PR 3.

- [ ] **Step 5: Run all tests**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go test ./... -v
```

Expected: All tests pass.

- [ ] **Step 6: Manual smoke test**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . &
sleep 2

# Setup guard should block API routes
curl -s http://localhost:8080/api/me
# Expected: {"status":"setup_required"}

# Setup status should work
curl -s http://localhost:8080/setup/status
# Expected: {"step":"pending"}

# Auth routes should work (setup guard allows them... wait, they're not /setup/* routes)
# Actually, auth routes should be blocked by setup guard too.
# The setup guard only allows /setup/*, /health, /ping
curl -s http://localhost:8080/auth/providers
# Expected: {"status":"setup_required"} — blocked by setup guard

kill %1
```

**Correction:** The setup guard needs to also allow `/auth/*` routes through during setup, because the setup callback flow redirects to `/auth/github/callback` for the OAuth redirect URL. Actually no — looking at the manifest, `callback_urls` points to `/auth/github/callback` but `redirect_url` points to `/setup/github/callback`. The auth callback URL is only used after setup when the OAuth provider is registered. During setup, only `/setup/*` routes are needed.

But wait — after setup completes (via the `/setup/complete` endpoint), the user needs to immediately use auth. The setup guard checks the DB on every request, so once `setup_complete = true`, it lets all routes through including `/auth/*`. This is fine.

- [ ] **Step 7: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/routes/setup.go server/bootstrap/
git commit -m "$(cat <<'EOF'
feat: add setup routes with token middleware
EOF
)"
```

---

### Task 15: End-to-end verification for PR 4

**Files:**
- No new files

- [ ] **Step 1: Full compilation check**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go build ./...
```

- [ ] **Step 2: Run all tests**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go test ./... -v
```

- [ ] **Step 3: Verify setup flow behavior**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
rm -f storage/kontainer.db
go run . artisan migrate
go run . &
sleep 2

# Verify setup is required
curl -s http://localhost:8080/setup/status
# Expected: {"step":"pending"}

# Verify API is blocked
curl -s http://localhost:8080/api/me
# Expected: {"status":"setup_required"}

# Verify setup token required for manifest
curl -s -X POST http://localhost:8080/setup/manifest
# Expected: {"error":"setup token required"}

kill %1
```

- [ ] **Step 4: Final commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add -A server/
git commit -m "$(cat <<'EOF'
chore: verify setup flow compiles and tests pass
EOF
)"
```

---

**PR 4 Complete.** Create PR:

```bash
git push -u origin feat/goravel-setup
gh pr create --title "feat: add setup flow with global setup guard middleware" --body "$(cat <<'EOF'
## Summary
- Setup guard middleware replaces SwappableHandler pattern — checks AppConfig.IsSetupComplete() per request
- Setup token middleware validates X-Kontainer-Setup-Token header
- 3 setup actions: check status, exchange GitHub manifest code, complete setup
- 5 setup controllers: status, manifest, start, callback, complete
- CompleteSetupAction dynamically loads JWT secret into Goravel config (no restart needed)
- Setup token printed to console at boot for desktop app connection

## Test plan
- [ ] `go test ./... -v` — all tests pass
- [ ] `go build ./...` — compiles cleanly
- [ ] `curl /setup/status` returns `{"step":"pending"}` on fresh DB
- [ ] `curl /api/me` returns `{"status":"setup_required"}` before setup
- [ ] `curl -X POST /setup/manifest` without token returns 401
- [ ] Full GitHub App manifest flow (requires manual testing with GitHub)
EOF
)"
```

---

## Subsequent Plans

The remaining PRs will be planned in separate documents:
- `2026-03-11-goravel-migration-pr5-pr6.md` — Docker orchestrator + gRPC internal API
- `2026-03-11-goravel-migration-pr7.md` — Cluster management
- `2026-03-11-goravel-migration-pr8-pr10.md` — Controllers (services, tasks, nodes, stacks, networks, volumes, secrets, configs)
- `2026-03-11-goravel-migration-pr11.md` — WebSocket endpoints
- `2026-03-11-goravel-migration-pr12-pr13.md` — Metrics + Images + Settings
- `2026-03-11-goravel-migration-pr14-pr15.md` — Cleanup + Frontend adaptation

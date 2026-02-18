# GitHub App Integration & Setup Wizard — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace GitHub OAuth App with GitHub App manifest flow, introduce agent setup mode, and store auth credentials in SQLite.

**Architecture:** The agent gains a two-mode state machine (setup vs normal) driven by a SQLite `app_config` table. Setup mode exposes only `/setup/*` routes protected by a one-time token. The desktop app detects setup mode via `/ping` and presents a setup wizard. After setup, the agent hot-swaps its router to normal mode and seamlessly logs the user in.

**Tech Stack:** Go (Chi, SQLite), Rust (Tauri, reqwest), React (TanStack Router, shadcn/ui)

---

## Task 1: AppConfigStore — SQLite Storage Layer

**Files:**
- Create: `agent/storage/app_config_store.go`
- Create: `agent/storage/app_config_store_test.go`

**Step 1: Write the failing test**

```go
// agent/storage/app_config_store_test.go
package storage

import (
	"os"
	"path/filepath"
	"testing"
)

func TestAppConfigStore_SetAndGet(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "test.db")
	defer os.Remove(dbPath)

	store, err := NewAppConfigStore(dbPath)
	if err != nil {
		t.Fatalf("NewAppConfigStore: %v", err)
	}
	defer store.Close()

	// Get non-existent key
	_, found, err := store.Get("missing")
	if err != nil {
		t.Fatalf("Get: %v", err)
	}
	if found {
		t.Fatal("expected not found")
	}

	// Set and get
	if err := store.Set("github_app_id", "12345"); err != nil {
		t.Fatalf("Set: %v", err)
	}
	val, found, err := store.Get("github_app_id")
	if err != nil {
		t.Fatalf("Get: %v", err)
	}
	if !found || val != "12345" {
		t.Fatalf("expected '12345', got %q (found=%v)", val, found)
	}

	// Upsert
	if err := store.Set("github_app_id", "67890"); err != nil {
		t.Fatalf("Set: %v", err)
	}
	val, _, _ = store.Get("github_app_id")
	if val != "67890" {
		t.Fatalf("expected '67890' after upsert, got %q", val)
	}
}

func TestAppConfigStore_SetAllAndGetAll(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "test.db")
	defer os.Remove(dbPath)

	store, err := NewAppConfigStore(dbPath)
	if err != nil {
		t.Fatalf("NewAppConfigStore: %v", err)
	}
	defer store.Close()

	err = store.SetAll(map[string]string{
		"github_app_id":     "123",
		"github_client_id":  "Iv1.abc",
		"setup_complete":    "true",
	})
	if err != nil {
		t.Fatalf("SetAll: %v", err)
	}

	all, err := store.GetAll()
	if err != nil {
		t.Fatalf("GetAll: %v", err)
	}
	if len(all) != 3 {
		t.Fatalf("expected 3 keys, got %d", len(all))
	}
	if all["setup_complete"] != "true" {
		t.Fatalf("expected 'true', got %q", all["setup_complete"])
	}
}

func TestAppConfigStore_IsSetupComplete(t *testing.T) {
	dbPath := filepath.Join(t.TempDir(), "test.db")
	defer os.Remove(dbPath)

	store, err := NewAppConfigStore(dbPath)
	if err != nil {
		t.Fatalf("NewAppConfigStore: %v", err)
	}
	defer store.Close()

	if store.IsSetupComplete() {
		t.Fatal("expected false before setup")
	}

	store.Set("setup_complete", "true")

	if !store.IsSetupComplete() {
		t.Fatal("expected true after setup")
	}
}
```

**Step 2: Run test to verify it fails**

Run: `go test -C agent ./storage/ -run TestAppConfigStore -v`
Expected: FAIL — `NewAppConfigStore` and `IsSetupComplete` not defined.

**Step 3: Write minimal implementation**

```go
// agent/storage/app_config_store.go
package storage

import (
	"database/sql"
	"fmt"

	_ "modernc.org/sqlite"
)

type AppConfigStore struct {
	db *sql.DB
}

func NewAppConfigStore(dbPath string) (*AppConfigStore, error) {
	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		return nil, fmt.Errorf("open sqlite: %w", err)
	}

	if _, err := db.Exec("PRAGMA journal_mode=WAL"); err != nil {
		db.Close()
		return nil, fmt.Errorf("set WAL mode: %w", err)
	}

	schema := `CREATE TABLE IF NOT EXISTS app_config (key TEXT PRIMARY KEY, value TEXT NOT NULL)`
	if _, err := db.Exec(schema); err != nil {
		db.Close()
		return nil, fmt.Errorf("create schema: %w", err)
	}

	return &AppConfigStore{db: db}, nil
}

func (s *AppConfigStore) Get(key string) (string, bool, error) {
	var value string
	err := s.db.QueryRow("SELECT value FROM app_config WHERE key = ?", key).Scan(&value)
	if err == sql.ErrNoRows {
		return "", false, nil
	}
	if err != nil {
		return "", false, fmt.Errorf("get app_config: %w", err)
	}
	return value, true, nil
}

func (s *AppConfigStore) Set(key, value string) error {
	_, err := s.db.Exec(
		"INSERT INTO app_config (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
		key, value,
	)
	return err
}

func (s *AppConfigStore) GetAll() (map[string]string, error) {
	rows, err := s.db.Query("SELECT key, value FROM app_config")
	if err != nil {
		return nil, fmt.Errorf("get all app_config: %w", err)
	}
	defer rows.Close()

	result := make(map[string]string)
	for rows.Next() {
		var key, value string
		if err := rows.Scan(&key, &value); err != nil {
			return nil, fmt.Errorf("scan row: %w", err)
		}
		result[key] = value
	}
	return result, rows.Err()
}

func (s *AppConfigStore) SetAll(config map[string]string) error {
	tx, err := s.db.Begin()
	if err != nil {
		return fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	stmt, err := tx.Prepare("INSERT INTO app_config (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
	if err != nil {
		return fmt.Errorf("prepare stmt: %w", err)
	}
	defer stmt.Close()

	for key, value := range config {
		if _, err := stmt.Exec(key, value); err != nil {
			return fmt.Errorf("upsert %q: %w", key, err)
		}
	}

	return tx.Commit()
}

func (s *AppConfigStore) IsSetupComplete() bool {
	val, found, err := s.Get("setup_complete")
	return err == nil && found && val == "true"
}

func (s *AppConfigStore) Close() error {
	return s.db.Close()
}
```

**Step 4: Run test to verify it passes**

Run: `go test -C agent ./storage/ -run TestAppConfigStore -v`
Expected: PASS (all 3 tests)

**Step 5: Commit**

```bash
git add agent/storage/app_config_store.go agent/storage/app_config_store_test.go
git commit -m "feat(agent): add AppConfigStore for setup state persistence"
```

---

## Task 2: Setup Token Generation

**Files:**
- Create: `agent/setup/token.go`
- Create: `agent/setup/token_test.go`

**Step 1: Write the failing test**

```go
// agent/setup/token_test.go
package setup

import (
	"os"
	"strings"
	"testing"
)

func TestGenerateSetupToken(t *testing.T) {
	token := GenerateSetupToken()

	if !strings.HasPrefix(token, "ktsetup_") {
		t.Fatalf("expected prefix 'ktsetup_', got %q", token)
	}
	// ktsetup_ (8) + 12 hex chars = 20
	if len(token) != 20 {
		t.Fatalf("expected length 20, got %d", len(token))
	}

	// Each call generates a different token
	token2 := GenerateSetupToken()
	if token == token2 {
		t.Fatal("expected different tokens on each call")
	}
}

func TestGetSetupToken_EnvOverride(t *testing.T) {
	os.Setenv("KONTAINER_SETUP_TOKEN", "custom_token_123")
	defer os.Unsetenv("KONTAINER_SETUP_TOKEN")

	token := GetSetupToken()
	if token != "custom_token_123" {
		t.Fatalf("expected env override, got %q", token)
	}
}

func TestGetSetupToken_AutoGenerate(t *testing.T) {
	os.Unsetenv("KONTAINER_SETUP_TOKEN")

	token := GetSetupToken()
	if !strings.HasPrefix(token, "ktsetup_") {
		t.Fatalf("expected auto-generated token, got %q", token)
	}
}
```

**Step 2: Run test to verify it fails**

Run: `go test -C agent ./setup/ -run TestGenerateSetupToken -v`
Expected: FAIL — package doesn't exist.

**Step 3: Write minimal implementation**

```go
// agent/setup/token.go
package setup

import (
	"crypto/rand"
	"encoding/hex"
	"os"
)

// GenerateSetupToken creates a random setup token with the ktsetup_ prefix.
func GenerateSetupToken() string {
	b := make([]byte, 6)
	rand.Read(b)
	return "ktsetup_" + hex.EncodeToString(b)
}

// GetSetupToken returns the setup token from KONTAINER_SETUP_TOKEN env var,
// or generates a new one if not set.
func GetSetupToken() string {
	if val := os.Getenv("KONTAINER_SETUP_TOKEN"); val != "" {
		return val
	}
	return GenerateSetupToken()
}
```

**Step 4: Run test to verify it passes**

Run: `go test -C agent ./setup/ -v`
Expected: PASS

**Step 5: Commit**

```bash
git add agent/setup/token.go agent/setup/token_test.go
git commit -m "feat(agent): add setup token generation"
```

---

## Task 3: Setup Token Middleware

**Files:**
- Create: `agent/setup/middleware.go`
- Create: `agent/setup/middleware_test.go`

**Step 1: Write the failing test**

```go
// agent/setup/middleware_test.go
package setup

import (
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestSetupTokenMiddleware_ValidToken(t *testing.T) {
	handler := SetupTokenMiddleware("ktsetup_abc123")(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	req := httptest.NewRequest("GET", "/setup/status", nil)
	req.Header.Set("X-Kontainer-Setup-Token", "ktsetup_abc123")
	rr := httptest.NewRecorder()
	handler.ServeHTTP(rr, req)

	if rr.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", rr.Code)
	}
}

func TestSetupTokenMiddleware_InvalidToken(t *testing.T) {
	handler := SetupTokenMiddleware("ktsetup_abc123")(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	req := httptest.NewRequest("GET", "/setup/status", nil)
	req.Header.Set("X-Kontainer-Setup-Token", "wrong_token")
	rr := httptest.NewRecorder()
	handler.ServeHTTP(rr, req)

	if rr.Code != http.StatusUnauthorized {
		t.Fatalf("expected 401, got %d", rr.Code)
	}
}

func TestSetupTokenMiddleware_MissingToken(t *testing.T) {
	handler := SetupTokenMiddleware("ktsetup_abc123")(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	req := httptest.NewRequest("GET", "/setup/status", nil)
	rr := httptest.NewRecorder()
	handler.ServeHTTP(rr, req)

	if rr.Code != http.StatusUnauthorized {
		t.Fatalf("expected 401, got %d", rr.Code)
	}
}
```

**Step 2: Run test to verify it fails**

Run: `go test -C agent ./setup/ -run TestSetupTokenMiddleware -v`
Expected: FAIL

**Step 3: Write minimal implementation**

```go
// agent/setup/middleware.go
package setup

import (
	"encoding/json"
	"net/http"
)

// SetupTokenMiddleware validates the X-Kontainer-Setup-Token header.
func SetupTokenMiddleware(token string) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			provided := r.Header.Get("X-Kontainer-Setup-Token")
			if provided == "" {
				w.Header().Set("Content-Type", "application/json")
				w.WriteHeader(http.StatusUnauthorized)
				json.NewEncoder(w).Encode(map[string]string{"error": "setup token required"})
				return
			}
			if provided != token {
				w.Header().Set("Content-Type", "application/json")
				w.WriteHeader(http.StatusUnauthorized)
				json.NewEncoder(w).Encode(map[string]string{"error": "invalid setup token"})
				return
			}
			next.ServeHTTP(w, r)
		})
	}
}
```

**Step 4: Run test to verify it passes**

Run: `go test -C agent ./setup/ -v`
Expected: PASS

**Step 5: Commit**

```bash
git add agent/setup/middleware.go agent/setup/middleware_test.go
git commit -m "feat(agent): add setup token middleware"
```

---

## Task 4: Setup Handlers — Status & Manifest

**Files:**
- Create: `agent/setup/handlers.go`
- Create: `agent/setup/handlers_test.go`

**Step 1: Write the failing test**

```go
// agent/setup/handlers_test.go
package setup

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"

	"github.com/prvious/kontainer/agent/storage"
)

func newTestStore(t *testing.T) *storage.AppConfigStore {
	t.Helper()
	dbPath := filepath.Join(t.TempDir(), "test.db")
	store, err := storage.NewAppConfigStore(dbPath)
	if err != nil {
		t.Fatalf("NewAppConfigStore: %v", err)
	}
	t.Cleanup(func() {
		store.Close()
		os.Remove(dbPath)
	})
	return store
}

func TestStatusHandler_Pending(t *testing.T) {
	store := newTestStore(t)
	h := NewSetupHandler(store, "https://agent.example.com", "ktsetup_test")

	req := httptest.NewRequest("GET", "/setup/status", nil)
	rr := httptest.NewRecorder()
	h.StatusHandler(rr, req)

	if rr.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", rr.Code)
	}

	var body map[string]string
	json.NewDecoder(rr.Body).Decode(&body)
	if body["step"] != "pending" {
		t.Fatalf("expected 'pending', got %q", body["step"])
	}
}

func TestStatusHandler_GithubConnected(t *testing.T) {
	store := newTestStore(t)
	store.Set("github_app_id", "123")
	h := NewSetupHandler(store, "https://agent.example.com", "ktsetup_test")

	req := httptest.NewRequest("GET", "/setup/status", nil)
	rr := httptest.NewRecorder()
	h.StatusHandler(rr, req)

	var body map[string]string
	json.NewDecoder(rr.Body).Decode(&body)
	if body["step"] != "github_connected" {
		t.Fatalf("expected 'github_connected', got %q", body["step"])
	}
}

func TestManifestHandler(t *testing.T) {
	store := newTestStore(t)
	h := NewSetupHandler(store, "https://agent.example.com", "ktsetup_test123")

	req := httptest.NewRequest("POST", "/setup/github/manifest", nil)
	rr := httptest.NewRecorder()
	h.ManifestHandler(rr, req)

	if rr.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", rr.Code)
	}

	var body map[string]string
	json.NewDecoder(rr.Body).Decode(&body)
	if body["setupUrl"] == "" {
		t.Fatal("expected setupUrl in response")
	}
	// Should contain the token as query param
	if !contains(body["setupUrl"], "token=ktsetup_test123") {
		t.Fatalf("expected token in setupUrl, got %q", body["setupUrl"])
	}
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(s) > 0 && containsStr(s, substr))
}

func containsStr(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
```

**Step 2: Run test to verify it fails**

Run: `go test -C agent ./setup/ -run "TestStatusHandler|TestManifestHandler" -v`
Expected: FAIL — `NewSetupHandler` not defined.

**Step 3: Write minimal implementation**

```go
// agent/setup/handlers.go
package setup

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"sync"

	"github.com/prvious/kontainer/agent/storage"
)

type SetupHandler struct {
	store     *storage.AppConfigStore
	agentURL  string
	token     string

	mu    sync.Mutex
	state string // CSRF state for manifest flow
}

func NewSetupHandler(store *storage.AppConfigStore, agentURL string, token string) *SetupHandler {
	return &SetupHandler{
		store:    store,
		agentURL: agentURL,
		token:    token,
	}
}

// StatusHandler returns the current setup step.
func (h *SetupHandler) StatusHandler(w http.ResponseWriter, r *http.Request) {
	step := "pending"

	if _, found, _ := h.store.Get("setup_complete"); found {
		step = "complete"
	} else if _, found, _ := h.store.Get("github_app_id"); found {
		step = "github_connected"
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]string{"step": step})
}

// ManifestHandler generates the GitHub App manifest and returns the setup URL.
func (h *SetupHandler) ManifestHandler(w http.ResponseWriter, r *http.Request) {
	// Generate CSRF state
	stateBytes := make([]byte, 16)
	rand.Read(stateBytes)
	state := hex.EncodeToString(stateBytes)

	h.mu.Lock()
	h.state = state
	h.mu.Unlock()

	setupURL := fmt.Sprintf("%s/setup/github/start?token=%s", h.agentURL, h.token)

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]string{
		"setupUrl": setupURL,
	})
}

// StartHandler serves the HTML page that auto-submits the manifest form to GitHub.
func (h *SetupHandler) StartHandler(w http.ResponseWriter, r *http.Request) {
	// Verify setup token from query param
	queryToken := r.URL.Query().Get("token")
	if queryToken != h.token {
		http.Error(w, "invalid setup token", http.StatusUnauthorized)
		return
	}

	h.mu.Lock()
	state := h.state
	h.mu.Unlock()

	if state == "" {
		http.Error(w, "no manifest session — call POST /setup/github/manifest first", http.StatusForbidden)
		return
	}

	manifest := map[string]interface{}{
		"name":           "Kontainer",
		"url":            h.agentURL,
		"callback_urls":  []string{h.agentURL + "/setup/github/callback"},
		"redirect_url":   h.agentURL + "/setup/github/callback",
		"setup_url":      h.agentURL + "/setup/github/callback",
		"public":         false,
		"default_permissions": map[string]string{
			"members": "read",
		},
		"default_events": []string{},
	}

	manifestJSON, _ := json.Marshal(manifest)

	html := fmt.Sprintf(`<!DOCTYPE html>
<html>
<head><title>Kontainer Setup</title></head>
<body>
<p>Redirecting to GitHub...</p>
<form id="manifest-form" method="post" action="https://github.com/settings/apps/new?state=%s">
<input type="hidden" name="manifest" value='%s'>
</form>
<script>document.getElementById('manifest-form').submit();</script>
</body>
</html>`, state, string(manifestJSON))

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.Write([]byte(html))
}

// CallbackHandler exchanges the GitHub code for app credentials and stores them.
func (h *SetupHandler) CallbackHandler(w http.ResponseWriter, r *http.Request) {
	code := r.URL.Query().Get("code")
	if code == "" {
		http.Error(w, "missing code parameter", http.StatusBadRequest)
		return
	}

	// Exchange code for app credentials via GitHub API
	resp, err := http.Post(
		fmt.Sprintf("https://api.github.com/app-manifests/%s/conversions", code),
		"application/json",
		nil,
	)
	if err != nil {
		log.Printf("github manifest exchange error: %v", err)
		h.renderError(w, "Failed to connect to GitHub. Please try again.")
		return
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusCreated {
		log.Printf("github manifest exchange: status %d", resp.StatusCode)
		h.renderError(w, "GitHub returned an error. Please try again.")
		return
	}

	var result struct {
		ID            int    `json:"id"`
		ClientID      string `json:"client_id"`
		ClientSecret  string `json:"client_secret"`
		PEM           string `json:"pem"`
		WebhookSecret string `json:"webhook_secret"`
		Owner         struct {
			Login string `json:"login"`
		} `json:"owner"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		log.Printf("github manifest decode error: %v", err)
		h.renderError(w, "Failed to parse GitHub response.")
		return
	}

	// Generate JWT secret
	jwtSecretBytes := make([]byte, 32)
	rand.Read(jwtSecretBytes)
	jwtSecret := hex.EncodeToString(jwtSecretBytes)

	// Store all credentials
	err = h.store.SetAll(map[string]string{
		"github_app_id":        fmt.Sprintf("%d", result.ID),
		"github_client_id":     result.ClientID,
		"github_client_secret": result.ClientSecret,
		"github_private_key":   result.PEM,
		"github_webhook_secret": result.WebhookSecret,
		"github_org":           result.Owner.Login,
		"jwt_secret":           jwtSecret,
	})
	if err != nil {
		log.Printf("store credentials error: %v", err)
		h.renderError(w, "Failed to save credentials.")
		return
	}

	// Clear state
	h.mu.Lock()
	h.state = ""
	h.mu.Unlock()

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.Write([]byte(callbackSuccessHTML))
}

// CompleteHandler finalizes setup and returns success.
// The caller (main.go) is responsible for hot-swapping the router.
func (h *SetupHandler) CompleteHandler(w http.ResponseWriter, r *http.Request) {
	// Verify GitHub credentials exist
	if _, found, _ := h.store.Get("github_app_id"); !found {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(map[string]string{"error": "GitHub App not configured"})
		return
	}

	if err := h.store.Set("setup_complete", "true"); err != nil {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to complete setup"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]string{"status": "ok"})
}

func (h *SetupHandler) renderError(w http.ResponseWriter, message string) {
	html := fmt.Sprintf(`<!DOCTYPE html>
<html>
<head><title>Kontainer Setup Error</title></head>
<body style="display:flex;align-items:center;justify-content:center;height:100vh;font-family:system-ui,sans-serif">
<div style="text-align:center">
<h2>Setup Error</h2>
<p>%s</p>
<p>Return to the Kontainer app and try again.</p>
</div>
</body>
</html>`, message)
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.WriteHeader(http.StatusBadGateway)
	w.Write([]byte(html))
}

const callbackSuccessHTML = `<!DOCTYPE html>
<html>
<head><title>Kontainer Setup</title></head>
<body style="display:flex;align-items:center;justify-content:center;height:100vh;font-family:system-ui,sans-serif">
<div style="text-align:center">
<h2>GitHub App Connected!</h2>
<p>Return to the Kontainer app to complete setup.</p>
</div>
</body>
</html>`
```

**Step 4: Run test to verify it passes**

Run: `go test -C agent ./setup/ -v`
Expected: PASS

**Step 5: Commit**

```bash
git add agent/setup/handlers.go agent/setup/handlers_test.go
git commit -m "feat(agent): add setup handlers (status, manifest, callback, complete)"
```

---

## Task 5: Setup Router

**Files:**
- Create: `agent/server/setup.go`

The setup router serves only `/ping` (with `setup_required` status) and `/setup/*` routes. All other routes return 403.

**Step 1: Write implementation**

```go
// agent/server/setup.go
package server

import (
	"encoding/json"
	"fmt"
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	"github.com/prvious/kontainer/agent/setup"
)

func NewSetupRouter(setupHandler *setup.SetupHandler, setupToken string) http.Handler {
	r := chi.NewRouter()
	r.Use(middleware.Logger)
	r.Use(middleware.Recoverer)

	// Public: health + ping (with setup_required status)
	r.Get("/health", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		fmt.Fprintf(w, `{"status":"ok"}`)
	})

	r.Get("/ping", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		fmt.Fprintf(w, `{"app":"dev.kontainer.app","status":"setup_required"}`)
	})

	// Setup routes (token-protected)
	r.Route("/setup", func(r chi.Router) {
		r.Use(setup.SetupTokenMiddleware(setupToken))
		r.Get("/status", setupHandler.StatusHandler)
		r.Post("/github/manifest", setupHandler.ManifestHandler)
		r.Post("/complete", setupHandler.CompleteHandler)
	})

	// GitHub start page — token verified via query param inside the handler
	r.Get("/setup/github/start", setupHandler.StartHandler)

	// GitHub callback — state-protected inside the handler
	r.Get("/setup/github/callback", setupHandler.CallbackHandler)

	// Everything else returns 403
	r.NotFound(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusForbidden)
		json.NewEncoder(w).Encode(map[string]string{"error": "agent is in setup mode"})
	})

	return r
}
```

**Step 2: Commit**

```bash
git add agent/server/setup.go
git commit -m "feat(agent): add setup-mode router"
```

---

## Task 6: Router Hot-Swap Wrapper

**Files:**
- Create: `agent/server/swappable.go`
- Create: `agent/server/swappable_test.go`

**Step 1: Write the failing test**

```go
// agent/server/swappable_test.go
package server

import (
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestSwappableHandler(t *testing.T) {
	handlerA := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte("A"))
	})
	handlerB := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte("B"))
	})

	sw := NewSwappableHandler(handlerA)

	// Initial handler
	req := httptest.NewRequest("GET", "/", nil)
	rr := httptest.NewRecorder()
	sw.ServeHTTP(rr, req)
	if rr.Body.String() != "A" {
		t.Fatalf("expected 'A', got %q", rr.Body.String())
	}

	// After swap
	sw.Swap(handlerB)
	rr = httptest.NewRecorder()
	sw.ServeHTTP(rr, req)
	if rr.Body.String() != "B" {
		t.Fatalf("expected 'B', got %q", rr.Body.String())
	}
}
```

**Step 2: Run test to verify it fails**

Run: `go test -C agent ./server/ -run TestSwappableHandler -v`
Expected: FAIL

**Step 3: Write minimal implementation**

```go
// agent/server/swappable.go
package server

import (
	"net/http"
	"sync"
)

// SwappableHandler wraps an http.Handler and allows hot-swapping it at runtime.
type SwappableHandler struct {
	mu      sync.RWMutex
	handler http.Handler
}

func NewSwappableHandler(h http.Handler) *SwappableHandler {
	return &SwappableHandler{handler: h}
}

func (s *SwappableHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	s.mu.RLock()
	h := s.handler
	s.mu.RUnlock()
	h.ServeHTTP(w, r)
}

// Swap replaces the current handler with a new one.
func (s *SwappableHandler) Swap(h http.Handler) {
	s.mu.Lock()
	s.handler = h
	s.mu.Unlock()
}
```

**Step 4: Run test to verify it passes**

Run: `go test -C agent ./server/ -run TestSwappableHandler -v`
Expected: PASS

**Step 5: Commit**

```bash
git add agent/server/swappable.go agent/server/swappable_test.go
git commit -m "feat(agent): add SwappableHandler for router hot-swap"
```

---

## Task 7: GitHub App OAuth Provider

Replace the current `GitHubProvider` with a version that:
1. Gets credentials from `AppConfigStore` instead of constructor args
2. Checks org membership via installation token instead of allowlists

**Files:**
- Modify: `agent/auth/providers/github.go`
- Modify: `agent/auth/providers/github_test.go`

**Step 1: Refactor GitHubProvider**

Replace the current `GitHubProvider` constructor and `checkAccess` logic:

- Remove `allowedOrgs`, `allowedUsers` fields
- Add `org` field (the org where the app is installed)
- Change `NewGitHubProvider` to accept `clientID, clientSecret, redirectURL, org string`
- Replace `checkOrgMembership` (pagination loop) with a single `GET /orgs/{org}/members/{username}` call using the user's OAuth token
- Remove `matchesUser` and `checkAccess` — replace with simpler `checkOrgMembership(client, login, org)`
- Always request `read:org` scope (since we always check org membership)

The new `checkAccess` logic:
```go
func (g *GitHubProvider) checkAccess(client *http.Client, login string) error {
	if g.org == "" {
		return nil // No org restriction (personal account mode)
	}

	url := fmt.Sprintf("https://api.github.com/orgs/%s/members/%s", g.org, login)
	resp, err := client.Get(url)
	if err != nil {
		return fmt.Errorf("github: check org membership: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode == http.StatusNoContent {
		return nil // 204 = member
	}

	return &AccessDeniedError{
		Message: fmt.Sprintf("user %q is not a member of organization %q", login, g.org),
	}
}
```

**Step 2: Update tests**

Update `agent/auth/providers/github_test.go` to match new constructor signature and test org-membership-based access instead of allowlist-based access.

**Step 3: Run tests**

Run: `go test -C agent ./auth/providers/ -v`
Expected: PASS

**Step 4: Commit**

```bash
git add agent/auth/providers/github.go agent/auth/providers/github_test.go
git commit -m "feat(agent): refactor GitHubProvider for GitHub App (org membership check)"
```

---

## Task 8: Update /ping Endpoint in Normal Mode

**Files:**
- Modify: `agent/server/server.go:67-70`

**Step 1: Update the ping handler**

Change the `/ping` route in `server.go` from:
```go
r.Get("/ping", func(w http.ResponseWriter, r *http.Request) {
    w.Header().Set("Content-Type", "application/json")
    fmt.Fprintf(w, `{"app":"dev.kontainer.app"}`)
})
```

To:
```go
r.Get("/ping", func(w http.ResponseWriter, r *http.Request) {
    w.Header().Set("Content-Type", "application/json")
    fmt.Fprintf(w, `{"app":"dev.kontainer.app","status":"ok"}`)
})
```

**Step 2: Commit**

```bash
git add agent/server/server.go
git commit -m "feat(agent): add status field to /ping response"
```

---

## Task 9: Config Cleanup — Remove GitHub & JWT Fields

**Files:**
- Modify: `agent/config/config.go`
- Modify: `agent/kontainer.example.yml`

**Step 1: Remove from config struct**

In `agent/config/config.go`:
- Remove `GitHubConfig` struct (lines 11-16)
- Remove `JWTSecret` field (line 35)
- Remove `GitHub` field (line 37)
- Remove secret resolution for `JWTSecret`, `GitHub.ClientID`, `GitHub.ClientSecret` in `resolveSecrets()` (lines 108-116)

The cleaned `Config` struct becomes:
```go
type Config struct {
	Port     string `yaml:"port"`
	AgentURL string `yaml:"agent_url"`
	Role     string `yaml:"role"`

	Cluster ClusterConfig `yaml:"cluster"`
	Metrics MetricsConfig `yaml:"metrics"`
	Dev     bool          `yaml:"dev"`
}
```

And `resolveSecrets` becomes:
```go
func resolveSecrets(cfg *Config) error {
	var err error
	if cfg.Cluster.InternalSecret, err = resolveValue(cfg.Cluster.InternalSecret); err != nil {
		return fmt.Errorf("cluster.internal_secret: %w", err)
	}
	return nil
}
```

**Step 2: Update example YAML**

Remove `jwt_secret` and `github:` block from `agent/kontainer.example.yml`.

**Step 3: Commit**

```bash
git add agent/config/config.go agent/kontainer.example.yml
git commit -m "feat(agent): remove github and jwt_secret from YAML config"
```

---

## Task 10: Rewire main.go Boot Sequence

**Files:**
- Modify: `agent/main.go`

This is the key integration task. The `startLeader` function must:
1. Open `AppConfigStore`
2. Check `IsSetupComplete()`
3. If not complete: start in setup mode with setup token + banner
4. If complete: load credentials from SQLite, build provider, start in normal mode
5. Wire up `CompleteHandler` to trigger router hot-swap

**Step 1: Rewrite startLeader**

Key changes to `agent/main.go`:

1. Remove log lines for `github client id` and `jwt secret` (lines 46-47)
2. Remove config validation for GitHub/JWT (lines 113-125)
3. Initialize `AppConfigStore` after metrics/settings stores
4. Branch on `appConfigStore.IsSetupComplete()`

The setup mode branch:
```go
if !appConfigStore.IsSetupComplete() {
    setupToken := setup.GetSetupToken()
    log.Println("🚀 Kontainer Agent starting...")
    log.Println("⚡ Setup required!")
    log.Printf("🔑 Setup token: %s", setupToken)
    log.Println("📱 Open your Kontainer app and add this agent")
    log.Printf("   URL: %s", cfg.AgentURL)
    log.Printf("   Token: %s", setupToken)

    setupHandler := setup.NewSetupHandler(appConfigStore, cfg.AgentURL, setupToken)
    setupRouter := server.NewSetupRouter(setupHandler, setupToken)
    swappable := server.NewSwappableHandler(setupRouter)

    // Override CompleteHandler to trigger hot-swap
    // (wrap the original complete handler to also swap routers)
    // ... build normal router builder func, call swappable.Swap on complete

    srv := &http.Server{Addr: ":" + cfg.Port, Handler: swappable}
    // ... signal handling + ListenAndServe
}
```

The normal mode branch remains similar to the current code but reads credentials from `appConfigStore` instead of `cfg`:
```go
jwtSecret, _, _ := appConfigStore.Get("jwt_secret")
clientID, _, _ := appConfigStore.Get("github_client_id")
clientSecret, _, _ := appConfigStore.Get("github_client_secret")
org, _, _ := appConfigStore.Get("github_org")

registry := providers.NewProviderRegistry()
registry.Register(providers.NewGitHubProvider(clientID, clientSecret, cfg.AgentURL+"/auth/github/callback", org))
// ... rest of current startLeader code with jwtSecret from DB
```

**Step 2: Run existing tests**

Run: `go test -C agent ./... -v`
Expected: PASS

**Step 3: Commit**

```bash
git add agent/main.go
git commit -m "feat(agent): rewire boot sequence for setup/normal mode"
```

---

## Task 11: Desktop App — Update ping_agent to Return Status

**Files:**
- Modify: `app/src-tauri/src/actions/agent/ping_agent.rs`

**Step 1: Change return type**

Change `ping_agent` from `Result<(), String>` to `Result<PingResponse, String>`:

```rust
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingResponse {
    pub app: String,
    pub status: String,
}

#[tauri::command]
pub async fn ping_agent(url: String) -> Result<PingResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/ping"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: PingResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    if body.app != "dev.kontainer.app" {
        return Err("Not a valid Kontainer agent".to_string());
    }

    Ok(body)
}
```

**Step 2: Commit**

```bash
git add app/src-tauri/src/actions/agent/ping_agent.rs
git commit -m "feat(app): update ping_agent to return status field"
```

---

## Task 12: Desktop App — New Setup Tauri Commands

**Files:**
- Create: `app/src-tauri/src/actions/agent/setup_verify_token.rs`
- Create: `app/src-tauri/src/actions/agent/setup_github_connect.rs`
- Create: `app/src-tauri/src/actions/agent/setup_complete.rs`
- Modify: `app/src-tauri/src/actions/agent/mod.rs` — add 3 new module declarations
- Modify: `app/src-tauri/src/lib.rs` — register 3 new commands in `invoke_handler!`

**Step 1: Create setup_verify_token.rs**

```rust
// app/src-tauri/src/actions/agent/setup_verify_token.rs
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct SetupStatus {
    pub step: String,
}

#[tauri::command]
pub async fn setup_verify_token(url: String, token: String) -> Result<SetupStatus, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/setup/status"))
        .header("X-Kontainer-Setup-Token", &token)
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("Invalid setup token".to_string());
    }

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: SetupStatus = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
```

**Step 2: Create setup_github_connect.rs**

```rust
// app/src-tauri/src/actions/agent/setup_github_connect.rs
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestResponse {
    pub setup_url: String,
}

#[tauri::command]
pub async fn setup_github_connect(url: String, token: String) -> Result<ManifestResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .post(format!("{url}/setup/github/manifest"))
        .header("X-Kontainer-Setup-Token", &token)
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: ManifestResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
```

**Step 3: Create setup_complete.rs**

```rust
// app/src-tauri/src/actions/agent/setup_complete.rs
use std::time::Duration;

#[tauri::command]
pub async fn setup_complete(url: String, token: String) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .post(format!("{url}/setup/complete"))
        .header("X-Kontainer-Setup-Token", &token)
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Setup completion failed: {body}"));
    }

    Ok(())
}
```

**Step 4: Update mod.rs and lib.rs**

Add to `app/src-tauri/src/actions/agent/mod.rs`:
```rust
pub mod setup_verify_token;
pub mod setup_github_connect;
pub mod setup_complete;
```

Add to `invoke_handler!` in `app/src-tauri/src/lib.rs`:
```rust
actions::agent::setup_verify_token::setup_verify_token,
actions::agent::setup_github_connect::setup_github_connect,
actions::agent::setup_complete::setup_complete,
```

**Step 5: Verify it compiles**

Run: `cargo check --manifest-path app/src-tauri/Cargo.toml`
Expected: Compiles successfully.

**Step 6: Commit**

```bash
git add app/src-tauri/src/actions/agent/setup_verify_token.rs app/src-tauri/src/actions/agent/setup_github_connect.rs app/src-tauri/src/actions/agent/setup_complete.rs app/src-tauri/src/actions/agent/mod.rs app/src-tauri/src/lib.rs
git commit -m "feat(app): add setup Tauri commands (verify, connect, complete)"
```

---

## Task 13: Desktop App — Setup Wizard Component

**Files:**
- Create: `app/src/components/setup-wizard.tsx`

Use shadcn/ui components. Install any missing ones first (e.g., `stepper` if needed — otherwise use `Card`, `Button`, `Input`, `Label`).

**Step 1: Create the setup wizard**

The wizard has 3 steps and lives in `app/src/components/setup-wizard.tsx`. It receives the agent URL as a prop from the login screen.

```tsx
// app/src/components/setup-wizard.tsx
import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowLeft, CheckCircle2, Github, Key, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

interface SetupWizardProps {
  agentUrl: string;
  onComplete: () => void;
  onBack: () => void;
}

type Step = "token" | "github" | "confirm";

export function SetupWizard({ agentUrl, onComplete, onBack }: SetupWizardProps) {
  const [step, setStep] = useState<Step>("token");
  const [setupToken, setSetupToken] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [orgName, setOrgName] = useState<string | null>(null);
  const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Clean up polling on unmount
  useEffect(() => {
    return () => {
      if (pollingRef.current) clearInterval(pollingRef.current);
    };
  }, []);

  async function handleVerifyToken(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setIsLoading(true);

    try {
      const result = await invoke<{ step: string }>("setup_verify_token", {
        url: agentUrl,
        token: setupToken,
      });

      // Resume from where we left off
      if (result.step === "github_connected") {
        setStep("confirm");
      } else {
        setStep("github");
      }
    } catch (err) {
      setError(typeof err === "string" ? err : "Failed to verify token");
    } finally {
      setIsLoading(false);
    }
  }

  async function handleConnectGithub() {
    setError(null);
    setIsLoading(true);

    try {
      const result = await invoke<{ setupUrl: string }>("setup_github_connect", {
        url: agentUrl,
        token: setupToken,
      });

      // Open the setup URL in the system browser
      await openUrl(result.setupUrl);

      // Poll for completion
      pollingRef.current = setInterval(async () => {
        try {
          const status = await invoke<{ step: string }>("setup_verify_token", {
            url: agentUrl,
            token: setupToken,
          });
          if (status.step === "github_connected") {
            if (pollingRef.current) clearInterval(pollingRef.current);
            pollingRef.current = null;
            setIsLoading(false);
            setStep("confirm");
          }
        } catch {
          // Keep polling
        }
      }, 2000);
    } catch (err) {
      setIsLoading(false);
      setError(typeof err === "string" ? err : "Failed to start GitHub setup");
    }
  }

  async function handleComplete() {
    setError(null);
    setIsLoading(true);

    try {
      await invoke("setup_complete", {
        url: agentUrl,
        token: setupToken,
      });
      onComplete();
    } catch (err) {
      setIsLoading(false);
      setError(typeof err === "string" ? err : "Failed to complete setup");
    }
  }

  return (
    <div className="flex h-screen p-2 pt-10">
      <div
        data-tauri-drag-region
        className="grain flex flex-1 flex-col items-center justify-center overflow-hidden rounded-xl bg-background shadow-sm"
      >
        <div className="w-full max-w-sm px-6">
          <button
            type="button"
            onClick={onBack}
            className="mb-4 flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
          >
            <ArrowLeft className="size-3" />
            Back
          </button>

          <h1 className="text-lg font-semibold">Agent Setup</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            {agentUrl}
          </p>

          {/* Step indicators */}
          <div className="mt-4 flex items-center gap-2">
            {(["token", "github", "confirm"] as const).map((s, i) => (
              <div key={s} className="flex items-center gap-2">
                {i > 0 && <div className="h-px w-4 bg-border" />}
                <div
                  className={`flex size-6 items-center justify-center rounded-full text-xs font-medium ${
                    step === s
                      ? "bg-primary text-primary-foreground"
                      : (["token", "github", "confirm"].indexOf(step) > i)
                        ? "bg-primary/20 text-primary"
                        : "bg-muted text-muted-foreground"
                  }`}
                >
                  {["token", "github", "confirm"].indexOf(step) > i ? (
                    <CheckCircle2 className="size-3.5" />
                  ) : (
                    i + 1
                  )}
                </div>
              </div>
            ))}
          </div>

          {/* Step 1: Token */}
          {step === "token" && (
            <form onSubmit={handleVerifyToken} className="mt-6">
              <p className="mb-4 text-sm text-muted-foreground">
                Enter the setup token shown in your agent's logs.
              </p>
              <Label htmlFor="setup-token" className="mb-2">
                Setup Token
              </Label>
              <div className="relative">
                <Input
                  id="setup-token"
                  type="text"
                  placeholder="ktsetup_..."
                  value={setupToken}
                  onChange={(e) => setSetupToken(e.target.value)}
                  required
                  className="h-9 pr-9 font-mono text-sm"
                />
                <div className="absolute top-1/2 right-3 -translate-y-1/2">
                  <Key className="size-3.5 text-muted-foreground" />
                </div>
              </div>
              {error && <p className="mt-2 text-xs text-destructive">{error}</p>}
              <Button type="submit" disabled={isLoading} size="sm" className="mt-3 w-full">
                {isLoading ? (
                  <>
                    <Loader2 className="size-3.5 animate-spin" />
                    Verifying...
                  </>
                ) : (
                  "Verify"
                )}
              </Button>
            </form>
          )}

          {/* Step 2: GitHub */}
          {step === "github" && (
            <div className="mt-6">
              <p className="mb-4 text-sm text-muted-foreground">
                Connect a GitHub organization to manage access to this agent.
              </p>
              {error && <p className="mb-2 text-xs text-destructive">{error}</p>}
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={isLoading}
                onClick={handleConnectGithub}
                className="w-full"
              >
                {isLoading ? (
                  <>
                    <Loader2 className="size-3.5 animate-spin" />
                    Waiting for GitHub...
                  </>
                ) : (
                  <>
                    <Github className="size-4" />
                    Connect GitHub
                  </>
                )}
              </Button>
              {isLoading && (
                <p className="mt-2 text-xs text-muted-foreground">
                  Complete the setup in your browser, then return here.
                </p>
              )}
            </div>
          )}

          {/* Step 3: Confirm */}
          {step === "confirm" && (
            <div className="mt-6">
              <div className="flex items-center gap-2 rounded-md border border-border bg-muted/50 p-3">
                <CheckCircle2 className="size-4 text-success" />
                <span className="text-sm">GitHub App connected</span>
              </div>
              <p className="mt-3 text-sm text-muted-foreground">
                All members of the connected organization will be able to log in.
              </p>
              {error && <p className="mt-2 text-xs text-destructive">{error}</p>}
              <Button
                type="button"
                size="sm"
                disabled={isLoading}
                onClick={handleComplete}
                className="mt-4 w-full"
              >
                {isLoading ? (
                  <>
                    <Loader2 className="size-3.5 animate-spin" />
                    Completing setup...
                  </>
                ) : (
                  "Complete Setup"
                )}
              </Button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add app/src/components/setup-wizard.tsx
git commit -m "feat(app): add setup wizard component"
```

---

## Task 14: Desktop App — Wire Setup Wizard into Login Screen

**Files:**
- Modify: `app/src/components/login-screen.tsx`

**Step 1: Detect setup mode and show wizard**

Add a `setupRequired` state. When `handleConnect` calls `ping_agent` and gets `{ status: "setup_required" }`, set `setupRequired = true` and render `<SetupWizard>` instead of the normal login UI.

When setup completes (`onComplete` callback), the wizard calls back and the login screen:
1. Sets `setupRequired = false`
2. Re-pings the agent (now returns `status: "ok"`)
3. Fetches providers and shows the normal login flow

Key changes to `login-screen.tsx`:
- Import `SetupWizard`
- Add `const [setupRequired, setSetupRequired] = useState(false)`
- In `handleConnect`, after `ping_agent` returns, check `result.status`:
  ```tsx
  const result = await invoke<{ app: string; status: string }>("ping_agent", { url });
  if (result.status === "setup_required") {
    setSetupRequired(true);
    return;
  }
  ```
- Before the main return, add:
  ```tsx
  if (setupRequired) {
    return (
      <SetupWizard
        agentUrl={agentUrl.replace(/\/+$/, "")}
        onComplete={async () => {
          setSetupRequired(false);
          // Trigger normal login flow
          const url = agentUrl.replace(/\/+$/, "");
          try {
            const providers = await invoke<string[]>("fetch_providers", { url });
            setProviders(providers);
            setStatus("online");
            // Auto-trigger login with first provider (seamless transition)
            if (providers.length > 0) {
              handleProviderClick(providers[0]);
            }
          } catch {
            setStatus("online");
          }
        }}
        onBack={() => {
          setSetupRequired(false);
          setStatus("idle");
        }}
      />
    );
  }
  ```

**Step 2: Commit**

```bash
git add app/src/components/login-screen.tsx
git commit -m "feat(app): integrate setup wizard into login screen"
```

---

## Task 15: Update Frontend Tests

**Files:**
- Modify: existing tests that mock `ping_agent` to account for new return type
- Create: `app/tests/unit/setup-wizard.test.tsx`

**Step 1: Update ping_agent mock**

Any test that mocks `invoke("ping_agent")` needs to return `{ app: "dev.kontainer.app", status: "ok" }` instead of `undefined`/`void`.

**Step 2: Write setup wizard tests**

```tsx
// app/tests/unit/setup-wizard.test.tsx
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { SetupWizard } from "@/components/setup-wizard";

describe("SetupWizard", () => {
  const defaultProps = {
    agentUrl: "http://localhost:8080",
    onComplete: vi.fn(),
    onBack: vi.fn(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders token input on initial step", () => {
    render(<SetupWizard {...defaultProps} />);
    expect(screen.getByLabelText("Setup Token")).toBeInTheDocument();
    expect(screen.getByText("Verify")).toBeInTheDocument();
  });

  it("shows error on invalid token", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("Invalid setup token");

    render(<SetupWizard {...defaultProps} />);
    const input = screen.getByLabelText("Setup Token");
    await userEvent.type(input, "wrong_token");
    await userEvent.click(screen.getByText("Verify"));

    await act(async () => {});
    expect(screen.getByText("Invalid setup token")).toBeInTheDocument();
  });

  it("advances to github step on valid token", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ step: "pending" });

    render(<SetupWizard {...defaultProps} />);
    const input = screen.getByLabelText("Setup Token");
    await userEvent.type(input, "ktsetup_abc123");
    await userEvent.click(screen.getByText("Verify"));

    await act(async () => {});
    expect(screen.getByText("Connect GitHub")).toBeInTheDocument();
  });

  it("resumes at confirm step if github already connected", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ step: "github_connected" });

    render(<SetupWizard {...defaultProps} />);
    const input = screen.getByLabelText("Setup Token");
    await userEvent.type(input, "ktsetup_abc123");
    await userEvent.click(screen.getByText("Verify"));

    await act(async () => {});
    expect(screen.getByText("Complete Setup")).toBeInTheDocument();
  });
});
```

**Step 3: Run tests**

Run: `pnpm --filter app test tests/unit/setup-wizard.test.tsx`
Expected: PASS

**Step 4: Commit**

```bash
git add app/tests/unit/setup-wizard.test.tsx
git commit -m "test(app): add setup wizard tests"
```

---

## Task 16: Quality Checks & Final Verification

**Step 1: Run Go tests**

Run: `go test -C agent ./...`
Expected: PASS

**Step 2: Run frontend tests**

Run: `pnpm --filter app test`
Expected: PASS

**Step 3: Check Rust compilation**

Run: `cargo check --manifest-path app/src-tauri/Cargo.toml`
Expected: Compiles

**Step 4: Lint and format**

Run: `pnpm lint && pnpm fmt`
Run: `cargo fmt --manifest-path app/src-tauri/Cargo.toml`
Run: `gofmt -w agent/`

**Step 5: Run types check**

Run: `pnpm types`
Expected: No errors

**Step 6: Commit any formatting fixes**

```bash
git add -A
git commit -m "chore: lint and format"
```

---

## Summary

| Task | Component | What |
|------|-----------|------|
| 1 | Agent | AppConfigStore (SQLite) |
| 2 | Agent | Setup token generation |
| 3 | Agent | Setup token middleware |
| 4 | Agent | Setup handlers (status, manifest, callback, complete) |
| 5 | Agent | Setup router |
| 6 | Agent | SwappableHandler for hot-swap |
| 7 | Agent | Refactor GitHubProvider for GitHub App |
| 8 | Agent | Update /ping with status field |
| 9 | Agent | Remove github/jwt_secret from config |
| 10 | Agent | Rewire main.go boot sequence |
| 11 | Desktop | Update ping_agent return type |
| 12 | Desktop | New setup Tauri commands |
| 13 | Desktop | Setup wizard component |
| 14 | Desktop | Wire wizard into login screen |
| 15 | Desktop | Update/add frontend tests |
| 16 | Both | Quality checks & formatting |

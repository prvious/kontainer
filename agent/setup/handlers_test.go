package setup

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
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
	t.Cleanup(func() { store.Close() })
	return store
}

func TestStatusHandler_Pending(t *testing.T) {
	store := newTestStore(t)
	h := NewSetupHandlers(store, "https://agent.example.com")

	req := httptest.NewRequest(http.MethodGet, "/setup/status", nil)
	rec := httptest.NewRecorder()

	h.StatusHandler(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusOK)
	}

	var body map[string]string
	if err := json.NewDecoder(rec.Body).Decode(&body); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if body["step"] != "pending" {
		t.Errorf("step = %q, want %q", body["step"], "pending")
	}
}

func TestStatusHandler_GithubConnected(t *testing.T) {
	store := newTestStore(t)
	if err := store.Set("github_app_id", "12345"); err != nil {
		t.Fatal(err)
	}
	h := NewSetupHandlers(store, "https://agent.example.com")

	req := httptest.NewRequest(http.MethodGet, "/setup/status", nil)
	rec := httptest.NewRecorder()

	h.StatusHandler(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusOK)
	}

	var body map[string]string
	if err := json.NewDecoder(rec.Body).Decode(&body); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if body["step"] != "github_connected" {
		t.Errorf("step = %q, want %q", body["step"], "github_connected")
	}
}

func TestStatusHandler_Complete(t *testing.T) {
	store := newTestStore(t)
	if err := store.Set("setup_complete", "true"); err != nil {
		t.Fatal(err)
	}
	h := NewSetupHandlers(store, "https://agent.example.com")

	req := httptest.NewRequest(http.MethodGet, "/setup/status", nil)
	rec := httptest.NewRecorder()

	h.StatusHandler(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusOK)
	}

	var body map[string]string
	if err := json.NewDecoder(rec.Body).Decode(&body); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if body["step"] != "complete" {
		t.Errorf("step = %q, want %q", body["step"], "complete")
	}
}

func TestManifestHandler(t *testing.T) {
	store := newTestStore(t)
	h := NewSetupHandlers(store, "https://agent.example.com")

	req := httptest.NewRequest(http.MethodGet, "/setup/github/manifest", nil)
	rec := httptest.NewRecorder()

	h.ManifestHandler(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusOK)
	}

	var body map[string]string
	if err := json.NewDecoder(rec.Body).Decode(&body); err != nil {
		t.Fatalf("decode: %v", err)
	}

	setupURL, ok := body["setupUrl"]
	if !ok {
		t.Fatal("response missing setupUrl")
	}

	// Should contain /setup/github/start?token=
	if !strings.Contains(setupURL, "/setup/github/start?token=") {
		t.Errorf("setupUrl = %q, expected it to contain /setup/github/start?token=", setupURL)
	}

	// Should start with the agent URL
	if !strings.HasPrefix(setupURL, "https://agent.example.com") {
		t.Errorf("setupUrl = %q, expected it to start with agent URL", setupURL)
	}
}

func TestStartHandler_InvalidToken(t *testing.T) {
	store := newTestStore(t)
	h := NewSetupHandlers(store, "https://agent.example.com")

	// First call ManifestHandler to generate a valid state
	manifestReq := httptest.NewRequest(http.MethodGet, "/setup/github/manifest", nil)
	manifestRec := httptest.NewRecorder()
	h.ManifestHandler(manifestRec, manifestReq)

	// Now call StartHandler with a wrong token
	req := httptest.NewRequest(http.MethodGet, "/setup/github/start?token=wrong-token", nil)
	rec := httptest.NewRecorder()

	h.StartHandler(rec, req)

	if rec.Code != http.StatusUnauthorized {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusUnauthorized)
	}
}

func TestStartHandler_NoState(t *testing.T) {
	store := newTestStore(t)
	h := NewSetupHandlers(store, "https://agent.example.com")

	// Call StartHandler without ever calling ManifestHandler (no state stored)
	req := httptest.NewRequest(http.MethodGet, "/setup/github/start?token=some-token", nil)
	rec := httptest.NewRecorder()

	h.StartHandler(rec, req)

	if rec.Code != http.StatusForbidden {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusForbidden)
	}
}

func TestCallbackHandler_MissingCode(t *testing.T) {
	store := newTestStore(t)
	h := NewSetupHandlers(store, "https://agent.example.com")

	req := httptest.NewRequest(http.MethodGet, "/setup/github/callback", nil)
	rec := httptest.NewRecorder()

	h.CallbackHandler(rec, req)

	if rec.Code != http.StatusBadRequest {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusBadRequest)
	}
}

func TestCompleteHandler_NoGitHub(t *testing.T) {
	store := newTestStore(t)
	h := NewSetupHandlers(store, "https://agent.example.com")

	req := httptest.NewRequest(http.MethodPost, "/setup/complete", nil)
	rec := httptest.NewRecorder()

	h.CompleteHandler(rec, req)

	if rec.Code != http.StatusBadRequest {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusBadRequest)
	}

	var body map[string]string
	if err := json.NewDecoder(rec.Body).Decode(&body); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if body["error"] == "" {
		t.Error("expected error message in response")
	}
}

func TestCompleteHandler_Success(t *testing.T) {
	store := newTestStore(t)
	if err := store.Set("github_app_id", "12345"); err != nil {
		t.Fatal(err)
	}
	h := NewSetupHandlers(store, "https://agent.example.com")

	req := httptest.NewRequest(http.MethodPost, "/setup/complete", nil)
	rec := httptest.NewRecorder()

	h.CompleteHandler(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("status = %d, want %d", rec.Code, http.StatusOK)
	}

	var body map[string]string
	if err := json.NewDecoder(rec.Body).Decode(&body); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if body["status"] != "complete" {
		t.Errorf("status = %q, want %q", body["status"], "complete")
	}

	// Verify setup_complete was set in the store
	if !store.IsSetupComplete() {
		t.Error("expected IsSetupComplete to be true after CompleteHandler")
	}
}

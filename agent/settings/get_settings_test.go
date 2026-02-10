package settings

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"testing"

	"github.com/prvious/kontainer/agent/storage"
)

func newTestStore(t *testing.T) *storage.SettingsStore {
	t.Helper()
	store, err := storage.NewSettingsStore(filepath.Join(t.TempDir(), "test.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { store.Close() })
	return store
}

func TestGetSettingsHandler_Empty(t *testing.T) {
	store := newTestStore(t)
	action := NewGetSettingsAction(store)
	handler := NewGetSettingsHandler(action)

	req := httptest.NewRequest("GET", "/api/settings", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Settings map[string]string `json:"settings"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Settings) != 0 {
		t.Errorf("expected empty settings, got %v", resp.Settings)
	}
}

func TestGetSettingsHandler_WithData(t *testing.T) {
	store := newTestStore(t)
	store.Set("theme", "dark")
	store.Set("lang", "en")

	action := NewGetSettingsAction(store)
	handler := NewGetSettingsHandler(action)

	req := httptest.NewRequest("GET", "/api/settings", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Settings map[string]string `json:"settings"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if resp.Settings["theme"] != "dark" {
		t.Errorf("expected dark, got %s", resp.Settings["theme"])
	}
	if resp.Settings["lang"] != "en" {
		t.Errorf("expected en, got %s", resp.Settings["lang"])
	}
}

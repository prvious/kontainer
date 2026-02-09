package settings

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestUpdateSettingsHandler_Success(t *testing.T) {
	store := newTestStore(t)
	getAction := NewGetSettingsAction(store)
	updateAction := NewUpdateSettingsAction(store)
	handler := NewUpdateSettingsHandler(updateAction, getAction)

	body := `{"settings":{"theme":"dark","lang":"en"}}`
	req := httptest.NewRequest("PUT", "/api/settings", strings.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", w.Code, w.Body.String())
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

func TestUpdateSettingsHandler_Overwrite(t *testing.T) {
	store := newTestStore(t)
	store.Set("theme", "light")

	getAction := NewGetSettingsAction(store)
	updateAction := NewUpdateSettingsAction(store)
	handler := NewUpdateSettingsHandler(updateAction, getAction)

	body := `{"settings":{"theme":"dark"}}`
	req := httptest.NewRequest("PUT", "/api/settings", strings.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
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
}

func TestUpdateSettingsHandler_InvalidBody(t *testing.T) {
	store := newTestStore(t)
	getAction := NewGetSettingsAction(store)
	updateAction := NewUpdateSettingsAction(store)
	handler := NewUpdateSettingsHandler(updateAction, getAction)

	req := httptest.NewRequest("PUT", "/api/settings", strings.NewReader("not json"))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}
}

func TestUpdateSettingsHandler_EmptySettings(t *testing.T) {
	store := newTestStore(t)
	getAction := NewGetSettingsAction(store)
	updateAction := NewUpdateSettingsAction(store)
	handler := NewUpdateSettingsHandler(updateAction, getAction)

	body := `{"settings":{}}`
	req := httptest.NewRequest("PUT", "/api/settings", strings.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}
}

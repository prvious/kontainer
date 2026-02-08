package services

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/docker/docker/errdefs"
	"github.com/go-chi/chi/v5"
)

func TestRemoveConfigHandler_Success(t *testing.T) {
	mock := &mockDockerClient{}

	action := NewRemoveConfigAction(mock)
	handler := NewRemoveConfigHandler(action)

	r := chi.NewRouter()
	r.Delete("/api/configs/{configID}", handler.ServeHTTP)

	req := httptest.NewRequest("DELETE", "/api/configs/cfg123", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp map[string]bool
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if !resp["removed"] {
		t.Error("expected removed to be true")
	}
}

func TestRemoveConfigHandler_NotFound(t *testing.T) {
	mock := &mockDockerClient{
		configRemoveErr: errdefs.NotFound(fmt.Errorf("config not found")),
	}

	action := NewRemoveConfigAction(mock)
	handler := NewRemoveConfigHandler(action)

	r := chi.NewRouter()
	r.Delete("/api/configs/{configID}", handler.ServeHTTP)

	req := httptest.NewRequest("DELETE", "/api/configs/nonexistent", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "config not found" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

func TestRemoveConfigHandler_DockerError(t *testing.T) {
	mock := &mockDockerClient{
		configRemoveErr: fmt.Errorf("connection refused"),
	}

	action := NewRemoveConfigAction(mock)
	handler := NewRemoveConfigHandler(action)

	r := chi.NewRouter()
	r.Delete("/api/configs/{configID}", handler.ServeHTTP)

	req := httptest.NewRequest("DELETE", "/api/configs/cfg123", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}
}

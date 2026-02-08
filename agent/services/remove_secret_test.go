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

func TestRemoveSecretHandler_Success(t *testing.T) {
	mock := &mockDockerClient{}

	action := NewRemoveSecretAction(mock)
	handler := NewRemoveSecretHandler(action)

	r := chi.NewRouter()
	r.Delete("/api/secrets/{secretID}", handler.ServeHTTP)

	req := httptest.NewRequest("DELETE", "/api/secrets/sec123", nil)
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

func TestRemoveSecretHandler_NotFound(t *testing.T) {
	mock := &mockDockerClient{
		secretRemoveErr: errdefs.NotFound(fmt.Errorf("secret not found")),
	}

	action := NewRemoveSecretAction(mock)
	handler := NewRemoveSecretHandler(action)

	r := chi.NewRouter()
	r.Delete("/api/secrets/{secretID}", handler.ServeHTTP)

	req := httptest.NewRequest("DELETE", "/api/secrets/nonexistent", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "secret not found" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

func TestRemoveSecretHandler_DockerError(t *testing.T) {
	mock := &mockDockerClient{
		secretRemoveErr: fmt.Errorf("connection refused"),
	}

	action := NewRemoveSecretAction(mock)
	handler := NewRemoveSecretHandler(action)

	r := chi.NewRouter()
	r.Delete("/api/secrets/{secretID}", handler.ServeHTTP)

	req := httptest.NewRequest("DELETE", "/api/secrets/sec123", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}
}

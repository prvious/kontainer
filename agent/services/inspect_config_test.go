package services

import (
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/docker/docker/api/types/swarm"
	"github.com/docker/docker/errdefs"
	"github.com/go-chi/chi/v5"
)

func TestInspectConfigHandler_HappyPath(t *testing.T) {
	now := time.Now().UTC()
	mock := &mockDockerClient{
		configInspect: swarm.Config{
			ID: "cfg-123",
			Spec: swarm.ConfigSpec{
				Annotations: swarm.Annotations{
					Name:   "nginx.conf",
					Labels: map[string]string{"env": "prod"},
				},
				Data: []byte("server { listen 80; }"),
			},
			Meta: swarm.Meta{
				CreatedAt: now,
				UpdatedAt: now,
			},
		},
	}

	action := NewInspectConfigAction(mock)
	handler := NewInspectConfigHandler(action)

	r := chi.NewRouter()
	r.Method("GET", "/configs/{configID}", handler)

	req := httptest.NewRequest("GET", "/configs/cfg-123", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp InspectConfigResponse
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if resp.ID != "cfg-123" {
		t.Errorf("expected id cfg-123, got %s", resp.ID)
	}
	if resp.Name != "nginx.conf" {
		t.Errorf("expected name nginx.conf, got %s", resp.Name)
	}
	if resp.Data != "server { listen 80; }" {
		t.Errorf("expected data 'server { listen 80; }', got %s", resp.Data)
	}
	if resp.Labels["env"] != "prod" {
		t.Errorf("expected label env=prod, got %v", resp.Labels)
	}
}

func TestInspectConfigHandler_NotFound(t *testing.T) {
	mock := &mockDockerClient{
		configInspectErr: errdefs.NotFound(errors.New("config not found")),
	}

	action := NewInspectConfigAction(mock)
	handler := NewInspectConfigHandler(action)

	r := chi.NewRouter()
	r.Method("GET", "/configs/{configID}", handler)

	req := httptest.NewRequest("GET", "/configs/nonexistent", nil)
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

func TestInspectConfigHandler_ServerError(t *testing.T) {
	mock := &mockDockerClient{
		configInspectErr: errors.New("connection refused"),
	}

	action := NewInspectConfigAction(mock)
	handler := NewInspectConfigHandler(action)

	r := chi.NewRouter()
	r.Method("GET", "/configs/{configID}", handler)

	req := httptest.NewRequest("GET", "/configs/cfg-123", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "failed to inspect config" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

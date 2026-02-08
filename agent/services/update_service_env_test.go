package services

import (
	"bytes"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/docker/docker/api/types/swarm"
	dockererrdefs "github.com/docker/docker/errdefs"
	"github.com/go-chi/chi/v5"
)

func setupEnvRouter(handler *UpdateServiceEnvHandler) *chi.Mux {
	r := chi.NewRouter()
	r.Put("/api/services/{serviceID}/env", handler.ServeHTTP)
	return r
}

func TestUpdateServiceEnvHandler_Success(t *testing.T) {
	mock := &mockDockerClient{
		inspectSvc: swarm.Service{
			ID: "svc1",
			Meta: swarm.Meta{
				Version: swarm.Version{Index: 10},
			},
			Spec: swarm.ServiceSpec{
				Annotations: swarm.Annotations{Name: "web"},
				TaskTemplate: swarm.TaskSpec{
					ContainerSpec: &swarm.ContainerSpec{
						Image: "nginx:latest",
						Env:   []string{"OLD_KEY=old_value"},
					},
				},
			},
		},
	}

	action := NewUpdateServiceEnvAction(mock)
	handler := NewUpdateServiceEnvHandler(action)
	router := setupEnvRouter(handler)

	body := `{"env":[{"key":"NODE_ENV","value":"production"},{"key":"PORT","value":"3000"}]}`
	req := httptest.NewRequest("PUT", "/api/services/svc1/env", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", w.Code, w.Body.String())
	}

	var resp updateEnvResponse
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if len(resp.Env) != 2 {
		t.Fatalf("expected 2 env vars, got %d", len(resp.Env))
	}
	if resp.Env[0].Key != "NODE_ENV" || resp.Env[0].Value != "production" {
		t.Errorf("unexpected first env var: %+v", resp.Env[0])
	}
	if resp.Env[1].Key != "PORT" || resp.Env[1].Value != "3000" {
		t.Errorf("unexpected second env var: %+v", resp.Env[1])
	}
}

func TestUpdateServiceEnvHandler_ClearAllEnvVars(t *testing.T) {
	mock := &mockDockerClient{
		inspectSvc: swarm.Service{
			ID:   "svc1",
			Meta: swarm.Meta{Version: swarm.Version{Index: 5}},
			Spec: swarm.ServiceSpec{
				TaskTemplate: swarm.TaskSpec{
					ContainerSpec: &swarm.ContainerSpec{
						Image: "nginx",
						Env:   []string{"FOO=bar", "BAZ=qux"},
					},
				},
			},
		},
	}

	action := NewUpdateServiceEnvAction(mock)
	handler := NewUpdateServiceEnvHandler(action)
	router := setupEnvRouter(handler)

	body := `{"env":[]}`
	req := httptest.NewRequest("PUT", "/api/services/svc1/env", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp updateEnvResponse
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Env) != 0 {
		t.Errorf("expected 0 env vars, got %d", len(resp.Env))
	}
}

func TestUpdateServiceEnvHandler_InvalidBody(t *testing.T) {
	mock := &mockDockerClient{}
	action := NewUpdateServiceEnvAction(mock)
	handler := NewUpdateServiceEnvHandler(action)
	router := setupEnvRouter(handler)

	req := httptest.NewRequest("PUT", "/api/services/svc1/env", bytes.NewBufferString("not json"))
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}
}

func TestUpdateServiceEnvHandler_MissingEnvField(t *testing.T) {
	mock := &mockDockerClient{}
	action := NewUpdateServiceEnvAction(mock)
	handler := NewUpdateServiceEnvHandler(action)
	router := setupEnvRouter(handler)

	body := `{}`
	req := httptest.NewRequest("PUT", "/api/services/svc1/env", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "env field is required" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

func TestUpdateServiceEnvHandler_EmptyKey(t *testing.T) {
	mock := &mockDockerClient{}
	action := NewUpdateServiceEnvAction(mock)
	handler := NewUpdateServiceEnvHandler(action)
	router := setupEnvRouter(handler)

	body := `{"env":[{"key":"","value":"oops"}]}`
	req := httptest.NewRequest("PUT", "/api/services/svc1/env", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "env var key cannot be empty" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

func TestUpdateServiceEnvHandler_ServiceNotFound(t *testing.T) {
	mock := &mockDockerClient{
		inspectErr: dockererrdefs.NotFound(errors.New("not found")),
	}

	action := NewUpdateServiceEnvAction(mock)
	handler := NewUpdateServiceEnvHandler(action)
	router := setupEnvRouter(handler)

	body := `{"env":[{"key":"FOO","value":"bar"}]}`
	req := httptest.NewRequest("PUT", "/api/services/nonexistent/env", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d", w.Code)
	}
}

func TestUpdateServiceEnvHandler_DockerUpdateError(t *testing.T) {
	mock := &mockDockerClient{
		inspectSvc: swarm.Service{
			ID:   "svc1",
			Meta: swarm.Meta{Version: swarm.Version{Index: 1}},
			Spec: swarm.ServiceSpec{
				TaskTemplate: swarm.TaskSpec{
					ContainerSpec: &swarm.ContainerSpec{
						Image: "nginx",
						Env:   []string{},
					},
				},
			},
		},
		updateErr: errors.New("update failed"),
	}

	action := NewUpdateServiceEnvAction(mock)
	handler := NewUpdateServiceEnvHandler(action)
	router := setupEnvRouter(handler)

	body := `{"env":[{"key":"FOO","value":"bar"}]}`
	req := httptest.NewRequest("PUT", "/api/services/svc1/env", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}
}

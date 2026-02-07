package services

import (
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/docker/docker/api/types/swarm"
	"github.com/go-chi/chi/v5"
)

func restartRouter(handler *RestartServiceHandler) *chi.Mux {
	r := chi.NewRouter()
	r.Method("POST", "/services/{serviceID}/restart", handler)
	return r
}

func TestRestartServiceHandler_Success(t *testing.T) {
	mock := &mockDockerClient{
		inspectSvc: swarm.Service{
			ID: "svc1",
			Meta: swarm.Meta{
				Version:   swarm.Version{Index: 10},
				UpdatedAt: time.Now().UTC(),
			},
			Spec: swarm.ServiceSpec{
				Annotations: swarm.Annotations{Name: "web"},
				Mode: swarm.ServiceMode{
					Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(3)},
				},
				TaskTemplate: swarm.TaskSpec{
					ContainerSpec: &swarm.ContainerSpec{Image: "nginx:latest"},
					ForceUpdate:   5,
				},
			},
		},
	}

	action := NewRestartServiceAction(mock)
	handler := NewRestartServiceHandler(action)
	router := restartRouter(handler)

	req := httptest.NewRequest("POST", "/services/svc1/restart", nil)
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", w.Code, w.Body.String())
	}

	var resp map[string]string
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}
	if resp["status"] != "restarting" {
		t.Errorf("expected status restarting, got %s", resp["status"])
	}
}

func TestRestartServiceHandler_ServiceNotFound(t *testing.T) {
	mock := &mockDockerClient{
		inspectErr: &notFoundError{},
	}

	action := NewRestartServiceAction(mock)
	handler := NewRestartServiceHandler(action)
	router := restartRouter(handler)

	req := httptest.NewRequest("POST", "/services/nonexistent/restart", nil)
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	if w.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d: %s", w.Code, w.Body.String())
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "service not found" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

func TestRestartServiceHandler_DockerUpdateError(t *testing.T) {
	mock := &mockDockerClient{
		inspectSvc: swarm.Service{
			ID: "svc1",
			Meta: swarm.Meta{
				Version:   swarm.Version{Index: 10},
				UpdatedAt: time.Now().UTC(),
			},
			Spec: swarm.ServiceSpec{
				Annotations: swarm.Annotations{Name: "web"},
				Mode: swarm.ServiceMode{
					Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(2)},
				},
				TaskTemplate: swarm.TaskSpec{
					ContainerSpec: &swarm.ContainerSpec{Image: "nginx:latest"},
				},
			},
		},
		updateErr: errors.New("docker daemon error"),
	}

	action := NewRestartServiceAction(mock)
	handler := NewRestartServiceHandler(action)
	router := restartRouter(handler)

	req := httptest.NewRequest("POST", "/services/svc1/restart", nil)
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "failed to restart service" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

func TestRestartServiceHandler_GlobalModeSuccess(t *testing.T) {
	mock := &mockDockerClient{
		inspectSvc: swarm.Service{
			ID: "svc-global",
			Meta: swarm.Meta{
				Version:   swarm.Version{Index: 1},
				UpdatedAt: time.Now().UTC(),
			},
			Spec: swarm.ServiceSpec{
				Annotations: swarm.Annotations{Name: "monitoring"},
				Mode: swarm.ServiceMode{
					Global: &swarm.GlobalService{},
				},
				TaskTemplate: swarm.TaskSpec{
					ContainerSpec: &swarm.ContainerSpec{Image: "prometheus:v2"},
				},
			},
		},
	}

	action := NewRestartServiceAction(mock)
	handler := NewRestartServiceHandler(action)
	router := restartRouter(handler)

	req := httptest.NewRequest("POST", "/services/svc-global/restart", nil)
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", w.Code, w.Body.String())
	}
}

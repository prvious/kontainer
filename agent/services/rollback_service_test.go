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

func rollbackRouter(handler *RollbackServiceHandler) *chi.Mux {
	r := chi.NewRouter()
	r.Method("POST", "/services/{serviceID}/rollback", handler)
	return r
}

func TestRollbackServiceHandler_Success(t *testing.T) {
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
					ContainerSpec: &swarm.ContainerSpec{Image: "nginx:2.0"},
				},
			},
			PreviousSpec: &swarm.ServiceSpec{
				TaskTemplate: swarm.TaskSpec{
					ContainerSpec: &swarm.ContainerSpec{Image: "nginx:1.0"},
				},
			},
		},
	}

	action := NewRollbackServiceAction(mock)
	handler := NewRollbackServiceHandler(action)
	router := rollbackRouter(handler)

	req := httptest.NewRequest("POST", "/services/svc1/rollback", nil)
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", w.Code, w.Body.String())
	}

	var resp map[string]string
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}
	if resp["status"] != "rolling_back" {
		t.Errorf("expected status rolling_back, got %s", resp["status"])
	}
}

func TestRollbackServiceHandler_NoPreviousSpec(t *testing.T) {
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
				},
			},
			// PreviousSpec is nil
		},
	}

	action := NewRollbackServiceAction(mock)
	handler := NewRollbackServiceHandler(action)
	router := rollbackRouter(handler)

	req := httptest.NewRequest("POST", "/services/svc1/rollback", nil)
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d: %s", w.Code, w.Body.String())
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "service has no previous spec to rollback to" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

func TestRollbackServiceHandler_ServiceNotFound(t *testing.T) {
	mock := &mockDockerClient{
		inspectErr: &notFoundError{},
	}

	action := NewRollbackServiceAction(mock)
	handler := NewRollbackServiceHandler(action)
	router := rollbackRouter(handler)

	req := httptest.NewRequest("POST", "/services/nonexistent/rollback", nil)
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

func TestRollbackServiceHandler_DockerUpdateError(t *testing.T) {
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
					ContainerSpec: &swarm.ContainerSpec{Image: "nginx:2.0"},
				},
			},
			PreviousSpec: &swarm.ServiceSpec{
				TaskTemplate: swarm.TaskSpec{
					ContainerSpec: &swarm.ContainerSpec{Image: "nginx:1.0"},
				},
			},
		},
		updateErr: errors.New("docker daemon error"),
	}

	action := NewRollbackServiceAction(mock)
	handler := NewRollbackServiceHandler(action)
	router := rollbackRouter(handler)

	req := httptest.NewRequest("POST", "/services/svc1/rollback", nil)
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "failed to rollback service" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

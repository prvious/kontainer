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

func TestRemoveStackHandler_Success(t *testing.T) {
	mock := &mockDockerClient{
		services: []swarm.Service{
			{
				ID: "svc1",
				Spec: swarm.ServiceSpec{
					Annotations: swarm.Annotations{
						Name:   "myapp_web",
						Labels: map[string]string{"com.docker.stack.namespace": "myapp"},
					},
					Mode: swarm.ServiceMode{
						Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(1)},
					},
					TaskTemplate: swarm.TaskSpec{
						ContainerSpec: &swarm.ContainerSpec{Image: "nginx"},
					},
				},
				Meta: swarm.Meta{UpdatedAt: time.Now().UTC()},
			},
			{
				ID: "svc2",
				Spec: swarm.ServiceSpec{
					Annotations: swarm.Annotations{
						Name:   "myapp_api",
						Labels: map[string]string{"com.docker.stack.namespace": "myapp"},
					},
					Mode: swarm.ServiceMode{
						Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(2)},
					},
					TaskTemplate: swarm.TaskSpec{
						ContainerSpec: &swarm.ContainerSpec{Image: "node:20"},
					},
				},
				Meta: swarm.Meta{UpdatedAt: time.Now().UTC()},
			},
		},
	}

	action := NewRemoveStackAction(mock)
	handler := NewRemoveStackHandler(action)

	r := chi.NewRouter()
	r.Delete("/api/stacks/{stackName}", handler.ServeHTTP)

	req := httptest.NewRequest("DELETE", "/api/stacks/myapp", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Removed int `json:"removed"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if resp.Removed != 2 {
		t.Errorf("expected 2 removed, got %d", resp.Removed)
	}

	if len(mock.removedIDs) != 2 {
		t.Fatalf("expected 2 removedIDs, got %d", len(mock.removedIDs))
	}
	if mock.removedIDs[0] != "svc1" || mock.removedIDs[1] != "svc2" {
		t.Errorf("unexpected removedIDs: %v", mock.removedIDs)
	}
}

func TestRemoveStackHandler_NoMatchingServices(t *testing.T) {
	mock := &mockDockerClient{
		services: []swarm.Service{},
	}

	action := NewRemoveStackAction(mock)
	handler := NewRemoveStackHandler(action)

	r := chi.NewRouter()
	r.Delete("/api/stacks/{stackName}", handler.ServeHTTP)

	req := httptest.NewRequest("DELETE", "/api/stacks/nonexistent", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "no services found for stack" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

func TestRemoveStackHandler_DockerListError(t *testing.T) {
	mock := &mockDockerClient{
		svcErr: errors.New("connection refused"),
	}

	action := NewRemoveStackAction(mock)
	handler := NewRemoveStackHandler(action)

	r := chi.NewRouter()
	r.Delete("/api/stacks/{stackName}", handler.ServeHTTP)

	req := httptest.NewRequest("DELETE", "/api/stacks/myapp", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}
}

func TestRemoveStackHandler_DockerRemoveError(t *testing.T) {
	mock := &mockDockerClient{
		services: []swarm.Service{
			{
				ID: "svc1",
				Spec: swarm.ServiceSpec{
					Annotations: swarm.Annotations{
						Name:   "myapp_web",
						Labels: map[string]string{"com.docker.stack.namespace": "myapp"},
					},
					Mode: swarm.ServiceMode{
						Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(1)},
					},
					TaskTemplate: swarm.TaskSpec{
						ContainerSpec: &swarm.ContainerSpec{Image: "nginx"},
					},
				},
				Meta: swarm.Meta{UpdatedAt: time.Now().UTC()},
			},
		},
		removeErr: errors.New("permission denied"),
	}

	action := NewRemoveStackAction(mock)
	handler := NewRemoveStackHandler(action)

	r := chi.NewRouter()
	r.Delete("/api/stacks/{stackName}", handler.ServeHTTP)

	req := httptest.NewRequest("DELETE", "/api/stacks/myapp", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}
}

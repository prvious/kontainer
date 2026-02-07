package services

import (
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/docker/docker/api/types/mount"
	"github.com/docker/docker/api/types/swarm"
	"github.com/go-chi/chi/v5"
)

func TestInspectTaskHandler_Success(t *testing.T) {
	now := time.Now().UTC()

	mock := &mockDockerClient{
		inspectTask: swarm.Task{
			ID:        "task1",
			ServiceID: "svc1",
			Slot:      1,
			NodeID:    "node1",
			Meta:      swarm.Meta{CreatedAt: now.Add(-10 * time.Minute)},
			Spec: swarm.TaskSpec{
				ContainerSpec: &swarm.ContainerSpec{
					Image: "nginx:latest@sha256:abc123",
					Env:   []string{"NODE_ENV=production", "PORT=8080"},
					Mounts: []mount.Mount{
						{
							Type:     mount.TypeVolume,
							Source:   "data-vol",
							Target:   "/data",
							ReadOnly: false,
						},
					},
				},
			},
			DesiredState: swarm.TaskStateRunning,
			Status: swarm.TaskStatus{
				State:     swarm.TaskStateRunning,
				Timestamp: now,
				ContainerStatus: &swarm.ContainerStatus{
					ContainerID: "abc123def456789",
					PID:         12345,
					ExitCode:    0,
				},
			},
			NetworksAttachments: []swarm.NetworkAttachment{
				{
					Network: swarm.Network{
						ID:   "net1",
						Spec: swarm.NetworkSpec{Annotations: swarm.Annotations{Name: "my-overlay"}},
					},
					Addresses: []string{"10.0.0.5/24"},
				},
			},
		},
		nodes: []swarm.Node{
			{
				ID:          "node1",
				Description: swarm.NodeDescription{Hostname: "worker-1"},
			},
		},
	}

	action := NewInspectTaskAction(mock)
	handler := NewInspectTaskHandler(action)

	r := chi.NewRouter()
	r.Get("/api/tasks/{taskID}", handler.ServeHTTP)

	req := httptest.NewRequest("GET", "/api/tasks/task1", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var detail TaskDetail
	if err := json.NewDecoder(w.Body).Decode(&detail); err != nil {
		t.Fatal(err)
	}

	if detail.ID != "task1" {
		t.Errorf("expected id task1, got %s", detail.ID)
	}
	if detail.ServiceID != "svc1" {
		t.Errorf("expected serviceId svc1, got %s", detail.ServiceID)
	}
	if detail.NodeName != "worker-1" {
		t.Errorf("expected worker-1, got %s", detail.NodeName)
	}
	if detail.Image != "nginx:latest" {
		t.Errorf("expected nginx:latest, got %s", detail.Image)
	}
	if detail.ContainerID != "abc123def456789" {
		t.Errorf("expected full container ID, got %s", detail.ContainerID)
	}
	if detail.PID != 12345 {
		t.Errorf("expected PID 12345, got %d", detail.PID)
	}
	if detail.State != "running" {
		t.Errorf("expected running, got %s", detail.State)
	}
	if detail.Slot != 1 {
		t.Errorf("expected slot 1, got %d", detail.Slot)
	}

	// Env vars
	if len(detail.Env) != 2 {
		t.Fatalf("expected 2 env vars, got %d", len(detail.Env))
	}
	if detail.Env[0].Key != "NODE_ENV" || detail.Env[0].Value != "production" {
		t.Errorf("unexpected env[0]: %+v", detail.Env[0])
	}

	// Mounts
	if len(detail.Mounts) != 1 {
		t.Fatalf("expected 1 mount, got %d", len(detail.Mounts))
	}
	if detail.Mounts[0].Source != "data-vol" || detail.Mounts[0].Target != "/data" {
		t.Errorf("unexpected mount: %+v", detail.Mounts[0])
	}

	// Networks
	if len(detail.Networks) != 1 {
		t.Fatalf("expected 1 network, got %d", len(detail.Networks))
	}
	if detail.Networks[0].Name != "my-overlay" {
		t.Errorf("expected my-overlay, got %s", detail.Networks[0].Name)
	}
	if len(detail.Networks[0].Addresses) != 1 || detail.Networks[0].Addresses[0] != "10.0.0.5/24" {
		t.Errorf("unexpected addresses: %v", detail.Networks[0].Addresses)
	}
}

func TestInspectTaskHandler_NotFound(t *testing.T) {
	mock := &mockDockerClient{
		inspectTaskErr: &notFoundError{},
	}

	action := NewInspectTaskAction(mock)
	handler := NewInspectTaskHandler(action)

	r := chi.NewRouter()
	r.Get("/api/tasks/{taskID}", handler.ServeHTTP)

	req := httptest.NewRequest("GET", "/api/tasks/nonexistent", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d", w.Code)
	}
}

func TestInspectTaskHandler_DockerError(t *testing.T) {
	mock := &mockDockerClient{
		inspectTaskErr: errors.New("connection refused"),
	}

	action := NewInspectTaskAction(mock)
	handler := NewInspectTaskHandler(action)

	r := chi.NewRouter()
	r.Get("/api/tasks/{taskID}", handler.ServeHTTP)

	req := httptest.NewRequest("GET", "/api/tasks/task1", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}
}

func TestInspectTaskHandler_NodeListError(t *testing.T) {
	now := time.Now().UTC()

	mock := &mockDockerClient{
		inspectTask: swarm.Task{
			ID:     "task1",
			NodeID: "node1abcdef12",
			Meta:   swarm.Meta{CreatedAt: now},
			Spec: swarm.TaskSpec{
				ContainerSpec: &swarm.ContainerSpec{Image: "nginx"},
			},
			Status: swarm.TaskStatus{
				State:     swarm.TaskStateRunning,
				Timestamp: now,
			},
		},
		nodeErr: errors.New("node list failed"),
	}

	action := NewInspectTaskAction(mock)
	handler := NewInspectTaskHandler(action)

	r := chi.NewRouter()
	r.Get("/api/tasks/{taskID}", handler.ServeHTTP)

	req := httptest.NewRequest("GET", "/api/tasks/task1", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var detail TaskDetail
	json.NewDecoder(w.Body).Decode(&detail)

	// Falls back to truncated node ID
	if detail.NodeName != "node1abcdef1" {
		t.Errorf("expected truncated node ID, got %s", detail.NodeName)
	}
}

func TestInspectTaskHandler_NoContainerStatus(t *testing.T) {
	now := time.Now().UTC()

	mock := &mockDockerClient{
		inspectTask: swarm.Task{
			ID:     "task1",
			NodeID: "node1",
			Meta:   swarm.Meta{CreatedAt: now},
			Spec: swarm.TaskSpec{
				ContainerSpec: &swarm.ContainerSpec{Image: "nginx"},
			},
			DesiredState: swarm.TaskStateRunning,
			Status: swarm.TaskStatus{
				State:     swarm.TaskStatePending,
				Timestamp: now,
			},
		},
		nodes: []swarm.Node{
			{
				ID:          "node1",
				Description: swarm.NodeDescription{Hostname: "worker-1"},
			},
		},
	}

	action := NewInspectTaskAction(mock)
	handler := NewInspectTaskHandler(action)

	r := chi.NewRouter()
	r.Get("/api/tasks/{taskID}", handler.ServeHTTP)

	req := httptest.NewRequest("GET", "/api/tasks/task1", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var detail TaskDetail
	json.NewDecoder(w.Body).Decode(&detail)

	if detail.ContainerID != "" {
		t.Errorf("expected empty container ID, got %s", detail.ContainerID)
	}
	if detail.PID != 0 {
		t.Errorf("expected PID 0, got %d", detail.PID)
	}
	if detail.ExitCode != nil {
		t.Errorf("expected nil exit code, got %v", detail.ExitCode)
	}
	if detail.State != "pending" {
		t.Errorf("expected pending, got %s", detail.State)
	}
}

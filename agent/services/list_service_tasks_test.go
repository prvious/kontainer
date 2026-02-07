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

func TestListServiceTasksHandler_Success(t *testing.T) {
	now := time.Now().UTC()
	earlier := now.Add(-5 * time.Minute)

	mock := &mockDockerClient{
		inspectSvc: swarm.Service{
			ID: "svc1",
			Spec: swarm.ServiceSpec{
				Annotations: swarm.Annotations{Name: "web"},
				Mode: swarm.ServiceMode{
					Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(2)},
				},
				TaskTemplate: swarm.TaskSpec{
					ContainerSpec: &swarm.ContainerSpec{Image: "nginx"},
				},
			},
		},
		tasks: []swarm.Task{
			{
				ID:           "task1",
				ServiceID:    "svc1",
				Slot:         1,
				NodeID:       "node1",
				DesiredState: swarm.TaskStateRunning,
				Status: swarm.TaskStatus{
					State:     swarm.TaskStateRunning,
					Timestamp: now,
					ContainerStatus: &swarm.ContainerStatus{
						ContainerID: "abc123def456789",
						ExitCode:    0,
					},
				},
			},
			{
				ID:           "task2",
				ServiceID:    "svc1",
				Slot:         2,
				NodeID:       "node2",
				DesiredState: swarm.TaskStateRunning,
				Status: swarm.TaskStatus{
					State:     swarm.TaskStateFailed,
					Err:       "task: non-zero exit (137)",
					Timestamp: earlier,
					ContainerStatus: &swarm.ContainerStatus{
						ContainerID: "xyz789",
						ExitCode:    137,
					},
				},
			},
		},
		nodes: []swarm.Node{
			{
				ID:          "node1",
				Description: swarm.NodeDescription{Hostname: "worker-1"},
			},
			{
				ID:          "node2",
				Description: swarm.NodeDescription{Hostname: "worker-2"},
			},
		},
	}

	action := NewListServiceTasksAction(mock)
	handler := NewListServiceTasksHandler(action)

	r := chi.NewRouter()
	r.Get("/api/services/{serviceID}/tasks", handler.ServeHTTP)

	req := httptest.NewRequest("GET", "/api/services/svc1/tasks", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Tasks []ServiceTaskItem `json:"tasks"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if len(resp.Tasks) != 2 {
		t.Fatalf("expected 2 tasks, got %d", len(resp.Tasks))
	}

	// Most recent first
	first := resp.Tasks[0]
	if first.ID != "task1" {
		t.Errorf("expected task1 first (most recent), got %s", first.ID)
	}
	if first.NodeName != "worker-1" {
		t.Errorf("expected worker-1, got %s", first.NodeName)
	}
	if first.State != "running" {
		t.Errorf("expected running, got %s", first.State)
	}
	if first.ContainerID != "abc123def456" {
		t.Errorf("expected truncated container ID abc123def456, got %s", first.ContainerID)
	}
	if first.Slot != 1 {
		t.Errorf("expected slot 1, got %d", first.Slot)
	}

	second := resp.Tasks[1]
	if second.ID != "task2" {
		t.Errorf("expected task2, got %s", second.ID)
	}
	if second.State != "failed" {
		t.Errorf("expected failed, got %s", second.State)
	}
	if second.Error != "task: non-zero exit (137)" {
		t.Errorf("unexpected error: %s", second.Error)
	}
	if second.ExitCode == nil || *second.ExitCode != 137 {
		t.Errorf("expected exit code 137, got %v", second.ExitCode)
	}
	if second.NodeName != "worker-2" {
		t.Errorf("expected worker-2, got %s", second.NodeName)
	}
}

func TestListServiceTasksHandler_EmptyTasks(t *testing.T) {
	mock := &mockDockerClient{
		inspectSvc: swarm.Service{
			ID: "svc1",
			Spec: swarm.ServiceSpec{
				Annotations: swarm.Annotations{Name: "web"},
				Mode: swarm.ServiceMode{
					Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(1)},
				},
				TaskTemplate: swarm.TaskSpec{
					ContainerSpec: &swarm.ContainerSpec{Image: "nginx"},
				},
			},
		},
		tasks: []swarm.Task{},
		nodes: []swarm.Node{},
	}

	action := NewListServiceTasksAction(mock)
	handler := NewListServiceTasksHandler(action)

	r := chi.NewRouter()
	r.Get("/api/services/{serviceID}/tasks", handler.ServeHTTP)

	req := httptest.NewRequest("GET", "/api/services/svc1/tasks", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Tasks []ServiceTaskItem `json:"tasks"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Tasks) != 0 {
		t.Errorf("expected 0 tasks, got %d", len(resp.Tasks))
	}
}

func TestListServiceTasksHandler_ServiceNotFound(t *testing.T) {
	mock := &mockDockerClient{
		inspectErr: &notFoundError{},
	}

	action := NewListServiceTasksAction(mock)
	handler := NewListServiceTasksHandler(action)

	r := chi.NewRouter()
	r.Get("/api/services/{serviceID}/tasks", handler.ServeHTTP)

	req := httptest.NewRequest("GET", "/api/services/nonexistent/tasks", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d", w.Code)
	}
}

func TestListServiceTasksHandler_TaskListError(t *testing.T) {
	mock := &mockDockerClient{
		inspectSvc: swarm.Service{
			ID: "svc1",
			Spec: swarm.ServiceSpec{
				Annotations: swarm.Annotations{Name: "web"},
				Mode: swarm.ServiceMode{
					Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(1)},
				},
				TaskTemplate: swarm.TaskSpec{
					ContainerSpec: &swarm.ContainerSpec{Image: "nginx"},
				},
			},
		},
		taskErr: errors.New("connection refused"),
	}

	action := NewListServiceTasksAction(mock)
	handler := NewListServiceTasksHandler(action)

	r := chi.NewRouter()
	r.Get("/api/services/{serviceID}/tasks", handler.ServeHTTP)

	req := httptest.NewRequest("GET", "/api/services/svc1/tasks", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}
}

func TestListServiceTasksHandler_NodeListError_StillReturnsTasks(t *testing.T) {
	now := time.Now().UTC()

	mock := &mockDockerClient{
		inspectSvc: swarm.Service{
			ID: "svc1",
			Spec: swarm.ServiceSpec{
				Annotations: swarm.Annotations{Name: "web"},
				Mode: swarm.ServiceMode{
					Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(1)},
				},
				TaskTemplate: swarm.TaskSpec{
					ContainerSpec: &swarm.ContainerSpec{Image: "nginx"},
				},
			},
		},
		tasks: []swarm.Task{
			{
				ID:           "task1",
				ServiceID:    "svc1",
				Slot:         1,
				NodeID:       "node1abcdef12",
				DesiredState: swarm.TaskStateRunning,
				Status: swarm.TaskStatus{
					State:     swarm.TaskStateRunning,
					Timestamp: now,
				},
			},
		},
		nodeErr: errors.New("node list failed"),
	}

	action := NewListServiceTasksAction(mock)
	handler := NewListServiceTasksHandler(action)

	r := chi.NewRouter()
	r.Get("/api/services/{serviceID}/tasks", handler.ServeHTTP)

	req := httptest.NewRequest("GET", "/api/services/svc1/tasks", nil)
	w := httptest.NewRecorder()
	r.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Tasks []ServiceTaskItem `json:"tasks"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Tasks) != 1 {
		t.Fatalf("expected 1 task, got %d", len(resp.Tasks))
	}

	// Falls back to truncated node ID
	if resp.Tasks[0].NodeName != "node1abcdef1" {
		t.Errorf("expected truncated node ID, got %s", resp.Tasks[0].NodeName)
	}
}

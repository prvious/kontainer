package services

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/swarm"
)

func TestListTasksHandler_Success(t *testing.T) {
	now := time.Now()
	mock := &MockDockerClient{
		TaskListFunc: func(ctx context.Context, options types.TaskListOptions) ([]swarm.Task, error) {
			return []swarm.Task{
				{
					ID:        "task1",
					ServiceID: "svc1",
					NodeID:    "node1",
					Slot:      1,
					Status: swarm.TaskStatus{
						State:     swarm.TaskStateRunning,
						Timestamp: now,
						ContainerStatus: &swarm.ContainerStatus{
							ContainerID: "container1",
							ExitCode:    0,
						},
					},
					DesiredState: swarm.TaskStateRunning,
					Spec: swarm.TaskSpec{
						ContainerSpec: &swarm.ContainerSpec{
							Image: "nginx:latest",
						},
					},
				},
			}, nil
		},
		NodeListFunc: func(ctx context.Context, options types.NodeListOptions) ([]swarm.Node, error) {
			return []swarm.Node{
				{
					ID: "node1",
					Description: swarm.NodeDescription{
						Hostname: "worker-1",
					},
				},
			}, nil
		},
		ServiceListFunc: func(ctx context.Context, options types.ServiceListOptions) ([]swarm.Service, error) {
			return []swarm.Service{
				{
					ID: "svc1",
					Spec: swarm.ServiceSpec{
						Annotations: swarm.Annotations{
							Name: "web",
						},
					},
				},
			}, nil
		},
	}

	action := NewListTasksAction(mock)
	handler := NewListTasksHandler(action)

	req := httptest.NewRequest("GET", "/api/tasks", nil)
	w := httptest.NewRecorder()

	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Tasks []TaskItem `json:"tasks"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if len(resp.Tasks) != 1 {
		t.Fatalf("expected 1 task, got %d", len(resp.Tasks))
	}

	task := resp.Tasks[0]
	if task.ID != "task1" {
		t.Errorf("expected task1, got %s", task.ID)
	}
	if task.ServiceName != "web" {
		t.Errorf("expected web, got %s", task.ServiceName)
	}
	if task.NodeName != "worker-1" {
		t.Errorf("expected worker-1, got %s", task.NodeName)
	}
	if task.State != "running" {
		t.Errorf("expected running, got %s", task.State)
	}
	if task.Image != "nginx:latest" {
		t.Errorf("expected nginx:latest, got %s", task.Image)
	}
}

func TestListTasksHandler_EmptyList(t *testing.T) {
	mock := &MockDockerClient{
		TaskListFunc: func(ctx context.Context, options types.TaskListOptions) ([]swarm.Task, error) {
			return []swarm.Task{}, nil
		},
		NodeListFunc: func(ctx context.Context, options types.NodeListOptions) ([]swarm.Node, error) {
			return []swarm.Node{}, nil
		},
		ServiceListFunc: func(ctx context.Context, options types.ServiceListOptions) ([]swarm.Service, error) {
			return []swarm.Service{}, nil
		},
	}

	action := NewListTasksAction(mock)
	handler := NewListTasksHandler(action)

	req := httptest.NewRequest("GET", "/api/tasks", nil)
	w := httptest.NewRecorder()

	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Tasks []TaskItem `json:"tasks"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if len(resp.Tasks) != 0 {
		t.Fatalf("expected 0 tasks, got %d", len(resp.Tasks))
	}
}

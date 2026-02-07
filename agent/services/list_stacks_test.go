package services

import (
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/docker/docker/api/types/swarm"
)

func TestListStacksHandler_MultipleStacks(t *testing.T) {
	now := time.Now().UTC()
	earlier := now.Add(-1 * time.Hour)

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
						Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(3)},
					},
					TaskTemplate: swarm.TaskSpec{
						ContainerSpec: &swarm.ContainerSpec{Image: "nginx:latest"},
					},
				},
				Meta: swarm.Meta{UpdatedAt: now},
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
				Meta: swarm.Meta{UpdatedAt: earlier},
			},
			{
				ID: "svc3",
				Spec: swarm.ServiceSpec{
					Annotations: swarm.Annotations{
						Name:   "monitoring_prometheus",
						Labels: map[string]string{"com.docker.stack.namespace": "monitoring"},
					},
					Mode: swarm.ServiceMode{
						Global: &swarm.GlobalService{},
					},
					TaskTemplate: swarm.TaskSpec{
						ContainerSpec: &swarm.ContainerSpec{Image: "prometheus:v2"},
					},
				},
				Meta: swarm.Meta{UpdatedAt: earlier},
			},
		},
		tasks: []swarm.Task{
			{ServiceID: "svc1", Status: swarm.TaskStatus{State: swarm.TaskStateRunning}},
			{ServiceID: "svc1", Status: swarm.TaskStatus{State: swarm.TaskStateRunning}},
			{ServiceID: "svc2", Status: swarm.TaskStatus{State: swarm.TaskStateRunning}},
			{ServiceID: "svc2", Status: swarm.TaskStatus{State: swarm.TaskStateRunning}},
			{ServiceID: "svc3", Status: swarm.TaskStatus{State: swarm.TaskStateRunning}},
			{ServiceID: "svc3", Status: swarm.TaskStatus{State: swarm.TaskStateRunning}},
			{ServiceID: "svc3", Status: swarm.TaskStatus{State: swarm.TaskStateRunning}},
		},
	}

	action := NewListStacksAction(mock)
	handler := NewListStacksHandler(action)

	req := httptest.NewRequest("GET", "/api/stacks", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Stacks []StackItem `json:"stacks"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if len(resp.Stacks) != 2 {
		t.Fatalf("expected 2 stacks, got %d", len(resp.Stacks))
	}

	// Sorted alphabetically: monitoring, myapp
	monitoring := resp.Stacks[0]
	if monitoring.Name != "monitoring" {
		t.Errorf("expected monitoring, got %s", monitoring.Name)
	}
	if monitoring.Services != 1 {
		t.Errorf("expected 1 service, got %d", monitoring.Services)
	}
	if monitoring.RunningReplicas != 3 {
		t.Errorf("expected 3 running, got %d", monitoring.RunningReplicas)
	}
	// Global mode: desired equals running
	if monitoring.DesiredReplicas != 3 {
		t.Errorf("expected 3 desired, got %d", monitoring.DesiredReplicas)
	}

	myapp := resp.Stacks[1]
	if myapp.Name != "myapp" {
		t.Errorf("expected myapp, got %s", myapp.Name)
	}
	if myapp.Services != 2 {
		t.Errorf("expected 2 services, got %d", myapp.Services)
	}
	if myapp.RunningReplicas != 4 {
		t.Errorf("expected 4 running, got %d", myapp.RunningReplicas)
	}
	if myapp.DesiredReplicas != 5 {
		t.Errorf("expected 5 desired, got %d", myapp.DesiredReplicas)
	}
	// Most recent updatedAt should be now
	expectedTime := now.Format("2006-01-02T15:04:05Z")
	if myapp.UpdatedAt != expectedTime {
		t.Errorf("expected updatedAt %s, got %s", expectedTime, myapp.UpdatedAt)
	}
}

func TestListStacksHandler_StandaloneServicesExcluded(t *testing.T) {
	mock := &mockDockerClient{
		services: []swarm.Service{
			{
				ID: "svc1",
				Spec: swarm.ServiceSpec{
					Annotations: swarm.Annotations{Name: "standalone-web"},
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
		tasks: []swarm.Task{},
	}

	action := NewListStacksAction(mock)
	handler := NewListStacksHandler(action)

	req := httptest.NewRequest("GET", "/api/stacks", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Stacks []StackItem `json:"stacks"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Stacks) != 0 {
		t.Errorf("expected 0 stacks, got %d", len(resp.Stacks))
	}
}

func TestListStacksHandler_EmptyServices(t *testing.T) {
	mock := &mockDockerClient{
		services: []swarm.Service{},
		tasks:    []swarm.Task{},
	}

	action := NewListStacksAction(mock)
	handler := NewListStacksHandler(action)

	req := httptest.NewRequest("GET", "/api/stacks", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Stacks []StackItem `json:"stacks"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Stacks) != 0 {
		t.Errorf("expected 0 stacks, got %d", len(resp.Stacks))
	}
}

func TestListStacksHandler_DockerError(t *testing.T) {
	mock := &mockDockerClient{
		svcErr: errors.New("connection refused"),
	}

	action := NewListStacksAction(mock)
	handler := NewListStacksHandler(action)

	req := httptest.NewRequest("GET", "/api/stacks", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "failed to list stacks" {
		t.Errorf("unexpected error message: %s", resp["error"])
	}
}

func TestListStacksHandler_TaskListError(t *testing.T) {
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
		taskErr: errors.New("task list failed"),
	}

	action := NewListStacksAction(mock)
	handler := NewListStacksHandler(action)

	req := httptest.NewRequest("GET", "/api/stacks", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}
}

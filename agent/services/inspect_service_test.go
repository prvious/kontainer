package services

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/docker/docker/api/types/mount"
	"github.com/docker/docker/api/types/swarm"
	"github.com/docker/docker/errdefs"
	"github.com/go-chi/chi/v5"
)

func timePtr(t time.Time) *time.Time { return &t }

func newInspectRouter(handler *InspectServiceHandler) http.Handler {
	r := chi.NewRouter()
	r.Method("GET", "/services/{serviceID}", handler)
	return r
}

func TestInspectServiceHandler_FullReplicatedService(t *testing.T) {
	now := time.Now().UTC()
	started := now.Add(-10 * time.Minute)
	completed := now.Add(-5 * time.Minute)

	mock := &mockDockerClient{
		inspectSvc: swarm.Service{
			ID: "svc1",
			Meta: swarm.Meta{
				CreatedAt: now.Add(-24 * time.Hour),
				UpdatedAt: now,
			},
			Spec: swarm.ServiceSpec{
				Annotations: swarm.Annotations{
					Name:   "web",
					Labels: map[string]string{"com.docker.stack.namespace": "myapp"},
				},
				Mode: swarm.ServiceMode{
					Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(3)},
				},
				TaskTemplate: swarm.TaskSpec{
					ContainerSpec: &swarm.ContainerSpec{
						Image: "nginx:latest@sha256:abc123",
						Env:   []string{"NODE_ENV=production", "DEBUG=true", "STANDALONE_KEY"},
						Mounts: []mount.Mount{
							{Type: mount.TypeBind, Source: "/host/data", Target: "/app/data", ReadOnly: true},
							{Type: mount.TypeVolume, Source: "db-vol", Target: "/var/lib/db", ReadOnly: false},
						},
					},
					Placement: &swarm.Placement{
						Constraints: []string{"node.role==manager", "node.labels.zone==us-east"},
					},
				},
			},
			Endpoint: swarm.Endpoint{
				Ports: []swarm.PortConfig{
					{PublishedPort: 8080, TargetPort: 80, Protocol: swarm.PortConfigProtocolTCP, PublishMode: swarm.PortConfigPublishModeIngress},
					{PublishedPort: 8443, TargetPort: 443, Protocol: swarm.PortConfigProtocolTCP, PublishMode: swarm.PortConfigPublishModeHost},
				},
			},
			UpdateStatus: &swarm.UpdateStatus{
				State:       swarm.UpdateStateCompleted,
				StartedAt:   timePtr(started),
				CompletedAt: timePtr(completed),
				Message:     "update completed",
			},
		},
	}

	action := NewInspectServiceAction(mock)
	handler := NewInspectServiceHandler(action)
	router := newInspectRouter(handler)

	req := httptest.NewRequest("GET", "/services/svc1", nil)
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var detail ServiceDetail
	if err := json.NewDecoder(w.Body).Decode(&detail); err != nil {
		t.Fatal(err)
	}

	if detail.ID != "svc1" {
		t.Errorf("expected id svc1, got %s", detail.ID)
	}
	if detail.Name != "web" {
		t.Errorf("expected name web, got %s", detail.Name)
	}
	if detail.Image != "nginx:latest" {
		t.Errorf("expected image nginx:latest, got %s", detail.Image)
	}
	if detail.Stack != "myapp" {
		t.Errorf("expected stack myapp, got %s", detail.Stack)
	}
	if detail.Mode != "replicated" {
		t.Errorf("expected mode replicated, got %s", detail.Mode)
	}
	if detail.Replicas == nil || *detail.Replicas != 3 {
		t.Errorf("expected replicas 3, got %v", detail.Replicas)
	}

	// Env
	if len(detail.Env) != 3 {
		t.Fatalf("expected 3 env vars, got %d", len(detail.Env))
	}
	if detail.Env[0].Key != "NODE_ENV" || detail.Env[0].Value != "production" {
		t.Errorf("unexpected env[0]: %+v", detail.Env[0])
	}
	if detail.Env[2].Key != "STANDALONE_KEY" || detail.Env[2].Value != "" {
		t.Errorf("expected env without = to have empty value: %+v", detail.Env[2])
	}

	// Mounts
	if len(detail.Mounts) != 2 {
		t.Fatalf("expected 2 mounts, got %d", len(detail.Mounts))
	}
	if detail.Mounts[0].Type != "bind" || !detail.Mounts[0].ReadOnly {
		t.Errorf("unexpected mount[0]: %+v", detail.Mounts[0])
	}

	// Constraints
	if len(detail.Constraints) != 2 {
		t.Fatalf("expected 2 constraints, got %d", len(detail.Constraints))
	}

	// Ports
	if len(detail.Ports) != 2 {
		t.Fatalf("expected 2 ports, got %d", len(detail.Ports))
	}
	if detail.Ports[0].PublishedPort != 8080 || detail.Ports[0].TargetPort != 80 {
		t.Errorf("unexpected port[0]: %+v", detail.Ports[0])
	}

	// Update status
	if detail.UpdateStatus == nil {
		t.Fatal("expected updateStatus to be set")
	}
	if detail.UpdateStatus.State != "completed" {
		t.Errorf("expected state completed, got %s", detail.UpdateStatus.State)
	}
	if detail.UpdateStatus.Message != "update completed" {
		t.Errorf("unexpected message: %s", detail.UpdateStatus.Message)
	}
}

func TestInspectServiceHandler_GlobalMode(t *testing.T) {
	mock := &mockDockerClient{
		inspectSvc: swarm.Service{
			ID: "svc-global",
			Meta: swarm.Meta{
				CreatedAt: time.Now().UTC(),
				UpdatedAt: time.Now().UTC(),
			},
			Spec: swarm.ServiceSpec{
				Annotations: swarm.Annotations{Name: "monitoring"},
				Mode: swarm.ServiceMode{
					Global: &swarm.GlobalService{},
				},
				TaskTemplate: swarm.TaskSpec{
					ContainerSpec: &swarm.ContainerSpec{
						Image: "prometheus:v2",
					},
				},
			},
		},
	}

	action := NewInspectServiceAction(mock)
	handler := NewInspectServiceHandler(action)
	router := newInspectRouter(handler)

	req := httptest.NewRequest("GET", "/services/svc-global", nil)
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	var detail ServiceDetail
	json.NewDecoder(w.Body).Decode(&detail)

	if detail.Mode != "global" {
		t.Errorf("expected mode global, got %s", detail.Mode)
	}
	if detail.Replicas != nil {
		t.Errorf("expected replicas nil for global, got %v", detail.Replicas)
	}
}

func TestInspectServiceHandler_MinimalService(t *testing.T) {
	mock := &mockDockerClient{
		inspectSvc: swarm.Service{
			ID: "svc-min",
			Meta: swarm.Meta{
				CreatedAt: time.Now().UTC(),
				UpdatedAt: time.Now().UTC(),
			},
			Spec: swarm.ServiceSpec{
				Annotations: swarm.Annotations{Name: "simple"},
				Mode: swarm.ServiceMode{
					Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(1)},
				},
				TaskTemplate: swarm.TaskSpec{
					ContainerSpec: &swarm.ContainerSpec{
						Image: "alpine:3",
					},
				},
			},
		},
	}

	action := NewInspectServiceAction(mock)
	handler := NewInspectServiceHandler(action)
	router := newInspectRouter(handler)

	req := httptest.NewRequest("GET", "/services/svc-min", nil)
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	var detail ServiceDetail
	json.NewDecoder(w.Body).Decode(&detail)

	if len(detail.Env) != 0 {
		t.Errorf("expected 0 env vars, got %d", len(detail.Env))
	}
	if len(detail.Mounts) != 0 {
		t.Errorf("expected 0 mounts, got %d", len(detail.Mounts))
	}
	if len(detail.Constraints) != 0 {
		t.Errorf("expected 0 constraints, got %d", len(detail.Constraints))
	}
	if len(detail.Ports) != 0 {
		t.Errorf("expected 0 ports, got %d", len(detail.Ports))
	}
	if detail.UpdateStatus != nil {
		t.Errorf("expected nil updateStatus, got %+v", detail.UpdateStatus)
	}
}

func TestInspectServiceHandler_DockerError(t *testing.T) {
	mock := &mockDockerClient{
		inspectErr: fmt.Errorf("connection refused"),
	}

	action := NewInspectServiceAction(mock)
	handler := NewInspectServiceHandler(action)
	router := newInspectRouter(handler)

	req := httptest.NewRequest("GET", "/services/svc1", nil)
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "failed to inspect service" {
		t.Errorf("unexpected error message: %s", resp["error"])
	}
}

func TestInspectServiceHandler_NotFound(t *testing.T) {
	mock := &mockDockerClient{
		inspectErr: errdefs.NotFound(fmt.Errorf("service not found")),
	}

	action := NewInspectServiceAction(mock)
	handler := NewInspectServiceHandler(action)
	router := newInspectRouter(handler)

	req := httptest.NewRequest("GET", "/services/nonexistent", nil)
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	if w.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "service not found" {
		t.Errorf("unexpected error message: %s", resp["error"])
	}
}

func TestInspectServiceHandler_EnvWithoutEquals(t *testing.T) {
	mock := &mockDockerClient{
		inspectSvc: swarm.Service{
			ID: "svc-env",
			Meta: swarm.Meta{
				CreatedAt: time.Now().UTC(),
				UpdatedAt: time.Now().UTC(),
			},
			Spec: swarm.ServiceSpec{
				Annotations: swarm.Annotations{Name: "env-test"},
				Mode: swarm.ServiceMode{
					Replicated: &swarm.ReplicatedService{Replicas: uint64Ptr(1)},
				},
				TaskTemplate: swarm.TaskSpec{
					ContainerSpec: &swarm.ContainerSpec{
						Image: "alpine:3",
						Env:   []string{"KEY_ONLY", "NORMAL=value"},
					},
				},
			},
		},
	}

	action := NewInspectServiceAction(mock)
	handler := NewInspectServiceHandler(action)
	router := newInspectRouter(handler)

	req := httptest.NewRequest("GET", "/services/svc-env", nil)
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	var detail ServiceDetail
	json.NewDecoder(w.Body).Decode(&detail)

	if len(detail.Env) != 2 {
		t.Fatalf("expected 2 env vars, got %d", len(detail.Env))
	}
	if detail.Env[0].Key != "KEY_ONLY" || detail.Env[0].Value != "" {
		t.Errorf("expected key-only env: %+v", detail.Env[0])
	}
	if detail.Env[1].Key != "NORMAL" || detail.Env[1].Value != "value" {
		t.Errorf("expected normal env: %+v", detail.Env[1])
	}
}

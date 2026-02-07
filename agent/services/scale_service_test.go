package services

import (
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/docker/docker/api/types/swarm"
	"github.com/docker/docker/errdefs"
	"github.com/go-chi/chi/v5"
)

func scaleRouter(handler *ScaleServiceHandler) *chi.Mux {
	r := chi.NewRouter()
	r.Method("PATCH", "/services/{serviceID}", handler)
	return r
}

func TestScaleServiceHandler_Success(t *testing.T) {
	mock := &mockDockerClient{
		inspectSvc: swarm.Service{
			ID: "svc1",
			Meta: swarm.Meta{
				Version: swarm.Version{Index: 42},
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
		},
	}

	action := NewScaleServiceAction(mock)
	handler := NewScaleServiceHandler(action)
	router := scaleRouter(handler)

	req := httptest.NewRequest("PATCH", "/services/svc1", strings.NewReader(`{"replicas":5}`))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", w.Code, w.Body.String())
	}

	var resp scaleResponse
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}
	if resp.Replicas != 5 {
		t.Errorf("expected replicas 5, got %d", resp.Replicas)
	}
}

func TestScaleServiceHandler_GlobalModeRejected(t *testing.T) {
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

	action := NewScaleServiceAction(mock)
	handler := NewScaleServiceHandler(action)
	router := scaleRouter(handler)

	req := httptest.NewRequest("PATCH", "/services/svc-global", strings.NewReader(`{"replicas":3}`))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "cannot scale a global service" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

func TestScaleServiceHandler_InvalidBody(t *testing.T) {
	mock := &mockDockerClient{}

	action := NewScaleServiceAction(mock)
	handler := NewScaleServiceHandler(action)
	router := scaleRouter(handler)

	req := httptest.NewRequest("PATCH", "/services/svc1", strings.NewReader(`not json`))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "invalid request body" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

func TestScaleServiceHandler_MissingReplicas(t *testing.T) {
	mock := &mockDockerClient{}

	action := NewScaleServiceAction(mock)
	handler := NewScaleServiceHandler(action)
	router := scaleRouter(handler)

	req := httptest.NewRequest("PATCH", "/services/svc1", strings.NewReader(`{}`))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "replicas field is required" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

type notFoundError struct{}

func (e *notFoundError) Error() string   { return "service not found" }
func (e *notFoundError) NotFound()       {}

func TestScaleServiceHandler_ServiceNotFound(t *testing.T) {
	mock := &mockDockerClient{
		inspectErr: &notFoundError{},
	}

	// Verify our error satisfies errdefs.IsNotFound
	if !errdefs.IsNotFound(mock.inspectErr) {
		t.Fatal("notFoundError should satisfy errdefs.IsNotFound")
	}

	action := NewScaleServiceAction(mock)
	handler := NewScaleServiceHandler(action)
	router := scaleRouter(handler)

	req := httptest.NewRequest("PATCH", "/services/nonexistent", strings.NewReader(`{"replicas":1}`))
	req.Header.Set("Content-Type", "application/json")
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

func TestScaleServiceHandler_DockerUpdateError(t *testing.T) {
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

	action := NewScaleServiceAction(mock)
	handler := NewScaleServiceHandler(action)
	router := scaleRouter(handler)

	req := httptest.NewRequest("PATCH", "/services/svc1", strings.NewReader(`{"replicas":5}`))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "failed to scale service" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

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

func setupAvailabilityRouter(handler *UpdateNodeAvailabilityHandler) *chi.Mux {
	r := chi.NewRouter()
	r.Put("/api/nodes/{nodeID}/availability", handler.ServeHTTP)
	return r
}

func TestUpdateNodeAvailabilityHandler_DrainSuccess(t *testing.T) {
	mock := &mockDockerClient{
		inspectNode: swarm.Node{
			ID:   "node1",
			Meta: swarm.Meta{Version: swarm.Version{Index: 5}},
			Spec: swarm.NodeSpec{
				Annotations:  swarm.Annotations{Name: "worker-1"},
				Role:         swarm.NodeRoleWorker,
				Availability: swarm.NodeAvailabilityActive,
			},
		},
	}

	action := NewUpdateNodeAvailabilityAction(mock)
	handler := NewUpdateNodeAvailabilityHandler(action)
	router := setupAvailabilityRouter(handler)

	body := `{"availability":"drain"}`
	req := httptest.NewRequest("PUT", "/api/nodes/node1/availability", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", w.Code, w.Body.String())
	}

	var resp updateNodeAvailabilityResponse
	json.NewDecoder(w.Body).Decode(&resp)
	if resp.Availability != "drain" {
		t.Errorf("expected drain, got %s", resp.Availability)
	}
}

func TestUpdateNodeAvailabilityHandler_ActivateSuccess(t *testing.T) {
	mock := &mockDockerClient{
		inspectNode: swarm.Node{
			ID:   "node1",
			Meta: swarm.Meta{Version: swarm.Version{Index: 5}},
			Spec: swarm.NodeSpec{
				Role:         swarm.NodeRoleWorker,
				Availability: swarm.NodeAvailabilityDrain,
			},
		},
	}

	action := NewUpdateNodeAvailabilityAction(mock)
	handler := NewUpdateNodeAvailabilityHandler(action)
	router := setupAvailabilityRouter(handler)

	body := `{"availability":"active"}`
	req := httptest.NewRequest("PUT", "/api/nodes/node1/availability", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}
}

func TestUpdateNodeAvailabilityHandler_InvalidAvailability(t *testing.T) {
	mock := &mockDockerClient{}
	action := NewUpdateNodeAvailabilityAction(mock)
	handler := NewUpdateNodeAvailabilityHandler(action)
	router := setupAvailabilityRouter(handler)

	body := `{"availability":"invalid"}`
	req := httptest.NewRequest("PUT", "/api/nodes/node1/availability", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "availability must be active, drain, or pause" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

func TestUpdateNodeAvailabilityHandler_InvalidBody(t *testing.T) {
	mock := &mockDockerClient{}
	action := NewUpdateNodeAvailabilityAction(mock)
	handler := NewUpdateNodeAvailabilityHandler(action)
	router := setupAvailabilityRouter(handler)

	req := httptest.NewRequest("PUT", "/api/nodes/node1/availability", bytes.NewBufferString("not json"))
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}
}

func TestUpdateNodeAvailabilityHandler_NodeNotFound(t *testing.T) {
	mock := &mockDockerClient{
		inspectNodeErr: dockererrdefs.NotFound(errors.New("not found")),
	}

	action := NewUpdateNodeAvailabilityAction(mock)
	handler := NewUpdateNodeAvailabilityHandler(action)
	router := setupAvailabilityRouter(handler)

	body := `{"availability":"drain"}`
	req := httptest.NewRequest("PUT", "/api/nodes/nonexistent/availability", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d", w.Code)
	}
}

func TestUpdateNodeAvailabilityHandler_DockerUpdateError(t *testing.T) {
	mock := &mockDockerClient{
		inspectNode: swarm.Node{
			ID:   "node1",
			Meta: swarm.Meta{Version: swarm.Version{Index: 5}},
			Spec: swarm.NodeSpec{
				Role:         swarm.NodeRoleWorker,
				Availability: swarm.NodeAvailabilityActive,
			},
		},
		nodeUpdateErr: errors.New("update failed"),
	}

	action := NewUpdateNodeAvailabilityAction(mock)
	handler := NewUpdateNodeAvailabilityHandler(action)
	router := setupAvailabilityRouter(handler)

	body := `{"availability":"drain"}`
	req := httptest.NewRequest("PUT", "/api/nodes/node1/availability", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}
}

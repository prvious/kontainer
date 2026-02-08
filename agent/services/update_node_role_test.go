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

func setupRoleRouter(handler *UpdateNodeRoleHandler) *chi.Mux {
	r := chi.NewRouter()
	r.Put("/api/nodes/{nodeID}/role", handler.ServeHTTP)
	return r
}

func TestUpdateNodeRoleHandler_PromoteSuccess(t *testing.T) {
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

	action := NewUpdateNodeRoleAction(mock)
	handler := NewUpdateNodeRoleHandler(action)
	router := setupRoleRouter(handler)

	body := `{"role":"manager"}`
	req := httptest.NewRequest("PUT", "/api/nodes/node1/role", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", w.Code, w.Body.String())
	}

	var resp updateNodeRoleResponse
	json.NewDecoder(w.Body).Decode(&resp)
	if resp.Role != "manager" {
		t.Errorf("expected manager, got %s", resp.Role)
	}
}

func TestUpdateNodeRoleHandler_DemoteSuccess(t *testing.T) {
	mock := &mockDockerClient{
		inspectNode: swarm.Node{
			ID:   "node1",
			Meta: swarm.Meta{Version: swarm.Version{Index: 5}},
			Spec: swarm.NodeSpec{
				Role:         swarm.NodeRoleManager,
				Availability: swarm.NodeAvailabilityActive,
			},
		},
	}

	action := NewUpdateNodeRoleAction(mock)
	handler := NewUpdateNodeRoleHandler(action)
	router := setupRoleRouter(handler)

	body := `{"role":"worker"}`
	req := httptest.NewRequest("PUT", "/api/nodes/node1/role", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}
}

func TestUpdateNodeRoleHandler_InvalidRole(t *testing.T) {
	mock := &mockDockerClient{}
	action := NewUpdateNodeRoleAction(mock)
	handler := NewUpdateNodeRoleHandler(action)
	router := setupRoleRouter(handler)

	body := `{"role":"admin"}`
	req := httptest.NewRequest("PUT", "/api/nodes/node1/role", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "role must be manager or worker" {
		t.Errorf("unexpected error: %s", resp["error"])
	}
}

func TestUpdateNodeRoleHandler_InvalidBody(t *testing.T) {
	mock := &mockDockerClient{}
	action := NewUpdateNodeRoleAction(mock)
	handler := NewUpdateNodeRoleHandler(action)
	router := setupRoleRouter(handler)

	req := httptest.NewRequest("PUT", "/api/nodes/node1/role", bytes.NewBufferString("not json"))
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}
}

func TestUpdateNodeRoleHandler_NodeNotFound(t *testing.T) {
	mock := &mockDockerClient{
		inspectNodeErr: dockererrdefs.NotFound(errors.New("not found")),
	}

	action := NewUpdateNodeRoleAction(mock)
	handler := NewUpdateNodeRoleHandler(action)
	router := setupRoleRouter(handler)

	body := `{"role":"manager"}`
	req := httptest.NewRequest("PUT", "/api/nodes/nonexistent/role", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusNotFound {
		t.Fatalf("expected 404, got %d", w.Code)
	}
}

func TestUpdateNodeRoleHandler_DockerUpdateError(t *testing.T) {
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

	action := NewUpdateNodeRoleAction(mock)
	handler := NewUpdateNodeRoleHandler(action)
	router := setupRoleRouter(handler)

	body := `{"role":"manager"}`
	req := httptest.NewRequest("PUT", "/api/nodes/node1/role", bytes.NewBufferString(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	router.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}
}

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

func TestListNodesHandler_ManagerAndWorker(t *testing.T) {
	now := time.Now().UTC()
	mock := &mockDockerClient{
		nodes: []swarm.Node{
			{
				ID: "node1",
				Spec: swarm.NodeSpec{
					Role:         swarm.NodeRoleManager,
					Availability: swarm.NodeAvailabilityActive,
				},
				Description: swarm.NodeDescription{
					Hostname: "manager-1",
					Platform: swarm.Platform{OS: "linux", Architecture: "x86_64"},
					Engine:   swarm.EngineDescription{EngineVersion: "24.0.7"},
				},
				Status: swarm.NodeStatus{
					State: swarm.NodeStateReady,
					Addr:  "10.0.0.1",
				},
				ManagerStatus: &swarm.ManagerStatus{Leader: true},
				Meta:          swarm.Meta{UpdatedAt: now},
			},
			{
				ID: "node2",
				Spec: swarm.NodeSpec{
					Role:         swarm.NodeRoleWorker,
					Availability: swarm.NodeAvailabilityActive,
				},
				Description: swarm.NodeDescription{
					Hostname: "worker-1",
					Platform: swarm.Platform{OS: "linux", Architecture: "aarch64"},
					Engine:   swarm.EngineDescription{EngineVersion: "24.0.7"},
				},
				Status: swarm.NodeStatus{
					State: swarm.NodeStateReady,
					Addr:  "10.0.0.2",
				},
				Meta: swarm.Meta{UpdatedAt: now},
			},
		},
	}

	action := NewListNodesAction(mock)
	handler := NewListNodesHandler(action)

	req := httptest.NewRequest("GET", "/api/nodes", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Nodes []NodeItem `json:"nodes"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	if len(resp.Nodes) != 2 {
		t.Fatalf("expected 2 nodes, got %d", len(resp.Nodes))
	}

	mgr := resp.Nodes[0]
	if mgr.ID != "node1" {
		t.Errorf("expected id node1, got %s", mgr.ID)
	}
	if mgr.Hostname != "manager-1" {
		t.Errorf("expected hostname manager-1, got %s", mgr.Hostname)
	}
	if mgr.Role != "manager" {
		t.Errorf("expected role manager, got %s", mgr.Role)
	}
	if mgr.Status != "ready" {
		t.Errorf("expected status ready, got %s", mgr.Status)
	}
	if mgr.Availability != "active" {
		t.Errorf("expected availability active, got %s", mgr.Availability)
	}
	if mgr.EngineVersion != "24.0.7" {
		t.Errorf("expected engine 24.0.7, got %s", mgr.EngineVersion)
	}
	if mgr.OS != "linux" {
		t.Errorf("expected os linux, got %s", mgr.OS)
	}
	if mgr.Architecture != "x86_64" {
		t.Errorf("expected arch x86_64, got %s", mgr.Architecture)
	}
	if mgr.IP != "10.0.0.1" {
		t.Errorf("expected ip 10.0.0.1, got %s", mgr.IP)
	}
	if !mgr.Leader {
		t.Error("expected leader to be true")
	}

	wrk := resp.Nodes[1]
	if wrk.Role != "worker" {
		t.Errorf("expected role worker, got %s", wrk.Role)
	}
	if wrk.Leader {
		t.Error("expected leader to be false for worker")
	}
	if wrk.Architecture != "aarch64" {
		t.Errorf("expected arch aarch64, got %s", wrk.Architecture)
	}
}

func TestListNodesHandler_ManagersSortedFirst(t *testing.T) {
	now := time.Now().UTC()
	mock := &mockDockerClient{
		nodes: []swarm.Node{
			{
				ID:   "worker1",
				Spec: swarm.NodeSpec{Role: swarm.NodeRoleWorker, Availability: swarm.NodeAvailabilityActive},
				Description: swarm.NodeDescription{
					Hostname: "worker-1",
					Platform: swarm.Platform{OS: "linux", Architecture: "x86_64"},
					Engine:   swarm.EngineDescription{EngineVersion: "24.0.7"},
				},
				Status: swarm.NodeStatus{State: swarm.NodeStateReady, Addr: "10.0.0.3"},
				Meta:   swarm.Meta{UpdatedAt: now},
			},
			{
				ID:   "mgr2",
				Spec: swarm.NodeSpec{Role: swarm.NodeRoleManager, Availability: swarm.NodeAvailabilityActive},
				Description: swarm.NodeDescription{
					Hostname: "manager-2",
					Platform: swarm.Platform{OS: "linux", Architecture: "x86_64"},
					Engine:   swarm.EngineDescription{EngineVersion: "24.0.7"},
				},
				Status:        swarm.NodeStatus{State: swarm.NodeStateReady, Addr: "10.0.0.2"},
				ManagerStatus: &swarm.ManagerStatus{Leader: false},
				Meta:          swarm.Meta{UpdatedAt: now},
			},
			{
				ID:   "worker2",
				Spec: swarm.NodeSpec{Role: swarm.NodeRoleWorker, Availability: swarm.NodeAvailabilityActive},
				Description: swarm.NodeDescription{
					Hostname: "worker-2",
					Platform: swarm.Platform{OS: "linux", Architecture: "x86_64"},
					Engine:   swarm.EngineDescription{EngineVersion: "24.0.7"},
				},
				Status: swarm.NodeStatus{State: swarm.NodeStateReady, Addr: "10.0.0.4"},
				Meta:   swarm.Meta{UpdatedAt: now},
			},
			{
				ID:   "mgr1",
				Spec: swarm.NodeSpec{Role: swarm.NodeRoleManager, Availability: swarm.NodeAvailabilityActive},
				Description: swarm.NodeDescription{
					Hostname: "manager-1",
					Platform: swarm.Platform{OS: "linux", Architecture: "x86_64"},
					Engine:   swarm.EngineDescription{EngineVersion: "24.0.7"},
				},
				Status:        swarm.NodeStatus{State: swarm.NodeStateReady, Addr: "10.0.0.1"},
				ManagerStatus: &swarm.ManagerStatus{Leader: true},
				Meta:          swarm.Meta{UpdatedAt: now},
			},
		},
	}

	action := NewListNodesAction(mock)
	handler := NewListNodesHandler(action)

	req := httptest.NewRequest("GET", "/api/nodes", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	var resp struct {
		Nodes []NodeItem `json:"nodes"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Nodes) != 4 {
		t.Fatalf("expected 4 nodes, got %d", len(resp.Nodes))
	}

	// Leader manager first
	if resp.Nodes[0].ID != "mgr1" {
		t.Errorf("expected leader manager first, got %s (role=%s, leader=%v)", resp.Nodes[0].ID, resp.Nodes[0].Role, resp.Nodes[0].Leader)
	}
	// Non-leader manager second
	if resp.Nodes[1].ID != "mgr2" {
		t.Errorf("expected non-leader manager second, got %s (role=%s, leader=%v)", resp.Nodes[1].ID, resp.Nodes[1].Role, resp.Nodes[1].Leader)
	}
	// Workers last
	if resp.Nodes[2].Role != "worker" {
		t.Errorf("expected worker at index 2, got role %s", resp.Nodes[2].Role)
	}
	if resp.Nodes[3].Role != "worker" {
		t.Errorf("expected worker at index 3, got role %s", resp.Nodes[3].Role)
	}
}

func TestListNodesHandler_DrainAvailability(t *testing.T) {
	mock := &mockDockerClient{
		nodes: []swarm.Node{
			{
				ID: "node-drain",
				Spec: swarm.NodeSpec{
					Role:         swarm.NodeRoleWorker,
					Availability: swarm.NodeAvailabilityDrain,
				},
				Description: swarm.NodeDescription{
					Hostname: "drain-node",
					Platform: swarm.Platform{OS: "linux", Architecture: "x86_64"},
					Engine:   swarm.EngineDescription{EngineVersion: "24.0.7"},
				},
				Status: swarm.NodeStatus{
					State: swarm.NodeStateReady,
					Addr:  "10.0.0.3",
				},
				Meta: swarm.Meta{UpdatedAt: time.Now().UTC()},
			},
		},
	}

	action := NewListNodesAction(mock)
	handler := NewListNodesHandler(action)

	req := httptest.NewRequest("GET", "/api/nodes", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	var resp struct {
		Nodes []NodeItem `json:"nodes"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Nodes) != 1 {
		t.Fatalf("expected 1 node, got %d", len(resp.Nodes))
	}
	if resp.Nodes[0].Availability != "drain" {
		t.Errorf("expected availability drain, got %s", resp.Nodes[0].Availability)
	}
}

func TestListNodesHandler_DownStatus(t *testing.T) {
	mock := &mockDockerClient{
		nodes: []swarm.Node{
			{
				ID: "node-down",
				Spec: swarm.NodeSpec{
					Role:         swarm.NodeRoleWorker,
					Availability: swarm.NodeAvailabilityActive,
				},
				Description: swarm.NodeDescription{
					Hostname: "down-node",
					Platform: swarm.Platform{OS: "linux", Architecture: "x86_64"},
					Engine:   swarm.EngineDescription{EngineVersion: "24.0.7"},
				},
				Status: swarm.NodeStatus{
					State: swarm.NodeStateDown,
					Addr:  "10.0.0.4",
				},
				Meta: swarm.Meta{UpdatedAt: time.Now().UTC()},
			},
		},
	}

	action := NewListNodesAction(mock)
	handler := NewListNodesHandler(action)

	req := httptest.NewRequest("GET", "/api/nodes", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	var resp struct {
		Nodes []NodeItem `json:"nodes"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if resp.Nodes[0].Status != "down" {
		t.Errorf("expected status down, got %s", resp.Nodes[0].Status)
	}
}

func TestListNodesHandler_EmptyList(t *testing.T) {
	mock := &mockDockerClient{
		nodes: []swarm.Node{},
	}

	action := NewListNodesAction(mock)
	handler := NewListNodesHandler(action)

	req := httptest.NewRequest("GET", "/api/nodes", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Nodes []NodeItem `json:"nodes"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	if len(resp.Nodes) != 0 {
		t.Errorf("expected 0 nodes, got %d", len(resp.Nodes))
	}
}

func TestListNodesHandler_DockerError(t *testing.T) {
	mock := &mockDockerClient{
		nodeErr: errors.New("connection refused"),
	}

	action := NewListNodesAction(mock)
	handler := NewListNodesHandler(action)

	req := httptest.NewRequest("GET", "/api/nodes", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Fatalf("expected 500, got %d", w.Code)
	}

	var resp map[string]string
	json.NewDecoder(w.Body).Decode(&resp)
	if resp["error"] != "failed to list nodes" {
		t.Errorf("unexpected error message: %s", resp["error"])
	}
}

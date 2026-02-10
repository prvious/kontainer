package services

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/prvious/kontainer/agent/cluster"
)

func TestListAgentsHandler_ReturnsAgentsFromRegistry(t *testing.T) {
	registry := cluster.NewRegistry()
	registry.Update([]cluster.AgentInfo{
		{NodeID: "node1", Hostname: "worker-1", Address: "10.0.0.2:8080", Role: cluster.RoleFollower, Status: "healthy"},
		{NodeID: "node2", Hostname: "worker-2", Address: "10.0.0.3:8080", Role: cluster.RoleFollower, Status: "healthy"},
	})

	self := cluster.AgentInfo{
		NodeID:   "",
		Hostname: "manager",
		Address:  "http://localhost:8080",
		Role:     cluster.RoleLeader,
		Status:   "healthy",
	}

	action := NewListAgentsAction(registry, self)
	handler := NewListAgentsHandler(action)

	req := httptest.NewRequest("GET", "/api/agents", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Agents []cluster.AgentInfo `json:"agents"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	// Self + 2 followers
	if len(resp.Agents) != 3 {
		t.Fatalf("expected 3 agents, got %d", len(resp.Agents))
	}

	// First agent should be self (leader)
	if resp.Agents[0].Role != cluster.RoleLeader {
		t.Errorf("expected first agent to be leader, got %s", resp.Agents[0].Role)
	}
	if resp.Agents[0].Hostname != "manager" {
		t.Errorf("expected hostname manager, got %s", resp.Agents[0].Hostname)
	}
}

func TestListAgentsHandler_EmptyRegistry(t *testing.T) {
	registry := cluster.NewRegistry()

	self := cluster.AgentInfo{
		Hostname: "manager",
		Address:  "http://localhost:8080",
		Role:     cluster.RoleLeader,
		Status:   "healthy",
	}

	action := NewListAgentsAction(registry, self)
	handler := NewListAgentsHandler(action)

	req := httptest.NewRequest("GET", "/api/agents", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	var resp struct {
		Agents []cluster.AgentInfo `json:"agents"`
	}
	json.NewDecoder(w.Body).Decode(&resp)

	// Only self
	if len(resp.Agents) != 1 {
		t.Fatalf("expected 1 agent (self), got %d", len(resp.Agents))
	}
	if resp.Agents[0].Role != cluster.RoleLeader {
		t.Errorf("expected leader, got %s", resp.Agents[0].Role)
	}
}

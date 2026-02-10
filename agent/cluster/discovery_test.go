package cluster

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/swarm"
)

type mockSwarmClient struct {
	tasks   []swarm.Task
	taskErr error
	nodes   []swarm.Node
	nodeErr error
}

func (m *mockSwarmClient) TaskList(_ context.Context, _ types.TaskListOptions) ([]swarm.Task, error) {
	return m.tasks, m.taskErr
}

func (m *mockSwarmClient) NodeList(_ context.Context, _ types.NodeListOptions) ([]swarm.Node, error) {
	return m.nodes, m.nodeErr
}

func TestDiscovery_DiscoverHealthyFollowers(t *testing.T) {
	// Start a fake follower health endpoint
	followerServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get(InternalSecretHeader) != "test-secret" {
			http.Error(w, "unauthorized", http.StatusUnauthorized)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{
			"status":   "ok",
			"nodeId":   "follower-node-1",
			"hostname": "worker-1",
		})
	}))
	defer followerServer.Close()

	// Extract host:port from test server URL
	addr := strings.TrimPrefix(followerServer.URL, "http://")
	parts := strings.SplitN(addr, ":", 2)
	ip := parts[0]
	port := parts[1]

	mock := &mockSwarmClient{
		tasks: []swarm.Task{
			{
				NodeID: "follower-node-1",
				Status: swarm.TaskStatus{State: swarm.TaskStateRunning},
				NetworksAttachments: []swarm.NetworkAttachment{
					{Addresses: []string{ip + "/24"}},
				},
			},
		},
		nodes: []swarm.Node{
			{ID: "follower-node-1", Description: swarm.NodeDescription{Hostname: "worker-1"}},
		},
	}

	registry := NewRegistry()
	d := NewDiscovery(mock, registry, "kontainer-agent", "test-secret", port)
	d.discover(context.Background())

	agents := registry.All()
	if len(agents) != 1 {
		t.Fatalf("expected 1 healthy agent, got %d", len(agents))
	}
	if agents[0].NodeID != "follower-node-1" {
		t.Errorf("expected node ID follower-node-1, got %s", agents[0].NodeID)
	}
	if agents[0].Status != "healthy" {
		t.Errorf("expected status healthy, got %s", agents[0].Status)
	}
	if agents[0].Role != RoleFollower {
		t.Errorf("expected role follower, got %s", agents[0].Role)
	}
}

func TestDiscovery_UnreachableFollower(t *testing.T) {
	mock := &mockSwarmClient{
		tasks: []swarm.Task{
			{
				NodeID: "follower-node-1",
				Status: swarm.TaskStatus{State: swarm.TaskStateRunning},
				NetworksAttachments: []swarm.NetworkAttachment{
					{Addresses: []string{"192.0.2.1/24"}}, // unreachable IP
				},
			},
		},
		nodes: []swarm.Node{
			{ID: "follower-node-1", Description: swarm.NodeDescription{Hostname: "worker-1"}},
		},
	}

	registry := NewRegistry()
	d := NewDiscovery(mock, registry, "kontainer-agent", "test-secret", "99999")
	d.discover(context.Background())

	agents := registry.All()
	if len(agents) != 0 {
		t.Fatalf("expected 0 agents for unreachable follower, got %d", len(agents))
	}
}

func TestDiscovery_FiltersNonRunningTasks(t *testing.T) {
	mock := &mockSwarmClient{
		tasks: []swarm.Task{
			{
				NodeID: "follower-node-1",
				Status: swarm.TaskStatus{State: swarm.TaskStateFailed},
				NetworksAttachments: []swarm.NetworkAttachment{
					{Addresses: []string{"10.0.0.5/24"}},
				},
			},
			{
				NodeID: "follower-node-2",
				Status: swarm.TaskStatus{State: swarm.TaskStateShutdown},
				NetworksAttachments: []swarm.NetworkAttachment{
					{Addresses: []string{"10.0.0.6/24"}},
				},
			},
		},
		nodes: []swarm.Node{},
	}

	registry := NewRegistry()
	d := NewDiscovery(mock, registry, "kontainer-agent", "test-secret", "8080")
	d.discover(context.Background())

	agents := registry.All()
	if len(agents) != 0 {
		t.Fatalf("expected 0 agents for non-running tasks, got %d", len(agents))
	}
}

func TestExtractTaskIP(t *testing.T) {
	tests := []struct {
		name     string
		task     swarm.Task
		expected string
	}{
		{
			name: "CIDR address",
			task: swarm.Task{
				NetworksAttachments: []swarm.NetworkAttachment{
					{Addresses: []string{"10.0.0.5/24"}},
				},
			},
			expected: "10.0.0.5",
		},
		{
			name: "plain address",
			task: swarm.Task{
				NetworksAttachments: []swarm.NetworkAttachment{
					{Addresses: []string{"10.0.0.5"}},
				},
			},
			expected: "10.0.0.5",
		},
		{
			name:     "no attachments",
			task:     swarm.Task{},
			expected: "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			ip := extractTaskIP(tt.task)
			if ip != tt.expected {
				t.Errorf("expected %q, got %q", tt.expected, ip)
			}
		})
	}
}

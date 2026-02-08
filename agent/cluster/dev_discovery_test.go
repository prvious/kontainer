package cluster

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestParseDevFollowers(t *testing.T) {
	tests := []struct {
		input    string
		expected []string
	}{
		{"", nil},
		{"localhost:8081", []string{"localhost:8081"}},
		{"localhost:8081,localhost:8082", []string{"localhost:8081", "localhost:8082"}},
		{" localhost:8081 , localhost:8082 ", []string{"localhost:8081", "localhost:8082"}},
		{"localhost:8081,,localhost:8082", []string{"localhost:8081", "localhost:8082"}},
	}
	for _, tt := range tests {
		result := ParseDevFollowers(tt.input)
		if len(result) != len(tt.expected) {
			t.Errorf("ParseDevFollowers(%q) = %v, want %v", tt.input, result, tt.expected)
			continue
		}
		for i := range result {
			if result[i] != tt.expected[i] {
				t.Errorf("ParseDevFollowers(%q)[%d] = %q, want %q", tt.input, i, result[i], tt.expected[i])
			}
		}
	}
}

func TestDevDiscovery_DiscoverHealthyFollowers(t *testing.T) {
	server1 := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get(InternalSecretHeader) != "dev-secret" {
			http.Error(w, "unauthorized", http.StatusUnauthorized)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{
			"status":   "ok",
			"nodeId":   "worker-1",
			"hostname": "worker-1",
		})
	}))
	defer server1.Close()

	server2 := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get(InternalSecretHeader) != "dev-secret" {
			http.Error(w, "unauthorized", http.StatusUnauthorized)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{
			"status":   "ok",
			"nodeId":   "worker-2",
			"hostname": "worker-2",
		})
	}))
	defer server2.Close()

	addr1 := strings.TrimPrefix(server1.URL, "http://")
	addr2 := strings.TrimPrefix(server2.URL, "http://")

	registry := NewRegistry()
	d := NewDevDiscovery([]string{addr1, addr2}, "dev-secret", registry)
	d.discover(context.Background())

	agents := registry.All()
	if len(agents) != 2 {
		t.Fatalf("expected 2 agents, got %d", len(agents))
	}

	for _, a := range agents {
		if a.Role != RoleFollower {
			t.Errorf("expected role follower, got %s", a.Role)
		}
		if a.Status != "healthy" {
			t.Errorf("expected status healthy, got %s", a.Status)
		}
	}
}

func TestDevDiscovery_UnreachableFollower(t *testing.T) {
	// One real server, one fake address
	server1 := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get(InternalSecretHeader) != "dev-secret" {
			http.Error(w, "unauthorized", http.StatusUnauthorized)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{
			"status":   "ok",
			"nodeId":   "worker-1",
			"hostname": "worker-1",
		})
	}))
	defer server1.Close()

	addr1 := strings.TrimPrefix(server1.URL, "http://")

	registry := NewRegistry()
	d := NewDevDiscovery([]string{addr1, "localhost:1"}, "dev-secret", registry)
	d.discover(context.Background())

	agents := registry.All()
	if len(agents) != 1 {
		t.Fatalf("expected 1 agent (unreachable one skipped), got %d", len(agents))
	}
	if agents[0].NodeID != "worker-1" {
		t.Errorf("expected worker-1, got %s", agents[0].NodeID)
	}
}

func TestDevDiscovery_WrongSecret(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get(InternalSecretHeader) != "real-secret" {
			http.Error(w, "unauthorized", http.StatusUnauthorized)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{"status": "ok"})
	}))
	defer server.Close()

	addr := strings.TrimPrefix(server.URL, "http://")
	registry := NewRegistry()
	d := NewDevDiscovery([]string{addr}, "wrong-secret", registry)
	d.discover(context.Background())

	agents := registry.All()
	if len(agents) != 0 {
		t.Fatalf("expected 0 agents with wrong secret, got %d", len(agents))
	}
}

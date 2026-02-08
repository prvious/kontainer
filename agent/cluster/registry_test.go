package cluster

import (
	"sort"
	"sync"
	"testing"
)

func TestRegistry_UpdateAndAll(t *testing.T) {
	r := NewRegistry()

	agents := []AgentInfo{
		{NodeID: "node1", Hostname: "host1", Address: "10.0.0.1:8080", Role: RoleFollower, Status: "healthy"},
		{NodeID: "node2", Hostname: "host2", Address: "10.0.0.2:8080", Role: RoleFollower, Status: "healthy"},
	}
	r.Update(agents)

	all := r.All()
	if len(all) != 2 {
		t.Fatalf("expected 2 agents, got %d", len(all))
	}

	// Sort for deterministic comparison
	sort.Slice(all, func(i, j int) bool { return all[i].NodeID < all[j].NodeID })
	if all[0].NodeID != "node1" || all[1].NodeID != "node2" {
		t.Errorf("unexpected agents: %+v", all)
	}
}

func TestRegistry_Get(t *testing.T) {
	r := NewRegistry()
	r.Update([]AgentInfo{
		{NodeID: "node1", Hostname: "host1", Address: "10.0.0.1:8080"},
	})

	a := r.Get("node1")
	if a == nil {
		t.Fatal("expected agent, got nil")
	}
	if a.Hostname != "host1" {
		t.Errorf("expected host1, got %s", a.Hostname)
	}

	if r.Get("nonexistent") != nil {
		t.Error("expected nil for nonexistent node")
	}
}

func TestRegistry_ByNodeID(t *testing.T) {
	r := NewRegistry()
	r.Update([]AgentInfo{
		{NodeID: "node1", Address: "10.0.0.1:8080"},
	})

	addr, ok := r.ByNodeID("node1")
	if !ok || addr != "10.0.0.1:8080" {
		t.Errorf("expected 10.0.0.1:8080, got %s (ok=%v)", addr, ok)
	}

	_, ok = r.ByNodeID("nonexistent")
	if ok {
		t.Error("expected ok=false for nonexistent node")
	}
}

func TestRegistry_UpdateReplaces(t *testing.T) {
	r := NewRegistry()
	r.Update([]AgentInfo{
		{NodeID: "node1", Address: "10.0.0.1:8080"},
		{NodeID: "node2", Address: "10.0.0.2:8080"},
	})

	// Replace with only node1
	r.Update([]AgentInfo{
		{NodeID: "node1", Address: "10.0.0.1:8080"},
	})

	if len(r.All()) != 1 {
		t.Fatalf("expected 1 agent after update, got %d", len(r.All()))
	}
	if r.Get("node2") != nil {
		t.Error("node2 should have been removed")
	}
}

func TestRegistry_ConcurrentAccess(t *testing.T) {
	r := NewRegistry()
	var wg sync.WaitGroup

	for i := 0; i < 100; i++ {
		wg.Add(2)
		go func() {
			defer wg.Done()
			r.Update([]AgentInfo{{NodeID: "node1", Address: "10.0.0.1:8080"}})
		}()
		go func() {
			defer wg.Done()
			_ = r.All()
			_ = r.Get("node1")
			_, _ = r.ByNodeID("node1")
		}()
	}

	wg.Wait()
}

package cluster

import (
	"sync"
	"time"
)

const (
	RoleLeader   = "leader"
	RoleFollower = "follower"
)

// AgentInfo represents a known agent in the cluster.
type AgentInfo struct {
	NodeID   string `json:"nodeId"`
	Hostname string `json:"hostname"`
	Address  string `json:"address"`
	Role     string `json:"role"`
	Status   string `json:"status"`
	LastSeen string `json:"lastSeen"`
}

// NewAgentInfo creates an AgentInfo with the current time as LastSeen.
func NewAgentInfo(nodeID, hostname, address, role, status string) AgentInfo {
	return AgentInfo{
		NodeID:   nodeID,
		Hostname: hostname,
		Address:  address,
		Role:     role,
		Status:   status,
		LastSeen: time.Now().UTC().Format(time.RFC3339),
	}
}

// Registry is a thread-safe in-memory store of known cluster agents.
type Registry struct {
	mu     sync.RWMutex
	agents map[string]*AgentInfo
}

func NewRegistry() *Registry {
	return &Registry{
		agents: make(map[string]*AgentInfo),
	}
}

// SetSelf registers the leader's own entry. This entry is preserved across
// UpdateFollowers calls so discovery never evicts the leader.
func (r *Registry) SetSelf(agent AgentInfo) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.agents[agent.NodeID] = &agent
}

// UpdateFollowers replaces all follower entries with the provided list while
// preserving any non-follower (leader) entries.
func (r *Registry) UpdateFollowers(agents []AgentInfo) {
	r.mu.Lock()
	defer r.mu.Unlock()

	for id, a := range r.agents {
		if a.Role == RoleFollower {
			delete(r.agents, id)
		}
	}

	for i := range agents {
		r.agents[agents[i].NodeID] = &agents[i]
	}
}

// Get returns a copy of the agent with the given node ID, or nil.
func (r *Registry) Get(nodeID string) *AgentInfo {
	r.mu.RLock()
	defer r.mu.RUnlock()
	if a, ok := r.agents[nodeID]; ok {
		cp := *a
		return &cp
	}
	return nil
}

// All returns a copy of all registered agents.
func (r *Registry) All() []AgentInfo {
	r.mu.RLock()
	defer r.mu.RUnlock()
	result := make([]AgentInfo, 0, len(r.agents))
	for _, a := range r.agents {
		result = append(result, *a)
	}
	return result
}

// ByNodeID returns the address of the agent with the given node ID.
func (r *Registry) ByNodeID(nodeID string) (string, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	if a, ok := r.agents[nodeID]; ok {
		return a.Address, true
	}
	return "", false
}

package cluster

import "sync"

type Registry struct {
	mu     sync.RWMutex
	agents map[string]*AgentInfo
}

func NewRegistry() *Registry {
	return &Registry{
		agents: make(map[string]*AgentInfo),
	}
}

func (r *Registry) Update(agents []AgentInfo) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.agents = make(map[string]*AgentInfo, len(agents))
	for i := range agents {
		r.agents[agents[i].NodeID] = &agents[i]
	}
}

func (r *Registry) Get(nodeID string) *AgentInfo {
	r.mu.RLock()
	defer r.mu.RUnlock()
	if a, ok := r.agents[nodeID]; ok {
		cp := *a
		return &cp
	}
	return nil
}

func (r *Registry) All() []AgentInfo {
	r.mu.RLock()
	defer r.mu.RUnlock()
	result := make([]AgentInfo, 0, len(r.agents))
	for _, a := range r.agents {
		result = append(result, *a)
	}
	return result
}

func (r *Registry) ByNodeID(nodeID string) (string, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	if a, ok := r.agents[nodeID]; ok {
		return a.Address, true
	}
	return "", false
}

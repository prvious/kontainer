package services

import (
	"encoding/json"
	"net/http"

	"github.com/prvious/kontainer/agent/cluster"
)

type ListAgentsAction struct {
	registry *cluster.Registry
	self     cluster.AgentInfo
}

func NewListAgentsAction(registry *cluster.Registry, self cluster.AgentInfo) *ListAgentsAction {
	return &ListAgentsAction{registry: registry, self: self}
}

func (a *ListAgentsAction) Execute() []cluster.AgentInfo {
	agents := []cluster.AgentInfo{a.self}
	agents = append(agents, a.registry.All()...)
	return agents
}

type ListAgentsHandler struct {
	action *ListAgentsAction
}

func NewListAgentsHandler(action *ListAgentsAction) *ListAgentsHandler {
	return &ListAgentsHandler{action: action}
}

func (h *ListAgentsHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	agents := h.action.Execute()
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]any{"agents": agents})
}

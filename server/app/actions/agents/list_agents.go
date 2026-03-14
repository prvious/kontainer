package agents

import (
	"github.com/prvious/kontainer/server/app/services/cluster"
)

type ListAgentsAction struct {
	registry *cluster.Registry
}

func NewListAgentsAction(registry *cluster.Registry) *ListAgentsAction {
	return &ListAgentsAction{registry: registry}
}

func (a *ListAgentsAction) Execute() []cluster.AgentInfo {
	return a.registry.All()
}

package nodes

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type ListNodesAction struct {
	orchestrator docker.Orchestrator
}

func NewListNodesAction(orchestrator docker.Orchestrator) *ListNodesAction {
	return &ListNodesAction{orchestrator: orchestrator}
}

func (a *ListNodesAction) Execute(ctx context.Context) ([]docker.Node, error) {
	return a.orchestrator.ListNodes(ctx)
}

package nodes

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type UpdateNodeRoleAction struct {
	orchestrator docker.Orchestrator
}

func NewUpdateNodeRoleAction(orchestrator docker.Orchestrator) *UpdateNodeRoleAction {
	return &UpdateNodeRoleAction{orchestrator: orchestrator}
}

func (a *UpdateNodeRoleAction) Execute(ctx context.Context, nodeID string, role string) error {
	return a.orchestrator.UpdateNodeRole(ctx, nodeID, role)
}

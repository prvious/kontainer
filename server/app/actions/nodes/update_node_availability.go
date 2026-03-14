package nodes

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type UpdateNodeAvailabilityAction struct {
	orchestrator docker.Orchestrator
}

func NewUpdateNodeAvailabilityAction(orchestrator docker.Orchestrator) *UpdateNodeAvailabilityAction {
	return &UpdateNodeAvailabilityAction{orchestrator: orchestrator}
}

func (a *UpdateNodeAvailabilityAction) Execute(ctx context.Context, nodeID string, availability string) error {
	return a.orchestrator.UpdateNodeAvailability(ctx, nodeID, availability)
}

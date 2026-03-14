package configs

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type InspectConfigAction struct {
	orchestrator docker.Orchestrator
}

func NewInspectConfigAction(orchestrator docker.Orchestrator) *InspectConfigAction {
	return &InspectConfigAction{orchestrator: orchestrator}
}

func (a *InspectConfigAction) Execute(ctx context.Context, configID string) (*docker.ConfigDetail, error) {
	return a.orchestrator.InspectConfig(ctx, configID)
}

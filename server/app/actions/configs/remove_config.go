package configs

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type RemoveConfigAction struct {
	orchestrator docker.Orchestrator
}

func NewRemoveConfigAction(orchestrator docker.Orchestrator) *RemoveConfigAction {
	return &RemoveConfigAction{orchestrator: orchestrator}
}

func (a *RemoveConfigAction) Execute(ctx context.Context, configID string) error {
	return a.orchestrator.RemoveConfig(ctx, configID)
}

package services

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type RestartServiceAction struct {
	orchestrator docker.Orchestrator
}

func NewRestartServiceAction(orchestrator docker.Orchestrator) *RestartServiceAction {
	return &RestartServiceAction{orchestrator: orchestrator}
}

func (a *RestartServiceAction) Execute(ctx context.Context, serviceID string) error {
	return a.orchestrator.RestartService(ctx, serviceID)
}

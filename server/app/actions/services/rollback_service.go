package services

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type RollbackServiceAction struct {
	orchestrator docker.Orchestrator
}

func NewRollbackServiceAction(orchestrator docker.Orchestrator) *RollbackServiceAction {
	return &RollbackServiceAction{orchestrator: orchestrator}
}

func (a *RollbackServiceAction) Execute(ctx context.Context, serviceID string) error {
	return a.orchestrator.RollbackService(ctx, serviceID)
}

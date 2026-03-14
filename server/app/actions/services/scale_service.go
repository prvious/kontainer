package services

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type ScaleServiceAction struct {
	orchestrator docker.Orchestrator
}

func NewScaleServiceAction(orchestrator docker.Orchestrator) *ScaleServiceAction {
	return &ScaleServiceAction{orchestrator: orchestrator}
}

func (a *ScaleServiceAction) Execute(ctx context.Context, serviceID string, replicas uint64) error {
	return a.orchestrator.ScaleService(ctx, serviceID, replicas)
}

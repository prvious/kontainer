package services

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type UpdateServiceImageAction struct {
	orchestrator docker.Orchestrator
}

func NewUpdateServiceImageAction(orchestrator docker.Orchestrator) *UpdateServiceImageAction {
	return &UpdateServiceImageAction{orchestrator: orchestrator}
}

func (a *UpdateServiceImageAction) Execute(ctx context.Context, serviceID string, image string) error {
	return a.orchestrator.UpdateServiceImage(ctx, serviceID, image)
}

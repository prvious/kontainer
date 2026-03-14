package volumes

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type ListVolumesAction struct {
	orchestrator docker.Orchestrator
}

func NewListVolumesAction(orchestrator docker.Orchestrator) *ListVolumesAction {
	return &ListVolumesAction{orchestrator: orchestrator}
}

func (a *ListVolumesAction) Execute(ctx context.Context) ([]docker.Volume, error) {
	return a.orchestrator.ListVolumes(ctx)
}

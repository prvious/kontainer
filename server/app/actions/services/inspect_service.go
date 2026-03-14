package services

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type InspectServiceAction struct {
	orchestrator docker.Orchestrator
}

func NewInspectServiceAction(orchestrator docker.Orchestrator) *InspectServiceAction {
	return &InspectServiceAction{orchestrator: orchestrator}
}

func (a *InspectServiceAction) Execute(ctx context.Context, serviceID string) (*docker.ServiceDetail, error) {
	return a.orchestrator.InspectService(ctx, serviceID)
}

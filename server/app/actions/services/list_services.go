package services

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type ListServicesAction struct {
	orchestrator docker.Orchestrator
}

func NewListServicesAction(orchestrator docker.Orchestrator) *ListServicesAction {
	return &ListServicesAction{orchestrator: orchestrator}
}

func (a *ListServicesAction) Execute(ctx context.Context, filters docker.ServiceFilters) ([]docker.Service, error) {
	return a.orchestrator.ListServices(ctx, filters)
}

package configs

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type ListConfigsAction struct {
	orchestrator docker.Orchestrator
}

func NewListConfigsAction(orchestrator docker.Orchestrator) *ListConfigsAction {
	return &ListConfigsAction{orchestrator: orchestrator}
}

func (a *ListConfigsAction) Execute(ctx context.Context) ([]docker.Config, error) {
	return a.orchestrator.ListConfigs(ctx)
}

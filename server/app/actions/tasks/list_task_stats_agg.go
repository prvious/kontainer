package tasks

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type ListTaskStatsAggAction struct {
	orchestrator docker.Orchestrator
}

func NewListTaskStatsAggAction(orchestrator docker.Orchestrator) *ListTaskStatsAggAction {
	return &ListTaskStatsAggAction{orchestrator: orchestrator}
}

func (a *ListTaskStatsAggAction) Execute(ctx context.Context) (*docker.TaskStatsAggregate, error) {
	return a.orchestrator.TaskStatsAgg(ctx)
}

package tasks

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type ListTaskStatsAction struct {
	orchestrator docker.Orchestrator
}

func NewListTaskStatsAction(orchestrator docker.Orchestrator) *ListTaskStatsAction {
	return &ListTaskStatsAction{orchestrator: orchestrator}
}

func (a *ListTaskStatsAction) Execute(ctx context.Context) ([]docker.ContainerStats, error) {
	return a.orchestrator.TaskStats(ctx)
}

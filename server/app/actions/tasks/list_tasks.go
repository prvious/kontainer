package tasks

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type ListTasksAction struct {
	orchestrator docker.Orchestrator
}

func NewListTasksAction(orchestrator docker.Orchestrator) *ListTasksAction {
	return &ListTasksAction{orchestrator: orchestrator}
}

func (a *ListTasksAction) Execute(ctx context.Context, filters docker.TaskFilters) ([]docker.Task, error) {
	return a.orchestrator.ListTasks(ctx, filters)
}

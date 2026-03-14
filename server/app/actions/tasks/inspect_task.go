package tasks

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type InspectTaskAction struct {
	orchestrator docker.Orchestrator
}

func NewInspectTaskAction(orchestrator docker.Orchestrator) *InspectTaskAction {
	return &InspectTaskAction{orchestrator: orchestrator}
}

func (a *InspectTaskAction) Execute(ctx context.Context, taskID string) (*docker.TaskDetail, error) {
	return a.orchestrator.InspectTask(ctx, taskID)
}

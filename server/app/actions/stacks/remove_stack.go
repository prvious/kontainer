package stacks

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type RemoveStackAction struct {
	orchestrator docker.Orchestrator
}

func NewRemoveStackAction(orchestrator docker.Orchestrator) *RemoveStackAction {
	return &RemoveStackAction{orchestrator: orchestrator}
}

func (a *RemoveStackAction) Execute(ctx context.Context, stackName string) error {
	return a.orchestrator.RemoveStack(ctx, stackName)
}

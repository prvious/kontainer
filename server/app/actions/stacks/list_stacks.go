package stacks

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type ListStacksAction struct {
	orchestrator docker.Orchestrator
}

func NewListStacksAction(orchestrator docker.Orchestrator) *ListStacksAction {
	return &ListStacksAction{orchestrator: orchestrator}
}

func (a *ListStacksAction) Execute(ctx context.Context) ([]docker.Stack, error) {
	return a.orchestrator.ListStacks(ctx)
}

package configs

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type CreateConfigAction struct {
	orchestrator docker.Orchestrator
}

func NewCreateConfigAction(orchestrator docker.Orchestrator) *CreateConfigAction {
	return &CreateConfigAction{orchestrator: orchestrator}
}

func (a *CreateConfigAction) Execute(ctx context.Context, name string, data []byte) error {
	return a.orchestrator.CreateConfig(ctx, name, data)
}

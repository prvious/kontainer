package secrets

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type CreateSecretAction struct {
	orchestrator docker.Orchestrator
}

func NewCreateSecretAction(orchestrator docker.Orchestrator) *CreateSecretAction {
	return &CreateSecretAction{orchestrator: orchestrator}
}

func (a *CreateSecretAction) Execute(ctx context.Context, name string, data []byte) error {
	return a.orchestrator.CreateSecret(ctx, name, data)
}

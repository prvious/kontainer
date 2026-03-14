package secrets

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type ListSecretsAction struct {
	orchestrator docker.Orchestrator
}

func NewListSecretsAction(orchestrator docker.Orchestrator) *ListSecretsAction {
	return &ListSecretsAction{orchestrator: orchestrator}
}

func (a *ListSecretsAction) Execute(ctx context.Context) ([]docker.Secret, error) {
	return a.orchestrator.ListSecrets(ctx)
}

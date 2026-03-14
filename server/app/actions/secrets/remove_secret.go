package secrets

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type RemoveSecretAction struct {
	orchestrator docker.Orchestrator
}

func NewRemoveSecretAction(orchestrator docker.Orchestrator) *RemoveSecretAction {
	return &RemoveSecretAction{orchestrator: orchestrator}
}

func (a *RemoveSecretAction) Execute(ctx context.Context, secretID string) error {
	return a.orchestrator.RemoveSecret(ctx, secretID)
}

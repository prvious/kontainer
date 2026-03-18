package services

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type UpdateServiceEnvAction struct {
	orchestrator docker.Orchestrator
}

func NewUpdateServiceEnvAction(orchestrator docker.Orchestrator) *UpdateServiceEnvAction {
	return &UpdateServiceEnvAction{orchestrator: orchestrator}
}

func (a *UpdateServiceEnvAction) Execute(ctx context.Context, serviceID string, env []docker.EnvVar) error {
	return a.orchestrator.UpdateServiceEnv(ctx, serviceID, env)
}

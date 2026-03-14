package networks

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type ListNetworksAction struct {
	orchestrator docker.Orchestrator
}

func NewListNetworksAction(orchestrator docker.Orchestrator) *ListNetworksAction {
	return &ListNetworksAction{orchestrator: orchestrator}
}

func (a *ListNetworksAction) Execute(ctx context.Context) ([]docker.Network, error) {
	return a.orchestrator.ListNetworks(ctx)
}

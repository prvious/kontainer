package swarm

import (
	"context"

	"github.com/docker/docker/api/types/network"

	"github.com/prvious/kontainer/server/app/services/docker"
)

func (s *SwarmOrchestrator) ListNetworks(ctx context.Context) ([]docker.Network, error) {
	nets, err := s.client.NetworkList(ctx, network.ListOptions{})
	if err != nil {
		return nil, err
	}

	items := make([]docker.Network, 0, len(nets))
	for _, n := range nets {
		items = append(items, docker.Network{
			ID:         n.ID,
			Name:       n.Name,
			Driver:     n.Driver,
			Scope:      n.Scope,
			Internal:   n.Internal,
			Attachable: n.Attachable,
			Ingress:    n.Ingress,
			Services:   []docker.NetworkServiceRef{},
			CreatedAt:  docker.FormatTime(n.Created),
		})
	}

	return items, nil
}

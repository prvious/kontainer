package swarm

import (
	"context"

	"github.com/docker/docker/api/types"
	dockerswarm "github.com/docker/docker/api/types/swarm"

	"github.com/prvious/kontainer/server/app/services/docker"
)

func (s *SwarmOrchestrator) ListNodes(ctx context.Context) ([]docker.Node, error) {
	nodes, err := s.client.NodeList(ctx, types.NodeListOptions{})
	if err != nil {
		return nil, err
	}

	items := make([]docker.Node, 0, len(nodes))
	for _, n := range nodes {
		ip := ""
		if n.ManagerStatus != nil {
			ip = n.ManagerStatus.Addr
		} else if n.Status.Addr != "" {
			ip = n.Status.Addr
		}

		leader := n.ManagerStatus != nil && n.ManagerStatus.Leader

		items = append(items, docker.Node{
			ID:            n.ID,
			Hostname:      n.Description.Hostname,
			Status:        string(n.Status.State),
			Availability:  string(n.Spec.Availability),
			Role:          string(n.Spec.Role),
			EngineVersion: n.Description.Engine.EngineVersion,
			OS:            n.Description.Platform.OS,
			Architecture:  n.Description.Platform.Architecture,
			IP:            ip,
			Leader:        leader,
			UpdatedAt:     docker.FormatTime(n.UpdatedAt),
		})
	}

	return items, nil
}

func (s *SwarmOrchestrator) UpdateNodeAvailability(ctx context.Context, id string, availability string) error {
	node, _, err := s.client.NodeInspectWithRaw(ctx, id)
	if err != nil {
		return err
	}

	node.Spec.Availability = dockerswarm.NodeAvailability(availability)
	return s.client.NodeUpdate(ctx, id, node.Version, node.Spec)
}

func (s *SwarmOrchestrator) UpdateNodeRole(ctx context.Context, id string, role string) error {
	node, _, err := s.client.NodeInspectWithRaw(ctx, id)
	if err != nil {
		return err
	}

	node.Spec.Role = dockerswarm.NodeRole(role)
	return s.client.NodeUpdate(ctx, id, node.Version, node.Spec)
}

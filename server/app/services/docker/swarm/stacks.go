package swarm

import (
	"context"

	"github.com/docker/docker/api/types"
	dfilters "github.com/docker/docker/api/types/filters"
	"github.com/docker/docker/api/types/network"
	dockerswarm "github.com/docker/docker/api/types/swarm"

	"github.com/prvious/kontainer/server/app/services/docker"
)

func (s *SwarmOrchestrator) ListStacks(ctx context.Context) ([]docker.Stack, error) {
	svcs, err := s.client.ServiceList(ctx, types.ServiceListOptions{})
	if err != nil {
		return nil, err
	}

	stackCounts := make(map[string]int)
	for _, svc := range svcs {
		ns := svc.Spec.Labels["com.docker.stack.namespace"]
		if ns != "" {
			stackCounts[ns]++
		}
	}

	items := make([]docker.Stack, 0, len(stackCounts))
	for name, count := range stackCounts {
		items = append(items, docker.Stack{
			Name:     name,
			Services: count,
		})
	}

	return items, nil
}

func (s *SwarmOrchestrator) RemoveStack(ctx context.Context, name string) error {
	f := dfilters.NewArgs()
	f.Add("label", "com.docker.stack.namespace="+name)

	// Remove services
	svcs, err := s.client.ServiceList(ctx, types.ServiceListOptions{Filters: f})
	if err != nil {
		return err
	}
	for _, svc := range svcs {
		if err := s.client.ServiceRemove(ctx, svc.ID); err != nil {
			return err
		}
	}

	// Remove networks
	nets, err := s.client.NetworkList(ctx, network.ListOptions{Filters: f})
	if err != nil {
		return err
	}
	for _, n := range nets {
		if err := s.client.NetworkRemove(ctx, n.ID); err != nil {
			return err
		}
	}

	// Remove secrets
	secrets, err := s.client.SecretList(ctx, dockerswarm.SecretListOptions{Filters: f})
	if err != nil {
		return err
	}
	for _, sec := range secrets {
		if err := s.client.SecretRemove(ctx, sec.ID); err != nil {
			return err
		}
	}

	// Remove configs
	configs, err := s.client.ConfigList(ctx, dockerswarm.ConfigListOptions{Filters: f})
	if err != nil {
		return err
	}
	for _, cfg := range configs {
		if err := s.client.ConfigRemove(ctx, cfg.ID); err != nil {
			return err
		}
	}

	return nil
}

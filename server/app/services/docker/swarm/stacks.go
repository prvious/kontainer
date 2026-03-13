package swarm

import (
	"context"

	"github.com/docker/docker/api/types"
	dfilters "github.com/docker/docker/api/types/filters"

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

	svcs, err := s.client.ServiceList(ctx, types.ServiceListOptions{Filters: f})
	if err != nil {
		return err
	}

	for _, svc := range svcs {
		if err := s.client.ServiceRemove(ctx, svc.ID); err != nil {
			return err
		}
	}

	return nil
}

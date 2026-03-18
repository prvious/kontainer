package swarm

import (
	"context"
	"fmt"

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

	type stackInfo struct {
		services        int
		desiredReplicas int
		updatedAt       string
	}
	stacks := make(map[string]*stackInfo)

	// Map service -> stack and count desired replicas
	svcToStack := make(map[string]string)
	for _, svc := range svcs {
		ns := svc.Spec.Labels["com.docker.stack.namespace"]
		if ns == "" {
			continue
		}
		svcToStack[svc.ID] = ns

		si, ok := stacks[ns]
		if !ok {
			si = &stackInfo{}
			stacks[ns] = si
		}
		si.services++

		if svc.Spec.Mode.Replicated != nil && svc.Spec.Mode.Replicated.Replicas != nil {
			si.desiredReplicas += int(*svc.Spec.Mode.Replicated.Replicas)
		}

		ts := docker.FormatTime(svc.UpdatedAt)
		if ts > si.updatedAt {
			si.updatedAt = ts
		}
	}

	// Count running tasks per stack
	taskFilter := dfilters.NewArgs()
	taskFilter.Add("desired-state", "running")
	tasks, taskErr := s.client.TaskList(ctx, types.TaskListOptions{Filters: taskFilter})
	if taskErr != nil {
		return nil, fmt.Errorf("failed to list tasks for stack replica count: %w", taskErr)
	}
	stackRunning := make(map[string]int)
	for _, t := range tasks {
		if t.Status.State == dockerswarm.TaskStateRunning {
			if ns := svcToStack[t.ServiceID]; ns != "" {
				stackRunning[ns]++
			}
		}
	}

	items := make([]docker.Stack, 0, len(stacks))
	for name, si := range stacks {
		items = append(items, docker.Stack{
			Name:            name,
			Services:        si.services,
			RunningReplicas: stackRunning[name],
			DesiredReplicas: si.desiredReplicas,
			UpdatedAt:       si.updatedAt,
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

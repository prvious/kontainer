package swarm

import (
	"context"
	"fmt"
	"strings"

	"github.com/docker/docker/api/types"
	dfilters "github.com/docker/docker/api/types/filters"
	dockerswarm "github.com/docker/docker/api/types/swarm"

	"github.com/prvious/kontainer/server/app/services/docker"
)

func (s *SwarmOrchestrator) ListServices(ctx context.Context, filters docker.ServiceFilters) ([]docker.Service, error) {
	opts := types.ServiceListOptions{}
	if filters.Stack != "" {
		f := dfilters.NewArgs()
		f.Add("label", "com.docker.stack.namespace="+filters.Stack)
		opts.Filters = f
	}

	svcs, err := s.client.ServiceList(ctx, opts)
	if err != nil {
		return nil, err
	}

	taskFilter := dfilters.NewArgs()
	taskFilter.Add("desired-state", "running")
	tasks, err := s.client.TaskList(ctx, types.TaskListOptions{Filters: taskFilter})
	if err != nil {
		return nil, err
	}

	runningCount := make(map[string]int)
	for _, t := range tasks {
		if t.Status.State == dockerswarm.TaskStateRunning {
			runningCount[t.ServiceID]++
		}
	}

	items := make([]docker.Service, 0, len(svcs))
	for _, svc := range svcs {
		img := svc.Spec.TaskTemplate.ContainerSpec.Image
		if idx := strings.Index(img, "@sha256:"); idx != -1 {
			img = img[:idx]
		}

		var desired int
		if svc.Spec.Mode.Replicated != nil && svc.Spec.Mode.Replicated.Replicas != nil {
			desired = int(*svc.Spec.Mode.Replicated.Replicas)
		} else if svc.Spec.Mode.Global != nil {
			desired = -1
		}

		items = append(items, docker.Service{
			ID:              svc.ID,
			Name:            svc.Spec.Name,
			Image:           img,
			Stack:           svc.Spec.Labels["com.docker.stack.namespace"],
			ReplicasRunning: runningCount[svc.ID],
			ReplicasDesired: desired,
			UpdatedAt:       docker.FormatTime(svc.UpdatedAt),
		})
	}

	return items, nil
}

func (s *SwarmOrchestrator) InspectService(ctx context.Context, id string) (*docker.ServiceDetail, error) {
	svc, _, err := s.client.ServiceInspectWithRaw(ctx, id, types.ServiceInspectOptions{})
	if err != nil {
		return nil, err
	}

	img := svc.Spec.TaskTemplate.ContainerSpec.Image
	if idx := strings.Index(img, "@sha256:"); idx != -1 {
		img = img[:idx]
	}

	var desired int
	if svc.Spec.Mode.Replicated != nil && svc.Spec.Mode.Replicated.Replicas != nil {
		desired = int(*svc.Spec.Mode.Replicated.Replicas)
	} else if svc.Spec.Mode.Global != nil {
		desired = -1
	}

	// Count running tasks
	taskFilter := dfilters.NewArgs()
	taskFilter.Add("service", id)
	taskFilter.Add("desired-state", "running")
	tasks, err := s.client.TaskList(ctx, types.TaskListOptions{Filters: taskFilter})
	if err != nil {
		return nil, fmt.Errorf("failed to list tasks for service %s: %w", id, err)
	}
	running := 0
	for _, t := range tasks {
		if t.Status.State == dockerswarm.TaskStateRunning {
			running++
		}
	}

	detail := &docker.ServiceDetail{
		Service: docker.Service{
			ID:              svc.ID,
			Name:            svc.Spec.Name,
			Image:           img,
			Stack:           svc.Spec.Labels["com.docker.stack.namespace"],
			ReplicasRunning: running,
			ReplicasDesired: desired,
			UpdatedAt:       docker.FormatTime(svc.UpdatedAt),
		},
		Env:    svc.Spec.TaskTemplate.ContainerSpec.Env,
		Labels: svc.Spec.Labels,
	}

	for _, p := range svc.Endpoint.Ports {
		detail.Ports = append(detail.Ports, docker.PortConfig{
			Protocol:      string(p.Protocol),
			TargetPort:    p.TargetPort,
			PublishedPort: p.PublishedPort,
			PublishMode:   string(p.PublishMode),
		})
	}

	for _, m := range svc.Spec.TaskTemplate.ContainerSpec.Mounts {
		detail.Mounts = append(detail.Mounts, docker.Mount{
			Type:   string(m.Type),
			Source: m.Source,
			Target: m.Target,
		})
	}

	if svc.Spec.TaskTemplate.Placement != nil {
		detail.Constraints = svc.Spec.TaskTemplate.Placement.Constraints
	}

	return detail, nil
}

func (s *SwarmOrchestrator) ScaleService(ctx context.Context, id string, replicas uint64) error {
	svc, _, err := s.client.ServiceInspectWithRaw(ctx, id, types.ServiceInspectOptions{})
	if err != nil {
		return err
	}

	if svc.Spec.Mode.Global != nil {
		return docker.ErrGlobalMode
	}
	if svc.Spec.Mode.Replicated == nil {
		return fmt.Errorf("service %s has no replicated mode configuration", id)
	}

	svc.Spec.Mode.Replicated.Replicas = &replicas
	_, err = s.client.ServiceUpdate(ctx, id, svc.Version, svc.Spec, dockerswarm.ServiceUpdateOptions{})
	return err
}

func (s *SwarmOrchestrator) RestartService(ctx context.Context, id string) error {
	svc, _, err := s.client.ServiceInspectWithRaw(ctx, id, types.ServiceInspectOptions{})
	if err != nil {
		return err
	}

	svc.Spec.TaskTemplate.ForceUpdate++
	_, err = s.client.ServiceUpdate(ctx, id, svc.Version, svc.Spec, dockerswarm.ServiceUpdateOptions{})
	return err
}

func (s *SwarmOrchestrator) RollbackService(ctx context.Context, id string) error {
	svc, _, err := s.client.ServiceInspectWithRaw(ctx, id, types.ServiceInspectOptions{})
	if err != nil {
		return err
	}

	_, err = s.client.ServiceUpdate(ctx, id, svc.Version, svc.Spec, dockerswarm.ServiceUpdateOptions{
		Rollback: "previous",
	})
	return err
}

func (s *SwarmOrchestrator) UpdateServiceEnv(ctx context.Context, id string, env []string) error {
	svc, _, err := s.client.ServiceInspectWithRaw(ctx, id, types.ServiceInspectOptions{})
	if err != nil {
		return err
	}

	svc.Spec.TaskTemplate.ContainerSpec.Env = env
	_, err = s.client.ServiceUpdate(ctx, id, svc.Version, svc.Spec, dockerswarm.ServiceUpdateOptions{})
	return err
}

func (s *SwarmOrchestrator) UpdateServiceImage(ctx context.Context, id string, image string) error {
	svc, _, err := s.client.ServiceInspectWithRaw(ctx, id, types.ServiceInspectOptions{})
	if err != nil {
		return err
	}

	svc.Spec.TaskTemplate.ContainerSpec.Image = image
	_, err = s.client.ServiceUpdate(ctx, id, svc.Version, svc.Spec, dockerswarm.ServiceUpdateOptions{})
	return err
}

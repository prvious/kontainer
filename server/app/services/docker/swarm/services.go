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

// parseEnvVars splits "KEY=VALUE" env strings into structured EnvVar slices.
func parseEnvVars(envs []string) []docker.EnvVar {
	vars := make([]docker.EnvVar, 0, len(envs))
	for _, e := range envs {
		key, value, _ := strings.Cut(e, "=")
		vars = append(vars, docker.EnvVar{Key: key, Value: value})
	}
	return vars
}

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

	mode := "replicated"
	var replicas *int
	if svc.Spec.Mode.Replicated != nil && svc.Spec.Mode.Replicated.Replicas != nil {
		r := int(*svc.Spec.Mode.Replicated.Replicas)
		replicas = &r
	} else if svc.Spec.Mode.Global != nil {
		mode = "global"
	}

	// Parse env vars into structured format
	envVars := make([]docker.EnvVar, 0, len(svc.Spec.TaskTemplate.ContainerSpec.Env))
	for _, e := range svc.Spec.TaskTemplate.ContainerSpec.Env {
		key, value, _ := strings.Cut(e, "=")
		envVars = append(envVars, docker.EnvVar{Key: key, Value: value})
	}

	labels := svc.Spec.Labels
	if labels == nil {
		labels = make(map[string]string)
	}

	detail := &docker.ServiceDetail{
		ID:        svc.ID,
		Name:      svc.Spec.Name,
		Image:     img,
		Stack:     svc.Spec.Labels["com.docker.stack.namespace"],
		Mode:      mode,
		Replicas:  replicas,
		CreatedAt: docker.FormatTime(svc.CreatedAt),
		UpdatedAt: docker.FormatTime(svc.UpdatedAt),
		Env:       envVars,
		Ports:     make([]docker.PortConfig, 0),
		Mounts:    make([]docker.Mount, 0),
		Labels:    labels,
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
			Type:     string(m.Type),
			Source:   m.Source,
			Target:   m.Target,
			ReadOnly: m.ReadOnly,
		})
	}

	if svc.Spec.TaskTemplate.Placement != nil {
		detail.Constraints = svc.Spec.TaskTemplate.Placement.Constraints
	}
	if detail.Constraints == nil {
		detail.Constraints = make([]string, 0)
	}

	// Populate update status if present
	if svc.UpdateStatus != nil && svc.UpdateStatus.State != "" {
		us := &docker.UpdateStatusInfo{
			State:   string(svc.UpdateStatus.State),
			Message: svc.UpdateStatus.Message,
		}
		if svc.UpdateStatus.StartedAt != nil {
			us.StartedAt = docker.FormatTime(*svc.UpdateStatus.StartedAt)
		}
		if svc.UpdateStatus.CompletedAt != nil {
			us.CompletedAt = docker.FormatTime(*svc.UpdateStatus.CompletedAt)
		}
		detail.UpdateStatus = us
	}

	// Populate previous image if available
	if svc.PreviousSpec != nil && svc.PreviousSpec.TaskTemplate.ContainerSpec != nil {
		prevImg := svc.PreviousSpec.TaskTemplate.ContainerSpec.Image
		if idx := strings.Index(prevImg, "@sha256:"); idx != -1 {
			prevImg = prevImg[:idx]
		}
		detail.PreviousImage = prevImg
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

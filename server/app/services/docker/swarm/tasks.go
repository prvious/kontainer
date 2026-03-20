package swarm

import (
	"context"
	"encoding/json"
	"strings"

	"github.com/docker/docker/api/types"
	dfilters "github.com/docker/docker/api/types/filters"
	dockerswarm "github.com/docker/docker/api/types/swarm"
	"github.com/goravel/framework/facades"

	"github.com/prvious/kontainer/server/app/services/docker"
)

func (s *SwarmOrchestrator) ListTasks(ctx context.Context, filters docker.TaskFilters) ([]docker.Task, error) {
	f := dfilters.NewArgs()
	if filters.ServiceID != "" {
		f.Add("service", filters.ServiceID)
	}
	if filters.NodeID != "" {
		f.Add("node", filters.NodeID)
	}

	tasks, err := s.client.TaskList(ctx, types.TaskListOptions{Filters: f})
	if err != nil {
		return nil, err
	}

	// Build service name lookup
	serviceNames := make(map[string]string)
	svcs, svcErr := s.client.ServiceList(ctx, types.ServiceListOptions{})
	if svcErr != nil {
		facades.Log().Warningf("ListTasks: failed to list services for name lookup: %v", svcErr)
	} else {
		for _, svc := range svcs {
			serviceNames[svc.ID] = svc.Spec.Name
		}
	}

	// Build node name lookup
	nodeNames := make(map[string]string)
	nodes, nodeErr := s.client.NodeList(ctx, types.NodeListOptions{})
	if nodeErr != nil {
		facades.Log().Warningf("ListTasks: failed to list nodes for name lookup: %v", nodeErr)
	} else {
		for _, n := range nodes {
			nodeNames[n.ID] = n.Description.Hostname
		}
	}

	items := make([]docker.Task, 0, len(tasks))
	for _, t := range tasks {
		img := ""
		if t.Spec.ContainerSpec != nil {
			img = t.Spec.ContainerSpec.Image
			if idx := strings.Index(img, "@sha256:"); idx != -1 {
				img = img[:idx]
			}
		}

		containerID := ""
		var exitCode *int
		if t.Status.ContainerStatus != nil {
			containerID = t.Status.ContainerStatus.ContainerID
			if t.Status.State == dockerswarm.TaskStateComplete ||
				t.Status.State == dockerswarm.TaskStateFailed ||
				t.Status.State == dockerswarm.TaskStateShutdown ||
				t.Status.State == dockerswarm.TaskStateRejected {
				ec := t.Status.ContainerStatus.ExitCode
				exitCode = &ec
			}
		}

		svcName := serviceNames[t.ServiceID]
		if svcName == "" {
			svcName = t.ServiceID
		}
		nodeName := nodeNames[t.NodeID]
		if nodeName == "" {
			nodeName = t.NodeID
		}

		items = append(items, docker.Task{
			ID:           t.ID,
			ServiceID:    t.ServiceID,
			ServiceName:  svcName,
			NodeID:       t.NodeID,
			NodeName:     nodeName,
			State:        string(t.Status.State),
			DesiredState: string(t.DesiredState),
			Timestamp:    docker.FormatTime(t.Status.Timestamp),
			ContainerID:  containerID,
			Image:        img,
			Slot:         t.Slot,
			ExitCode:     exitCode,
			Error:        t.Status.Err,
		})
	}

	return items, nil
}

func (s *SwarmOrchestrator) InspectTask(ctx context.Context, id string) (*docker.TaskDetail, error) {
	t, _, err := s.client.TaskInspectWithRaw(ctx, id)
	if err != nil {
		return nil, err
	}

	img := ""
	var envVars []docker.EnvVar
	var mounts []docker.Mount
	if t.Spec.ContainerSpec != nil {
		img = t.Spec.ContainerSpec.Image
		if idx := strings.Index(img, "@sha256:"); idx != -1 {
			img = img[:idx]
		}
		envVars = parseEnvVars(t.Spec.ContainerSpec.Env)
		for _, m := range t.Spec.ContainerSpec.Mounts {
			mounts = append(mounts, docker.Mount{
				Type:     string(m.Type),
				Source:   m.Source,
				Target:   m.Target,
				ReadOnly: m.ReadOnly,
			})
		}
	}
	if envVars == nil {
		envVars = make([]docker.EnvVar, 0)
	}
	if mounts == nil {
		mounts = make([]docker.Mount, 0)
	}

	containerID := ""
	pid := 0
	var exitCode *int
	if t.Status.ContainerStatus != nil {
		containerID = t.Status.ContainerStatus.ContainerID
		pid = int(t.Status.ContainerStatus.PID)
		if t.Status.State == dockerswarm.TaskStateComplete ||
			t.Status.State == dockerswarm.TaskStateFailed ||
			t.Status.State == dockerswarm.TaskStateShutdown ||
			t.Status.State == dockerswarm.TaskStateRejected {
			ec := t.Status.ContainerStatus.ExitCode
			exitCode = &ec
		}
	}

	// Build networks info
	networks := make([]docker.TaskNetworkInfo, 0, len(t.NetworksAttachments))
	for _, na := range t.NetworksAttachments {
		name := ""
		if na.Network.Spec.Name != "" {
			name = na.Network.Spec.Name
		}
		addrs := na.Addresses
		if addrs == nil {
			addrs = make([]string, 0)
		}
		networks = append(networks, docker.TaskNetworkInfo{
			Name:      name,
			Addresses: addrs,
		})
	}

	detail := &docker.TaskDetail{
		Task: docker.Task{
			ID:           t.ID,
			ServiceID:    t.ServiceID,
			NodeID:       t.NodeID,
			State:        string(t.Status.State),
			DesiredState: string(t.DesiredState),
			Timestamp:    docker.FormatTime(t.Status.Timestamp),
			ContainerID:  containerID,
			Image:        img,
			Slot:         t.Slot,
			ExitCode:     exitCode,
			Error:        t.Status.Err,
		},
		Pid:       pid,
		CreatedAt: docker.FormatTime(t.CreatedAt),
		UpdatedAt: docker.FormatTime(t.UpdatedAt),
		Env:       envVars,
		Mounts:    mounts,
		Networks:  networks,
	}

	// Resolve service name (fall back to ID if resolution fails)
	svc, _, svcErr := s.client.ServiceInspectWithRaw(ctx, t.ServiceID, types.ServiceInspectOptions{})
	if svcErr != nil {
		facades.Log().Warningf("InspectTask: failed to resolve service name for %s: %v", t.ServiceID, svcErr)
		detail.ServiceName = t.ServiceID
	} else {
		detail.ServiceName = svc.Spec.Name
	}

	// Resolve node name (fall back to ID if resolution fails)
	node, _, nodeErr := s.client.NodeInspectWithRaw(ctx, t.NodeID)
	if nodeErr != nil {
		facades.Log().Warningf("InspectTask: failed to resolve node name for %s: %v", t.NodeID, nodeErr)
		detail.NodeName = t.NodeID
	} else {
		detail.NodeName = node.Description.Hostname
	}

	return detail, nil
}

func (s *SwarmOrchestrator) TaskStats(ctx context.Context) ([]docker.ContainerStats, error) {
	taskFilter := dfilters.NewArgs()
	taskFilter.Add("desired-state", "running")
	tasks, err := s.client.TaskList(ctx, types.TaskListOptions{Filters: taskFilter})
	if err != nil {
		return nil, err
	}

	var stats []docker.ContainerStats
	for _, t := range tasks {
		if t.Status.ContainerStatus == nil || t.Status.State != dockerswarm.TaskStateRunning {
			continue
		}
		if t.NodeID != s.nodeID {
			continue
		}

		containerID := t.Status.ContainerStatus.ContainerID
		statsResp, err := s.client.ContainerStats(ctx, containerID, false)
		if err != nil {
			facades.Log().Warningf("TaskStats: failed to get stats for container %s: %v", containerID, err)
			continue
		}
		resp := statsResp.Body

		var v struct {
			CPUStats struct {
				CPUUsage struct {
					TotalUsage uint64 `json:"total_usage"`
				} `json:"cpu_usage"`
				SystemCPUUsage uint64 `json:"system_cpu_usage"`
				OnlineCPUs     int    `json:"online_cpus"`
			} `json:"cpu_stats"`
			PreCPUStats struct {
				CPUUsage struct {
					TotalUsage uint64 `json:"total_usage"`
				} `json:"cpu_usage"`
				SystemCPUUsage uint64 `json:"system_cpu_usage"`
			} `json:"precpu_stats"`
			MemoryStats struct {
				Usage uint64 `json:"usage"`
				Limit uint64 `json:"limit"`
			} `json:"memory_stats"`
			Name string `json:"name"`
		}

		if err := json.NewDecoder(resp).Decode(&v); err != nil {
			facades.Log().Warningf("TaskStats: failed to decode stats for container %s: %v", containerID, err)
			resp.Close()
			continue
		}
		resp.Close()

		cpuDelta := float64(v.CPUStats.CPUUsage.TotalUsage - v.PreCPUStats.CPUUsage.TotalUsage)
		systemDelta := float64(v.CPUStats.SystemCPUUsage - v.PreCPUStats.SystemCPUUsage)
		cpuPercent := 0.0
		if systemDelta > 0 && cpuDelta > 0 {
			cpuPercent = (cpuDelta / systemDelta) * float64(v.CPUStats.OnlineCPUs) * 100.0
		}

		memPercent := 0.0
		if v.MemoryStats.Limit > 0 {
			memPercent = float64(v.MemoryStats.Usage) / float64(v.MemoryStats.Limit) * 100.0
		}

		name := strings.TrimPrefix(v.Name, "/")

		stats = append(stats, docker.ContainerStats{
			ContainerID: containerID,
			Name:        name,
			ServiceID:   t.ServiceID,
			NodeID:      t.NodeID,
			CPUPercent:  cpuPercent,
			MemUsage:    int64(v.MemoryStats.Usage),
			MemLimit:    int64(v.MemoryStats.Limit),
			MemPercent:  memPercent,
		})
	}

	return stats, nil
}

func (s *SwarmOrchestrator) TaskStatsAgg(ctx context.Context) (*docker.TaskStatsAggregate, error) {
	tasks, err := s.client.TaskList(ctx, types.TaskListOptions{})
	if err != nil {
		return nil, err
	}
	agg := &docker.TaskStatsAggregate{}
	for _, t := range tasks {
		agg.Total++
		switch t.Status.State {
		case dockerswarm.TaskStateRunning:
			agg.Running++
		case dockerswarm.TaskStateFailed:
			agg.Failed++
		case dockerswarm.TaskStatePreparing, dockerswarm.TaskStateAssigned, dockerswarm.TaskStateAccepted, dockerswarm.TaskStatePending, dockerswarm.TaskStateReady, dockerswarm.TaskStateStarting:
			agg.Preparing++
		case dockerswarm.TaskStateComplete:
			agg.Complete++
		case dockerswarm.TaskStateShutdown, dockerswarm.TaskStateRejected, dockerswarm.TaskStateOrphaned, dockerswarm.TaskStateRemove:
			agg.Shutdown++
		}
		if t.DesiredState == dockerswarm.TaskStateRunning {
			agg.DesiredRunning++
		}
	}
	return agg, nil
}

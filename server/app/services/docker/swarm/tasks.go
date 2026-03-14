package swarm

import (
	"context"
	"encoding/json"
	"log"
	"strings"

	"github.com/docker/docker/api/types"
	dfilters "github.com/docker/docker/api/types/filters"
	dockerswarm "github.com/docker/docker/api/types/swarm"

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
		if t.Status.ContainerStatus != nil {
			containerID = t.Status.ContainerStatus.ContainerID
		}

		items = append(items, docker.Task{
			ID:           t.ID,
			ServiceID:    t.ServiceID,
			NodeID:       t.NodeID,
			Status:       string(t.Status.State),
			DesiredState: string(t.DesiredState),
			Timestamp:    docker.FormatTime(t.Status.Timestamp),
			ContainerID:  containerID,
			Image:        img,
			Slot:         t.Slot,
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
	if t.Spec.ContainerSpec != nil {
		img = t.Spec.ContainerSpec.Image
		if idx := strings.Index(img, "@sha256:"); idx != -1 {
			img = img[:idx]
		}
	}

	containerID := ""
	if t.Status.ContainerStatus != nil {
		containerID = t.Status.ContainerStatus.ContainerID
	}

	detail := &docker.TaskDetail{
		Task: docker.Task{
			ID:           t.ID,
			ServiceID:    t.ServiceID,
			NodeID:       t.NodeID,
			Status:       string(t.Status.State),
			DesiredState: string(t.DesiredState),
			Timestamp:    docker.FormatTime(t.Status.Timestamp),
			ContainerID:  containerID,
			Image:        img,
			Slot:         t.Slot,
			Error:        t.Status.Err,
		},
	}

	// Resolve service name
	svc, _, svcErr := s.client.ServiceInspectWithRaw(ctx, t.ServiceID, types.ServiceInspectOptions{})
	if svcErr == nil {
		detail.ServiceName = svc.Spec.Name
	}

	// Resolve node name
	node, _, nodeErr := s.client.NodeInspectWithRaw(ctx, t.NodeID)
	if nodeErr == nil {
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
			log.Printf("TaskStats: failed to get stats for container %s: %v", containerID, err)
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
			log.Printf("TaskStats: failed to decode stats for container %s: %v", containerID, err)
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

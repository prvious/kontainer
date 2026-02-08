package metrics

import (
	"context"
	"encoding/json"
	"strings"

	"github.com/docker/docker/api/types/container"
)

// MetricsDockerClient defines the Docker API subset needed for metrics collection.
type MetricsDockerClient interface {
	ContainerList(ctx context.Context, options container.ListOptions) ([]container.Summary, error)
	ContainerStatsOneShot(ctx context.Context, containerID string) (container.StatsResponseReader, error)
}

type CollectContainerMetricsAction struct {
	client MetricsDockerClient
}

func NewCollectContainerMetricsAction(client MetricsDockerClient) *CollectContainerMetricsAction {
	return &CollectContainerMetricsAction{client: client}
}

func (a *CollectContainerMetricsAction) Execute(ctx context.Context) ([]ContainerMetrics, error) {
	containers, err := a.client.ContainerList(ctx, container.ListOptions{})
	if err != nil {
		return nil, err
	}

	result := make([]ContainerMetrics, 0, len(containers))
	for _, c := range containers {
		stats, err := a.client.ContainerStatsOneShot(ctx, c.ID)
		if err != nil {
			continue
		}

		var statsJSON container.StatsResponse
		if err := json.NewDecoder(stats.Body).Decode(&statsJSON); err != nil {
			stats.Body.Close()
			continue
		}
		stats.Body.Close()

		cpuPct := calculateCPUPercent(&statsJSON)
		memUsed := statsJSON.MemoryStats.Usage
		if v, ok := statsJSON.MemoryStats.Stats["total_inactive_file"]; ok && v < memUsed {
			memUsed -= v
		} else if v, ok := statsJSON.MemoryStats.Stats["inactive_file"]; ok && v < memUsed {
			memUsed -= v
		}
		memLimit := statsJSON.MemoryStats.Limit
		memPct := 0.0
		if memLimit > 0 {
			memPct = float64(memUsed) / float64(memLimit) * 100.0
		}

		name := ""
		if len(c.Names) > 0 {
			name = strings.TrimPrefix(c.Names[0], "/")
		}

		serviceName := c.Labels["com.docker.swarm.service.name"]

		result = append(result, ContainerMetrics{
			ContainerID:   c.ID[:12],
			Name:          name,
			ServiceName:   serviceName,
			CPUPercent:    cpuPct,
			MemoryUsed:    memUsed,
			MemoryLimit:   memLimit,
			MemoryPercent: memPct,
		})
	}

	return result, nil
}

func calculateCPUPercent(stats *container.StatsResponse) float64 {
	cpuDelta := float64(stats.CPUStats.CPUUsage.TotalUsage - stats.PreCPUStats.CPUUsage.TotalUsage)
	systemDelta := float64(stats.CPUStats.SystemUsage - stats.PreCPUStats.SystemUsage)
	if systemDelta > 0 && cpuDelta > 0 {
		return (cpuDelta / systemDelta) * float64(stats.CPUStats.OnlineCPUs) * 100.0
	}
	return 0.0
}

package metrics

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type ContainerMetrics struct {
	ContainerID   string  `json:"containerId"`
	Name          string  `json:"name"`
	ServiceID     string  `json:"serviceId"`
	CPUPercent    float64 `json:"cpuPercent"`
	MemoryUsed    int64   `json:"memoryUsed"`
	MemoryLimit   int64   `json:"memoryLimit"`
	MemoryPercent float64 `json:"memoryPercent"`
}

type CollectContainerMetricsAction struct {
	orchestrator docker.Orchestrator
}

func NewCollectContainerMetricsAction(orchestrator docker.Orchestrator) *CollectContainerMetricsAction {
	return &CollectContainerMetricsAction{orchestrator: orchestrator}
}

func (a *CollectContainerMetricsAction) Execute(ctx context.Context) ([]ContainerMetrics, error) {
	stats, err := a.orchestrator.TaskStats(ctx)
	if err != nil {
		return nil, err
	}

	result := make([]ContainerMetrics, 0, len(stats))
	for _, s := range stats {
		result = append(result, ContainerMetrics{
			ContainerID:   s.ContainerID,
			Name:          s.Name,
			ServiceID:     s.ServiceID,
			CPUPercent:    s.CPUPercent,
			MemoryUsed:    s.MemUsage,
			MemoryLimit:   s.MemLimit,
			MemoryPercent: s.MemPercent,
		})
	}
	return result, nil
}

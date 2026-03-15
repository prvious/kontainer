package metrics

import (
	"context"
	"time"

	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/app/models"
	"github.com/prvious/kontainer/server/app/services/cluster"
)

type GetClusterMetricsAction struct {
	collector *CollectAndStoreAction
	registry  *cluster.Registry
}

func NewGetClusterMetricsAction(
	collector *CollectAndStoreAction,
	registry *cluster.Registry,
) *GetClusterMetricsAction {
	return &GetClusterMetricsAction{collector: collector, registry: registry}
}

func (a *GetClusterMetricsAction) Execute(ctx context.Context, rangeDuration time.Duration) (ClusterMetricsResponse, error) {
	// Collect real-time local snapshot
	localSnapshot, _ := a.collector.CollectLocal(ctx)
	agents := []AgentMetricsSnapshot{localSnapshot}

	// Fan out to followers for real-time snapshots
	for _, follower := range a.registry.All() {
		snapshot, err := a.collector.FetchFollowerMetrics(ctx, follower.Address)
		if err != nil {
			facades.Log().Errorf("cluster metrics: follower %s error: %v", follower.Address, err)
			continue
		}
		agents = append(agents, snapshot)
	}

	// Query history from DB
	since := time.Now().Add(-rangeDuration).UTC()
	var metrics []models.NodeMetric
	if err := facades.Orm().Query().Where("timestamp >= ?", since).OrderBy("timestamp", "asc").Get(&metrics); err != nil {
		facades.Log().Errorf("cluster metrics: history query error: %v", err)
	}

	history := make([]AgentMetricsSnapshot, 0, len(metrics))
	for _, m := range metrics {
		history = append(history, AgentMetricsSnapshot{
			Timestamp: m.Timestamp, NodeID: m.NodeID, Hostname: m.Hostname,
			Node: NodeMetrics{
				CPUs: m.CPUs, CPUPercent: m.CPUPercent,
				MemoryTotal: m.MemTotal, MemoryUsed: m.MemUsed, MemoryPercent: m.MemPercent,
				DiskTotal: m.DiskTotal, DiskUsed: m.DiskUsed, DiskPercent: m.DiskPercent,
			},
			Containers: []ContainerMetrics{},
		})
	}

	return ClusterMetricsResponse{Agents: agents, History: history}, nil
}

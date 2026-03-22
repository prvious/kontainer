package metrics

import (
	"context"
	"fmt"
	"time"

	"github.com/goravel/framework/facades"

	"github.com/prvious/kontainer/server/app/models"
)

type GetClusterMetricsAction struct{}

func NewGetClusterMetricsAction() *GetClusterMetricsAction {
	return &GetClusterMetricsAction{}
}

func (a *GetClusterMetricsAction) Execute(_ context.Context, rangeDuration time.Duration) (ClusterMetricsResponse, error) {
	agents, err := a.latestSnapshots()
	if err != nil {
		return ClusterMetricsResponse{}, fmt.Errorf("query latest metrics: %w", err)
	}

	since := time.Now().Add(-rangeDuration).UTC()
	history, err := a.nodeHistory(since)
	if err != nil {
		return ClusterMetricsResponse{}, fmt.Errorf("query metrics history: %w", err)
	}

	return ClusterMetricsResponse{Agents: agents, History: history}, nil
}

// latestSnapshots returns the most recent snapshot per node, including container metrics.
// It queries node rows first, then fetches containers for the matched timestamps.
func (a *GetClusterMetricsAction) latestSnapshots() ([]AgentMetricsSnapshot, error) {
	cutoff := time.Now().Add(-2 * time.Minute).UTC()

	// Step 1: Get latest node row per node_id.
	var nodeRows []models.Metric
	if err := facades.Orm().Query().
		Where("kind = ? AND timestamp >= ?", models.MetricKindNode, cutoff).
		OrderBy("timestamp", "desc").
		Limit(100).
		Get(&nodeRows); err != nil {
		return nil, fmt.Errorf("query latest node metrics: %w", err)
	}

	// Deduplicate: keep the most recent per node_id.
	type nodeEntry struct {
		snapshot AgentMetricsSnapshot
		ts       time.Time
	}
	nodes := map[string]*nodeEntry{}
	for _, r := range nodeRows {
		if _, ok := nodes[r.NodeID]; !ok {
			nodes[r.NodeID] = &nodeEntry{snapshot: nodeSnapshotFromRow(r), ts: r.Timestamp}
		}
	}

	// Step 2: Fetch containers for those nodes' latest timestamps.
	for nodeID, entry := range nodes {
		var containerRows []models.Metric
		if err := facades.Orm().Query().
			Where("kind = ? AND node_id = ? AND timestamp = ?", models.MetricKindContainer, nodeID, entry.ts).
			Limit(500).
			Get(&containerRows); err != nil {
			return nil, fmt.Errorf("query containers for node %s: %w", nodeID, err)
		}
		for _, c := range containerRows {
			entry.snapshot.Containers = append(entry.snapshot.Containers, containerMetricsFromRow(c))
		}
	}

	agents := make([]AgentMetricsSnapshot, 0, len(nodes))
	for _, entry := range nodes {
		agents = append(agents, entry.snapshot)
	}
	return agents, nil
}

// nodeHistory returns node-kind metrics within the given time range for charting.
func (a *GetClusterMetricsAction) nodeHistory(since time.Time) ([]AgentMetricsSnapshot, error) {
	var rows []models.Metric
	if err := facades.Orm().Query().
		Where("kind = ? AND timestamp >= ?", models.MetricKindNode, since).
		OrderBy("timestamp", "asc").
		Limit(5000).
		Get(&rows); err != nil {
		return nil, fmt.Errorf("query node history: %w", err)
	}

	history := make([]AgentMetricsSnapshot, 0, len(rows))
	for _, r := range rows {
		snap := nodeSnapshotFromRow(r)
		snap.Containers = []ContainerMetrics{}
		history = append(history, snap)
	}
	return history, nil
}

func nodeSnapshotFromRow(r models.Metric) AgentMetricsSnapshot {
	return AgentMetricsSnapshot{
		Timestamp: r.Timestamp,
		NodeID:    r.NodeID,
		Hostname:  r.SubjectName,
		Node: NodeMetrics{
			CPUs: r.CPUs, CPUPercent: r.CPUPercent,
			MemoryTotal: uint64(r.MemTotal), MemoryUsed: uint64(r.MemUsed), MemoryPercent: r.MemPercent,
			DiskTotal: uint64(r.DiskTotal), DiskUsed: uint64(r.DiskUsed), DiskPercent: r.DiskPercent,
		},
		Containers: []ContainerMetrics{},
	}
}

func containerMetricsFromRow(r models.Metric) ContainerMetrics {
	return ContainerMetrics{
		ContainerID:   r.SubjectID,
		Name:          r.SubjectName,
		ServiceID:     r.ServiceID,
		CPUPercent:    r.CPUPercent,
		MemoryUsed:    r.MemUsed,
		MemoryLimit:   r.MemTotal,
		MemoryPercent: r.MemPercent,
	}
}

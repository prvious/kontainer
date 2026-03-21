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
func (a *GetClusterMetricsAction) latestSnapshots() ([]AgentMetricsSnapshot, error) {
	cutoff := time.Now().Add(-2 * time.Minute).UTC()
	var rows []models.Metric
	if err := facades.Orm().Query().
		Where("timestamp >= ?", cutoff).
		OrderBy("timestamp", "desc").
		Get(&rows); err != nil {
		return nil, err
	}

	type nodeEntry struct {
		snapshot AgentMetricsSnapshot
		ts       time.Time
	}
	nodes := map[string]*nodeEntry{}
	var containerRows []models.Metric

	for _, r := range rows {
		if r.Kind == "node" {
			if existing, ok := nodes[r.NodeID]; ok {
				if r.Timestamp.After(existing.ts) {
					nodes[r.NodeID] = &nodeEntry{snapshot: nodeSnapshotFromRow(r), ts: r.Timestamp}
				}
			} else {
				nodes[r.NodeID] = &nodeEntry{snapshot: nodeSnapshotFromRow(r), ts: r.Timestamp}
			}
		} else {
			containerRows = append(containerRows, r)
		}
	}

	for _, c := range containerRows {
		entry, ok := nodes[c.NodeID]
		if !ok {
			continue
		}
		if c.Timestamp.Equal(entry.ts) {
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
		Where("kind = ? AND timestamp >= ?", "node", since).
		OrderBy("timestamp", "asc").
		Get(&rows); err != nil {
		return nil, err
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

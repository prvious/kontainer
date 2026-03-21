package metrics

import (
	"context"
	"fmt"
	"time"

	"github.com/goravel/framework/facades"

	"github.com/prvious/kontainer/server/app/models"
)

type GetNodeMetricsAction struct{}

func NewGetNodeMetricsAction() *GetNodeMetricsAction {
	return &GetNodeMetricsAction{}
}

func (a *GetNodeMetricsAction) Execute(_ context.Context, nodeID string) (AgentMetricsSnapshot, error) {
	cutoff := time.Now().Add(-2 * time.Minute).UTC()

	var nodeRow models.Metric
	if err := facades.Orm().Query().
		Where("kind = ? AND node_id = ? AND timestamp >= ?", models.MetricKindNode, nodeID, cutoff).
		OrderBy("timestamp", "desc").
		First(&nodeRow); err != nil {
		return AgentMetricsSnapshot{}, fmt.Errorf("query node metrics for %s: %w", nodeID, err)
	}
	if nodeRow.ID == 0 {
		return AgentMetricsSnapshot{}, ErrNoRecentMetrics
	}

	snapshot := nodeSnapshotFromRow(nodeRow)

	var containerRows []models.Metric
	if err := facades.Orm().Query().
		Where("kind = ? AND node_id = ? AND timestamp = ?", models.MetricKindContainer, nodeID, nodeRow.Timestamp).
		Get(&containerRows); err != nil {
		return AgentMetricsSnapshot{}, fmt.Errorf("query container metrics for node %s: %w", nodeID, err)
	}

	for _, c := range containerRows {
		snapshot.Containers = append(snapshot.Containers, containerMetricsFromRow(c))
	}

	return snapshot, nil
}

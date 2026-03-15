package metrics

import (
	"context"
	"fmt"

	"github.com/prvious/kontainer/server/app/services/cluster"
)

type GetNodeMetricsAction struct {
	collector    *CollectAndStoreAction
	registry     *cluster.Registry
	leaderNodeID string
}

func NewGetNodeMetricsAction(
	collector *CollectAndStoreAction,
	registry *cluster.Registry,
	leaderNodeID string,
) *GetNodeMetricsAction {
	return &GetNodeMetricsAction{collector: collector, registry: registry, leaderNodeID: leaderNodeID}
}

func (a *GetNodeMetricsAction) Execute(ctx context.Context, nodeID string) (AgentMetricsSnapshot, error) {
	if nodeID == "" || nodeID == a.leaderNodeID {
		return a.collector.CollectLocal(ctx)
	}

	addr, ok := a.registry.ByNodeID(nodeID)
	if !ok {
		return AgentMetricsSnapshot{}, fmt.Errorf("node %s not found in registry", nodeID)
	}

	return a.collector.FetchFollowerMetrics(ctx, addr)
}

package metrics

import (
	"context"
	"time"

	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/app/models"
	"github.com/prvious/kontainer/server/app/services/cluster"
	pb "github.com/prvious/kontainer/server/proto/agent"
)

type CollectAndStoreAction struct {
	nodeAction      *CollectNodeMetricsAction
	containerAction *CollectContainerMetricsAction
	registry        *cluster.Registry
	pool            *cluster.GrpcPool
	nodeID          string
	hostname        string
}

func NewCollectAndStoreAction(
	nodeAction *CollectNodeMetricsAction,
	containerAction *CollectContainerMetricsAction,
	registry *cluster.Registry,
	pool *cluster.GrpcPool,
	nodeID, hostname string,
) *CollectAndStoreAction {
	return &CollectAndStoreAction{
		nodeAction: nodeAction, containerAction: containerAction,
		registry: registry, pool: pool,
		nodeID: nodeID, hostname: hostname,
	}
}

func (a *CollectAndStoreAction) Execute(ctx context.Context) {
	localSnapshot, err := a.collectLocal(ctx)
	if err != nil {
		facades.Log().Errorf("metrics collector: local collect error: %v", err)
	} else {
		if err := a.persistNodeMetrics(localSnapshot); err != nil {
			facades.Log().Errorf("metrics collector: local insert error: %v", err)
		}
	}

	for _, follower := range a.registry.All() {
		if follower.NodeID == a.nodeID {
			continue
		}
		snapshot, err := a.FetchFollowerMetrics(ctx, follower.Address)
		if err != nil {
			facades.Log().Errorf("metrics collector: follower %s (%s) error: %v", follower.NodeID, follower.Address, err)
			continue
		}
		if err := a.persistNodeMetrics(snapshot); err != nil {
			facades.Log().Errorf("metrics collector: follower insert error: %v", err)
		}
	}
}

// NodeID returns the leader's node ID so callers can skip self in fan-out loops.
func (a *CollectAndStoreAction) NodeID() string {
	return a.nodeID
}

// CollectLocal gathers local metrics. Exported for reuse.
func (a *CollectAndStoreAction) CollectLocal(ctx context.Context) (AgentMetricsSnapshot, error) {
	return a.collectLocal(ctx)
}

func (a *CollectAndStoreAction) collectLocal(ctx context.Context) (AgentMetricsSnapshot, error) {
	nodeMetrics, err := a.nodeAction.Execute()
	if err != nil {
		return AgentMetricsSnapshot{}, err
	}
	containerMetrics, err := a.containerAction.Execute(ctx)
	if err != nil {
		return AgentMetricsSnapshot{}, err
	}
	return AgentMetricsSnapshot{
		Timestamp: time.Now().UTC(), NodeID: a.nodeID, Hostname: a.hostname,
		Node: nodeMetrics, Containers: containerMetrics,
	}, nil
}

// FetchFollowerMetrics fetches metrics from a follower via gRPC.
func (a *CollectAndStoreAction) FetchFollowerMetrics(ctx context.Context, address string) (AgentMetricsSnapshot, error) {
	ctx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()

	client, err := a.pool.GetClient(address)
	if err != nil {
		return AgentMetricsSnapshot{}, err
	}

	resp, err := client.Metrics(ctx, &pb.MetricsRequest{})
	if err != nil {
		return AgentMetricsSnapshot{}, err
	}

	ts, err := time.Parse(time.RFC3339, resp.Timestamp)
	if err != nil {
		facades.Log().Warningf("metrics: follower %s sent unparseable timestamp %q, using current time: %v", address, resp.Timestamp, err)
		ts = time.Now().UTC()
	}

	snapshot := AgentMetricsSnapshot{
		Timestamp: ts, NodeID: resp.NodeId, Hostname: resp.Hostname,
	}

	if resp.Node != nil {
		snapshot.Node = NodeMetrics{
			CPUs: int(resp.Node.Cpus), CPUPercent: resp.Node.CpuPercent,
			MemoryTotal: uint64(resp.Node.MemoryTotal), MemoryUsed: uint64(resp.Node.MemoryUsed),
			MemoryPercent: resp.Node.MemoryPercent,
			DiskTotal: uint64(resp.Node.DiskTotal), DiskUsed: uint64(resp.Node.DiskUsed),
			DiskPercent: resp.Node.DiskPercent,
		}
	}

	for _, c := range resp.Containers {
		snapshot.Containers = append(snapshot.Containers, ContainerMetrics{
			ContainerID: c.ContainerId, Name: c.Name, ServiceID: c.ServiceId,
			CPUPercent: c.CpuPercent,
			MemoryUsed: c.MemoryUsage, MemoryLimit: c.MemoryLimit,
			MemoryPercent: c.MemoryPercent,
		})
	}

	return snapshot, nil
}

func (a *CollectAndStoreAction) persistNodeMetrics(s AgentMetricsSnapshot) error {
	metric := &models.NodeMetric{
		Timestamp: s.Timestamp, NodeID: s.NodeID, Hostname: s.Hostname,
		CPUs: s.Node.CPUs, CPUPercent: s.Node.CPUPercent,
		MemUsed: s.Node.MemoryUsed, MemTotal: s.Node.MemoryTotal, MemPercent: s.Node.MemoryPercent,
		DiskUsed: s.Node.DiskUsed, DiskTotal: s.Node.DiskTotal, DiskPercent: s.Node.DiskPercent,
	}
	return facades.Orm().Query().Create(metric)
}

package cluster

import (
	"context"
	"time"

	"github.com/goravel/framework/facades"
	pb "github.com/prvious/kontainer/server/proto/agent"
)

// DevDiscovery pings a static list of follower addresses for local development.
type DevDiscovery struct {
	addresses []string
	registry  *Registry
	pool      *GrpcPool
}

func NewDevDiscovery(addresses []string, registry *Registry, pool *GrpcPool) *DevDiscovery {
	return &DevDiscovery{
		addresses: addresses,
		registry:  registry,
		pool:      pool,
	}
}

func (d *DevDiscovery) Discover(ctx context.Context) {
	var healthy []AgentInfo
	for _, addr := range d.addresses {
		info, err := d.pingFollower(ctx, addr)
		if err != nil {
			facades.Log().Warningf("dev discovery: follower %s unreachable: %v", addr, err)
			continue
		}

		info.Address = addr
		info.Role = RoleFollower
		info.Status = "healthy"
		info.LastSeen = time.Now().UTC().Format(time.RFC3339)

		healthy = append(healthy, info)
	}
	d.registry.UpdateFollowers(healthy)
}

func (d *DevDiscovery) pingFollower(ctx context.Context, addr string) (AgentInfo, error) {
	ctx, cancel := context.WithTimeout(ctx, 3*time.Second)
	defer cancel()

	grpcClient, err := d.pool.GetClient(addr)
	if err != nil {
		return AgentInfo{}, err
	}

	resp, err := grpcClient.Health(ctx, &pb.HealthRequest{})
	if err != nil {
		d.pool.Remove(addr)
		return AgentInfo{}, err
	}

	return AgentInfo{
		NodeID:   resp.NodeId,
		Hostname: resp.Hostname,
	}, nil
}

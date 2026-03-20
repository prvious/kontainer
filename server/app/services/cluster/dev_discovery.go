package cluster

import (
	"context"
	"time"

	"github.com/goravel/framework/facades"
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
		info, err := pingFollower(ctx, d.pool, addr)
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

	if len(healthy) == 0 && len(d.addresses) > 0 {
		facades.Log().Warningf("dev discovery: all %d configured followers are unreachable", len(d.addresses))
	}
}

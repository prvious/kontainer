package providers

import (
	"fmt"
	"strings"

	dockerclient "github.com/docker/docker/client"
	"github.com/goravel/framework/contracts/foundation"

	"github.com/prvious/kontainer/server/app/facades"
	"github.com/prvious/kontainer/server/app/services/cluster"
)

type ClusterServiceProvider struct{}

func (p *ClusterServiceProvider) Register(app foundation.Application) {
	app.Singleton("cluster.registry", func(app foundation.Application) (any, error) {
		return cluster.NewRegistry(), nil
	})

	app.Singleton("cluster.pool", func(app foundation.Application) (any, error) {
		secret := facades.Config().GetString("cluster.secret")
		return cluster.NewGrpcPool(secret), nil
	})

	app.Singleton("cluster.discovery", func(app foundation.Application) (any, error) {
		registryInstance, err := app.Make("cluster.registry")
		if err != nil {
			return nil, fmt.Errorf("failed to resolve cluster.registry: %w", err)
		}
		registry, ok := registryInstance.(*cluster.Registry)
		if !ok {
			return nil, fmt.Errorf("cluster.registry is not *cluster.Registry")
		}

		poolInstance, err := app.Make("cluster.pool")
		if err != nil {
			return nil, fmt.Errorf("failed to resolve cluster.pool: %w", err)
		}
		pool, ok := poolInstance.(*cluster.GrpcPool)
		if !ok {
			return nil, fmt.Errorf("cluster.pool is not *cluster.GrpcPool")
		}

		devFollowers := facades.Config().GetString("cluster.dev_followers")
		if devFollowers != "" {
			addresses := strings.Split(devFollowers, ",")
			for i := range addresses {
				addresses[i] = strings.TrimSpace(addresses[i])
			}
			return cluster.NewDevDiscovery(addresses, registry, pool), nil
		}

		// Swarm discovery needs Docker client
		clientInstance, err := app.Make("docker.client")
		if err != nil {
			return nil, fmt.Errorf("failed to resolve docker.client for cluster discovery: %w", err)
		}
		docker, ok := clientInstance.(*dockerclient.Client)
		if !ok {
			return nil, fmt.Errorf("docker.client is not *dockerclient.Client")
		}

		followerService := facades.Config().GetString("cluster.follower_service")
		grpcPort := facades.Config().GetString("grpc.port", "9000")
		return cluster.NewSwarmDiscovery(docker, registry, pool, followerService, grpcPort), nil
	})
}

func (p *ClusterServiceProvider) Boot(app foundation.Application) {
	role := facades.Config().GetString("cluster.role")
	if role != "leader" {
		return
	}

	registryInstance, err := app.Make("cluster.registry")
	if err != nil {
		facades.Log().Errorf("ClusterServiceProvider Boot: failed to resolve registry: %v", err)
		return
	}
	registry, ok := registryInstance.(*cluster.Registry)
	if !ok {
		facades.Log().Errorf("ClusterServiceProvider Boot: cluster.registry is not *cluster.Registry")
		return
	}

	// Resolve nodeID from Docker service provider
	nodeIDInstance, err := app.Make("docker.node_id")
	if err != nil {
		facades.Log().Warningf("ClusterServiceProvider Boot: could not resolve docker.node_id: %v", err)
	}
	nodeID, ok := nodeIDInstance.(string)
	if !ok || nodeID == "" {
		facades.Log().Warningf("ClusterServiceProvider Boot: docker.node_id is empty, leader will have no node ID")
	}

	var hostname string
	hostnameInstance, err := app.Make("docker.hostname")
	if err != nil {
		facades.Log().Errorf("ClusterServiceProvider Boot: could not resolve hostname, using 'unknown': %v", err)
		hostname = "unknown"
	} else {
		hostname, _ = hostnameInstance.(string)
		if hostname == "" {
			facades.Log().Warningf("ClusterServiceProvider Boot: hostname resolved but is empty, using 'unknown'")
			hostname = "unknown"
		}
	}
	grpcPort := facades.Config().GetString("grpc.port", "9000")
	selfAddr := fmt.Sprintf("%s:%s", hostname, grpcPort)

	self := cluster.NewAgentInfo(nodeID, hostname, selfAddr, cluster.RoleLeader, "healthy")
	registry.SetSelf(self)
}

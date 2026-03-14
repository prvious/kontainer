package cluster

import (
	"context"
	"fmt"
	"time"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/filters"
	"github.com/docker/docker/api/types/swarm"
	"github.com/docker/docker/client"
	"github.com/goravel/framework/facades"
	pb "github.com/prvious/kontainer/server/proto/agent"
)

// SwarmDiscovery discovers followers by querying Docker Swarm tasks
// and verifying health via gRPC.
type SwarmDiscovery struct {
	docker          *client.Client
	registry        *Registry
	pool            *GrpcPool
	followerService string
	grpcPort        string
}

func NewSwarmDiscovery(
	docker *client.Client,
	registry *Registry,
	pool *GrpcPool,
	followerService string,
	grpcPort string,
) *SwarmDiscovery {
	return &SwarmDiscovery{
		docker:          docker,
		registry:        registry,
		pool:            pool,
		followerService: followerService,
		grpcPort:        grpcPort,
	}
}

func (d *SwarmDiscovery) Discover(ctx context.Context) {
	taskFilter := filters.NewArgs()
	taskFilter.Add("service", d.followerService)
	taskFilter.Add("desired-state", "running")

	tasks, err := d.docker.TaskList(ctx, types.TaskListOptions{Filters: taskFilter})
	if err != nil {
		facades.Log().Errorf("cluster discovery: task list error: %v", err)
		return
	}

	nodes, err := d.docker.NodeList(ctx, types.NodeListOptions{})
	if err != nil {
		facades.Log().Errorf("cluster discovery: node list error: %v", err)
		return
	}
	nodeHostnames := make(map[string]string, len(nodes))
	for _, n := range nodes {
		nodeHostnames[n.ID] = n.Description.Hostname
	}

	var healthy []AgentInfo
	for _, task := range tasks {
		if task.Status.State != swarm.TaskStateRunning {
			continue
		}

		ip := extractTaskIP(task)
		if ip == "" {
			continue
		}

		addr := fmt.Sprintf("%s:%s", ip, d.grpcPort)
		info, err := d.pingFollower(ctx, addr)
		if err != nil {
			facades.Log().Warningf("cluster discovery: follower %s unreachable: %v", addr, err)
			continue
		}

		info.Address = addr
		info.Role = RoleFollower
		info.Status = "healthy"
		info.LastSeen = time.Now().UTC().Format(time.RFC3339)
		if info.Hostname == "" {
			info.Hostname = nodeHostnames[task.NodeID]
		}
		if info.NodeID == "" {
			info.NodeID = task.NodeID
		}

		healthy = append(healthy, info)
	}

	d.registry.UpdateFollowers(healthy)
}

func extractTaskIP(task swarm.Task) string {
	for _, att := range task.NetworksAttachments {
		for _, addr := range att.Addresses {
			for i, c := range addr {
				if c == '/' {
					return addr[:i]
				}
			}
			return addr
		}
	}
	return ""
}

func (d *SwarmDiscovery) pingFollower(ctx context.Context, addr string) (AgentInfo, error) {
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

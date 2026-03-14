package cluster

import (
	"context"
	"fmt"
	"log"
	"time"

	"github.com/docker/docker/api/types/filters"
	"github.com/docker/docker/api/types/swarm"
	"github.com/docker/docker/client"
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

	tasks, err := d.docker.TaskList(ctx, swarm.TaskListOptions{Filters: taskFilter})
	if err != nil {
		log.Printf("cluster discovery: task list error, clearing stale followers: %v", err)
		d.registry.UpdateFollowers(nil)
		return
	}

	nodes, err := d.docker.NodeList(ctx, swarm.NodeListOptions{})
	if err != nil {
		log.Printf("cluster discovery: node list error, clearing stale followers: %v", err)
		d.registry.UpdateFollowers(nil)
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
			log.Printf("cluster discovery: task %s on node %s has no network attachment, skipping", task.ID[:12], task.NodeID)
			continue
		}

		addr := fmt.Sprintf("%s:%s", ip, d.grpcPort)
		info, err := pingFollower(ctx, d.pool, addr)
		if err != nil {
			log.Printf("cluster discovery: follower %s unreachable: %v", addr, err)
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

// pingFollower checks a follower's health via gRPC. Shared by both
// SwarmDiscovery and DevDiscovery.
func pingFollower(ctx context.Context, pool *GrpcPool, addr string) (AgentInfo, error) {
	ctx, cancel := context.WithTimeout(ctx, 3*time.Second)
	defer cancel()

	grpcClient, err := pool.GetClient(addr)
	if err != nil {
		return AgentInfo{}, err
	}

	resp, err := grpcClient.Health(ctx, &pb.HealthRequest{})
	if err != nil {
		pool.Remove(addr)
		return AgentInfo{}, err
	}

	return AgentInfo{
		NodeID:   resp.NodeId,
		Hostname: resp.Hostname,
	}, nil
}

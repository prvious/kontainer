package cluster

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"time"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/filters"
	"github.com/docker/docker/api/types/swarm"
)

// SwarmDiscoveryClient is a narrow interface for the Docker API methods needed by discovery.
type SwarmDiscoveryClient interface {
	TaskList(ctx context.Context, options types.TaskListOptions) ([]swarm.Task, error)
	NodeList(ctx context.Context, options types.NodeListOptions) ([]swarm.Node, error)
}

type Discovery struct {
	client          SwarmDiscoveryClient
	registry        *Registry
	followerService string
	internalSecret  string
	port            string
	interval        time.Duration
	cancel          context.CancelFunc
}

func NewDiscovery(client SwarmDiscoveryClient, registry *Registry, followerService, internalSecret, port string) *Discovery {
	return &Discovery{
		client:          client,
		registry:        registry,
		followerService: followerService,
		internalSecret:  internalSecret,
		port:            port,
		interval:        10 * time.Second,
	}
}

func (d *Discovery) Start(ctx context.Context) {
	ctx, d.cancel = context.WithCancel(ctx)
	go d.loop(ctx)
}

func (d *Discovery) Stop() {
	if d.cancel != nil {
		d.cancel()
	}
}

func (d *Discovery) loop(ctx context.Context) {
	d.discover(ctx)
	ticker := time.NewTicker(d.interval)
	defer ticker.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			d.discover(ctx)
		}
	}
}

func (d *Discovery) discover(ctx context.Context) {
	taskFilter := filters.NewArgs()
	taskFilter.Add("service", d.followerService)
	taskFilter.Add("desired-state", "running")

	tasks, err := d.client.TaskList(ctx, types.TaskListOptions{Filters: taskFilter})
	if err != nil {
		log.Printf("cluster discovery: task list error: %v", err)
		return
	}

	// Build node hostname map
	nodes, err := d.client.NodeList(ctx, types.NodeListOptions{})
	if err != nil {
		log.Printf("cluster discovery: node list error: %v", err)
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

		addr := fmt.Sprintf("%s:%s", ip, d.port)
		info, err := d.pingFollower(ctx, addr)
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

	d.registry.Update(healthy)
}

func extractTaskIP(task swarm.Task) string {
	for _, att := range task.NetworksAttachments {
		for _, addr := range att.Addresses {
			// addresses are in CIDR form: "10.0.0.5/24"
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

func (d *Discovery) pingFollower(ctx context.Context, addr string) (AgentInfo, error) {
	url := fmt.Sprintf("http://%s/internal/health", addr)
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return AgentInfo{}, err
	}
	req.Header.Set(InternalSecretHeader, d.internalSecret)

	httpClient := &http.Client{Timeout: 3 * time.Second}
	resp, err := httpClient.Do(req)
	if err != nil {
		return AgentInfo{}, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return AgentInfo{}, fmt.Errorf("unexpected status %d", resp.StatusCode)
	}

	var info AgentInfo
	if err := json.NewDecoder(resp.Body).Decode(&info); err != nil {
		return AgentInfo{}, err
	}
	return info, nil
}

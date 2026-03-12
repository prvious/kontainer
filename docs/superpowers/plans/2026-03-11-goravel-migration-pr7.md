# Goravel Agent Migration: PR 7 (Cluster Management)

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement cluster management — thread-safe agent registry, gRPC client connection pool, Swarm and dev discovery as Goravel scheduled tasks, agent listing endpoint, and the ClusterServiceProvider that wires everything together.

**Architecture:** The registry is an in-memory thread-safe map of AgentInfo structs keyed by node ID. The gRPC pool manages dynamic `grpc.ClientConn` connections to followers, injecting shared secret via client interceptors. Discovery (Swarm-based or dev-static) runs as a Goravel scheduled task using `SkipIfStillRunning()`. The ClusterServiceProvider registers registry and pool as singletons; the console Kernel schedules discovery based on config.

**Tech Stack:** Goravel framework (service providers, scheduling), gRPC (`google.golang.org/grpc`), protobuf-generated client (`pb.AgentServiceClient` from PR 6)

**Spec:** `docs/superpowers/specs/2026-03-11-goravel-agent-migration-design.md`

**Prerequisites:** PR 5 (Orchestrator interface), PR 6 (gRPC proto + server)

**Goravel patterns used:**
- Service providers: `Singleton()` bindings, `Boot()` for wiring
- Scheduling: `facades.Schedule().Call(closure).EveryTenSeconds().SkipIfStillRunning()`
- Config: `facades.Config().GetString("cluster.secret")` etc.
- Logging: `facades.Log().Infof()`, `facades.Log().Errorf()`

---

## Chunk 1: Registry + gRPC Pool

### File Structure

```
server/app/services/cluster/
├── registry.go          # Thread-safe agent registry
├── grpc_pool.go         # gRPC client connection pool with secret auth
├── discovery.go         # Discovery interface
├── swarm_discovery.go   # Swarm-based discovery (calls gRPC Health)
└── dev_discovery.go     # Static address discovery (calls gRPC Health)

server/app/http/controllers/agents/
└── list_agents_controller.go

server/app/actions/agents/
└── list_agents.go

server/app/providers/
└── cluster_service_provider.go

server/app/console/
└── kernel.go            # Add discovery scheduled tasks

server/routes/
└── api.go               # Add /api/agents route
```

### Task 1: Agent Registry

Ports `agent/cluster/registry.go` and `agent/cluster/agent.go`. Thread-safe map keyed by nodeID. Uses its own `AgentInfo` type (not the Orchestrator domain types, since agent info is a cluster concept, not a Docker concept).

**Files:**
- Create: `server/app/services/cluster/registry.go`

- [ ] **Step 1: Create the registry file**

```go
package cluster

import (
	"sync"
	"time"
)

const (
	RoleLeader   = "leader"
	RoleFollower = "follower"
)

// AgentInfo represents a known agent in the cluster.
type AgentInfo struct {
	NodeID   string `json:"nodeId"`
	Hostname string `json:"hostname"`
	Address  string `json:"address"`
	Role     string `json:"role"`
	Status   string `json:"status"`
	LastSeen string `json:"lastSeen"`
}

// NewAgentInfo creates an AgentInfo with the current time as LastSeen.
func NewAgentInfo(nodeID, hostname, address, role, status string) AgentInfo {
	return AgentInfo{
		NodeID:   nodeID,
		Hostname: hostname,
		Address:  address,
		Role:     role,
		Status:   status,
		LastSeen: time.Now().UTC().Format(time.RFC3339),
	}
}

// Registry is a thread-safe in-memory store of known cluster agents.
type Registry struct {
	mu     sync.RWMutex
	agents map[string]*AgentInfo
}

func NewRegistry() *Registry {
	return &Registry{
		agents: make(map[string]*AgentInfo),
	}
}

// Update replaces the entire agent map with the provided list.
func (r *Registry) Update(agents []AgentInfo) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.agents = make(map[string]*AgentInfo, len(agents))
	for i := range agents {
		r.agents[agents[i].NodeID] = &agents[i]
	}
}

// Get returns a copy of the agent with the given node ID, or nil.
func (r *Registry) Get(nodeID string) *AgentInfo {
	r.mu.RLock()
	defer r.mu.RUnlock()
	if a, ok := r.agents[nodeID]; ok {
		cp := *a
		return &cp
	}
	return nil
}

// All returns a copy of all registered agents.
func (r *Registry) All() []AgentInfo {
	r.mu.RLock()
	defer r.mu.RUnlock()
	result := make([]AgentInfo, 0, len(r.agents))
	for _, a := range r.agents {
		result = append(result, *a)
	}
	return result
}

// ByNodeID returns the address of the agent with the given node ID.
func (r *Registry) ByNodeID(nodeID string) (string, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	if a, ok := r.agents[nodeID]; ok {
		return a.Address, true
	}
	return "", false
}
```

- [ ] **Step 2: Commit**

```bash
cd server && git add app/services/cluster/registry.go
git commit -m "feat(cluster): add thread-safe agent registry"
```

### Task 2: gRPC Client Pool

New component — replaces `agent/cluster/client.go` (HTTP client). Manages dynamic gRPC connections to followers. Client-side interceptors inject the shared secret into gRPC metadata on every call.

**Files:**
- Create: `server/app/services/cluster/grpc_pool.go`

- [ ] **Step 1: Create the gRPC pool**

```go
package cluster

import (
	"context"
	"fmt"
	"sync"

	pb "github.com/prvious/kontainer/server/proto/agent"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/metadata"
)

// GrpcPool manages gRPC client connections to follower agents.
// Connections are created on demand and cached by address.
type GrpcPool struct {
	mu     sync.RWMutex
	conns  map[string]*grpc.ClientConn
	secret string
}

func NewGrpcPool(secret string) *GrpcPool {
	return &GrpcPool{
		conns:  make(map[string]*grpc.ClientConn),
		secret: secret,
	}
}

// GetClient returns a gRPC AgentServiceClient for the given address.
// Creates and caches the connection if not already established.
func (p *GrpcPool) GetClient(address string) (pb.AgentServiceClient, error) {
	p.mu.RLock()
	conn, ok := p.conns[address]
	p.mu.RUnlock()
	if ok {
		return pb.NewAgentServiceClient(conn), nil
	}

	p.mu.Lock()
	defer p.mu.Unlock()

	// Double-check after acquiring write lock
	if conn, ok = p.conns[address]; ok {
		return pb.NewAgentServiceClient(conn), nil
	}

	conn, err := grpc.NewClient(address,
		grpc.WithTransportCredentials(insecure.NewCredentials()),
		grpc.WithUnaryInterceptor(p.unaryInterceptor()),
		grpc.WithStreamInterceptor(p.streamInterceptor()),
	)
	if err != nil {
		return nil, fmt.Errorf("grpc dial %s: %w", address, err)
	}

	p.conns[address] = conn
	return pb.NewAgentServiceClient(conn), nil
}

// Remove closes and removes the connection for the given address.
func (p *GrpcPool) Remove(address string) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if conn, ok := p.conns[address]; ok {
		conn.Close()
		delete(p.conns, address)
	}
}

// CloseAll closes all connections in the pool.
func (p *GrpcPool) CloseAll() {
	p.mu.Lock()
	defer p.mu.Unlock()
	for addr, conn := range p.conns {
		conn.Close()
		delete(p.conns, addr)
	}
}

func (p *GrpcPool) unaryInterceptor() grpc.UnaryClientInterceptor {
	return func(ctx context.Context, method string, req, reply any, cc *grpc.ClientConn, invoker grpc.UnaryInvoker, opts ...grpc.CallOption) error {
		ctx = metadata.AppendToOutgoingContext(ctx, "x-kontainer-secret", p.secret)
		return invoker(ctx, method, req, reply, cc, opts...)
	}
}

func (p *GrpcPool) streamInterceptor() grpc.StreamClientInterceptor {
	return func(ctx context.Context, desc *grpc.StreamDesc, cc *grpc.ClientConn, method string, streamer grpc.Streamer, opts ...grpc.CallOption) (grpc.ClientStream, error) {
		ctx = metadata.AppendToOutgoingContext(ctx, "x-kontainer-secret", p.secret)
		return streamer(ctx, desc, cc, method, opts...)
	}
}
```

- [ ] **Step 2: Commit**

```bash
cd server && git add app/services/cluster/grpc_pool.go
git commit -m "feat(cluster): add gRPC client connection pool with secret auth"
```

### Task 3: Discovery Interface

Shared interface for both Swarm and dev discovery modes.

**Files:**
- Create: `server/app/services/cluster/discovery.go`

- [ ] **Step 1: Create the discovery interface**

```go
package cluster

import "context"

// Discovery defines the contract for cluster follower discovery.
// Implementations discover followers and update the registry.
type Discovery interface {
	Discover(ctx context.Context)
}
```

- [ ] **Step 2: Commit**

```bash
cd server && git add app/services/cluster/discovery.go
git commit -m "feat(cluster): add Discovery interface"
```

### Task 4: Swarm Discovery

Ports `agent/cluster/discovery.go`. Instead of HTTP pinging `/internal/health`, calls `AgentService.Health()` via gRPC pool. Queries Docker Swarm tasks to find follower IPs.

**Files:**
- Create: `server/app/services/cluster/swarm_discovery.go`

- [ ] **Step 1: Create Swarm discovery**

```go
package cluster

import (
	"context"
	"fmt"
	"time"

	"github.com/goravel/framework/facades"
	pb "github.com/prvious/kontainer/server/proto/agent"
)

// SwarmDiscoveryClient is a narrow interface for the Docker API methods needed by discovery.
// This avoids importing the full Orchestrator interface.
type SwarmDiscoveryClient interface {
	// DiscoverFollowerIPs returns the IP addresses and node IDs of running follower tasks.
	// Each entry is {nodeID, ip, hostname}.
	DiscoverFollowerIPs(ctx context.Context, followerService string) ([]FollowerEndpoint, error)
}

// FollowerEndpoint represents a discovered follower's network endpoint.
type FollowerEndpoint struct {
	NodeID   string
	IP       string
	Hostname string
}

// SwarmDiscovery discovers followers by querying Docker Swarm tasks
// and verifying health via gRPC.
type SwarmDiscovery struct {
	client          SwarmDiscoveryClient
	registry        *Registry
	pool            *GrpcPool
	followerService string
	grpcPort        string
}

func NewSwarmDiscovery(
	client SwarmDiscoveryClient,
	registry *Registry,
	pool *GrpcPool,
	followerService string,
	grpcPort string,
) *SwarmDiscovery {
	return &SwarmDiscovery{
		client:          client,
		registry:        registry,
		pool:            pool,
		followerService: followerService,
		grpcPort:        grpcPort,
	}
}

func (d *SwarmDiscovery) Discover(ctx context.Context) {
	endpoints, err := d.client.DiscoverFollowerIPs(ctx, d.followerService)
	if err != nil {
		facades.Log().Errorf("cluster discovery: %v", err)
		return
	}

	var healthy []AgentInfo
	for _, ep := range endpoints {
		addr := fmt.Sprintf("%s:%s", ep.IP, d.grpcPort)
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
			info.Hostname = ep.Hostname
		}
		if info.NodeID == "" {
			info.NodeID = ep.NodeID
		}

		healthy = append(healthy, info)
	}

	d.registry.Update(healthy)
}

func (d *SwarmDiscovery) pingFollower(ctx context.Context, addr string) (AgentInfo, error) {
	ctx, cancel := context.WithTimeout(ctx, 3*time.Second)
	defer cancel()

	client, err := d.pool.GetClient(addr)
	if err != nil {
		return AgentInfo{}, err
	}

	resp, err := client.Health(ctx, &pb.HealthRequest{})
	if err != nil {
		// Remove stale connection on failure
		d.pool.Remove(addr)
		return AgentInfo{}, err
	}

	return AgentInfo{
		NodeID:   resp.NodeId,
		Hostname: resp.Hostname,
	}, nil
}
```

**Note:** The `SwarmDiscoveryClient` interface will be implemented by the SwarmOrchestrator (PR 5). We'll add a `DiscoverFollowerIPs` method to the Orchestrator or create a separate adapter that uses the Docker client directly. The simpler approach is to add a standalone function in the swarm package that takes the Docker client.

Actually, let's keep this cleaner — the SwarmDiscovery should use the Docker client directly (like the old code does), not go through the Orchestrator:

- [ ] **Step 2: Update SwarmDiscovery to use Docker client directly**

Replace the `SwarmDiscoveryClient` interface with direct Docker SDK usage:

```go
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

	d.registry.Update(healthy)
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
```

- [ ] **Step 3: Commit**

```bash
cd server && git add app/services/cluster/swarm_discovery.go
git commit -m "feat(cluster): add Swarm-based gRPC discovery"
```

### Task 5: Dev Discovery

Ports `agent/cluster/dev_discovery.go`. Pings a static list of addresses via gRPC Health RPC.

**Files:**
- Create: `server/app/services/cluster/dev_discovery.go`

- [ ] **Step 1: Create dev discovery**

```go
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
	d.registry.Update(healthy)
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
```

- [ ] **Step 2: Commit**

```bash
cd server && git add app/services/cluster/dev_discovery.go
git commit -m "feat(cluster): add dev discovery with static addresses"
```

## Chunk 2: Agent Listing + Service Provider + Scheduling

### Task 6: List Agents Action

Ports `agent/services/list_agents.go`. Returns leader info + all followers from registry.

**Files:**
- Create: `server/app/actions/agents/list_agents.go`

- [ ] **Step 1: Create the action**

```go
package agents

import (
	"github.com/prvious/kontainer/server/app/services/cluster"
)

type ListAgentsAction struct {
	registry *cluster.Registry
	self     cluster.AgentInfo
}

func NewListAgentsAction(registry *cluster.Registry, self cluster.AgentInfo) *ListAgentsAction {
	return &ListAgentsAction{registry: registry, self: self}
}

func (a *ListAgentsAction) Execute() []cluster.AgentInfo {
	agents := []cluster.AgentInfo{a.self}
	agents = append(agents, a.registry.All()...)
	return agents
}
```

- [ ] **Step 2: Commit**

```bash
cd server && git add app/actions/agents/list_agents.go
git commit -m "feat(agents): add list agents action"
```

### Task 7: List Agents Controller

**Files:**
- Create: `server/app/http/controllers/agents/list_agents_controller.go`

- [ ] **Step 1: Create the controller**

```go
package agents

import (
	"github.com/goravel/framework/contracts/http"
	agentActions "github.com/prvious/kontainer/server/app/actions/agents"
)

type ListAgentsController struct {
	action *agentActions.ListAgentsAction
}

func NewListAgentsController(action *agentActions.ListAgentsAction) *ListAgentsController {
	return &ListAgentsController{action: action}
}

func (c *ListAgentsController) Handle(ctx http.Context) http.Response {
	agents := c.action.Execute()
	return ctx.Response().Json(200, map[string]any{
		"agents": agents,
	})
}
```

- [ ] **Step 2: Commit**

```bash
cd server && git add app/http/controllers/agents/list_agents_controller.go
git commit -m "feat(agents): add list agents controller"
```

### Task 8: Cluster Service Provider

Registers registry, gRPC pool, and discovery as singletons. The discovery implementation depends on config (dev vs production).

**Files:**
- Create: `server/app/providers/cluster_service_provider.go`

- [ ] **Step 1: Create the service provider**

```go
package providers

import (
	"os"
	"strings"

	"github.com/docker/docker/client"
	"github.com/goravel/framework/contracts/foundation"
	"github.com/goravel/framework/facades"
	agentActions "github.com/prvious/kontainer/server/app/actions/agents"
	agentControllers "github.com/prvious/kontainer/server/app/http/controllers/agents"
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
		registry := app.Make("cluster.registry").(*cluster.Registry)
		pool := app.Make("cluster.pool").(*cluster.GrpcPool)

		devFollowers := facades.Config().GetString("cluster.dev_followers")
		if devFollowers != "" {
			addresses := strings.Split(devFollowers, ",")
			for i := range addresses {
				addresses[i] = strings.TrimSpace(addresses[i])
			}
			return cluster.NewDevDiscovery(addresses, registry, pool), nil
		}

		// Swarm discovery needs Docker client
		docker, err := client.NewClientWithOpts(client.FromEnv, client.WithAPIVersionNegotiation())
		if err != nil {
			return nil, err
		}

		followerService := facades.Config().GetString("cluster.follower_service")
		grpcPort := facades.Config().GetString("grpc.port", "9001")
		return cluster.NewSwarmDiscovery(docker, registry, pool, followerService, grpcPort), nil
	})
}

func (p *ClusterServiceProvider) Boot(app foundation.Application) {
	// Register the list agents controller
	registry := app.Make("cluster.registry").(*cluster.Registry)

	nodeID := getNodeID()
	hostname, _ := os.Hostname()
	self := cluster.NewAgentInfo(
		nodeID,
		hostname,
		facades.Config().GetString("app.url"),
		cluster.RoleLeader,
		"healthy",
	)

	action := agentActions.NewListAgentsAction(registry, self)
	controller := agentControllers.NewListAgentsController(action)
	app.Bind("controllers.agents.list", func(app foundation.Application) (any, error) {
		return controller, nil
	})
}

func getNodeID() string {
	// Try to get node ID from Docker Swarm info
	docker, err := client.NewClientWithOpts(client.FromEnv, client.WithAPIVersionNegotiation())
	if err != nil {
		return "unknown"
	}
	defer docker.Close()

	info, err := docker.Info(context.Background())
	if err != nil {
		return "unknown"
	}
	return info.Swarm.NodeID
}
```

**Note:** The `getNodeID()` helper reads the Docker Swarm node ID at boot. This is a simplified version — the full implementation may cache this or get it from the Docker service provider. Add missing `context` import.

- [ ] **Step 2: Register the provider**

Add `&providers.ClusterServiceProvider{}` to the providers list in `server/config/app.go`.

- [ ] **Step 3: Commit**

```bash
cd server && git add app/providers/cluster_service_provider.go config/app.go
git commit -m "feat(cluster): add ClusterServiceProvider with registry, pool, and discovery"
```

### Task 9: Console Kernel — Scheduled Tasks

Update the console kernel to schedule cluster discovery based on agent role and mode.

**Files:**
- Modify: `server/app/console/kernel.go`

- [ ] **Step 1: Add discovery schedule**

```go
package console

import (
	"context"

	"github.com/goravel/framework/contracts/schedule"
	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/app/services/cluster"
)

type Kernel struct{}

func (kernel *Kernel) Schedule() []schedule.Event {
	events := []schedule.Event{}

	// Only run discovery on leader nodes
	if facades.Config().GetString("agent.role") != "leader" {
		return events
	}

	discovery := facades.App().Make("cluster.discovery").(cluster.Discovery)
	devMode := facades.Config().GetString("cluster.dev_followers") != ""

	if devMode {
		events = append(events, facades.Schedule().Call(func() {
			discovery.Discover(context.Background())
		}).EveryFiveSeconds().SkipIfStillRunning())
	} else {
		events = append(events, facades.Schedule().Call(func() {
			discovery.Discover(context.Background())
		}).EveryTenSeconds().SkipIfStillRunning())
	}

	return events
}
```

- [ ] **Step 2: Ensure scheduler is started in bootstrap**

In `server/main.go` (or the bootstrap file), ensure:

```go
go facades.Schedule().Run()
```

is called after app boot.

- [ ] **Step 3: Commit**

```bash
cd server && git add app/console/kernel.go
git commit -m "feat(cluster): schedule discovery as Goravel scheduled task"
```

### Task 10: Route Registration

Add the `/api/agents` route to the API route file.

**Files:**
- Modify: `server/routes/api.go`

- [ ] **Step 1: Add agents route**

Add inside the JWT-protected group in `routes/api.go`:

```go
// Agent listing
listAgentsCtrl := facades.App().Make("controllers.agents.list").(*agentControllers.ListAgentsController)
router.Get("/agents", listAgentsCtrl.Handle)
```

- [ ] **Step 2: Commit**

```bash
cd server && git add routes/api.go
git commit -m "feat(routes): add /api/agents endpoint"
```

### Task 11: Integration Verification

- [ ] **Step 1: Verify build**

```bash
cd server && go build ./...
```

Expected: clean compilation.

- [ ] **Step 2: Verify scheduled task registration**

Check that the kernel returns events for leader mode by reviewing the schedule configuration.

- [ ] **Step 3: Final commit**

```bash
git add -A && git commit -m "feat(cluster): complete PR 7 — cluster management"
```

---

## Summary

| Component | Old (`agent/`) | New (`server/`) |
|---|---|---|
| Registry | `cluster/registry.go` + `cluster/agent.go` | `app/services/cluster/registry.go` |
| HTTP Client | `cluster/client.go` | `app/services/cluster/grpc_pool.go` (gRPC) |
| Swarm Discovery | `cluster/discovery.go` (goroutine) | `app/services/cluster/swarm_discovery.go` (scheduled task) |
| Dev Discovery | `cluster/dev_discovery.go` (goroutine) | `app/services/cluster/dev_discovery.go` (scheduled task) |
| Agent Listing | `services/list_agents.go` | `app/actions/agents/` + `app/http/controllers/agents/` |
| Internal Auth | `cluster/middleware.go` (HTTP) | PR 6 gRPC interceptor |
| Wiring | `main.go` | `app/providers/cluster_service_provider.go` |

### Key Changes from Old Architecture
1. **gRPC replaces HTTP** for all leader↔follower communication
2. **Goravel scheduled tasks** replace raw goroutines for discovery
3. **Service provider pattern** replaces manual wiring in `main.go`
4. **`SkipIfStillRunning()`** prevents discovery pile-up (was manual in goroutine)
5. **gRPC pool** with client interceptors replaces `cluster.Client` with manual header setting

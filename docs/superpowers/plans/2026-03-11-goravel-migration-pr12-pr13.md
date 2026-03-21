# Goravel Agent Migration: PR 12 (Metrics System) + PR 13 (Images & Settings)

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the metrics collection system (node + container metrics, gRPC fan-out, scheduled collection + pruning, history storage) and the images/settings endpoints (cluster image aggregation via gRPC fan-out, prune/remove, settings CRUD via Goravel ORM).

**Architecture:** Metrics collection runs as two Goravel scheduled tasks: `metrics:collect` (every 30s) and `metrics:prune` (every 5min). Collection gathers local node+container metrics and fans out to followers via gRPC `AgentService.Metrics()`. Node metrics are persisted to the `node_metrics` table via Goravel ORM. Container metrics are ephemeral (included in API response but not stored). Image operations fan out to followers via gRPC `AgentService.ListImages()`, `RemoveImage()`, `PruneImages()`. Settings use the `Setting` model (PR 2) with Goravel ORM.

**Tech Stack:** Goravel framework (scheduling, ORM, controllers), gRPC fan-out (PR 6+7), gopsutil (node metrics), Docker SDK (container metrics)

**Spec:** `docs/superpowers/specs/2026-03-11-goravel-agent-migration-design.md`

**Prerequisites:** PR 2 (models), PR 5 (Orchestrator), PR 6 (gRPC), PR 7 (cluster registry + pool)

---

## Chunk 1: PR 12 — Metrics System

### File Structure

```
server/app/actions/metrics/
├── collect_node_metrics.go       # gopsutil node metrics
├── collect_container_metrics.go  # Docker container stats
├── collect_and_store.go          # Orchestrates local + fan-out + persist
├── prune_metrics.go              # Delete old metrics rows
├── get_cluster_metrics.go        # API: real-time + history
└── get_node_metrics.go           # API: single node metrics

server/app/http/controllers/metrics/
├── cluster_metrics_controller.go
└── node_metrics_controller.go
```

### Task 1: Collect Node Metrics Action

Ports `agent/metrics/collect_node_metrics.go`. Uses gopsutil to read CPU, memory, disk.

**Files:**
- Create: `server/app/actions/metrics/collect_node_metrics.go`

- [ ] **Step 1: Create the action**

```go
package metrics

import (
	"os"
	"runtime"

	"github.com/shirou/gopsutil/v4/cpu"
	"github.com/shirou/gopsutil/v4/disk"
	"github.com/shirou/gopsutil/v4/mem"
)

type NodeMetrics struct {
	CPUs          int     `json:"cpus"`
	CPUPercent    float64 `json:"cpuPercent"`
	MemoryTotal   uint64  `json:"memoryTotal"`
	MemoryUsed    uint64  `json:"memoryUsed"`
	MemoryPercent float64 `json:"memoryPercent"`
	DiskTotal     uint64  `json:"diskTotal"`
	DiskUsed      uint64  `json:"diskUsed"`
	DiskPercent   float64 `json:"diskPercent"`
}

type CollectNodeMetricsAction struct{}

func NewCollectNodeMetricsAction() *CollectNodeMetricsAction {
	if _, err := os.Stat("/host/proc"); err == nil {
		os.Setenv("HOST_PROC", "/host/proc")
	}
	return &CollectNodeMetricsAction{}
}

func (a *CollectNodeMetricsAction) Execute() (NodeMetrics, error) {
	v, err := mem.VirtualMemory()
	if err != nil {
		return NodeMetrics{}, err
	}

	cpuPercent, err := cpu.Percent(0, false)
	if err != nil {
		return NodeMetrics{}, err
	}

	d, err := disk.Usage("/")
	if err != nil {
		return NodeMetrics{}, err
	}

	cpuPct := 0.0
	if len(cpuPercent) > 0 {
		cpuPct = cpuPercent[0]
	}

	return NodeMetrics{
		CPUs:          runtime.NumCPU(),
		CPUPercent:    cpuPct,
		MemoryTotal:   v.Total,
		MemoryUsed:    v.Used,
		MemoryPercent: v.UsedPercent,
		DiskTotal:     d.Total,
		DiskUsed:      d.Used,
		DiskPercent:   d.UsedPercent,
	}, nil
}
```

- [ ] **Step 2: Commit**

```bash
cd server && git add app/actions/metrics/collect_node_metrics.go
git commit -m "feat(metrics): add node metrics collection via gopsutil"
```

### Task 2: Collect Container Metrics Action

Ports `agent/metrics/collect_container_metrics.go`. Uses Docker SDK to get container stats.

**Files:**
- Create: `server/app/actions/metrics/collect_container_metrics.go`

- [ ] **Step 1: Create the action**

```go
package metrics

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type ContainerMetrics struct {
	ContainerID   string  `json:"containerId"`
	Name          string  `json:"name"`
	ServiceName   string  `json:"serviceName"`
	CPUPercent    float64 `json:"cpuPercent"`
	MemoryUsed    uint64  `json:"memoryUsed"`
	MemoryLimit   uint64  `json:"memoryLimit"`
	MemoryPercent float64 `json:"memoryPercent"`
	CreatedAt     int64   `json:"createdAt"`
}

type CollectContainerMetricsAction struct {
	orchestrator docker.Orchestrator
}

func NewCollectContainerMetricsAction(orchestrator docker.Orchestrator) *CollectContainerMetricsAction {
	return &CollectContainerMetricsAction{orchestrator: orchestrator}
}

func (a *CollectContainerMetricsAction) Execute(ctx context.Context) ([]ContainerMetrics, error) {
	stats, err := a.orchestrator.TaskStats(ctx)
	if err != nil {
		return nil, err
	}

	result := make([]ContainerMetrics, 0, len(stats))
	for _, s := range stats {
		// NOTE: ContainerStats (PR5) has ServiceID, MemUsage, MemLimit — not ServiceName,
		// MemoryUsed, MemoryLimit, MemoryPercent, or CreatedAt. Compute MemoryPercent locally.
		memPct := 0.0
		if s.MemLimit > 0 {
			memPct = float64(s.MemUsage) / float64(s.MemLimit) * 100.0
		}
		result = append(result, ContainerMetrics{
			ContainerID:   s.ContainerID,
			Name:          s.Name,
			ServiceName:   s.ServiceID, // ServiceID used as fallback — see cross-plan note below
			CPUPercent:    s.CPUPercent,
			MemoryUsed:    uint64(s.MemUsage),
			MemoryLimit:   uint64(s.MemLimit),
			MemoryPercent: memPct,
		})
	}
	return result, nil
}
```

**Note:** The `Orchestrator.TaskStats()` method (from PR 5) wraps the Docker container list + stats calls and returns `[]ContainerStats` domain types. The action maps these to the API response type.

**Cross-plan dependency (PR5):** If `ContainerStats` needs `ServiceName`, add it to the domain type in `orchestrator.go`. Currently `ContainerStats` only has `ServiceID` (not `ServiceName`), `MemUsage`/`MemLimit` (int64, not uint64), no `MemoryPercent`, and no `CreatedAt`. The code above uses `ServiceID` as a fallback for `ServiceName` and computes `MemoryPercent` locally. If the human-readable service name is needed, either:
1. Add `ServiceName string` to `ContainerStats` in PR5's `orchestrator.go` and resolve it during `TaskStats()` collection, or
2. Perform a separate `orchestrator.ListServices()` lookup in this action to map `ServiceID` -> `ServiceName`.

- [ ] **Step 2: Commit**

```bash
cd server && git add app/actions/metrics/collect_container_metrics.go
git commit -m "feat(metrics): add container metrics collection"
```

### Task 3: Metrics Snapshot Types

Shared types used by collector, handlers, and gRPC.

**Files:**
- Create: `server/app/actions/metrics/types.go`

- [ ] **Step 1: Create types**

```go
package metrics

import "time"

// AgentMetricsSnapshot is the complete metrics snapshot for a single agent.
// NOTE: Timestamp is time.Time (not string) to match the NodeMetric model (PR2).
type AgentMetricsSnapshot struct {
	Timestamp  time.Time          `json:"timestamp"`
	NodeID     string             `json:"nodeId"`
	Hostname   string             `json:"hostname"`
	Node       NodeMetrics        `json:"node"`
	Containers []ContainerMetrics `json:"containers"`
}

// ClusterMetricsResponse is the API response for GET /api/metrics.
type ClusterMetricsResponse struct {
	Agents  []AgentMetricsSnapshot `json:"agents"`
	History []AgentMetricsSnapshot `json:"history"`
}
```

- [ ] **Step 2: Commit**

```bash
cd server && git add app/actions/metrics/types.go
git commit -m "feat(metrics): add metrics snapshot types"
```

### Task 4: Collect and Store (Scheduled Task)

Ports `agent/metrics/collector.go`. Collects local metrics, fans out to followers via gRPC, and persists node metrics to DB. Runs as a Goravel scheduled task.

**Files:**
- Create: `server/app/actions/metrics/collect_and_store.go`

- [ ] **Step 1: Create the collector action**

```go
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
		nodeAction:      nodeAction,
		containerAction: containerAction,
		registry:        registry,
		pool:            pool,
		nodeID:          nodeID,
		hostname:        hostname,
	}
}

func (a *CollectAndStoreAction) Execute(ctx context.Context) {
	// Collect and store local metrics
	localSnapshot, err := a.collectLocal(ctx)
	if err != nil {
		facades.Log().Errorf("metrics collector: local collect error: %v", err)
	} else {
		if err := a.persistNodeMetrics(localSnapshot); err != nil {
			facades.Log().Errorf("metrics collector: local insert error: %v", err)
		}
	}

	// Fan out to followers via gRPC
	for _, follower := range a.registry.All() {
		snapshot, err := a.fetchFollowerMetrics(ctx, follower.Address)
		if err != nil {
			facades.Log().Errorf("metrics collector: follower %s (%s) error: %v", follower.NodeID, follower.Address, err)
			continue
		}
		if err := a.persistNodeMetrics(snapshot); err != nil {
			facades.Log().Errorf("metrics collector: follower insert error: %v", err)
		}
	}
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
		Timestamp:  time.Now().UTC(),
		NodeID:     a.nodeID,
		Hostname:   a.hostname,
		Node:       nodeMetrics,
		Containers: containerMetrics,
	}, nil
}

func (a *CollectAndStoreAction) fetchFollowerMetrics(ctx context.Context, address string) (AgentMetricsSnapshot, error) {
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

	// Convert gRPC response to snapshot (parse string timestamp to time.Time)
	ts, err := time.Parse(time.RFC3339, resp.Timestamp)
	if err != nil {
		ts = time.Now().UTC()
	}
	snapshot := AgentMetricsSnapshot{
		Timestamp: ts,
		NodeID:    resp.NodeId,
		Hostname:  resp.Hostname,
		Node: NodeMetrics{
			CPUs:          int(resp.Node.Cpus),
			CPUPercent:    resp.Node.CpuPercent,
			MemoryTotal:   resp.Node.MemoryTotal,
			MemoryUsed:    resp.Node.MemoryUsed,
			MemoryPercent: resp.Node.MemoryPercent,
			DiskTotal:     resp.Node.DiskTotal,
			DiskUsed:      resp.Node.DiskUsed,
			DiskPercent:   resp.Node.DiskPercent,
		},
	}

	for _, c := range resp.Containers {
		snapshot.Containers = append(snapshot.Containers, ContainerMetrics{
			ContainerID:   c.ContainerId,
			Name:          c.Name,
			ServiceName:   c.ServiceName,
			CPUPercent:    c.CpuPercent,
			MemoryUsed:    c.MemoryUsed,
			MemoryLimit:   c.MemoryLimit,
			MemoryPercent: c.MemoryPercent,
			CreatedAt:     c.CreatedAt,
		})
	}

	return snapshot, nil
}

func (a *CollectAndStoreAction) persistNodeMetrics(s AgentMetricsSnapshot) error {
	metric := &models.NodeMetric{
		Timestamp:   s.Timestamp,
		NodeID:      s.NodeID,
		Hostname:    s.Hostname,
		CPUs:        s.Node.CPUs,
		CPUPercent:  s.Node.CPUPercent,
		MemUsed:     s.Node.MemoryUsed,
		MemTotal:    s.Node.MemoryTotal,
		MemPercent:  s.Node.MemoryPercent,
		DiskUsed:    s.Node.DiskUsed,
		DiskTotal:   s.Node.DiskTotal,
		DiskPercent: s.Node.DiskPercent,
	}
	return facades.Orm().Query().Create(metric)
}
```

- [ ] **Step 2: Commit**

```bash
cd server && git add app/actions/metrics/collect_and_store.go
git commit -m "feat(metrics): add collect-and-store action with gRPC fan-out"
```

### Task 5: Prune Metrics Action

Ports the pruning logic from `agent/metrics/collector.go` (tick count % 10).

**Files:**
- Create: `server/app/actions/metrics/prune_metrics.go`

- [ ] **Step 1: Create prune action**

```go
package metrics

import (
	"time"

	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/app/models"
)

type PruneMetricsAction struct {
	retention time.Duration
}

func NewPruneMetricsAction(retention time.Duration) *PruneMetricsAction {
	return &PruneMetricsAction{retention: retention}
}

func (a *PruneMetricsAction) Execute() error {
	// Compare time.Time directly — NodeMetric.Timestamp is time.Time, not a string.
	cutoff := time.Now().Add(-a.retention).UTC()
	_, err := facades.Orm().Query().Where("timestamp < ?", cutoff).Delete(&models.NodeMetric{})
	return err
}
```

- [ ] **Step 2: Commit**

```bash
cd server && git add app/actions/metrics/prune_metrics.go
git commit -m "feat(metrics): add metrics pruning action"
```

### Task 6: Cluster Metrics Controller (GET /api/metrics)

Ports `agent/metrics/handler.go` ClusterMetricsHandler. Returns real-time snapshots from all agents + history from DB.

**Files:**
- Create: `server/app/actions/metrics/get_cluster_metrics.go`
- Create: `server/app/http/controllers/metrics/cluster_metrics_controller.go`

- [ ] **Step 1: Create the action**

```go
package metrics

import (
	"context"
	"time"

	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/app/models"
	"github.com/prvious/kontainer/server/app/services/cluster"
)

type GetClusterMetricsAction struct {
	nodeAction      *CollectNodeMetricsAction
	containerAction *CollectContainerMetricsAction
	registry        *cluster.Registry
	pool            *cluster.GrpcPool
	nodeID          string
	hostname        string
}

func NewGetClusterMetricsAction(
	nodeAction *CollectNodeMetricsAction,
	containerAction *CollectContainerMetricsAction,
	registry *cluster.Registry,
	pool *cluster.GrpcPool,
	nodeID, hostname string,
) *GetClusterMetricsAction {
	return &GetClusterMetricsAction{
		nodeAction:      nodeAction,
		containerAction: containerAction,
		registry:        registry,
		pool:            pool,
		nodeID:          nodeID,
		hostname:        hostname,
	}
}

func (a *GetClusterMetricsAction) Execute(ctx context.Context, rangeDuration time.Duration) (ClusterMetricsResponse, error) {
	// Collect real-time local snapshot
	localNode, _ := a.nodeAction.Execute()
	localContainers, _ := a.containerAction.Execute(ctx)
	localSnapshot := AgentMetricsSnapshot{
		Timestamp:  time.Now().UTC(),
		NodeID:     a.nodeID,
		Hostname:   a.hostname,
		Node:       localNode,
		Containers: localContainers,
	}

	agents := []AgentMetricsSnapshot{localSnapshot}

	// Fan out to followers for real-time snapshots
	// Reuse CollectAndStoreAction for its fetchFollowerMetrics helper — all required fields must be set.
	collector := &CollectAndStoreAction{
		nodeAction:      a.nodeAction,
		containerAction: a.containerAction,
		registry:        a.registry,
		pool:            a.pool,
		nodeID:          a.nodeID,
		hostname:        a.hostname,
	}
	for _, follower := range a.registry.All() {
		snapshot, err := collector.fetchFollowerMetrics(ctx, follower.Address)
		if err != nil {
			facades.Log().Errorf("cluster metrics: follower %s error: %v", follower.Address, err)
			continue
		}
		agents = append(agents, snapshot)
	}

	// Query history from DB — use time.Time directly (matches NodeMetric.Timestamp type).
	since := time.Now().Add(-rangeDuration).UTC()
	var metrics []models.NodeMetric
	if err := facades.Orm().Query().Where("timestamp >= ?", since).OrderBy("timestamp", "asc").Get(&metrics); err != nil {
		facades.Log().Errorf("cluster metrics: history query error: %v", err)
	}

	history := make([]AgentMetricsSnapshot, 0, len(metrics))
	for _, m := range metrics {
		history = append(history, AgentMetricsSnapshot{
			Timestamp: m.Timestamp,
			NodeID:    m.NodeID,
			Hostname:  m.Hostname,
			Node: NodeMetrics{
				CPUs:          m.CPUs,
				CPUPercent:    m.CPUPercent,
				MemoryTotal:   m.MemTotal,
				MemoryUsed:    m.MemUsed,
				MemoryPercent: m.MemPercent,
				DiskTotal:     m.DiskTotal,
				DiskUsed:      m.DiskUsed,
				DiskPercent:   m.DiskPercent,
			},
			Containers: []ContainerMetrics{},
		})
	}

	return ClusterMetricsResponse{
		Agents:  agents,
		History: history,
	}, nil
}
```

- [ ] **Step 2: Create the controller**

```go
package metrics

import (
	"time"

	"github.com/goravel/framework/contracts/http"
	metricsActions "github.com/prvious/kontainer/server/app/actions/metrics"
)

type ClusterMetricsController struct {
	action *metricsActions.GetClusterMetricsAction
}

func NewClusterMetricsController(action *metricsActions.GetClusterMetricsAction) *ClusterMetricsController {
	return &ClusterMetricsController{action: action}
}

func (c *ClusterMetricsController) Handle(ctx http.Context) http.Response {
	rangeDuration := 1 * time.Hour
	switch ctx.Request().Query("range", "1h") {
	case "6h":
		rangeDuration = 6 * time.Hour
	case "24h":
		rangeDuration = 24 * time.Hour
	}

	resp, err := c.action.Execute(ctx.Context(), rangeDuration)
	if err != nil {
		return ctx.Response().Json(500, map[string]string{"error": "failed to collect metrics"})
	}

	return ctx.Response().Json(200, resp)
}
```

- [ ] **Step 3: Commit**

```bash
cd server && git add app/actions/metrics/ app/http/controllers/metrics/
git commit -m "feat(metrics): add cluster metrics controller with history"
```

### Task 7: Node Metrics Controller (GET /api/metrics/{nodeID})

Ports `agent/metrics/handler.go` NodeMetricsHandler. Returns metrics for a specific node (local or via gRPC proxy).

**Files:**
- Create: `server/app/actions/metrics/get_node_metrics.go`
- Create: `server/app/http/controllers/metrics/node_metrics_controller.go`

- [ ] **Step 1: Create action + controller**

The action checks if the nodeID is local, otherwise fetches from the follower via gRPC.

- [ ] **Step 2: Commit**

```bash
cd server && git add app/actions/metrics/get_node_metrics.go app/http/controllers/metrics/node_metrics_controller.go
git commit -m "feat(metrics): add node metrics controller with gRPC proxy"
```

### Task 8: Register Metrics Scheduled Tasks

Update the console kernel to add metrics collection and pruning.

**Files:**
- Modify: `server/app/console/kernel.go`

- [ ] **Step 1: Add metrics schedules**

Add to the `Schedule()` method:

```go
// Metrics collection (leader only, every 30s)
if facades.Config().GetString("agent.role") == "leader" {
    collector := facades.App().Make("metrics.collector").(*metrics.CollectAndStoreAction)
    events = append(events, facades.Schedule().Call(func() {
        collector.Execute(context.Background())
    }).EveryThirtySeconds().SkipIfStillRunning())

    pruner := facades.App().Make("metrics.pruner").(*metrics.PruneMetricsAction)
    events = append(events, facades.Schedule().Call(func() {
        if err := pruner.Execute(); err != nil {
            facades.Log().Errorf("metrics prune error: %v", err)
        }
    }).EveryFiveMinutes())
}
```

- [ ] **Step 2: Register collector and pruner in a MetricsServiceProvider**

Create `server/app/providers/metrics_service_provider.go` to bind `metrics.collector` and `metrics.pruner` singletons.

- [ ] **Step 3: Commit**

```bash
cd server && git add app/console/kernel.go app/providers/metrics_service_provider.go config/app.go
git commit -m "feat(metrics): schedule collection every 30s and pruning every 5min"
```

### Task 9: Register Metrics Routes

- [ ] **Step 1: Add routes**

```go
router.Get("/metrics", clusterMetricsCtrl.Handle)
router.Get("/metrics/{nodeID}", nodeMetricsCtrl.Handle)
```

- [ ] **Step 2: Commit**

```bash
cd server && git add routes/api.go
git commit -m "feat(routes): register metrics endpoints"
```

---

## Chunk 2: PR 13 — Images & Settings

### File Structure

```
server/app/actions/images/
├── list_cluster_images.go
├── remove_image.go
└── prune_images.go

server/app/actions/settings/
├── get_settings.go
└── update_settings.go

server/app/http/controllers/images/
├── cluster_images_controller.go
├── remove_image_controller.go
└── prune_images_controller.go

server/app/http/controllers/settings/
├── get_settings_controller.go
└── update_settings_controller.go
```

### Task 10: Cluster Images Action

Ports `agent/images/handler.go` ClusterImagesHandler. Collects images from leader + all followers via gRPC.

**Files:**
- Create: `server/app/actions/images/list_cluster_images.go`

- [ ] **Step 1: Create the action**

```go
package images

import (
	"context"

	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/app/services/cluster"
	"github.com/prvious/kontainer/server/app/services/docker"
	pb "github.com/prvious/kontainer/server/proto/agent"
)

type ImageItem struct {
	ID       string   `json:"id"`
	Tags     []string `json:"tags"`
	Size     int64    `json:"size"`
	Created  int64    `json:"created"`
	NodeID   string   `json:"nodeId"`
	Hostname string   `json:"hostname"`
}

type NodeImages struct {
	NodeID   string      `json:"nodeId"`
	Hostname string      `json:"hostname"`
	Images   []ImageItem `json:"images"`
}

type ClusterImagesResponse struct {
	Nodes []NodeImages `json:"nodes"`
}

type ListClusterImagesAction struct {
	orchestrator docker.Orchestrator
	registry     *cluster.Registry
	pool         *cluster.GrpcPool
	nodeID       string
	hostname     string
}

func NewListClusterImagesAction(
	orchestrator docker.Orchestrator,
	registry *cluster.Registry,
	pool *cluster.GrpcPool,
	nodeID, hostname string,
) *ListClusterImagesAction {
	return &ListClusterImagesAction{
		orchestrator: orchestrator,
		registry:     registry,
		pool:         pool,
		nodeID:       nodeID,
		hostname:     hostname,
	}
}

func (a *ListClusterImagesAction) Execute(ctx context.Context) (ClusterImagesResponse, error) {
	// Local images
	localImages, err := a.orchestrator.ListImages(ctx)
	if err != nil {
		return ClusterImagesResponse{}, err
	}

	localItems := make([]ImageItem, 0, len(localImages))
	for _, img := range localImages {
		localItems = append(localItems, ImageItem{
			ID:       img.ID,
			Tags:     img.Tags,
			Size:     img.Size,
			Created:  img.Created,
			NodeID:   a.nodeID,
			Hostname: a.hostname,
		})
	}

	nodes := []NodeImages{{
		NodeID:   a.nodeID,
		Hostname: a.hostname,
		Images:   localItems,
	}}

	// Fan out to followers via gRPC
	for _, follower := range a.registry.All() {
		nodeImages, err := a.fetchFollowerImages(ctx, follower)
		if err != nil {
			facades.Log().Errorf("cluster images: follower %s error: %v", follower.Address, err)
			continue
		}
		nodes = append(nodes, nodeImages)
	}

	return ClusterImagesResponse{Nodes: nodes}, nil
}

func (a *ListClusterImagesAction) fetchFollowerImages(ctx context.Context, follower cluster.AgentInfo) (NodeImages, error) {
	client, err := a.pool.GetClient(follower.Address)
	if err != nil {
		return NodeImages{}, err
	}

	resp, err := client.ListImages(ctx, &pb.ListImagesRequest{})
	if err != nil {
		return NodeImages{}, err
	}

	items := make([]ImageItem, 0, len(resp.Images))
	for _, img := range resp.Images {
		items = append(items, ImageItem{
			ID:       img.Id,
			Tags:     img.Tags,
			Size:     img.Size,
			Created:  img.Created,
			NodeID:   follower.NodeID,
			Hostname: follower.Hostname,
		})
	}

	return NodeImages{
		NodeID:   follower.NodeID,
		Hostname: follower.Hostname,
		Images:   items,
	}, nil
}
```

- [ ] **Step 2: Commit**

```bash
cd server && git add app/actions/images/list_cluster_images.go
git commit -m "feat(images): add cluster images aggregation with gRPC fan-out"
```

### Task 11: Remove Image Action

Ports `agent/images/handler.go` RemoveImageHandler. Removes from local or proxies to follower via gRPC.

**Files:**
- Create: `server/app/actions/images/remove_image.go`
- Create: `server/app/http/controllers/images/remove_image_controller.go`

- [ ] **Step 1: Create action + controller**

Action checks `nodeID` param: if local, calls `orchestrator.RemoveImage()`. If remote, calls `pool.GetClient(addr).RemoveImage()` via gRPC.

- [ ] **Step 2: Commit**

```bash
cd server && git add app/actions/images/remove_image.go app/http/controllers/images/remove_image_controller.go
git commit -m "feat(images): add remove image with local/remote routing"
```

### Task 12: Prune Images Action

Ports `agent/images/handler.go` PruneImagesHandler. Prunes locally + fans out to followers.

**Files:**
- Create: `server/app/actions/images/prune_images.go`
- Create: `server/app/http/controllers/images/prune_images_controller.go`

- [ ] **Step 1: Create action + controller**

If `node` query param: prune specific node (local or gRPC proxy).
If no `node` param: prune all nodes (local + fan-out).

Response: `{"spaceReclaimed": ..., "imagesDeleted": ...}` (aggregated across nodes).

- [ ] **Step 2: Commit**

```bash
cd server && git add app/actions/images/prune_images.go app/http/controllers/images/prune_images_controller.go
git commit -m "feat(images): add image prune with cluster fan-out"
```

### Task 13: Cluster Images Controller

**Files:**
- Create: `server/app/http/controllers/images/cluster_images_controller.go`

- [ ] **Step 1: Create the controller**

```go
package images

import (
	"github.com/goravel/framework/contracts/http"
	imageActions "github.com/prvious/kontainer/server/app/actions/images"
)

type ClusterImagesController struct {
	action *imageActions.ListClusterImagesAction
}

func NewClusterImagesController(action *imageActions.ListClusterImagesAction) *ClusterImagesController {
	return &ClusterImagesController{action: action}
}

func (c *ClusterImagesController) Handle(ctx http.Context) http.Response {
	resp, err := c.action.Execute(ctx.Context())
	if err != nil {
		return ctx.Response().Json(500, map[string]string{"error": "failed to list images"})
	}

	return ctx.Response().Json(200, resp)
}
```

- [ ] **Step 2: Commit**

```bash
cd server && git add app/http/controllers/images/cluster_images_controller.go
git commit -m "feat(images): add cluster images controller"
```

### Task 14: Settings CRUD

Ports `agent/settings/get_settings.go` and `update_settings.go`. Uses Goravel ORM with the `Setting` model (PR 2) instead of raw SQL.

**Files:**
- Create: `server/app/actions/settings/get_settings.go`
- Create: `server/app/actions/settings/update_settings.go`
- Create: `server/app/http/controllers/settings/get_settings_controller.go`
- Create: `server/app/http/controllers/settings/update_settings_controller.go`

- [ ] **Step 1: Get Settings action**

```go
package settings

import (
	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/app/models"
)

type GetSettingsAction struct{}

func NewGetSettingsAction() *GetSettingsAction {
	return &GetSettingsAction{}
}

func (a *GetSettingsAction) Execute() (map[string]string, error) {
	var settings []models.Setting
	if err := facades.Orm().Query().Get(&settings); err != nil {
		return nil, err
	}

	result := make(map[string]string, len(settings))
	for _, s := range settings {
		result[s.Key] = s.Value
	}
	return result, nil
}
```

- [ ] **Step 2: Update Settings action**

```go
package settings

import (
	"fmt"

	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/app/models"
)

type UpdateSettingsAction struct{}

func NewUpdateSettingsAction() *UpdateSettingsAction {
	return &UpdateSettingsAction{}
}

func (a *UpdateSettingsAction) Execute(settings map[string]string) error {
	// Process ALL keys before returning errors — do not bail on the first failure.
	var errs []error
	for key, value := range settings {
		var setting models.Setting
		if err := facades.Orm().Query().UpdateOrCreate(&setting,
			models.Setting{Key: key},
			models.Setting{Value: value},
		); err != nil {
			errs = append(errs, fmt.Errorf("failed to save %s: %w", key, err))
		}
	}
	if len(errs) > 0 {
		return fmt.Errorf("settings update errors: %v", errs)
	}
	return nil
}
```

- [ ] **Step 3: Create controllers**

GetSettingsController:
```go
func (c *GetSettingsController) Handle(ctx http.Context) http.Response {
    settings, err := c.action.Execute()
    if err != nil {
        return ctx.Response().Json(500, map[string]string{"error": "failed to get settings"})
    }
    return ctx.Response().Json(200, map[string]any{"settings": settings})
}
```

UpdateSettingsController:
```go
func (c *UpdateSettingsController) Handle(ctx http.Context) http.Response {
    var req struct {
        Settings map[string]string `json:"settings"`
    }
    if err := ctx.Request().Bind(&req); err != nil {
        return ctx.Response().Json(400, map[string]string{"error": "invalid request body"})
    }

    if err := c.updateAction.Execute(req.Settings); err != nil {
        return ctx.Response().Json(500, map[string]string{"error": "failed to update settings"})
    }

    // Return updated settings
    settings, err := c.getAction.Execute()
    if err != nil {
        return ctx.Response().Json(500, map[string]string{"error": "failed to read updated settings"})
    }
    return ctx.Response().Json(200, map[string]any{"settings": settings})
}
```

- [ ] **Step 4: Commit**

```bash
cd server && git add app/actions/settings/ app/http/controllers/settings/
git commit -m "feat(settings): add settings CRUD with Goravel ORM"
```

### Task 15: Register Image & Settings Routes

- [ ] **Step 1: Add routes**

```go
// Images
router.Get("/images", clusterImagesCtrl.Handle)
router.Delete("/images/{imageID}", removeImageCtrl.Handle)
router.Post("/images/prune", pruneImagesCtrl.Handle)

// Settings
router.Get("/settings", getSettingsCtrl.Handle)
router.Put("/settings", updateSettingsCtrl.Handle)
```

- [ ] **Step 2: Commit**

```bash
cd server && git add routes/api.go
git commit -m "feat(routes): register image and settings routes"
```

### Task 16: Build Verification

- [ ] **Step 1: Verify build**

```bash
cd server && go build ./...
```

- [ ] **Step 2: Final commit**

```bash
git add -A && git commit -m "feat: complete PR 12-13 — metrics, images, and settings"
```

---

## Summary

### PR 12: Metrics System

| Component | Old | New |
|---|---|---|
| Node metrics | `metrics/collect_node_metrics.go` | `app/actions/metrics/collect_node_metrics.go` |
| Container metrics | `metrics/collect_container_metrics.go` | `app/actions/metrics/collect_container_metrics.go` |
| Collector goroutine | `metrics/collector.go` (goroutine) | `app/actions/metrics/collect_and_store.go` (scheduled task) |
| Prune (tick count) | `collector.go` line 58 | `app/actions/metrics/prune_metrics.go` (scheduled task) |
| Cluster metrics API | `metrics/handler.go` ClusterMetricsHandler | `app/http/controllers/metrics/cluster_metrics_controller.go` |
| Node metrics API | `metrics/handler.go` NodeMetricsHandler | `app/http/controllers/metrics/node_metrics_controller.go` |
| HTTP fan-out | `fetchFollowerMetrics()` HTTP client | gRPC `AgentService.Metrics()` via pool |
| Metrics storage | `storage/MetricsStore` (raw SQL) | Goravel ORM `NodeMetric` model |

### PR 13: Images & Settings

| Component | Old | New |
|---|---|---|
| Cluster images | `images/handler.go` ClusterImagesHandler | `app/actions/images/list_cluster_images.go` |
| Remove image | `images/handler.go` RemoveImageHandler | `app/actions/images/remove_image.go` |
| Prune images | `images/handler.go` PruneImagesHandler | `app/actions/images/prune_images.go` |
| HTTP fan-out | `fetchFollowerImages()` HTTP client | gRPC `AgentService.ListImages()` via pool |
| Settings storage | `storage/SettingsStore` (raw SQL) | Goravel ORM `Setting` model |
| Get settings | `settings/get_settings.go` | `app/actions/settings/get_settings.go` |
| Update settings | `settings/update_settings.go` | `app/actions/settings/update_settings.go` |

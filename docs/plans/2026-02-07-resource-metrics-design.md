# Resource Metrics Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Expose node CPU/memory/disk and container CPU/mem from each agent, display per-node metrics on the nodes page, aggregate on the dashboard, and container stats on the service tasks view.

**Architecture:** Each agent (leader + follower) collects its own node metrics via `gopsutil` (prod) or `docker.Info()` (dev DinD fallback), plus container stats via Docker API. Followers expose `/internal/metrics`, the leader aggregates all agents on `/api/metrics`. Frontend polls via TanStack Query at 5s intervals.

**Tech Stack:** Go (Chi, Docker SDK, gopsutil), Rust (Tauri, reqwest, serde), TypeScript/React (TanStack Query, Shadcn UI, Tailwind CSS v4)

---

## Task 1: Add gopsutil dependency to Go agent

**Files:**
- Modify: `agent/go.mod`

**Step 1: Add the gopsutil module**

Run:
```bash
go get -C /Users/clovismuneza/Apps/devoper/agent github.com/shirou/gopsutil/v4@latest
```

**Step 2: Verify it was added**

Run:
```bash
grep gopsutil /Users/clovismuneza/Apps/devoper/agent/go.mod
```
Expected: Line containing `github.com/shirou/gopsutil/v4`

**Step 3: Commit**

```bash
git add agent/go.mod agent/go.sum
git commit -m "chore(agent): add gopsutil dependency for node metrics"
```

---

## Task 2: Create metrics types and node metrics collector

**Files:**
- Create: `agent/metrics/types.go`
- Create: `agent/metrics/collect_node_metrics.go`
- Create: `agent/metrics/collect_node_metrics_test.go`

**Step 1: Create the types file**

Create `agent/metrics/types.go`:

```go
package metrics

type NodeMetrics struct {
	CPUs          int     `json:"cpus"`
	MemoryTotal   uint64  `json:"memoryTotal"`
	MemoryUsed    uint64  `json:"memoryUsed"`
	MemoryPercent float64 `json:"memoryPercent"`
	CPUPercent    float64 `json:"cpuPercent"`
	DiskTotal     uint64  `json:"diskTotal"`
	DiskUsed      uint64  `json:"diskUsed"`
	DiskPercent   float64 `json:"diskPercent"`
}

type ContainerMetrics struct {
	ContainerID   string  `json:"containerId"`
	Name          string  `json:"name"`
	ServiceName   string  `json:"serviceName"`
	CPUPercent    float64 `json:"cpuPercent"`
	MemoryUsed    uint64  `json:"memoryUsed"`
	MemoryLimit   uint64  `json:"memoryLimit"`
	MemoryPercent float64 `json:"memoryPercent"`
}

type AgentMetricsSnapshot struct {
	Timestamp  string             `json:"timestamp"`
	NodeID     string             `json:"nodeId"`
	Hostname   string             `json:"hostname"`
	Node       NodeMetrics        `json:"node"`
	Containers []ContainerMetrics `json:"containers"`
}

type ClusterMetricsResponse struct {
	Agents  []AgentMetricsSnapshot `json:"agents"`
	History []AgentMetricsSnapshot `json:"history"`
}
```

**Step 2: Create the node metrics collector**

Create `agent/metrics/collect_node_metrics.go`. This follows the action pattern — `CollectNodeMetricsAction` struct with `Execute` method, Docker client injected via constructor.

The collector interface (`NodeCollector`) allows the action to use either gopsutil (production) or Docker Info (dev/DinD). The action picks the right collector based on whether `DEVOPER_DEV` is set.

```go
package metrics

import (
	"context"
	"os"
	"runtime"

	"github.com/docker/docker/client"
	"github.com/shirou/gopsutil/v4/cpu"
	"github.com/shirou/gopsutil/v4/disk"
	"github.com/shirou/gopsutil/v4/mem"
)

type CollectNodeMetricsAction struct {
	dockerClient *client.Client
}

func NewCollectNodeMetricsAction(dockerClient *client.Client) *CollectNodeMetricsAction {
	return &CollectNodeMetricsAction{dockerClient: dockerClient}
}

func (a *CollectNodeMetricsAction) Execute(ctx context.Context) (NodeMetrics, error) {
	if os.Getenv("DEVOPER_DEV") == "true" {
		return a.collectFromDocker(ctx)
	}
	return collectFromProc()
}

func (a *CollectNodeMetricsAction) collectFromDocker(ctx context.Context) (NodeMetrics, error) {
	info, err := a.dockerClient.Info(ctx)
	if err != nil {
		return NodeMetrics{}, err
	}

	return NodeMetrics{
		CPUs:        info.NCPU,
		MemoryTotal: uint64(info.MemTotal),
	}, nil
}

func collectFromProc() (NodeMetrics, error) {
	// When running in a container, /host/proc should be bind-mounted
	if _, err := os.Stat("/host/proc"); err == nil {
		os.Setenv("HOST_PROC", "/host/proc")
	}

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
		MemoryTotal:   v.Total,
		MemoryUsed:    v.Used,
		MemoryPercent: v.UsedPercent,
		CPUPercent:    cpuPct,
		DiskTotal:     d.Total,
		DiskUsed:      d.Used,
		DiskPercent:   d.UsedPercent,
	}, nil
}
```

**Step 3: Write the test**

Create `agent/metrics/collect_node_metrics_test.go`:

```go
package metrics

import (
	"context"
	"testing"
)

func TestCollectFromProc(t *testing.T) {
	// This test runs on the host — it should return real metrics
	m, err := collectFromProc()
	if err != nil {
		t.Fatalf("collectFromProc error: %v", err)
	}

	if m.CPUs <= 0 {
		t.Errorf("expected CPUs > 0, got %d", m.CPUs)
	}
	if m.MemoryTotal == 0 {
		t.Error("expected MemoryTotal > 0")
	}
	if m.DiskTotal == 0 {
		t.Error("expected DiskTotal > 0")
	}
}

func TestCollectNodeMetrics_DevMode(t *testing.T) {
	// Dev mode requires a Docker client — skip if not available
	t.Setenv("DEVOPER_DEV", "true")

	action := NewCollectNodeMetricsAction(nil)
	// With nil client, this should error
	_, err := action.Execute(context.Background())
	if err == nil {
		t.Error("expected error with nil docker client in dev mode")
	}
}
```

**Step 4: Run tests**

Run:
```bash
go test -C /Users/clovismuneza/Apps/devoper/agent ./metrics/... -v
```
Expected: PASS

**Step 5: Commit**

```bash
git add agent/metrics/
git commit -m "feat(agent): add node metrics types and collector with gopsutil"
```

---

## Task 3: Create container metrics collector

**Files:**
- Create: `agent/metrics/collect_container_metrics.go`
- Create: `agent/metrics/collect_container_metrics_test.go`

**Step 1: Create the container metrics collector**

Create `agent/metrics/collect_container_metrics.go`. Uses `ContainerList` to find running containers, then `ContainerStatsOneShot` (non-streaming) for each.

```go
package metrics

import (
	"context"
	"encoding/json"
	"strings"

	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/container"
)

// MetricsDockerClient defines the Docker API subset needed for metrics collection.
type MetricsDockerClient interface {
	ContainerList(ctx context.Context, options container.ListOptions) ([]types.Container, error)
	ContainerStatsOneShot(ctx context.Context, containerID string) (container.StatsResponseReader, error)
}

type CollectContainerMetricsAction struct {
	client MetricsDockerClient
}

func NewCollectContainerMetricsAction(client MetricsDockerClient) *CollectContainerMetricsAction {
	return &CollectContainerMetricsAction{client: client}
}

func (a *CollectContainerMetricsAction) Execute(ctx context.Context) ([]ContainerMetrics, error) {
	containers, err := a.client.ContainerList(ctx, container.ListOptions{})
	if err != nil {
		return nil, err
	}

	result := make([]ContainerMetrics, 0, len(containers))
	for _, c := range containers {
		stats, err := a.client.ContainerStatsOneShot(ctx, c.ID)
		if err != nil {
			continue // skip containers we can't stat
		}

		var statsJSON container.StatsResponse
		if err := json.NewDecoder(stats.Body).Decode(&statsJSON); err != nil {
			stats.Body.Close()
			continue
		}
		stats.Body.Close()

		cpuPct := calculateCPUPercent(&statsJSON)
		memUsed := statsJSON.MemoryStats.Usage - statsJSON.MemoryStats.Stats["cache"]
		memLimit := statsJSON.MemoryStats.Limit
		memPct := 0.0
		if memLimit > 0 {
			memPct = float64(memUsed) / float64(memLimit) * 100.0
		}

		name := ""
		if len(c.Names) > 0 {
			name = strings.TrimPrefix(c.Names[0], "/")
		}

		serviceName := c.Labels["com.docker.swarm.service.name"]

		result = append(result, ContainerMetrics{
			ContainerID:   c.ID[:12],
			Name:          name,
			ServiceName:   serviceName,
			CPUPercent:    cpuPct,
			MemoryUsed:    memUsed,
			MemoryLimit:   memLimit,
			MemoryPercent: memPct,
		})
	}

	return result, nil
}

func calculateCPUPercent(stats *container.StatsResponse) float64 {
	cpuDelta := float64(stats.CPUStats.CPUUsage.TotalUsage - stats.PreCPUStats.CPUUsage.TotalUsage)
	systemDelta := float64(stats.CPUStats.SystemUsage - stats.PreCPUStats.SystemUsage)
	if systemDelta > 0 && cpuDelta > 0 {
		return (cpuDelta / systemDelta) * float64(stats.CPUStats.OnlineCPUs) * 100.0
	}
	return 0.0
}
```

**Step 2: Write the test**

Create `agent/metrics/collect_container_metrics_test.go`:

```go
package metrics

import (
	"testing"

	"github.com/docker/docker/api/types/container"
)

func TestCalculateCPUPercent(t *testing.T) {
	stats := &container.StatsResponse{}
	stats.CPUStats.CPUUsage.TotalUsage = 200
	stats.PreCPUStats.CPUUsage.TotalUsage = 100
	stats.CPUStats.SystemUsage = 2000
	stats.PreCPUStats.SystemUsage = 1000
	stats.CPUStats.OnlineCPUs = 2

	pct := calculateCPUPercent(stats)
	// (200-100)/(2000-1000) * 2 * 100 = 20%
	if pct < 19.9 || pct > 20.1 {
		t.Errorf("expected ~20%%, got %f", pct)
	}
}

func TestCalculateCPUPercent_ZeroDelta(t *testing.T) {
	stats := &container.StatsResponse{}
	pct := calculateCPUPercent(stats)
	if pct != 0 {
		t.Errorf("expected 0%%, got %f", pct)
	}
}
```

**Step 3: Run tests**

Run:
```bash
go test -C /Users/clovismuneza/Apps/devoper/agent ./metrics/... -v
```
Expected: PASS

**Step 4: Commit**

```bash
git add agent/metrics/
git commit -m "feat(agent): add container metrics collector via Docker stats API"
```

---

## Task 4: Create metrics HTTP handlers and wire into follower + leader

**Files:**
- Create: `agent/metrics/handler.go`
- Modify: `agent/server/follower.go` — add `/internal/metrics` route
- Modify: `agent/server/server.go` — add `/api/metrics` and `/api/metrics/{nodeID}` routes, add handler params
- Modify: `agent/main.go` — instantiate metrics actions/handlers, pass to `server.New` and `server.NewFollower`

**Step 1: Create the handler file**

Create `agent/metrics/handler.go`. Three handlers:

1. `InternalMetricsHandler` — used by both follower and leader on `/internal/metrics`
2. `ClusterMetricsHandler` — leader only, aggregates from all agents on `/api/metrics`
3. `NodeMetricsHandler` — leader only, proxies to a specific agent on `/api/metrics/{nodeID}`

```go
package metrics

import (
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"os"
	"time"

	"github.com/clovismuneza/devoper/agent/cluster"
	"github.com/go-chi/chi/v5"
)

// InternalMetricsHandler serves metrics for this agent (used on /internal/metrics).
type InternalMetricsHandler struct {
	nodeAction      *CollectNodeMetricsAction
	containerAction *CollectContainerMetricsAction
	nodeID          string
	hostname        string
}

func NewInternalMetricsHandler(
	nodeAction *CollectNodeMetricsAction,
	containerAction *CollectContainerMetricsAction,
	nodeID string,
	hostname string,
) *InternalMetricsHandler {
	return &InternalMetricsHandler{
		nodeAction:      nodeAction,
		containerAction: containerAction,
		nodeID:          nodeID,
		hostname:        hostname,
	}
}

func (h *InternalMetricsHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	nodeMetrics, err := h.nodeAction.Execute(r.Context())
	if err != nil {
		log.Printf("collect node metrics error: %v", err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to collect node metrics"})
		return
	}

	containers, err := h.containerAction.Execute(r.Context())
	if err != nil {
		log.Printf("collect container metrics error: %v", err)
		containers = []ContainerMetrics{}
	}

	snapshot := AgentMetricsSnapshot{
		Timestamp:  time.Now().UTC().Format(time.RFC3339),
		NodeID:     h.nodeID,
		Hostname:   h.hostname,
		Node:       nodeMetrics,
		Containers: containers,
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(snapshot)
}

// ClusterMetricsHandler aggregates metrics from all agents (leader + followers).
type ClusterMetricsHandler struct {
	localHandler *InternalMetricsHandler
	registry     *cluster.Registry
	secret       string
}

func NewClusterMetricsHandler(
	localHandler *InternalMetricsHandler,
	registry *cluster.Registry,
	secret string,
) *ClusterMetricsHandler {
	return &ClusterMetricsHandler{
		localHandler: localHandler,
		registry:     registry,
		secret:       secret,
	}
}

func (h *ClusterMetricsHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	// Collect local (leader) metrics
	localNode, _ := h.localHandler.nodeAction.Execute(r.Context())
	localContainers, _ := h.localHandler.containerAction.Execute(r.Context())

	leaderSnapshot := AgentMetricsSnapshot{
		Timestamp:  time.Now().UTC().Format(time.RFC3339),
		NodeID:     h.localHandler.nodeID,
		Hostname:   h.localHandler.hostname,
		Node:       localNode,
		Containers: localContainers,
	}
	if leaderSnapshot.Containers == nil {
		leaderSnapshot.Containers = []ContainerMetrics{}
	}

	agents := []AgentMetricsSnapshot{leaderSnapshot}

	// Collect from followers
	for _, follower := range h.registry.All() {
		snapshot, err := fetchFollowerMetrics(r.Context(), follower.Address, h.secret)
		if err != nil {
			log.Printf("fetch metrics from %s (%s): %v", follower.Hostname, follower.Address, err)
			continue
		}
		agents = append(agents, snapshot)
	}

	resp := ClusterMetricsResponse{
		Agents:  agents,
		History: []AgentMetricsSnapshot{},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(resp)
}

// NodeMetricsHandler returns metrics for a single node by ID.
type NodeMetricsHandler struct {
	localHandler *InternalMetricsHandler
	registry     *cluster.Registry
	secret       string
}

func NewNodeMetricsHandler(
	localHandler *InternalMetricsHandler,
	registry *cluster.Registry,
	secret string,
) *NodeMetricsHandler {
	return &NodeMetricsHandler{
		localHandler: localHandler,
		registry:     registry,
		secret:       secret,
	}
}

func (h *NodeMetricsHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	nodeID := chi.URLParam(r, "nodeID")

	// Check if it's the leader itself
	if nodeID == h.localHandler.nodeID {
		h.localHandler.ServeHTTP(w, r)
		return
	}

	// Look up follower address
	addr, ok := h.registry.ByNodeID(nodeID)
	if !ok {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusNotFound)
		json.NewEncoder(w).Encode(map[string]string{"error": "node not found"})
		return
	}

	snapshot, err := fetchFollowerMetrics(r.Context(), addr, h.secret)
	if err != nil {
		log.Printf("fetch metrics from node %s: %v", nodeID, err)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadGateway)
		json.NewEncoder(w).Encode(map[string]string{"error": "failed to reach agent"})
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(snapshot)
}

func fetchFollowerMetrics(ctx context.Context, addr string, secret string) (AgentMetricsSnapshot, error) {
	scheme := "http"
	if os.Getenv("DEVOPER_DEV") != "true" {
		scheme = "http" // followers are on overlay network, no TLS internally
	}
	url := fmt.Sprintf("%s://%s/internal/metrics", scheme, addr)
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return AgentMetricsSnapshot{}, err
	}
	req.Header.Set(cluster.InternalSecretHeader, secret)

	client := &http.Client{Timeout: 5 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		return AgentMetricsSnapshot{}, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return AgentMetricsSnapshot{}, fmt.Errorf("unexpected status %d", resp.StatusCode)
	}

	var snapshot AgentMetricsSnapshot
	if err := json.NewDecoder(resp.Body).Decode(&snapshot); err != nil {
		return AgentMetricsSnapshot{}, err
	}
	return snapshot, nil
}
```

**Step 2: Add `/internal/metrics` to follower**

Modify `agent/server/follower.go`:
- Add `*metrics.InternalMetricsHandler` parameter to `NewFollower`
- Register `r.Method("GET", "/metrics", metricsHandler)` inside the `/internal` route group

The function signature becomes:
```go
func NewFollower(identity FollowerIdentity, internalSecret string, metricsHandler http.Handler) http.Handler {
```

And add inside the `r.Route("/internal", ...)`:
```go
r.Method("GET", "/metrics", metricsHandler)
```

**Step 3: Add `/api/metrics` routes to leader**

Modify `agent/server/server.go`:
- Add two new parameters to `New()`: `clusterMetricsHandler *metrics.ClusterMetricsHandler`, `nodeMetricsHandler *metrics.NodeMetricsHandler`
- Register inside the JWT-protected group:
```go
r.Method("GET", "/metrics", clusterMetricsHandler)
r.Method("GET", "/metrics/{nodeID}", nodeMetricsHandler)
```

Also add the internal metrics endpoint for the leader (so it can serve its own metrics when queried by nodeID):
- Add `internalMetricsHandler` parameter (same `http.Handler`)
- Add an `/internal` route group with secret auth + metrics endpoint on the leader too

**Step 4: Wire everything in `main.go`**

Modify `agent/main.go`:

In `startFollower()`:
```go
// Create Docker client for metrics
dockerClient, err := client.NewClientWithOpts(client.FromEnv, client.WithAPIVersionNegotiation())
if err != nil {
    log.Fatalf("Failed to create Docker client: %v", err)
}

nodeAction := metrics.NewCollectNodeMetricsAction(dockerClient)
containerAction := metrics.NewCollectContainerMetricsAction(dockerClient)
metricsHandler := metrics.NewInternalMetricsHandler(nodeAction, containerAction, nodeID, hostname)

handler := server.NewFollower(identity, cfg.InternalSecret, metricsHandler)
```

In `startLeader()`, after existing Docker client creation:
```go
nodeAction := metrics.NewCollectNodeMetricsAction(dockerClient)
containerAction := metrics.NewCollectContainerMetricsAction(dockerClient)
leaderNodeID := os.Getenv("DOCKER_NODE_ID")
internalMetricsHandler := metrics.NewInternalMetricsHandler(nodeAction, containerAction, leaderNodeID, hostname)
clusterMetricsHandler := metrics.NewClusterMetricsHandler(internalMetricsHandler, clusterRegistry, cfg.InternalSecret)
nodeMetricsHandler := metrics.NewNodeMetricsHandler(internalMetricsHandler, clusterRegistry, cfg.InternalSecret)
```

Pass `clusterMetricsHandler` and `nodeMetricsHandler` to `server.New(...)`.

**Step 5: Verify compilation**

Run:
```bash
go build -C /Users/clovismuneza/Apps/devoper/agent .
```
Expected: No errors

**Step 6: Run all tests**

Run:
```bash
go test -C /Users/clovismuneza/Apps/devoper/agent ./... -v
```
Expected: PASS

**Step 7: Commit**

```bash
git add agent/
git commit -m "feat(agent): add metrics endpoints for follower and leader"
```

---

## Task 5: Create Tauri commands for metrics

**Files:**
- Create: `app/src-tauri/src/actions/agent/list_cluster_metrics.rs`
- Create: `app/src-tauri/src/actions/agent/list_node_metrics.rs`
- Modify: `app/src-tauri/src/actions/agent/mod.rs` — add module declarations
- Modify: `app/src-tauri/src/lib.rs` — register commands in `invoke_handler!`

**Step 1: Create `list_cluster_metrics.rs`**

Follow exact pattern from `list_nodes.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeMetrics {
    pub cpus: i32,
    pub memory_total: u64,
    pub memory_used: u64,
    pub memory_percent: f64,
    pub cpu_percent: f64,
    pub disk_total: u64,
    pub disk_used: u64,
    pub disk_percent: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerMetrics {
    pub container_id: String,
    pub name: String,
    pub service_name: String,
    pub cpu_percent: f64,
    pub memory_used: u64,
    pub memory_limit: u64,
    pub memory_percent: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMetricsSnapshot {
    pub timestamp: String,
    pub node_id: String,
    pub hostname: String,
    pub node: NodeMetrics,
    pub containers: Vec<ContainerMetrics>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClusterMetricsResponse {
    pub agents: Vec<AgentMetricsSnapshot>,
    pub history: Vec<AgentMetricsSnapshot>,
}

#[tauri::command]
pub async fn list_cluster_metrics(
    url: String,
    token: String,
) -> Result<ClusterMetricsResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/api/metrics"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: ClusterMetricsResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
```

**Step 2: Create `list_node_metrics.rs`**

```rust
use super::list_cluster_metrics::AgentMetricsSnapshot;
use std::time::Duration;

#[tauri::command]
pub async fn list_node_metrics(
    url: String,
    token: String,
    node_id: String,
) -> Result<AgentMetricsSnapshot, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/api/metrics/{node_id}"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: AgentMetricsSnapshot = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
```

**Step 3: Add module declarations to `mod.rs`**

Add to `agent/mod.rs`:
```rust
pub mod list_cluster_metrics;
pub mod list_node_metrics;
```

**Step 4: Register in `lib.rs`**

Add to the `invoke_handler!` macro:
```rust
actions::agent::list_cluster_metrics::list_cluster_metrics,
actions::agent::list_node_metrics::list_node_metrics,
```

**Step 5: Verify compilation**

Run:
```bash
cargo check --manifest-path /Users/clovismuneza/Apps/devoper/app/src-tauri/Cargo.toml
```
Expected: No errors

**Step 6: Commit**

```bash
git add app/src-tauri/
git commit -m "feat(app): add Tauri commands for cluster and node metrics"
```

---

## Task 6: Create TypeScript types and `useClusterMetrics` hook

**Files:**
- Create: `app/src/types/metrics.ts`
- Create: `app/src/hooks/use-cluster-metrics.ts`

**Step 1: Create TypeScript types**

Create `app/src/types/metrics.ts`:

```typescript
export interface NodeMetrics {
    cpus: number;
    memoryTotal: number;
    memoryUsed: number;
    memoryPercent: number;
    cpuPercent: number;
    diskTotal: number;
    diskUsed: number;
    diskPercent: number;
}

export interface ContainerMetrics {
    containerId: string;
    name: string;
    serviceName: string;
    cpuPercent: number;
    memoryUsed: number;
    memoryLimit: number;
    memoryPercent: number;
}

export interface AgentMetricsSnapshot {
    timestamp: string;
    nodeId: string;
    hostname: string;
    node: NodeMetrics;
    containers: ContainerMetrics[];
}

export interface ClusterMetricsResponse {
    agents: AgentMetricsSnapshot[];
    history: AgentMetricsSnapshot[];
}
```

**Step 2: Create the hook**

Create `app/src/hooks/use-cluster-metrics.ts`. Follow the exact pattern from `use-nodes.ts`:

```typescript
import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import type { ClusterMetricsResponse } from "@/types/metrics";

export function useClusterMetrics() {
    const { agentUrl, token } = useAuth();

    return useQuery({
        queryKey: ["cluster-metrics"],
        queryFn: () =>
            invoke<ClusterMetricsResponse>("list_cluster_metrics", {
                url: agentUrl,
                token,
            }),
        enabled: !!agentUrl && !!token,
        refetchInterval: 5000,
    });
}
```

**Step 3: Commit**

```bash
git add app/src/types/metrics.ts app/src/hooks/use-cluster-metrics.ts
git commit -m "feat(app): add metrics TypeScript types and useClusterMetrics hook"
```

---

## Task 7: Add metrics stat cards to dashboard

**Files:**
- Modify: `app/src/routes/_authenticated/index.tsx`

**Step 1: Add metrics cards to the dashboard**

Import the hook and types at the top:
```typescript
import { useClusterMetrics } from "@/hooks/use-cluster-metrics";
import { Cpu, HardDrive, MemoryStick } from "lucide-react";
```

Add `useClusterMetrics()` call alongside the existing hooks in `DashboardPage`:
```typescript
const { data: metricsData } = useClusterMetrics();
```

Add a helper function to compute cluster-wide aggregates:
```typescript
function computeClusterMetrics(agents: AgentMetricsSnapshot[]) {
    if (agents.length === 0) return null;
    const avgCpu = agents.reduce((sum, a) => sum + a.node.cpuPercent, 0) / agents.length;
    const totalMemUsed = agents.reduce((sum, a) => sum + a.node.memoryUsed, 0);
    const totalMemTotal = agents.reduce((sum, a) => sum + a.node.memoryTotal, 0);
    const totalDiskUsed = agents.reduce((sum, a) => sum + a.node.diskUsed, 0);
    const totalDiskTotal = agents.reduce((sum, a) => sum + a.node.diskTotal, 0);
    const memPct = totalMemTotal > 0 ? (totalMemUsed / totalMemTotal) * 100 : 0;
    const diskPct = totalDiskTotal > 0 ? (totalDiskUsed / totalDiskTotal) * 100 : 0;
    return { avgCpu, totalMemUsed, totalMemTotal, memPct, totalDiskUsed, totalDiskTotal, diskPct };
}
```

Add a byte formatter:
```typescript
function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}
```

Insert 3 new `<StatCard>` components after the existing 4 stat cards (change grid to 7 columns or keep it adaptive — `lg:grid-cols-4` will wrap them nicely into a second row):

```tsx
{clusterMetrics && (
    <>
        <StatCard label='Avg CPU' value={`${Math.round(clusterMetrics.avgCpu)}%`} icon={Cpu} accent='bg-muted' delay={200} />
        <StatCard
            label='Memory'
            value={`${formatBytes(clusterMetrics.totalMemUsed)} / ${formatBytes(clusterMetrics.totalMemTotal)}`}
            icon={MemoryStick}
            accent='bg-muted'
            delay={250}
        />
        <StatCard
            label='Disk'
            value={`${formatBytes(clusterMetrics.totalDiskUsed)} / ${formatBytes(clusterMetrics.totalDiskTotal)}`}
            icon={HardDrive}
            accent='bg-muted'
            delay={300}
        />
    </>
)}
```

**Step 2: Verify lint passes**

Run:
```bash
pnpm --filter app run lint
```
Expected: No errors

**Step 3: Format**

Run:
```bash
pnpm fmt
```

**Step 4: Commit**

```bash
git add app/src/routes/_authenticated/index.tsx
git commit -m "feat(app): add CPU, memory, disk stat cards to dashboard"
```

---

## Task 8: Add metrics columns and expandable rows to nodes page

**Files:**
- Modify: `app/src/routes/_authenticated/nodes/index.tsx`

**Step 1: Enhance the nodes table**

Import the hook and add utility functions:
```typescript
import { useClusterMetrics } from "@/hooks/use-cluster-metrics";
import { Progress } from "@/components/ui/progress";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { ChevronRight } from "lucide-react";
import type { AgentMetricsSnapshot, ContainerMetrics } from "@/types/metrics";
```

Note: `Collapsible` may need to be installed: `pnpm dlx shadcn@latest add collapsible`

Add `formatBytes` function (same as dashboard).

Add a `metricsColorClass` helper:
```typescript
function metricsColorClass(pct: number): string {
    if (pct >= 85) return "text-destructive";
    if (pct >= 60) return "text-amber-500";
    return "text-success";
}
```

Modify `NodesTable` to accept a `metricsMap` parameter mapping nodeId → `AgentMetricsSnapshot`.

Add 3 new `<TableHead>` columns: CPU, Memory, Disk (after "OS / Arch", before "Updated").

For each node row, look up metrics via `metricsMap[node.id]` and render:
- CPU: `<Progress>` bar + text
- Memory: `<Progress>` bar + `used/total` text
- Disk: `<Progress>` bar + text

Make each row expandable: clicking a row toggles a detail panel showing:
- Full metrics breakdown
- Container stats table with columns: Name, Service, CPU%, Memory

Use `useState` to track which node ID is expanded (`expandedNodeId`). When a row is clicked, toggle expansion. The expanded content renders in a full-width `<TableRow>` with `colSpan`.

**Step 2: Wire it up in `NodesPage`**

Call `useClusterMetrics()` alongside `useNodes()`. Build the metrics map:
```typescript
const metricsMap = useMemo(() => {
    const map: Record<string, AgentMetricsSnapshot> = {};
    for (const agent of metricsData?.agents ?? []) {
        map[agent.nodeId] = agent;
    }
    return map;
}, [metricsData]);
```

Pass `metricsMap` to `<NodesTable>`.

**Step 3: Install collapsible if needed**

Run:
```bash
pnpm dlx shadcn@latest add collapsible
```

**Step 4: Verify lint passes**

Run:
```bash
pnpm --filter app run lint
```

**Step 5: Format**

Run:
```bash
pnpm fmt
```

**Step 6: Commit**

```bash
git add app/src/
git commit -m "feat(app): add metrics columns and expandable rows to nodes table"
```

---

## Task 9: Add CPU/memory columns to service tasks table

**Files:**
- Modify: `app/src/components/service-tasks.tsx`

**Step 1: Add metrics columns**

Import the hook:
```typescript
import { useClusterMetrics } from "@/hooks/use-cluster-metrics";
import type { ContainerMetrics } from "@/types/metrics";
```

Add `formatBytes` helper (same as before).

In `ServiceTasks`, call `useClusterMetrics()` and build a container metrics map by container ID:
```typescript
const containerMetricsMap = useMemo(() => {
    const map: Record<string, ContainerMetrics> = {};
    for (const agent of metricsData?.agents ?? []) {
        for (const c of agent.containers) {
            map[c.containerId] = c;
        }
    }
    return map;
}, [metricsData]);
```

Pass `containerMetricsMap` to `<TasksTable>`.

In `TasksTable`, add two new `<TableHead>` columns after "Container": **CPU** and **Memory**.

For each task row, look up metrics using `task.containerId` (which is already a 12-char prefix). Render:
- CPU: `containerMetrics?.cpuPercent.toFixed(1)%` or `-`
- Memory: `formatBytes(containerMetrics?.memoryUsed) / formatBytes(containerMetrics?.memoryLimit)` or `-`

**Step 2: Verify lint passes**

Run:
```bash
pnpm --filter app run lint
```

**Step 3: Format**

Run:
```bash
pnpm fmt
```

**Step 4: Commit**

```bash
git add app/src/components/service-tasks.tsx
git commit -m "feat(app): add CPU and memory columns to service tasks table"
```

---

## Task 10: Final verification and quality checks

**Step 1: Run all Go tests**

Run:
```bash
go test -C /Users/clovismuneza/Apps/devoper/agent ./... -v
```
Expected: PASS

**Step 2: Run Rust check**

Run:
```bash
cargo check --manifest-path /Users/clovismuneza/Apps/devoper/app/src-tauri/Cargo.toml
```
Expected: No errors

**Step 3: Run frontend lint**

Run:
```bash
pnpm --filter app run lint
```
Expected: No errors

**Step 4: Run frontend format**

Run:
```bash
pnpm fmt
```

**Step 5: Run frontend tests**

Run:
```bash
pnpm --filter app test
```
Expected: PASS

**Step 6: Commit any remaining changes**

```bash
git add -A
git commit -m "chore: final lint and format pass for resource metrics"
```

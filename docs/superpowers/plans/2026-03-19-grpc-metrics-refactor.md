# gRPC Metrics Refactor — Deduplicate and Use Actions

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove duplicate `collectNodeMetrics()` function and `os.Setenv` from the gRPC metrics controller, delegating to the existing `CollectNodeMetricsAction`. Also bundles: move `ManifestState` out of controller package, replace `net/http.Post` with `facades.Http()`, and add interfaces for kernel type assertions.

**Architecture:** The gRPC metrics controller currently duplicates `CollectNodeMetricsAction` logic. Refactor to inject the action via the `AgentController` constructor. The `HOST_PROC` env setup is already handled in `MetricsServiceProvider.Boot()`, so the duplicate in the controller can be deleted.

**Tech Stack:** Go, Goravel framework, gopsutil, gRPC

**Relevant skills:** `goravel-integrations`, `goravel-foundations`

---

## File Structure

### Modified Files
- `server/app/grpc/controllers/agent_controller.go` — Add `nodeMetrics` field to struct
- `server/app/grpc/controllers/metrics_controller.go` — Delete `collectNodeMetrics()`, use injected action
- `server/routes/grpc.go` — Pass action to controller constructor
- `server/app/http/controllers/setup/manifest_controller.go` — Remove `ManifestState` type

### Created Files
- `server/app/services/setup/manifest_state.go` — Moved from controller package
- `server/app/services/metrics/interfaces.go` — Interfaces for metrics actions (for kernel assertions)

---

### Task 1: Move ManifestState from controller package to services/setup/

**Files:**
- Create: `server/app/services/setup/manifest_state.go`
- Modify: `server/app/http/controllers/setup/manifest_controller.go` — Remove ManifestState, import from services
- Modify: `server/routes/setup.go` — Update import to use services package

- [ ] **Step 1: Create `server/app/services/setup/manifest_state.go`**

Move the `ManifestState` struct and its methods (New, Set, Get, ClearToken, ClearState) from `manifest_controller.go` lines 13-48 into this new file. Package should be `setup`.

```go
package setup

import "sync"

type ManifestState struct {
    mu    sync.Mutex
    state string
    token string
    org   string
}

func NewManifestState() *ManifestState {
    return &ManifestState{}
}

func (s *ManifestState) Set(state, token, org string) { ... }
func (s *ManifestState) Get() (state, token, org string) { ... }
func (s *ManifestState) ClearToken() { ... }
func (s *ManifestState) ClearState() { ... }
```

- [ ] **Step 2: Update manifest_controller.go**

Remove the ManifestState type definition (lines 13-48). Update the `ManifestController` struct to reference `*setupservice.ManifestState`. Update `NewManifestController` parameter type. Add import for `setupservice "github.com/prvious/kontainer/server/app/services/setup"`.

- [ ] **Step 3: Update routes/setup.go imports**

Replace `setupcontrollers.NewManifestState()` with `setupservice.NewManifestState()`. Add import. Update all controller constructors that receive `ManifestState` to use the new type.

- [ ] **Step 4: Run `go vet`, commit**

```
git add server/app/services/setup/manifest_state.go server/app/http/controllers/setup/manifest_controller.go server/routes/setup.go
git commit -m "refactor: move ManifestState from controller package to services/setup/"
```

---

### Task 2: Refactor gRPC metrics controller to use CollectNodeMetricsAction

**Files:**
- Modify: `server/app/grpc/controllers/agent_controller.go` — Read this first to see the struct
- Modify: `server/app/grpc/controllers/metrics_controller.go` — Delete `collectNodeMetrics()`, use action
- Modify: `server/routes/grpc.go` — Pass action to constructor

- [ ] **Step 1: Read `agent_controller.go` to understand the struct**

The `AgentController` struct needs a new field for the node metrics action.

- [ ] **Step 2: Add `nodeMetrics` field to AgentController**

In `agent_controller.go`:
```go
type AgentController struct {
    nodeID       string
    hostname     string
    orchestrator docker.Orchestrator
    nodeMetrics  *metrics.CollectNodeMetricsAction  // ADD THIS
}
```

Update `NewAgentController` to accept and assign it:
```go
func NewAgentController(nodeID string, orchestrator docker.Orchestrator, nodeMetrics *metrics.CollectNodeMetricsAction) *AgentController {
    ...
}
```

Add import for `metrics "github.com/prvious/kontainer/server/app/actions/metrics"`.

- [ ] **Step 3: Rewrite `metrics_controller.go`**

Delete the entire `collectNodeMetrics()` function and the `initHostProc` var. Remove `os`, `runtime`, `sync`, `time`, and gopsutil imports.

Rewrite the `Metrics` method to use the injected action:

```go
func (c *AgentController) Metrics(ctx context.Context, req *pb.MetricsRequest) (*pb.MetricsResponse, error) {
    nodeMetrics, err := c.nodeMetrics.Execute()
    if err != nil {
        return nil, status.Errorf(codes.Internal, "collect node metrics: %v", err)
    }

    containerStats, err := c.orchestrator.TaskStats(ctx)
    if err != nil {
        return nil, status.Errorf(codes.Internal, "collect container stats: %v", err)
    }

    containers := make([]*pb.ContainerMetrics, 0, len(containerStats))
    for _, s := range containerStats {
        containers = append(containers, &pb.ContainerMetrics{
            ContainerId:   s.ContainerID,
            Name:          s.Name,
            ServiceId:     s.ServiceID,
            CpuPercent:    s.CPUPercent,
            MemoryUsage:   s.MemUsage,
            MemoryLimit:   s.MemLimit,
            MemoryPercent: s.MemPercent,
        })
    }

    return &pb.MetricsResponse{
        Timestamp:  time.Now().UTC().Format(time.RFC3339),
        NodeId:     c.nodeID,
        Hostname:   c.hostname,
        Node: &pb.NodeMetrics{
            Cpus:          int32(nodeMetrics.CPUs),
            CpuPercent:    nodeMetrics.CPUPercent,
            MemoryUsed:    int64(nodeMetrics.MemoryUsed),
            MemoryTotal:   int64(nodeMetrics.MemoryTotal),
            MemoryPercent: nodeMetrics.MemoryPercent,
            DiskUsed:      int64(nodeMetrics.DiskUsed),
            DiskTotal:     int64(nodeMetrics.DiskTotal),
            DiskPercent:   nodeMetrics.DiskPercent,
        },
        Containers: containers,
    }, nil
}
```

- [ ] **Step 4: Update `routes/grpc.go` to pass the action**

```go
nodeMetrics := metrics.NewCollectNodeMetricsAction()
agentCtrl := controllers.NewAgentController(nodeID, orchestrator, nodeMetrics)
```

Add import for `metrics "github.com/prvious/kontainer/server/app/actions/metrics"`.

- [ ] **Step 5: Run `go vet`, commit**

```
git add server/app/grpc/controllers/ server/routes/grpc.go
git commit -m "refactor: use CollectNodeMetricsAction in gRPC controller, remove duplicate"
```

---

### Task 3: Replace net/http.Post with facades.Http() in exchange_code.go

**Files:**
- Modify: `server/app/actions/setup/exchange_code.go`

- [ ] **Step 1: Check facades.Http() API**

Read the Goravel HTTP client docs or the `goravel-integrations` skill. The key method is `facades.Http().Post(url, ...)`.

- [ ] **Step 2: Replace the http.Post call**

Replace:
```go
resp, err := http.Post(conversionURL, "application/json", nil)
```

With the Goravel HTTP client:
```go
resp, err := facades.Http().Post(conversionURL, nil)
```

Update the response body reading accordingly (Goravel's HTTP response may have different methods than `*http.Response`). Remove the `"net/http"` import.

Note: If facades.Http() returns a different response type, you may need to adjust how the response body is read. Read the Goravel HTTP client contracts to confirm.

- [ ] **Step 3: Run `go vet`, commit**

```
git add server/app/actions/setup/exchange_code.go
git commit -m "refactor: use facades.Http() instead of bare net/http.Post in exchange_code"
```

---

### Task 4: Define interfaces for metrics actions in kernel

**Files:**
- Modify: `server/app/console/kernel.go`

- [ ] **Step 1: Define local interfaces at the top of kernel.go**

```go
type metricsCollector interface {
    Execute(ctx context.Context) error
}

type metricsPruner interface {
    Execute() error
}
```

- [ ] **Step 2: Replace concrete type assertions**

Replace:
```go
collector, ok := collectorInstance.(*metrics.CollectAndStoreAction)
```
with:
```go
collector, ok := collectorInstance.(metricsCollector)
```

And:
```go
pruner, ok := prunerInstance.(*metrics.PruneMetricsAction)
```
with:
```go
pruner, ok := prunerInstance.(metricsPruner)
```

Remove the `metrics` import if no longer needed.

- [ ] **Step 3: Run `go vet`, commit**

```
git add server/app/console/kernel.go
git commit -m "refactor: use interfaces instead of concrete types in kernel assertions"
```

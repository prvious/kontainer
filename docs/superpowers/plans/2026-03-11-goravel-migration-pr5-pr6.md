# Goravel Agent Migration: PR 5 (Docker Orchestrator) + PR 6 (gRPC Internal API)

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a Docker Orchestrator abstraction interface with a Swarm implementation (enabling future Kubernetes support), and replace the HTTP-based leader↔follower internal API with gRPC using protobuf definitions, shared-secret interceptor, and Goravel's gRPC routing.

**Architecture:** The `Orchestrator` interface defines ~30 platform-agnostic operations (list services, scale, exec, etc.). The Swarm implementation wraps the Docker SDK client. Registered as a singleton via `DockerServiceProvider`. The gRPC internal API replaces `/internal/*` HTTP endpoints with a `proto/agent/agent.proto` service. Followers run a gRPC server with a shared-secret unary interceptor. Leaders connect to followers via `facades.Grpc().Connect()`. The Goravel Runner interface handles role-based bootstrap (leader runs HTTP+gRPC client, follower runs gRPC server only).

**Tech Stack:** Goravel framework (service providers, gRPC support, Runner interface), Docker SDK, protobuf/gRPC

**Spec:** `docs/superpowers/specs/2026-03-11-goravel-agent-migration-design.md`

**Goravel patterns used:**
- Service providers: `Register()` for singletons, `Boot()` for wiring, `Runners()` for server lifecycle
- `facades.Grpc().Server()` → gRPC server for route registration
- `facades.Grpc().Connect("name")` → gRPC client connections
- gRPC interceptors registered via `WithGrpcServerInterceptors` in bootstrap
- gRPC controllers implement generated protobuf server interfaces

---

## Chunk 1: PR 5 — Docker Orchestrator Interface

### File Structure

```
server/
├── app/
│   ├── services/docker/
│   │   ├── orchestrator.go                # Orchestrator interface + domain types
│   │   └── swarm/
│   │       ├── client.go                  # Swarm implementation of Orchestrator
│   │       ├── services.go                # Service operations (list, inspect, scale, restart, rollback, update)
│   │       ├── tasks.go                   # Task operations (list, inspect, stats)
│   │       ├── nodes.go                   # Node operations (list, update availability/role)
│   │       ├── networks.go                # Network operations (list)
│   │       ├── volumes.go                 # Volume operations (list)
│   │       ├── stacks.go                  # Stack operations (list, remove)
│   │       ├── secrets.go                 # Secret operations (list, create, remove)
│   │       ├── configs.go                 # Config operations (list, create, remove, inspect)
│   │       ├── images.go                  # Image operations (list, remove, prune)
│   │       ├── logs.go                    # Log streaming (service, task)
│   │       └── exec.go                    # Container exec session
│   └── providers/
│       └── docker_service_provider.go     # Registers Orchestrator singleton
└── tests/
    └── docker/
        └── orchestrator_test.go           # Interface satisfaction test
```

### Mapping from old agent code

| Old (`agent/`) | New (`server/app/services/docker/`) |
|---|---|
| `services/list_services.go` (DockerClient interface) | `orchestrator.go` (Orchestrator interface) |
| `services/*.go` (47 methods on DockerClient) | `swarm/*.go` (methods on SwarmOrchestrator) |
| Direct `docker.Client` usage everywhere | Injected `Orchestrator` interface |
| Manual construction in `main.go` | Singleton via `DockerServiceProvider` |

---

### Task 1: Define the Orchestrator interface and domain types

**Files:**
- Create: `server/app/services/docker/orchestrator.go`

This is the key abstraction. All Docker/Kubernetes-specific types are hidden behind domain types. Controllers and actions depend on the interface, not the implementation.

- [ ] **Step 1: Create the Orchestrator interface with domain types**

Create `server/app/services/docker/orchestrator.go`:

```go
package docker

import (
    "context"
    "io"
    "time"
)

// --- Domain Types ---

type Service struct {
    ID              string
    Name            string
    Image           string
    Stack           string
    ReplicasRunning int
    ReplicasDesired int
    UpdatedAt       string
}

type ServiceDetail struct {
    Service
    Env         []string
    Ports       []PortConfig
    Mounts      []Mount
    Constraints []string
    Labels      map[string]string
    RawSpec     any // For fields not yet abstracted
}

type PortConfig struct {
    Protocol      string
    TargetPort    uint32
    PublishedPort uint32
    PublishMode   string
}

type Mount struct {
    Type   string
    Source string
    Target string
}

type Task struct {
    ID           string
    ServiceID    string
    NodeID       string
    Status       string
    DesiredState string
    Timestamp    string
    ContainerID  string
    Image        string
    Slot         int
    Error        string
}

type TaskDetail struct {
    Task
    ServiceName string
    NodeName    string
}

type ContainerStats struct {
    ContainerID string
    Name        string
    ServiceID   string
    NodeID      string
    CPUPercent  float64
    MemUsage    int64
    MemLimit    int64
    MemPercent  float64
}

type Node struct {
    ID           string
    Hostname     string
    Status       string
    Availability string
    Role         string
    EngineVersion string
    IP           string
}

type Network struct {
    ID        string
    Name      string
    Driver    string
    Scope     string
    Internal  bool
    Attachable bool
}

type Volume struct {
    Name       string
    Driver     string
    Mountpoint string
    Scope      string
    CreatedAt  string
}

type Stack struct {
    Name     string
    Services int
}

type Secret struct {
    ID        string
    Name      string
    CreatedAt string
    UpdatedAt string
}

type Config struct {
    ID        string
    Name      string
    CreatedAt string
    UpdatedAt string
}

type ConfigDetail struct {
    Config
    Data string
}

type Image struct {
    ID       string
    Tags     []string
    Size     int64
    Created  int64
    NodeID   string
    Hostname string
}

type LogStreamOptions struct {
    Follow     bool
    Tail       string
    Since      string
    Timestamps bool
}

type LogStream interface {
    io.ReadCloser
}

type ExecOptions struct {
    Shell string
    User  string
    Cols  uint
    Rows  uint
}

type ExecSession interface {
    io.ReadWriteCloser
    Resize(cols, rows uint) error
}

type ImageRemoveResult struct {
    Deleted  []string
    Untagged []string
}

type ImagePruneResult struct {
    SpaceReclaimed uint64
    ImagesDeleted  int
}

// --- Filter Types (for cross-plan compatibility with PR8-10 controllers) ---

// ServiceFilters controls which services to return.
type ServiceFilters struct {
    Stack string // filter by com.docker.stack.namespace label
}

// TaskFilters controls which tasks to return.
type TaskFilters struct {
    ServiceID string
    NodeID    string
}

// --- Orchestrator Interface ---

type Orchestrator interface {
    // Services
    ListServices(ctx context.Context, filters ServiceFilters) ([]Service, error)
    InspectService(ctx context.Context, id string) (*ServiceDetail, error)
    ScaleService(ctx context.Context, id string, replicas uint64) error
    RestartService(ctx context.Context, id string) error
    RollbackService(ctx context.Context, id string) error
    UpdateServiceEnv(ctx context.Context, id string, env []string) error
    UpdateServiceImage(ctx context.Context, id string, image string) error

    // Tasks
    ListTasks(ctx context.Context, filters TaskFilters) ([]Task, error)
    InspectTask(ctx context.Context, id string) (*TaskDetail, error)
    // TaskStats returns container stats. The ContainerStats type includes ServiceID for correlation with service names.
    TaskStats(ctx context.Context) ([]ContainerStats, error)

    // Nodes
    ListNodes(ctx context.Context) ([]Node, error)
    UpdateNodeAvailability(ctx context.Context, id string, availability string) error
    UpdateNodeRole(ctx context.Context, id string, role string) error

    // Networks & Volumes
    ListNetworks(ctx context.Context) ([]Network, error)
    ListVolumes(ctx context.Context) ([]Volume, error)

    // Stacks
    ListStacks(ctx context.Context) ([]Stack, error)
    RemoveStack(ctx context.Context, name string) error

    // Secrets
    ListSecrets(ctx context.Context) ([]Secret, error)
    CreateSecret(ctx context.Context, name string, data []byte) error
    RemoveSecret(ctx context.Context, id string) error

    // Configs
    ListConfigs(ctx context.Context) ([]Config, error)
    CreateConfig(ctx context.Context, name string, data []byte) error
    RemoveConfig(ctx context.Context, id string) error
    InspectConfig(ctx context.Context, id string) (*ConfigDetail, error)

    // Images (local node only)
    ListImages(ctx context.Context) ([]Image, error)
    RemoveImage(ctx context.Context, id string, force bool) (*ImageRemoveResult, error)
    PruneImages(ctx context.Context) (*ImagePruneResult, error)

    // Logs
    StreamServiceLogs(ctx context.Context, id string, opts LogStreamOptions) (LogStream, error)
    StreamTaskLogs(ctx context.Context, id string, opts LogStreamOptions) (LogStream, error)

    // Exec
    ExecContainer(ctx context.Context, containerID string, opts ExecOptions) (ExecSession, error)
}
```

- [ ] **Step 2: Verify compilation**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go build ./app/services/docker/
```

Expected: Compiles cleanly (interface only, no implementation yet).

- [ ] **Step 3: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/services/docker/orchestrator.go
git commit -m "$(cat <<'EOF'
feat: define Orchestrator interface with platform-agnostic domain types
EOF
)"
```

---

### Task 2: Create Swarm implementation — client scaffold and service operations

**Files:**
- Create: `server/app/services/docker/swarm/client.go`
- Create: `server/app/services/docker/swarm/services.go`

- [ ] **Step 1: Create the Swarm client scaffold**

Create `server/app/services/docker/swarm/client.go`:

```go
package swarm

import (
    "context"
    "io"

    "github.com/docker/docker/api/types"
    "github.com/docker/docker/api/types/container"
    "github.com/docker/docker/api/types/filters"
    "github.com/docker/docker/api/types/image"
    "github.com/docker/docker/api/types/network"
    dockerswarm "github.com/docker/docker/api/types/swarm"
    "github.com/docker/docker/api/types/volume"

    "github.com/prvious/kontainer/server/app/services/docker"
)

// DockerAPI is a narrow interface for the Docker SDK methods we use.
// This makes the Swarm implementation testable with mocks.
type DockerAPI interface {
    ServiceList(ctx context.Context, opts types.ServiceListOptions) ([]dockerswarm.Service, error)
    ServiceInspectWithRaw(ctx context.Context, serviceID string, opts types.ServiceInspectOptions) (dockerswarm.Service, []byte, error)
    ServiceUpdate(ctx context.Context, serviceID string, version dockerswarm.Version, service dockerswarm.ServiceSpec, options dockerswarm.ServiceUpdateOptions) (dockerswarm.ServiceUpdateResponse, error)
    ServiceRemove(ctx context.Context, serviceID string) error
    ServiceLogs(ctx context.Context, serviceID string, opts container.LogsOptions) (io.ReadCloser, error)
    TaskList(ctx context.Context, opts types.TaskListOptions) ([]dockerswarm.Task, error)
    TaskInspectWithRaw(ctx context.Context, taskID string) (dockerswarm.Task, []byte, error)
    TaskLogs(ctx context.Context, taskID string, opts container.LogsOptions) (io.ReadCloser, error)
    NodeList(ctx context.Context, opts types.NodeListOptions) ([]dockerswarm.Node, error)
    NodeInspectWithRaw(ctx context.Context, nodeID string) (dockerswarm.Node, []byte, error)
    NodeUpdate(ctx context.Context, nodeID string, version dockerswarm.Version, node dockerswarm.NodeSpec) error
    NetworkList(ctx context.Context, opts network.ListOptions) ([]network.Summary, error)
    VolumeList(ctx context.Context, opts volume.ListOptions) (volume.ListResponse, error)
    SecretList(ctx context.Context, opts dockerswarm.SecretListOptions) ([]dockerswarm.Secret, error)
    SecretCreate(ctx context.Context, secret dockerswarm.SecretSpec) (dockerswarm.SecretCreateResponse, error)
    SecretRemove(ctx context.Context, id string) error
    ConfigList(ctx context.Context, opts dockerswarm.ConfigListOptions) ([]dockerswarm.Config, error)
    ConfigCreate(ctx context.Context, config dockerswarm.ConfigSpec) (dockerswarm.ConfigCreateResponse, error)
    ConfigInspectWithRaw(ctx context.Context, id string) (dockerswarm.Config, []byte, error)
    ConfigRemove(ctx context.Context, id string) error
    ImageList(ctx context.Context, opts image.ListOptions) ([]image.Summary, error)
    ImageRemove(ctx context.Context, imageID string, opts image.RemoveOptions) ([]image.DeleteResponse, error)
    ImagesPrune(ctx context.Context, pruneFilters filters.Args) (image.PruneReport, error)
    // Docker SDK exec types (github.com/docker/docker/api/types/container):
    //   ContainerExecCreate takes container.ExecOptions, returns types.IDResponse
    //   ContainerExecAttach takes container.ExecStartOptions (NOT ExecAttachOptions), returns types.HijackedResponse
    // Note: older SDK versions used types.ExecConfig / types.ExecStartCheck — those are deprecated.
    ContainerExecCreate(ctx context.Context, container string, config container.ExecOptions) (types.IDResponse, error)
    ContainerExecAttach(ctx context.Context, execID string, config container.ExecStartOptions) (types.HijackedResponse, error)
    ContainerExecResize(ctx context.Context, execID string, options container.ResizeOptions) error
    ContainerExecInspect(ctx context.Context, execID string) (container.ExecInspect, error)
    ContainerStats(ctx context.Context, containerID string, stream bool) (io.ReadCloser, error)
    Info(ctx context.Context) (types.Info, error)
}

// SwarmOrchestrator implements the Orchestrator interface for Docker Swarm.
type SwarmOrchestrator struct {
    client   DockerAPI
    nodeID   string
    hostname string
}

func NewSwarmOrchestrator(client DockerAPI, nodeID, hostname string) *SwarmOrchestrator {
    return &SwarmOrchestrator{
        client:   client,
        nodeID:   nodeID,
        hostname: hostname,
    }
}

// Compile-time check that SwarmOrchestrator implements Orchestrator
var _ docker.Orchestrator = (*SwarmOrchestrator)(nil)
```

- [ ] **Step 2: Create service operations**

Create `server/app/services/docker/swarm/services.go`:

```go
package swarm

import (
    "context"
    "strings"

    "github.com/docker/docker/api/types"
    dfilters "github.com/docker/docker/api/types/filters"
    dockerswarm "github.com/docker/docker/api/types/swarm"

    "github.com/prvious/kontainer/server/app/services/docker"
)

func (s *SwarmOrchestrator) ListServices(ctx context.Context, filters docker.ServiceFilters) ([]docker.Service, error) {
    opts := types.ServiceListOptions{}
    if filters.Stack != "" {
        f := dfilters.NewArgs()
        f.Add("label", "com.docker.stack.namespace="+filters.Stack)
        opts.Filters = f
    }

    svcs, err := s.client.ServiceList(ctx, opts)
    if err != nil {
        return nil, err
    }

    taskFilter := dfilters.NewArgs()
    taskFilter.Add("desired-state", "running")
    tasks, err := s.client.TaskList(ctx, types.TaskListOptions{Filters: taskFilter})
    if err != nil {
        return nil, err
    }

    runningCount := make(map[string]int)
    for _, t := range tasks {
        if t.Status.State == dockerswarm.TaskStateRunning {
            runningCount[t.ServiceID]++
        }
    }

    items := make([]docker.Service, 0, len(svcs))
    for _, svc := range svcs {
        img := svc.Spec.TaskTemplate.ContainerSpec.Image
        if idx := strings.Index(img, "@sha256:"); idx != -1 {
            img = img[:idx]
        }

        var desired int
        if svc.Spec.Mode.Replicated != nil && svc.Spec.Mode.Replicated.Replicas != nil {
            desired = int(*svc.Spec.Mode.Replicated.Replicas)
        } else if svc.Spec.Mode.Global != nil {
            desired = -1
        }

        items = append(items, docker.Service{
            ID:              svc.ID,
            Name:            svc.Spec.Name,
            Image:           img,
            Stack:           svc.Spec.Labels["com.docker.stack.namespace"],
            ReplicasRunning: runningCount[svc.ID],
            ReplicasDesired: desired,
            UpdatedAt:       svc.UpdatedAt.UTC().Format("2006-01-02T15:04:05Z"),
        })
    }

    return items, nil
}

func (s *SwarmOrchestrator) InspectService(ctx context.Context, id string) (*docker.ServiceDetail, error) {
    svc, _, err := s.client.ServiceInspectWithRaw(ctx, id, types.ServiceInspectOptions{})
    if err != nil {
        return nil, err
    }

    img := svc.Spec.TaskTemplate.ContainerSpec.Image
    if idx := strings.Index(img, "@sha256:"); idx != -1 {
        img = img[:idx]
    }

    var desired int
    if svc.Spec.Mode.Replicated != nil && svc.Spec.Mode.Replicated.Replicas != nil {
        desired = int(*svc.Spec.Mode.Replicated.Replicas)
    }

    // Count running tasks
    taskFilter := dfilters.NewArgs()
    taskFilter.Add("service", id)
    taskFilter.Add("desired-state", "running")
    tasks, _ := s.client.TaskList(ctx, types.TaskListOptions{Filters: taskFilter})
    running := 0
    for _, t := range tasks {
        if t.Status.State == dockerswarm.TaskStateRunning {
            running++
        }
    }

    detail := &docker.ServiceDetail{
        Service: docker.Service{
            ID:              svc.ID,
            Name:            svc.Spec.Name,
            Image:           img,
            Stack:           svc.Spec.Labels["com.docker.stack.namespace"],
            ReplicasRunning: running,
            ReplicasDesired: desired,
            UpdatedAt:       svc.UpdatedAt.UTC().Format("2006-01-02T15:04:05Z"),
        },
        Env:    svc.Spec.TaskTemplate.ContainerSpec.Env,
        Labels: svc.Spec.Labels,
    }

    for _, p := range svc.Endpoint.Ports {
        detail.Ports = append(detail.Ports, docker.PortConfig{
            Protocol:      string(p.Protocol),
            TargetPort:    p.TargetPort,
            PublishedPort: p.PublishedPort,
            PublishMode:   string(p.PublishMode),
        })
    }

    for _, m := range svc.Spec.TaskTemplate.ContainerSpec.Mounts {
        detail.Mounts = append(detail.Mounts, docker.Mount{
            Type:   string(m.Type),
            Source: m.Source,
            Target: m.Target,
        })
    }

    if svc.Spec.TaskTemplate.Placement != nil {
        detail.Constraints = svc.Spec.TaskTemplate.Placement.Constraints
    }

    return detail, nil
}

func (s *SwarmOrchestrator) ScaleService(ctx context.Context, id string, replicas uint64) error {
    svc, _, err := s.client.ServiceInspectWithRaw(ctx, id, types.ServiceInspectOptions{})
    if err != nil {
        return err
    }

    if svc.Spec.Mode.Global != nil {
        return docker.ErrGlobalMode
    }

    svc.Spec.Mode.Replicated.Replicas = &replicas
    _, err = s.client.ServiceUpdate(ctx, id, svc.Version, svc.Spec, dockerswarm.ServiceUpdateOptions{})
    return err
}

func (s *SwarmOrchestrator) RestartService(ctx context.Context, id string) error {
    svc, _, err := s.client.ServiceInspectWithRaw(ctx, id, types.ServiceInspectOptions{})
    if err != nil {
        return err
    }

    svc.Spec.TaskTemplate.ForceUpdate++
    _, err = s.client.ServiceUpdate(ctx, id, svc.Version, svc.Spec, dockerswarm.ServiceUpdateOptions{})
    return err
}

func (s *SwarmOrchestrator) RollbackService(ctx context.Context, id string) error {
    svc, _, err := s.client.ServiceInspectWithRaw(ctx, id, types.ServiceInspectOptions{})
    if err != nil {
        return err
    }

    _, err = s.client.ServiceUpdate(ctx, id, svc.Version, svc.Spec, dockerswarm.ServiceUpdateOptions{
        Rollback: "previous",
    })
    return err
}

func (s *SwarmOrchestrator) UpdateServiceEnv(ctx context.Context, id string, env []string) error {
    svc, _, err := s.client.ServiceInspectWithRaw(ctx, id, types.ServiceInspectOptions{})
    if err != nil {
        return err
    }

    svc.Spec.TaskTemplate.ContainerSpec.Env = env
    _, err = s.client.ServiceUpdate(ctx, id, svc.Version, svc.Spec, dockerswarm.ServiceUpdateOptions{})
    return err
}

func (s *SwarmOrchestrator) UpdateServiceImage(ctx context.Context, id string, image string) error {
    svc, _, err := s.client.ServiceInspectWithRaw(ctx, id, types.ServiceInspectOptions{})
    if err != nil {
        return err
    }

    svc.Spec.TaskTemplate.ContainerSpec.Image = image
    _, err = s.client.ServiceUpdate(ctx, id, svc.Version, svc.Spec, dockerswarm.ServiceUpdateOptions{})
    return err
}
```

- [ ] **Step 3: Add ErrGlobalMode to orchestrator.go**

Add to `server/app/services/docker/orchestrator.go`:

```go
import "errors"

var ErrGlobalMode = errors.New("cannot scale a global service")
```

- [ ] **Step 4: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/services/docker/
git commit -m "$(cat <<'EOF'
feat: add Orchestrator interface and Swarm service operations
EOF
)"
```

---

### Task 3: Create remaining Swarm operations

**Files:**
- Create: `server/app/services/docker/swarm/tasks.go`
- Create: `server/app/services/docker/swarm/nodes.go`
- Create: `server/app/services/docker/swarm/networks.go`
- Create: `server/app/services/docker/swarm/volumes.go`
- Create: `server/app/services/docker/swarm/stacks.go`
- Create: `server/app/services/docker/swarm/secrets.go`
- Create: `server/app/services/docker/swarm/configs.go`
- Create: `server/app/services/docker/swarm/images.go`
- Create: `server/app/services/docker/swarm/logs.go`
- Create: `server/app/services/docker/swarm/exec.go`

Each file is a direct port from the corresponding `agent/services/*.go` action, but returns domain types instead of writing HTTP responses. The implementations follow the same Docker SDK calls as the current agent.

- [ ] **Step 1: Create tasks.go**

Port logic from `agent/services/list_tasks.go`, `list_service_tasks.go`, `list_node_tasks.go`, `inspect_task.go`, `list_task_stats.go`. Map Docker `swarm.Task` → `docker.Task` domain type. Note: `ListServiceTasks` and `ListNodeTasks` are subsumed by `TaskFilters` — use `ListTasks(ctx, TaskFilters{ServiceID: id})` or `ListTasks(ctx, TaskFilters{NodeID: id})` instead.

Key method signatures:
```go
func (s *SwarmOrchestrator) ListTasks(ctx context.Context, filters docker.TaskFilters) ([]docker.Task, error)
func (s *SwarmOrchestrator) InspectTask(ctx context.Context, id string) (*docker.TaskDetail, error)
func (s *SwarmOrchestrator) TaskStats(ctx context.Context) ([]docker.ContainerStats, error)
```

The `ListTasks` implementation should apply `filters.ServiceID` and `filters.NodeID` as Docker API filters when non-empty.

- [ ] **Step 2: Create nodes.go**

Port from `agent/services/list_nodes.go`, `update_node_availability.go`, `update_node_role.go`.

```go
func (s *SwarmOrchestrator) ListNodes(ctx context.Context) ([]docker.Node, error)
func (s *SwarmOrchestrator) UpdateNodeAvailability(ctx context.Context, id string, availability string) error
func (s *SwarmOrchestrator) UpdateNodeRole(ctx context.Context, id string, role string) error
```

- [ ] **Step 3: Create networks.go, volumes.go, stacks.go**

Port from `agent/services/list_networks.go`, `list_volumes.go`, `list_stacks.go`, `remove_stack.go`.

- [ ] **Step 4: Create secrets.go, configs.go**

Port from `agent/services/list_secrets.go`, `create_secret.go`, `remove_secret.go`, `list_configs.go`, `create_config.go`, `remove_config.go`, `inspect_config.go`.

- [ ] **Step 5: Create images.go**

Port from `agent/images/list_images.go`. Note: `ListImages`, `RemoveImage`, `PruneImages` are local operations only. Cluster-wide aggregation happens at the controller/action level, not in the orchestrator.

```go
func (s *SwarmOrchestrator) ListImages(ctx context.Context) ([]docker.Image, error)
func (s *SwarmOrchestrator) RemoveImage(ctx context.Context, id string, force bool) (*docker.ImageRemoveResult, error)
func (s *SwarmOrchestrator) PruneImages(ctx context.Context) (*docker.ImagePruneResult, error)
```

- [ ] **Step 6: Create logs.go**

Port log streaming. Returns an `io.ReadCloser` for Docker mux stream parsing.

```go
func (s *SwarmOrchestrator) StreamServiceLogs(ctx context.Context, id string, opts docker.LogStreamOptions) (docker.LogStream, error)
func (s *SwarmOrchestrator) StreamTaskLogs(ctx context.Context, id string, opts docker.LogStreamOptions) (docker.LogStream, error)
```

- [ ] **Step 7: Create exec.go**

Port container exec. Returns an `ExecSession` wrapping Docker's hijacked connection.

```go
func (s *SwarmOrchestrator) ExecContainer(ctx context.Context, containerID string, opts docker.ExecOptions) (docker.ExecSession, error)
```

The `ExecSession` implementation wraps `types.HijackedResponse` with Read/Write/Close/Resize methods.

- [ ] **Step 8: Verify compilation**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go build ./app/services/docker/...
```

Expected: Compiles. The `var _ docker.Orchestrator = (*SwarmOrchestrator)(nil)` check in `client.go` ensures all interface methods are implemented.

- [ ] **Step 9: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/services/docker/swarm/
git commit -m "$(cat <<'EOF'
feat: add complete Swarm implementation of Orchestrator interface
EOF
)"
```

---

### Task 4: Create Docker service provider

**Files:**
- Create: `server/app/providers/docker_service_provider.go`
- Modify: `server/bootstrap/providers.go` (register provider)

- [ ] **Step 1: Create the Docker service provider**

Create `server/app/providers/docker_service_provider.go`:

```go
package providers

import (
    "context"
    "os"

    dockerclient "github.com/docker/docker/client"
    "github.com/goravel/framework/contracts/foundation"
    "github.com/goravel/framework/facades"

    "github.com/prvious/kontainer/server/app/services/docker"
    "github.com/prvious/kontainer/server/app/services/docker/swarm"
)

type DockerServiceProvider struct{}

func (r *DockerServiceProvider) Register(app foundation.Application) {
    // Docker API client (raw SDK)
    app.Singleton("docker.client", func(app foundation.Application) (any, error) {
        return dockerclient.NewClientWithOpts(
            dockerclient.FromEnv,
            dockerclient.WithAPIVersionNegotiation(),
        )
    })

    // Orchestrator interface (Swarm implementation)
    app.Singleton("docker.orchestrator", func(app foundation.Application) (any, error) {
        clientInstance, err := app.Make("docker.client")
        if err != nil {
            return nil, err
        }
        client := clientInstance.(*dockerclient.Client)

        // Detect node identity
        hostname, _ := os.Hostname()
        nodeID := ""
        info, err := client.Info(context.Background())
        if err == nil && info.Swarm.NodeID != "" {
            nodeID = info.Swarm.NodeID
        }

        return swarm.NewSwarmOrchestrator(client, nodeID, hostname), nil
    })
}

func (r *DockerServiceProvider) Boot(app foundation.Application) {
    // No boot actions needed — orchestrator is ready to use
}
```

- [ ] **Step 2: Add Docker SDK dependency**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go get github.com/docker/docker
```

- [ ] **Step 3: Register the provider**

In `server/bootstrap/providers.go`, add:

```go
&providers.DockerServiceProvider{},
```

- [ ] **Step 4: Verify app boots**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go run . artisan about
```

Expected: No errors (Docker client connects to local socket or skips gracefully).

- [ ] **Step 5: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/providers/docker_service_provider.go server/bootstrap/providers.go server/go.mod server/go.sum
git commit -m "$(cat <<'EOF'
feat: add Docker service provider registering Orchestrator singleton
EOF
)"
```

---

### Task 5: Interface satisfaction test

**Files:**
- Create: `server/tests/docker/orchestrator_test.go`

- [ ] **Step 1: Create the test**

Create `server/tests/docker/orchestrator_test.go`:

```go
package docker_test

import (
    "testing"

    "github.com/prvious/kontainer/server/app/services/docker"
    "github.com/prvious/kontainer/server/app/services/docker/swarm"
)

func TestSwarmOrchestrator_ImplementsInterface(t *testing.T) {
    // Compile-time check is already in swarm/client.go via:
    //   var _ docker.Orchestrator = (*SwarmOrchestrator)(nil)
    // This test exists as documentation and for future mock implementations.
    var _ docker.Orchestrator = (*swarm.SwarmOrchestrator)(nil)
    t.Log("SwarmOrchestrator satisfies Orchestrator interface")
}
```

- [ ] **Step 2: Run tests**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go test ./... -v
```

Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/tests/docker/
git commit -m "$(cat <<'EOF'
test: verify SwarmOrchestrator satisfies Orchestrator interface
EOF
)"
```

---

**PR 5 Complete.** Create PR:

```bash
git push -u origin feat/goravel-orchestrator
gh pr create --title "feat: add Docker Orchestrator interface with Swarm implementation" --body "$(cat <<'EOF'
## Summary
- `Orchestrator` interface with ~30 platform-agnostic methods and domain types
- `SwarmOrchestrator` implementation wrapping Docker SDK client
- `DockerServiceProvider` registers Orchestrator as singleton
- All current Docker operations ported (services, tasks, nodes, networks, volumes, stacks, secrets, configs, images, logs, exec)
- Future Kubernetes implementation would add `kubernetes/` alongside `swarm/`

## Test plan
- [ ] `go build ./...` — compiles cleanly
- [ ] `go test ./... -v` — all tests pass
- [ ] Interface satisfaction verified at compile time
EOF
)"
```

---

## Chunk 2: PR 6 — gRPC Internal API

### File Structure

```
server/
├── proto/agent/
│   ├── agent.proto                                # Protobuf service definition
│   └── agent_grpc.pb.go                           # Generated (do not edit)
│   └── agent.pb.go                                # Generated (do not edit)
├── app/
│   ├── grpc/
│   │   ├── controllers/
│   │   │   ├── health_controller.go               # Health + Info RPC
│   │   │   ├── metrics_controller.go              # Metrics RPC
│   │   │   ├── images_controller.go               # Images CRUD RPCs
│   │   │   └── exec_controller.go                 # Bidirectional exec stream
│   │   └── interceptors/
│   │       └── secret_auth.go                     # Shared secret validation
│   └── providers/
│       └── grpc_service_provider.go               # gRPC server + client setup
├── config/
│   └── grpc.go                                    # Modify: gRPC port, interceptors
├── routes/
│   └── grpc.go                                    # gRPC service registration
└── bootstrap/
    └── app.go                                     # Modify: add gRPC interceptors
```

### Mapping from old agent code

| Old (`agent/`) | New (`server/`) |
|---|---|
| `server/follower.go` `/internal/health` | `grpc/controllers/health_controller.go` Health RPC |
| `server/follower.go` `/internal/info` | `grpc/controllers/health_controller.go` Info RPC |
| `server/follower.go` `/internal/metrics` | `grpc/controllers/metrics_controller.go` Metrics RPC |
| `server/follower.go` `/internal/images` | `grpc/controllers/images_controller.go` ListImages RPC |
| `images/handler.go` internal remove/prune | `grpc/controllers/images_controller.go` Remove/Prune RPCs |
| `services/exec_container.go` follower WS | `grpc/controllers/exec_controller.go` Exec bidirectional stream |
| `cluster/middleware.go` X-Kontainer-Secret | `grpc/interceptors/secret_auth.go` gRPC metadata validation |

---

### Task 6: Define protobuf service

**Files:**
- Create: `server/proto/agent/agent.proto`

- [ ] **Step 1: Create the proto definition**

Create `server/proto/agent/agent.proto`:

```protobuf
syntax = "proto3";
package agent;
option go_package = "github.com/prvious/kontainer/server/proto/agent";

service AgentService {
    rpc Health(HealthRequest) returns (HealthResponse);
    rpc Info(InfoRequest) returns (InfoResponse);
    rpc Metrics(MetricsRequest) returns (MetricsResponse);
    rpc ListImages(ListImagesRequest) returns (ListImagesResponse);
    rpc RemoveImage(RemoveImageRequest) returns (RemoveImageResponse);
    rpc PruneImages(PruneImagesRequest) returns (PruneImagesResponse);
    rpc Exec(stream ExecMessage) returns (stream ExecMessage);
}

// --- Health ---
message HealthRequest {}
message HealthResponse {
    string status = 1;
    string node_id = 2;
    string hostname = 3;
}

// --- Info ---
message InfoRequest {}
message InfoResponse {
    string node_id = 1;
    string hostname = 2;
    string role = 3;
}

// --- Metrics ---
message MetricsRequest {}
message MetricsResponse {
    string timestamp = 1;
    string node_id = 2;
    string hostname = 3;
    NodeMetrics node = 4;
    repeated ContainerMetrics containers = 5;
}

message NodeMetrics {
    int32 cpus = 1;
    double cpu_percent = 2;
    int64 memory_used = 3;
    int64 memory_total = 4;
    double memory_percent = 5;
    int64 disk_used = 6;
    int64 disk_total = 7;
    double disk_percent = 8;
}

message ContainerMetrics {
    string container_id = 1;
    string name = 2;
    string service_id = 3;
    double cpu_percent = 4;
    int64 memory_usage = 5;
    int64 memory_limit = 6;
    double memory_percent = 7;
}

// --- Images ---
message ListImagesRequest {}
message ListImagesResponse {
    string node_id = 1;
    string hostname = 2;
    repeated ImageItem images = 3;
}

message ImageItem {
    string id = 1;
    repeated string tags = 2;
    int64 size = 3;
    int64 created = 4;
}

message RemoveImageRequest {
    string image_id = 1;
    bool force = 2;
}

message RemoveImageResponse {
    repeated string deleted = 1;
    repeated string untagged = 2;
}

message PruneImagesRequest {}
message PruneImagesResponse {
    uint64 space_reclaimed = 1;
    int32 images_deleted = 2;
}

// --- Exec (bidirectional stream) ---
// Uses flat enum-based message format (matches PR11 expectations) instead of oneof.
message ExecMessage {
  enum Type {
    INIT = 0;
    DATA = 1;
    RESIZE = 2;
    EXIT = 3;
    ERROR = 4;
  }
  Type type = 1;
  bytes data = 2;      // for DATA type
  uint32 cols = 3;     // for RESIZE type
  uint32 rows = 4;     // for RESIZE type
  int32 exit_code = 5; // for EXIT type
  string error = 6;    // for ERROR type
  string shell = 7;    // for INIT type
}
```

- [ ] **Step 2: Install protobuf tools**

```bash
# Install protoc-gen-go and protoc-gen-go-grpc
go install google.golang.org/protobuf/cmd/protoc-gen-go@latest
go install google.golang.org/grpc/cmd/protoc-gen-go-grpc@latest
```

- [ ] **Step 3: Generate Go code**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
protoc --go_out=. --go_opt=paths=source_relative \
       --go-grpc_out=. --go-grpc_opt=paths=source_relative \
       proto/agent/agent.proto
```

Expected: Creates `proto/agent/agent.pb.go` and `proto/agent/agent_grpc.pb.go`.

- [ ] **Step 4: Add gRPC dependencies**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go get google.golang.org/grpc
go get google.golang.org/protobuf
```

- [ ] **Step 5: Verify generated code compiles**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go build ./proto/agent/
```

- [ ] **Step 6: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/proto/ server/go.mod server/go.sum
git commit -m "$(cat <<'EOF'
feat: define gRPC AgentService proto with health, metrics, images, exec
EOF
)"
```

---

### Task 7: Create shared secret interceptor

**Files:**
- Create: `server/app/grpc/interceptors/secret_auth.go`

Replaces `agent/cluster/middleware.go` (`InternalAuth`) for gRPC.

- [ ] **Step 1: Create the interceptor**

Create `server/app/grpc/interceptors/secret_auth.go`:

```go
package interceptors

import (
    "context"

    "github.com/goravel/framework/facades"
    "google.golang.org/grpc"
    "google.golang.org/grpc/codes"
    "google.golang.org/grpc/metadata"
    "google.golang.org/grpc/status"
)

const SecretMetadataKey = "x-kontainer-secret"

// SecretAuth validates the shared secret from gRPC metadata.
// Used as a unary server interceptor on follower gRPC services.
func SecretAuth(ctx context.Context, req any, info *grpc.UnaryServerInfo, handler grpc.UnaryHandler) (any, error) {
    secret := facades.Config().GetString("cluster.secret")
    if secret == "" {
        return nil, status.Error(codes.Internal, "cluster secret not configured")
    }

    md, ok := metadata.FromIncomingContext(ctx)
    if !ok {
        return nil, status.Error(codes.Unauthenticated, "missing metadata")
    }

    values := md.Get(SecretMetadataKey)
    if len(values) == 0 || values[0] != secret {
        return nil, status.Error(codes.Unauthenticated, "invalid secret")
    }

    return handler(ctx, req)
}

// SecretAuthStream validates the shared secret for streaming RPCs (e.g., Exec).
func SecretAuthStream(srv any, ss grpc.ServerStream, info *grpc.StreamServerInfo, handler grpc.StreamHandler) error {
    secret := facades.Config().GetString("cluster.secret")
    if secret == "" {
        return status.Error(codes.Internal, "cluster secret not configured")
    }

    md, ok := metadata.FromIncomingContext(ss.Context())
    if !ok {
        return status.Error(codes.Unauthenticated, "missing metadata")
    }

    values := md.Get(SecretMetadataKey)
    if len(values) == 0 || values[0] != secret {
        return status.Error(codes.Unauthenticated, "invalid secret")
    }

    return handler(srv, ss)
}
```

- [ ] **Step 2: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/grpc/interceptors/
git commit -m "$(cat <<'EOF'
feat: add gRPC shared secret interceptor for follower auth
EOF
)"
```

---

### Task 8: Create gRPC controllers

**Files:**
- Create: `server/app/grpc/controllers/health_controller.go`
- Create: `server/app/grpc/controllers/metrics_controller.go`
- Create: `server/app/grpc/controllers/images_controller.go`
- Create: `server/app/grpc/controllers/exec_controller.go`

gRPC controllers implement the generated `AgentServiceServer` interface. They call the `Orchestrator` for Docker operations.

- [ ] **Step 1: Create health controller**

Create `server/app/grpc/controllers/health_controller.go`:

```go
package controllers

import (
    "context"
    "os"

    "github.com/goravel/framework/facades"

    pb "github.com/prvious/kontainer/server/proto/agent"
)

type AgentController struct {
    pb.UnimplementedAgentServiceServer
    nodeID string
}

func NewAgentController(nodeID string) *AgentController {
    return &AgentController{nodeID: nodeID}
}

func (c *AgentController) Health(ctx context.Context, req *pb.HealthRequest) (*pb.HealthResponse, error) {
    hostname, _ := os.Hostname()
    return &pb.HealthResponse{
        Status:   "ok",
        NodeId:   c.nodeID,
        Hostname: hostname,
    }, nil
}

func (c *AgentController) Info(ctx context.Context, req *pb.InfoRequest) (*pb.InfoResponse, error) {
    hostname, _ := os.Hostname()
    role := facades.Config().GetString("cluster.role", "follower")
    return &pb.InfoResponse{
        NodeId:   c.nodeID,
        Hostname: hostname,
        Role:     role,
    }, nil
}
```

**IMPORTANT: NodeID must NOT be empty.** The `AgentController` must receive `nodeID` via constructor injection (e.g., `NewAgentController(nodeID string)`). The NodeID should be populated from the Docker SDK (`client.Info(ctx).Swarm.NodeID`) during `DockerServiceProvider.Register()`, or from a config value (`cluster.node_id`) as a fallback. Returning an empty NodeID will break leader-side discovery and registry correlation. Wire this in Task 9 when constructing the controller from the service provider.

- [ ] **Step 2: Create metrics controller**

Create `server/app/grpc/controllers/metrics_controller.go`. Calls `gopsutil` for node metrics and Docker stats API for container metrics — same logic as `agent/metrics/handler.go`:

```go
func (c *AgentController) Metrics(ctx context.Context, req *pb.MetricsRequest) (*pb.MetricsResponse, error) {
    // Collect node metrics via gopsutil
    // Collect container metrics via Docker stats API
    // Return as protobuf MetricsResponse
}
```

Port the `CollectNodeMetricsAction` and `CollectContainerMetricsAction` logic from `agent/metrics/`.

- [ ] **Step 3: Create images controller**

Create `server/app/grpc/controllers/images_controller.go`. Calls `Orchestrator.ListImages()`, `RemoveImage()`, `PruneImages()`:

```go
func (c *AgentController) ListImages(ctx context.Context, req *pb.ListImagesRequest) (*pb.ListImagesResponse, error) {
    images, err := c.orchestrator.ListImages(ctx)
    // Map docker.Image → pb.ImageItem
}

func (c *AgentController) RemoveImage(ctx context.Context, req *pb.RemoveImageRequest) (*pb.RemoveImageResponse, error) {
    result, err := c.orchestrator.RemoveImage(ctx, req.ImageId, req.Force)
    // Map docker.ImageRemoveResult → pb.RemoveImageResponse
}

func (c *AgentController) PruneImages(ctx context.Context, req *pb.PruneImagesRequest) (*pb.PruneImagesResponse, error) {
    result, err := c.orchestrator.PruneImages(ctx)
    // Map docker.ImagePruneResult → pb.PruneImagesResponse
}
```

- [ ] **Step 4: Create exec controller (bidirectional stream)**

Create `server/app/grpc/controllers/exec_controller.go`. This is the most complex RPC — it wraps Docker exec attach as a bidirectional gRPC stream:

```go
func (c *AgentController) Exec(stream pb.AgentService_ExecServer) error {
    // 1. Receive ExecStart message from leader
    // 2. Create Docker exec session via Orchestrator.ExecContainer()
    // 3. Relay data bidirectionally:
    //    - gRPC stream → Docker exec stdin
    //    - Docker exec stdout → gRPC stream
    //    - Handle resize messages
    //    - Send ExecExit on completion
}
```

Port the `runExecSession` logic from `agent/services/exec_container.go`, adapting from WebSocket to gRPC stream framing.

- [ ] **Step 5: Verify compilation**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go build ./app/grpc/...
```

- [ ] **Step 6: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/app/grpc/controllers/
git commit -m "$(cat <<'EOF'
feat: add gRPC controllers (health, metrics, images, exec)
EOF
)"
```

---

### Task 9: Create gRPC routes and service provider

**Files:**
- Create: `server/routes/grpc.go`
- Create: `server/app/providers/grpc_service_provider.go`
- Modify: `server/bootstrap/app.go` (add gRPC interceptors, WithRouting)
- Modify: `server/config/grpc.go` (port config)

- [ ] **Step 1: Create gRPC routes**

Create `server/routes/grpc.go`:

```go
package routes

import (
    "github.com/goravel/framework/facades"

    "github.com/prvious/kontainer/server/app/grpc/controllers"
    pb "github.com/prvious/kontainer/server/proto/agent"
)

func Grpc() {
    // nodeID must be resolved from DockerServiceProvider or config.
    // Retrieve from app container: app.Make("docker.node_id") or facades.Config().
    nodeID := "" // TODO: resolve from service provider — see Task 9
    agentCtrl := controllers.NewAgentController(nodeID)
    pb.RegisterAgentServiceServer(facades.Grpc().Server(), agentCtrl)
}
```

- [ ] **Step 2: Register gRPC interceptors in bootstrap**

In `server/bootstrap/app.go`, add the shared secret interceptor:

```go
import (
    "github.com/prvious/kontainer/server/app/grpc/interceptors"
    "google.golang.org/grpc"
)

// In the Setup chain:
foundation.Setup().
    WithGrpcServerInterceptors([]grpc.UnaryServerInterceptor{
        interceptors.SecretAuth,
    }).
    WithGrpcServerStreamInterceptors([]grpc.StreamServerInterceptor{
        interceptors.SecretAuthStream,
    })
```

**Confirmed:** Per Goravel gRPC docs, `facades.Grpc().Server()` IS correct for route registration (`proto.RegisterXxxServer(facades.Grpc().Server(), controller)` in `routes/grpc.go`), `WithGrpcServerInterceptors` IS a valid bootstrap function for unary interceptors, and `WithGrpcClientInterceptors` is available for client-side interceptors. Routes are registered in `bootstrap/app.go::WithRouting`.

- [ ] **Step 3: Update gRPC config**

Ensure `server/config/grpc.go` has the correct port:

```go
config.Add("grpc", map[string]any{
    "host": "",
    "port": config.Env("GRPC_PORT", "9000"),
})
```

- [ ] **Step 4: Register gRPC routes in bootstrap**

In `bootstrap/app.go`, add the gRPC route registration:

```go
WithRouting(func() {
    routes.Auth()
    routes.Api()
    routes.Setup()
    routes.Grpc()
})
```

- [ ] **Step 5: Add gopsutil dependency**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go get github.com/shirou/gopsutil/v4
```

- [ ] **Step 6: Commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add server/routes/grpc.go server/app/providers/ server/bootstrap/ server/config/grpc.go server/go.mod server/go.sum
git commit -m "$(cat <<'EOF'
feat: register gRPC routes with shared secret interceptor
EOF
)"
```

---

### Task 10: End-to-end verification for PR 6

**Files:**
- No new files

- [ ] **Step 1: Full compilation**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go build ./...
```

- [ ] **Step 2: Run all tests**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
go test ./... -v
```

- [ ] **Step 3: Verify gRPC server starts (follower mode)**

```bash
cd /Users/clovismuneza/Apps/kontainer/server
AGENT_ROLE=follower CLUSTER_SECRET=test-secret GRPC_PORT=9000 go run . &
sleep 2

# Test with grpcurl if available
grpcurl -plaintext -d '{}' -H 'x-kontainer-secret: test-secret' localhost:9000 agent.AgentService/Health

kill %1
```

Expected: Returns health response with status "ok".

- [ ] **Step 4: Final commit**

```bash
cd /Users/clovismuneza/Apps/kontainer
git add -A server/
git commit -m "$(cat <<'EOF'
chore: verify gRPC internal API compiles and boots
EOF
)"
```

---

**PR 6 Complete.** Create PR:

```bash
git push -u origin feat/goravel-grpc
gh pr create --title "feat: add gRPC internal API replacing HTTP follower endpoints" --body "$(cat <<'EOF'
## Summary
- Protobuf service definition: Health, Info, Metrics, ListImages, RemoveImage, PruneImages, Exec (bidirectional stream)
- gRPC controllers implementing AgentServiceServer interface
- Shared secret interceptor for both unary and streaming RPCs
- gRPC routes registered via Goravel's facades.Grpc().Server()
- Replaces HTTP /internal/* endpoints used for leader↔follower communication

## Test plan
- [ ] `go build ./...` — compiles cleanly
- [ ] `go test ./... -v` — all tests pass
- [ ] gRPC server starts in follower mode on port 9000
- [ ] `grpcurl` Health RPC returns status "ok" with valid secret
- [ ] `grpcurl` Health RPC returns Unauthenticated with invalid/missing secret
EOF
)"
```

---

## Subsequent Plans

The remaining PRs will be planned in separate documents:
- `2026-03-11-goravel-migration-pr7.md` — Cluster management (registry, discovery, gRPC client pool)
- `2026-03-11-goravel-migration-pr8-pr10.md` — Controllers (services, tasks, nodes, stacks, networks, volumes, secrets, configs)
- `2026-03-11-goravel-migration-pr11.md` — WebSocket endpoints (logs, exec)
- `2026-03-11-goravel-migration-pr12-pr13.md` — Metrics + Images + Settings
- `2026-03-11-goravel-migration-pr14-pr15.md` — Cleanup + Frontend adaptation

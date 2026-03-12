# Goravel Agent Migration: PR 8 (Services & Tasks) + PR 9 (Nodes, Stacks, Networks, Volumes) + PR 10 (Secrets & Configs)

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port all REST API controllers from Chi handlers to Goravel controllers + actions. Each endpoint follows the pattern: thin controller (parse request, call action, return response) + action (business logic, depends on Orchestrator interface). Uses Goravel request validation where applicable.

**Architecture:** Every endpoint is one controller + one action, each in its own file. Controllers receive `http.Context`, call the action's `Execute()`, and return `ctx.Response().Json()`. Actions depend on the `Orchestrator` interface (PR 5) — no direct Docker SDK imports in controllers or actions. Route params via `ctx.Request().Route("param")`, query params via `ctx.Request().Query("param")`, body via `ctx.Request().Bind(&struct)`.

**Tech Stack:** Goravel framework (controllers, request, response, validation, routing)

**Spec:** `docs/superpowers/specs/2026-03-11-goravel-agent-migration-design.md`

**Prerequisites:** PR 5 (Orchestrator interface + Swarm implementation)

**Goravel patterns used:**
- Controllers: `Handle(ctx http.Context) http.Response`
- Route params: `ctx.Request().Route("serviceID")`
- Query params: `ctx.Request().Query("stack", "")`
- Body binding: `ctx.Request().Bind(&req)`
- Validation: `ctx.Request().Validate(rules, messages)`
- Response: `ctx.Response().Json(200, data)`, `ctx.Response().Json(404, err)`
- Logging: `facades.Log().Errorf()`

---

## Chunk 1: PR 8 — Services & Tasks Controllers

### File Structure

```
server/app/actions/services/
├── list_services.go
├── inspect_service.go
├── scale_service.go
├── restart_service.go
├── rollback_service.go
├── update_service_env.go
└── update_service_image.go

server/app/actions/tasks/
├── list_tasks.go
├── list_service_tasks.go
├── inspect_task.go
└── list_task_stats.go

server/app/http/controllers/services/
├── list_services_controller.go
├── inspect_service_controller.go
├── scale_service_controller.go
├── restart_service_controller.go
├── rollback_service_controller.go
├── update_service_env_controller.go
└── update_service_image_controller.go

server/app/http/controllers/tasks/
├── list_tasks_controller.go
├── list_service_tasks_controller.go
├── inspect_task_controller.go
└── list_task_stats_controller.go
```

### Task 1: List Services

Ports `agent/services/list_services.go`. Lists all services, optionally filtered by stack.

**Files:**
- Create: `server/app/actions/services/list_services.go`
- Create: `server/app/http/controllers/services/list_services_controller.go`

- [ ] **Step 1: Create the action**

```go
package services

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type ListServicesAction struct {
	orchestrator docker.Orchestrator
}

func NewListServicesAction(orchestrator docker.Orchestrator) *ListServicesAction {
	return &ListServicesAction{orchestrator: orchestrator}
}

func (a *ListServicesAction) Execute(ctx context.Context, stack string) ([]docker.Service, error) {
	filters := docker.ServiceFilters{Stack: stack}
	return a.orchestrator.ListServices(ctx, filters)
}
```

- [ ] **Step 2: Create the controller**

```go
package services

import (
	"github.com/goravel/framework/contracts/http"
	"github.com/goravel/framework/facades"
	serviceActions "github.com/prvious/kontainer/server/app/actions/services"
)

type ListServicesController struct {
	action *serviceActions.ListServicesAction
}

func NewListServicesController(action *serviceActions.ListServicesAction) *ListServicesController {
	return &ListServicesController{action: action}
}

func (c *ListServicesController) Handle(ctx http.Context) http.Response {
	stack := ctx.Request().Query("stack", "")

	services, err := c.action.Execute(ctx.Context(), stack)
	if err != nil {
		facades.Log().Errorf("list services error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to list services"})
	}

	return ctx.Response().Json(200, map[string]any{"services": services})
}
```

- [ ] **Step 3: Commit**

```bash
cd server && git add app/actions/services/list_services.go app/http/controllers/services/list_services_controller.go
git commit -m "feat(services): add list services action and controller"
```

### Task 2: Inspect Service

Ports `agent/services/inspect_service.go`. Returns detailed service info including env, mounts, ports, constraints, update status.

**Files:**
- Create: `server/app/actions/services/inspect_service.go`
- Create: `server/app/http/controllers/services/inspect_service_controller.go`

- [ ] **Step 1: Create the action**

```go
package services

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type InspectServiceAction struct {
	orchestrator docker.Orchestrator
}

func NewInspectServiceAction(orchestrator docker.Orchestrator) *InspectServiceAction {
	return &InspectServiceAction{orchestrator: orchestrator}
}

func (a *InspectServiceAction) Execute(ctx context.Context, serviceID string) (*docker.ServiceDetail, error) {
	return a.orchestrator.InspectService(ctx, serviceID)
}
```

- [ ] **Step 2: Create the controller**

```go
package services

import (
	"github.com/goravel/framework/contracts/http"
	"github.com/goravel/framework/facades"
	serviceActions "github.com/prvious/kontainer/server/app/actions/services"
)

type InspectServiceController struct {
	action *serviceActions.InspectServiceAction
}

func NewInspectServiceController(action *serviceActions.InspectServiceAction) *InspectServiceController {
	return &InspectServiceController{action: action}
}

func (c *InspectServiceController) Handle(ctx http.Context) http.Response {
	serviceID := ctx.Request().Route("serviceID")

	detail, err := c.action.Execute(ctx.Context(), serviceID)
	if err != nil {
		facades.Log().Errorf("inspect service error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to inspect service"})
	}

	return ctx.Response().Json(200, detail)
}
```

**Note:** The Orchestrator's `InspectService` should return a not-found sentinel error that the controller can check to return 404. Add error type checking:

```go
if errors.Is(err, docker.ErrNotFound) {
    return ctx.Response().Json(404, map[string]string{"error": "service not found"})
}
```

Define `ErrNotFound` in `server/app/services/docker/errors.go`:

```go
package docker

import "errors"

var ErrNotFound = errors.New("not found")
```

The SwarmOrchestrator wraps Docker SDK's `errdefs.IsNotFound(err)` into `ErrNotFound`.

- [ ] **Step 3: Commit**

```bash
cd server && git add app/actions/services/inspect_service.go app/http/controllers/services/inspect_service_controller.go app/services/docker/errors.go
git commit -m "feat(services): add inspect service action, controller, and ErrNotFound"
```

### Task 3: Scale Service

Ports `agent/services/scale_service.go`. Uses Goravel validation for the replicas field.

**Files:**
- Create: `server/app/actions/services/scale_service.go`
- Create: `server/app/http/controllers/services/scale_service_controller.go`

- [ ] **Step 1: Create the action**

```go
package services

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type ScaleServiceAction struct {
	orchestrator docker.Orchestrator
}

func NewScaleServiceAction(orchestrator docker.Orchestrator) *ScaleServiceAction {
	return &ScaleServiceAction{orchestrator: orchestrator}
}

func (a *ScaleServiceAction) Execute(ctx context.Context, serviceID string, replicas int) error {
	return a.orchestrator.ScaleService(ctx, serviceID, replicas)
}
```

- [ ] **Step 2: Create the controller with validation**

```go
package services

import (
	"errors"

	"github.com/goravel/framework/contracts/http"
	"github.com/goravel/framework/facades"
	serviceActions "github.com/prvious/kontainer/server/app/actions/services"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type ScaleServiceController struct {
	action *serviceActions.ScaleServiceAction
}

func NewScaleServiceController(action *serviceActions.ScaleServiceAction) *ScaleServiceController {
	return &ScaleServiceController{action: action}
}

type scaleRequest struct {
	Replicas *int `form:"replicas" json:"replicas"`
}

func (c *ScaleServiceController) Handle(ctx http.Context) http.Response {
	serviceID := ctx.Request().Route("serviceID")

	var req scaleRequest
	if errs := ctx.Request().Validate(map[string]string{
		"replicas": "required|min_value:0",
	}, nil); errs != nil {
		return ctx.Response().Json(400, map[string]any{"error": errs.All()})
	}

	if err := ctx.Request().Bind(&req); err != nil || req.Replicas == nil {
		return ctx.Response().Json(400, map[string]string{"error": "replicas field is required"})
	}

	err := c.action.Execute(ctx.Context(), serviceID, *req.Replicas)
	if err != nil {
		if errors.Is(err, docker.ErrNotFound) {
			return ctx.Response().Json(404, map[string]string{"error": "service not found"})
		}
		if errors.Is(err, docker.ErrGlobalMode) {
			return ctx.Response().Json(400, map[string]string{"error": "cannot scale a global service"})
		}
		facades.Log().Errorf("scale service error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to scale service"})
	}

	return ctx.Response().Json(200, map[string]any{"replicas": *req.Replicas})
}
```

Add `ErrGlobalMode` to `server/app/services/docker/errors.go`:

```go
var ErrGlobalMode = errors.New("cannot scale a global service")
```

- [ ] **Step 3: Commit**

```bash
cd server && git add app/actions/services/scale_service.go app/http/controllers/services/scale_service_controller.go app/services/docker/errors.go
git commit -m "feat(services): add scale service with validation"
```

### Task 4: Restart Service

Ports `agent/services/restart_service.go`.

**Files:**
- Create: `server/app/actions/services/restart_service.go`
- Create: `server/app/http/controllers/services/restart_service_controller.go`

- [ ] **Step 1: Create action + controller**

Action:
```go
package services

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type RestartServiceAction struct {
	orchestrator docker.Orchestrator
}

func NewRestartServiceAction(orchestrator docker.Orchestrator) *RestartServiceAction {
	return &RestartServiceAction{orchestrator: orchestrator}
}

func (a *RestartServiceAction) Execute(ctx context.Context, serviceID string) error {
	return a.orchestrator.RestartService(ctx, serviceID)
}
```

Controller:
```go
package services

import (
	"errors"

	"github.com/goravel/framework/contracts/http"
	"github.com/goravel/framework/facades"
	serviceActions "github.com/prvious/kontainer/server/app/actions/services"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type RestartServiceController struct {
	action *serviceActions.RestartServiceAction
}

func NewRestartServiceController(action *serviceActions.RestartServiceAction) *RestartServiceController {
	return &RestartServiceController{action: action}
}

func (c *RestartServiceController) Handle(ctx http.Context) http.Response {
	serviceID := ctx.Request().Route("serviceID")

	if err := c.action.Execute(ctx.Context(), serviceID); err != nil {
		if errors.Is(err, docker.ErrNotFound) {
			return ctx.Response().Json(404, map[string]string{"error": "service not found"})
		}
		facades.Log().Errorf("restart service error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to restart service"})
	}

	return ctx.Response().Json(200, map[string]string{"status": "restarting"})
}
```

- [ ] **Step 2: Commit**

```bash
cd server && git add app/actions/services/restart_service.go app/http/controllers/services/restart_service_controller.go
git commit -m "feat(services): add restart service"
```

### Task 5: Rollback Service

Ports `agent/services/rollback_service.go`. Same pattern as restart.

**Files:**
- Create: `server/app/actions/services/rollback_service.go`
- Create: `server/app/http/controllers/services/rollback_service_controller.go`

- [ ] **Step 1: Create action + controller**

Action calls `orchestrator.RollbackService(ctx, serviceID)`.

Controller pattern identical to restart, but with `"status": "rolling_back"` response.

- [ ] **Step 2: Commit**

```bash
cd server && git add app/actions/services/rollback_service.go app/http/controllers/services/rollback_service_controller.go
git commit -m "feat(services): add rollback service"
```

### Task 6: Update Service Env

Ports `agent/services/update_service_env.go`.

**Files:**
- Create: `server/app/actions/services/update_service_env.go`
- Create: `server/app/http/controllers/services/update_service_env_controller.go`

- [ ] **Step 1: Create action + controller**

Action:
```go
package services

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type UpdateServiceEnvAction struct {
	orchestrator docker.Orchestrator
}

func NewUpdateServiceEnvAction(orchestrator docker.Orchestrator) *UpdateServiceEnvAction {
	return &UpdateServiceEnvAction{orchestrator: orchestrator}
}

func (a *UpdateServiceEnvAction) Execute(ctx context.Context, serviceID string, env map[string]string) error {
	return a.orchestrator.UpdateServiceEnv(ctx, serviceID, env)
}
```

Controller:
```go
package services

import (
	"errors"

	"github.com/goravel/framework/contracts/http"
	"github.com/goravel/framework/facades"
	serviceActions "github.com/prvious/kontainer/server/app/actions/services"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type UpdateServiceEnvController struct {
	action *serviceActions.UpdateServiceEnvAction
}

func NewUpdateServiceEnvController(action *serviceActions.UpdateServiceEnvAction) *UpdateServiceEnvController {
	return &UpdateServiceEnvController{action: action}
}

type updateEnvRequest struct {
	Env map[string]string `json:"env"`
}

func (c *UpdateServiceEnvController) Handle(ctx http.Context) http.Response {
	serviceID := ctx.Request().Route("serviceID")

	var req updateEnvRequest
	if err := ctx.Request().Bind(&req); err != nil {
		return ctx.Response().Json(400, map[string]string{"error": "invalid request body"})
	}

	if err := c.action.Execute(ctx.Context(), serviceID, req.Env); err != nil {
		if errors.Is(err, docker.ErrNotFound) {
			return ctx.Response().Json(404, map[string]string{"error": "service not found"})
		}
		facades.Log().Errorf("update service env error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to update service env"})
	}

	return ctx.Response().Json(200, map[string]string{"status": "updated"})
}
```

- [ ] **Step 2: Commit**

```bash
cd server && git add app/actions/services/update_service_env.go app/http/controllers/services/update_service_env_controller.go
git commit -m "feat(services): add update service env"
```

### Task 7: Update Service Image

Ports `agent/services/update_service_image.go`. Same pattern.

**Files:**
- Create: `server/app/actions/services/update_service_image.go`
- Create: `server/app/http/controllers/services/update_service_image_controller.go`

- [ ] **Step 1: Create action + controller**

Action calls `orchestrator.UpdateServiceImage(ctx, serviceID, image)`.

Controller binds `{"image": "..."}` request body, validates image is non-empty.

- [ ] **Step 2: Commit**

```bash
cd server && git add app/actions/services/update_service_image.go app/http/controllers/services/update_service_image_controller.go
git commit -m "feat(services): add update service image"
```

### Task 8: Task Controllers

Ports `agent/services/list_tasks.go`, `list_service_tasks.go`, `inspect_task.go`, `list_task_stats.go`.

**Files:**
- Create: `server/app/actions/tasks/list_tasks.go`
- Create: `server/app/actions/tasks/list_service_tasks.go`
- Create: `server/app/actions/tasks/inspect_task.go`
- Create: `server/app/actions/tasks/list_task_stats.go`
- Create: `server/app/http/controllers/tasks/list_tasks_controller.go`
- Create: `server/app/http/controllers/tasks/list_service_tasks_controller.go`
- Create: `server/app/http/controllers/tasks/inspect_task_controller.go`
- Create: `server/app/http/controllers/tasks/list_task_stats_controller.go`

- [ ] **Step 1: Create list tasks action + controller**

All follow the same thin pattern. Example for list tasks:

Action:
```go
package tasks

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type ListTasksAction struct {
	orchestrator docker.Orchestrator
}

func NewListTasksAction(orchestrator docker.Orchestrator) *ListTasksAction {
	return &ListTasksAction{orchestrator: orchestrator}
}

func (a *ListTasksAction) Execute(ctx context.Context, filters docker.TaskFilters) ([]docker.Task, error) {
	return a.orchestrator.ListTasks(ctx, filters)
}
```

Controller:
```go
package tasks

import (
	"github.com/goravel/framework/contracts/http"
	"github.com/goravel/framework/facades"
	taskActions "github.com/prvious/kontainer/server/app/actions/tasks"
	"github.com/prvious/kontainer/server/app/services/docker"
)

type ListTasksController struct {
	action *taskActions.ListTasksAction
}

func NewListTasksController(action *taskActions.ListTasksAction) *ListTasksController {
	return &ListTasksController{action: action}
}

func (c *ListTasksController) Handle(ctx http.Context) http.Response {
	tasks, err := c.action.Execute(ctx.Context(), docker.TaskFilters{})
	if err != nil {
		facades.Log().Errorf("list tasks error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to list tasks"})
	}

	return ctx.Response().Json(200, map[string]any{"tasks": tasks})
}
```

- [ ] **Step 2: Create list service tasks** — filter by `serviceID` route param
- [ ] **Step 3: Create inspect task** — route param `taskID`, returns `*docker.TaskDetail`
- [ ] **Step 4: Create list task stats** — calls `orchestrator.TaskStats(ctx)`, returns container stats
- [ ] **Step 5: Commit all task controllers**

```bash
cd server && git add app/actions/tasks/ app/http/controllers/tasks/
git commit -m "feat(tasks): add task listing, inspection, and stats controllers"
```

### Task 9: Register Service & Task Routes

**Files:**
- Modify: `server/routes/api.go`

- [ ] **Step 1: Add all service and task routes**

Inside the JWT-protected group in `routes/api.go`:

```go
// Services
router.Get("/services", listServicesCtrl.Handle)
router.Get("/services/{serviceID}", inspectServiceCtrl.Handle)
router.Patch("/services/{serviceID}", scaleServiceCtrl.Handle)
router.Post("/services/{serviceID}/restart", restartServiceCtrl.Handle)
router.Post("/services/{serviceID}/rollback", rollbackServiceCtrl.Handle)
router.Put("/services/{serviceID}/env", updateServiceEnvCtrl.Handle)
router.Put("/services/{serviceID}/image", updateServiceImageCtrl.Handle)
router.Get("/services/{serviceID}/tasks", listServiceTasksCtrl.Handle)

// Tasks
router.Get("/tasks", listTasksCtrl.Handle)
router.Get("/tasks/{taskID}", inspectTaskCtrl.Handle)
router.Get("/task-stats", listTaskStatsCtrl.Handle)
```

Controllers are resolved from the service container or created in a route setup function.

- [ ] **Step 2: Commit**

```bash
cd server && git add routes/api.go
git commit -m "feat(routes): register service and task API routes"
```

---

## Chunk 2: PR 9 — Nodes, Stacks, Networks, Volumes

### File Structure

```
server/app/actions/nodes/
├── list_nodes.go
├── list_node_tasks.go
├── update_node_availability.go
└── update_node_role.go

server/app/actions/stacks/
├── list_stacks.go
└── remove_stack.go

server/app/actions/networks/
└── list_networks.go

server/app/actions/volumes/
└── list_volumes.go

server/app/http/controllers/nodes/
├── list_nodes_controller.go
├── list_node_tasks_controller.go
├── update_node_availability_controller.go
└── update_node_role_controller.go

server/app/http/controllers/stacks/
├── list_stacks_controller.go
└── remove_stack_controller.go

server/app/http/controllers/networks/
└── list_networks_controller.go

server/app/http/controllers/volumes/
└── list_volumes_controller.go
```

### Task 10: Node Controllers

Ports node listing, node task listing, update availability, update role.

- [ ] **Step 1: List Nodes action + controller**

Action calls `orchestrator.ListNodes(ctx)`.

Controller returns `{"nodes": [...]}`.

- [ ] **Step 2: List Node Tasks action + controller**

Action calls `orchestrator.ListTasks(ctx, docker.TaskFilters{NodeID: nodeID})`.

Controller gets `nodeID` from route param.

- [ ] **Step 3: Update Node Availability action + controller**

Action calls `orchestrator.UpdateNodeAvailability(ctx, nodeID, availability)`.

Controller validates `availability` is one of `"active"`, `"drain"`, `"pause"`:

```go
func (c *UpdateNodeAvailabilityController) Handle(ctx http.Context) http.Response {
	nodeID := ctx.Request().Route("nodeID")

	var req struct {
		Availability string `json:"availability"`
	}
	if err := ctx.Request().Bind(&req); err != nil {
		return ctx.Response().Json(400, map[string]string{"error": "invalid request body"})
	}

	switch req.Availability {
	case "active", "drain", "pause":
		// valid
	default:
		return ctx.Response().Json(400, map[string]string{"error": "availability must be active, drain, or pause"})
	}

	if err := c.action.Execute(ctx.Context(), nodeID, req.Availability); err != nil {
		if errors.Is(err, docker.ErrNotFound) {
			return ctx.Response().Json(404, map[string]string{"error": "node not found"})
		}
		facades.Log().Errorf("update node availability error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to update node availability"})
	}

	return ctx.Response().Json(200, map[string]string{"availability": req.Availability})
}
```

- [ ] **Step 4: Update Node Role** — same pattern, validates `"manager"` or `"worker"`
- [ ] **Step 5: Commit**

```bash
cd server && git add app/actions/nodes/ app/http/controllers/nodes/
git commit -m "feat(nodes): add node controllers with availability and role updates"
```

### Task 11: Stack Controllers

Ports `agent/services/list_stacks.go` and `agent/services/remove_stack.go`.

- [ ] **Step 1: List Stacks** — action calls `orchestrator.ListStacks(ctx)`
- [ ] **Step 2: Remove Stack** — action calls `orchestrator.RemoveStack(ctx, stackName)`, controller gets `stackName` from route param
- [ ] **Step 3: Commit**

```bash
cd server && git add app/actions/stacks/ app/http/controllers/stacks/
git commit -m "feat(stacks): add stack listing and removal"
```

### Task 12: Network & Volume Controllers

- [ ] **Step 1: List Networks** — action calls `orchestrator.ListNetworks(ctx)`
- [ ] **Step 2: List Volumes** — action calls `orchestrator.ListVolumes(ctx)`
- [ ] **Step 3: Commit**

```bash
cd server && git add app/actions/networks/ app/actions/volumes/ app/http/controllers/networks/ app/http/controllers/volumes/
git commit -m "feat: add network and volume listing controllers"
```

### Task 13: Register Node, Stack, Network, Volume Routes

- [ ] **Step 1: Add routes to `routes/api.go`**

```go
// Nodes
router.Get("/nodes", listNodesCtrl.Handle)
router.Get("/nodes/{nodeID}/tasks", listNodeTasksCtrl.Handle)
router.Put("/nodes/{nodeID}/availability", updateNodeAvailabilityCtrl.Handle)
router.Put("/nodes/{nodeID}/role", updateNodeRoleCtrl.Handle)

// Stacks
router.Get("/stacks", listStacksCtrl.Handle)
router.Delete("/stacks/{stackName}", removeStackCtrl.Handle)

// Networks
router.Get("/networks", listNetworksCtrl.Handle)

// Volumes
router.Get("/volumes", listVolumesCtrl.Handle)
```

- [ ] **Step 2: Commit**

```bash
cd server && git add routes/api.go
git commit -m "feat(routes): register node, stack, network, volume routes"
```

---

## Chunk 3: PR 10 — Secrets & Configs

### File Structure

```
server/app/actions/secrets/
├── list_secrets.go
├── create_secret.go
└── remove_secret.go

server/app/actions/configs/
├── list_configs.go
├── create_config.go
├── inspect_config.go
└── remove_config.go

server/app/http/controllers/secrets/
├── list_secrets_controller.go
├── create_secret_controller.go
└── remove_secret_controller.go

server/app/http/controllers/configs/
├── list_configs_controller.go
├── create_config_controller.go
├── inspect_config_controller.go
└── remove_config_controller.go
```

### Task 14: Secret Controllers

Ports `agent/services/list_secrets.go`, `create_secret.go`, `remove_secret.go`.

- [ ] **Step 1: List Secrets** — action calls `orchestrator.ListSecrets(ctx)`
- [ ] **Step 2: Create Secret with validation**

Action:
```go
package secrets

import (
	"context"

	"github.com/prvious/kontainer/server/app/services/docker"
)

type CreateSecretAction struct {
	orchestrator docker.Orchestrator
}

func NewCreateSecretAction(orchestrator docker.Orchestrator) *CreateSecretAction {
	return &CreateSecretAction{orchestrator: orchestrator}
}

func (a *CreateSecretAction) Execute(ctx context.Context, name string, data []byte) error {
	return a.orchestrator.CreateSecret(ctx, name, data)
}
```

Controller validates `name` is non-empty:

```go
func (c *CreateSecretController) Handle(ctx http.Context) http.Response {
	var req struct {
		Name string `json:"name"`
		Data string `json:"data"`
	}
	if err := ctx.Request().Bind(&req); err != nil {
		return ctx.Response().Json(400, map[string]string{"error": "invalid request body"})
	}

	if req.Name == "" {
		return ctx.Response().Json(400, map[string]string{"error": "name is required"})
	}

	if err := c.action.Execute(ctx.Context(), req.Name, []byte(req.Data)); err != nil {
		facades.Log().Errorf("create secret error: %v", err)
		return ctx.Response().Json(500, map[string]string{"error": "failed to create secret"})
	}

	return ctx.Response().Json(201, map[string]string{"name": req.Name})
}
```

- [ ] **Step 3: Remove Secret** — route param `secretID`, calls `orchestrator.RemoveSecret(ctx, id)`
- [ ] **Step 4: Commit**

```bash
cd server && git add app/actions/secrets/ app/http/controllers/secrets/
git commit -m "feat(secrets): add secret CRUD controllers"
```

### Task 15: Config Controllers

Ports `agent/services/list_configs.go`, `create_config.go`, `inspect_config.go`, `remove_config.go`.

- [ ] **Step 1: List Configs** — `orchestrator.ListConfigs(ctx)`
- [ ] **Step 2: Create Config** — validates name, calls `orchestrator.CreateConfig(ctx, name, data)`
- [ ] **Step 3: Inspect Config** — route param `configID`, calls `orchestrator.InspectConfig(ctx, id)`
- [ ] **Step 4: Remove Config** — route param `configID`, calls `orchestrator.RemoveConfig(ctx, id)`
- [ ] **Step 5: Commit**

```bash
cd server && git add app/actions/configs/ app/http/controllers/configs/
git commit -m "feat(configs): add config CRUD controllers"
```

### Task 16: Register Secret & Config Routes

- [ ] **Step 1: Add routes**

```go
// Secrets
router.Get("/secrets", listSecretsCtrl.Handle)
router.Post("/secrets", createSecretCtrl.Handle)
router.Delete("/secrets/{secretID}", removeSecretCtrl.Handle)

// Configs
router.Get("/configs", listConfigsCtrl.Handle)
router.Post("/configs", createConfigCtrl.Handle)
router.Get("/configs/{configID}", inspectConfigCtrl.Handle)
router.Delete("/configs/{configID}", removeConfigCtrl.Handle)
```

- [ ] **Step 2: Commit**

```bash
cd server && git add routes/api.go
git commit -m "feat(routes): register secret and config routes"
```

### Task 17: Controller Wiring via Service Provider

Create or update a service provider to wire all controllers with the Orchestrator.

**Files:**
- Create: `server/app/providers/api_service_provider.go`

- [ ] **Step 1: Create API service provider**

The provider resolves the Orchestrator singleton (from DockerServiceProvider, PR 5) and creates all actions + controllers. Binds each controller to the service container for route file resolution.

```go
package providers

import (
	"github.com/goravel/framework/contracts/foundation"
	"github.com/prvious/kontainer/server/app/services/docker"
	// ... import all action and controller packages
)

type ApiServiceProvider struct{}

func (p *ApiServiceProvider) Register(app foundation.Application) {}

func (p *ApiServiceProvider) Boot(app foundation.Application) {
	orchestrator := app.Make("docker.orchestrator").(docker.Orchestrator)

	// Services
	listSvcAction := services.NewListServicesAction(orchestrator)
	app.Bind("controllers.services.list", func(app foundation.Application) (any, error) {
		return servicesCtrl.NewListServicesController(listSvcAction), nil
	})
	// ... repeat for all controllers
}
```

- [ ] **Step 2: Register the provider in `config/app.go`**
- [ ] **Step 3: Commit**

```bash
cd server && git add app/providers/api_service_provider.go config/app.go
git commit -m "feat: add ApiServiceProvider to wire all REST controllers"
```

### Task 18: Build Verification

- [ ] **Step 1: Verify build**

```bash
cd server && go build ./...
```

- [ ] **Step 2: Final commit**

```bash
git add -A && git commit -m "feat: complete PR 8-10 — all REST API controllers"
```

---

## Route Summary

| Method | Path | Controller | Old Handler |
|---|---|---|---|
| GET | /api/services | ListServicesController | ListServicesHandler |
| GET | /api/services/{serviceID} | InspectServiceController | InspectServiceHandler |
| PATCH | /api/services/{serviceID} | ScaleServiceController | ScaleServiceHandler |
| POST | /api/services/{serviceID}/restart | RestartServiceController | RestartServiceHandler |
| POST | /api/services/{serviceID}/rollback | RollbackServiceController | RollbackServiceHandler |
| PUT | /api/services/{serviceID}/env | UpdateServiceEnvController | UpdateServiceEnvHandler |
| PUT | /api/services/{serviceID}/image | UpdateServiceImageController | UpdateServiceImageHandler |
| GET | /api/services/{serviceID}/tasks | ListServiceTasksController | ListServiceTasksHandler |
| GET | /api/tasks | ListTasksController | ListTasksHandler |
| GET | /api/tasks/{taskID} | InspectTaskController | InspectTaskHandler |
| GET | /api/task-stats | ListTaskStatsController | ListTaskStatsHandler |
| GET | /api/nodes | ListNodesController | ListNodesHandler |
| GET | /api/nodes/{nodeID}/tasks | ListNodeTasksController | ListNodeTasksHandler |
| PUT | /api/nodes/{nodeID}/availability | UpdateNodeAvailabilityController | UpdateNodeAvailabilityHandler |
| PUT | /api/nodes/{nodeID}/role | UpdateNodeRoleController | UpdateNodeRoleHandler |
| GET | /api/stacks | ListStacksController | ListStacksHandler |
| DELETE | /api/stacks/{stackName} | RemoveStackController | RemoveStackHandler |
| GET | /api/networks | ListNetworksController | ListNetworksHandler |
| GET | /api/volumes | ListVolumesController | ListVolumesHandler |
| GET | /api/secrets | ListSecretsController | ListSecretsHandler |
| POST | /api/secrets | CreateSecretController | CreateSecretHandler |
| DELETE | /api/secrets/{secretID} | RemoveSecretController | RemoveSecretHandler |
| GET | /api/configs | ListConfigsController | ListConfigsHandler |
| POST | /api/configs | CreateConfigController | CreateConfigHandler |
| GET | /api/configs/{configID} | InspectConfigController | InspectConfigHandler |
| DELETE | /api/configs/{configID} | RemoveConfigController | RemoveConfigHandler |

### Key Pattern

Every controller follows the same structure:

```go
type XxxController struct {
    action *XxxAction
}

func (c *XxxController) Handle(ctx http.Context) http.Response {
    // 1. Parse route/query/body params
    // 2. Validate (if mutating)
    // 3. Call action.Execute()
    // 4. Handle errors (ErrNotFound → 404, etc.)
    // 5. Return JSON response
}
```

Every action follows:

```go
type XxxAction struct {
    orchestrator docker.Orchestrator
}

func (a *XxxAction) Execute(ctx context.Context, ...params) (Result, error) {
    return a.orchestrator.SomeMethod(ctx, ...params)
}
```

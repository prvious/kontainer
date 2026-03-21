# Validation Requests — Add ValidateRequest Structs

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace inline field validation in mutation controllers with dedicated Goravel request structs using `ctx.Request().ValidateRequest()`, following Goravel's validation conventions.

**Architecture:** Create `app/http/requests/` directory with request structs that implement `Authorize()` and `Rules()`. Each mutation controller replaces manual `Bind()` + field checks with a single `ValidateRequest()` call. Goravel handles validation errors automatically with a consistent error envelope.

**Tech Stack:** Go, Goravel framework validation

**Relevant skills:** `goravel-http` (validation section)

---

## Pre-work: Understand the Goravel Request Pattern

Before implementing, read the `goravel-http` skill's validation section. The pattern is:

```go
// app/http/requests/create_config_request.go
type CreateConfigRequest struct {
    Name string `form:"name" json:"name"`
    Data string `form:"data" json:"data"`
}

func (r *CreateConfigRequest) Authorize(ctx http.Context) error {
    return nil // already handled by JWT middleware
}

func (r *CreateConfigRequest) Rules(ctx http.Context) map[string]string {
    return map[string]string{
        "name": "required|string",
        "data": "required|string",
    }
}

// In controller:
var req requests.CreateConfigRequest
errors, err := ctx.Request().ValidateRequest(&req)
if err != nil {
    return ctx.Response().Json(400, map[string]any{"error": err.Error()})
}
if errors != nil {
    return ctx.Response().Json(422, map[string]any{"errors": errors.All()})
}
```

## Scope

The following mutation controllers need request structs:

| Controller | File | Current validation |
|-----------|------|-------------------|
| CreateConfigController | `configs/create_config_controller.go` | Manual check: name, data required |
| CreateSecretController | `secrets/create_secret_controller.go` | Manual check: name, data required |
| ScaleServiceController | `services/scale_service_controller.go` | Manual check: replicas required |
| UpdateServiceEnvController | `services/update_service_env_controller.go` | Manual check: env array required |
| UpdateServiceImageController | `services/update_service_image_controller.go` | Manual check: image required |
| UpdateNodeAvailabilityController | `nodes/update_node_availability_controller.go` | Manual check: availability in [active, pause, drain] |
| UpdateNodeRoleController | `nodes/update_node_role_controller.go` | Manual check: role in [manager, worker] |
| UpdateSettingsController | `settings/update_settings_controller.go` | Anonymous struct, manual check: settings map required |

---

## File Structure

### Created Files
- `server/app/http/requests/create_config_request.go`
- `server/app/http/requests/create_secret_request.go`
- `server/app/http/requests/scale_service_request.go`
- `server/app/http/requests/update_service_env_request.go`
- `server/app/http/requests/update_service_image_request.go`
- `server/app/http/requests/update_node_availability_request.go`
- `server/app/http/requests/update_node_role_request.go`
- `server/app/http/requests/update_settings_request.go`

### Modified Files
- Each corresponding controller file listed above

---

### Task 1: Create request structs for config and secret endpoints

**Files:**
- Create: `server/app/http/requests/create_config_request.go`
- Create: `server/app/http/requests/create_secret_request.go`
- Modify: `server/app/http/controllers/configs/create_config_controller.go`
- Modify: `server/app/http/controllers/secrets/create_secret_controller.go`

- [ ] **Step 1: Read the current controllers to understand exact fields**

Read `create_config_controller.go` and `create_secret_controller.go` to see the existing struct fields and validation logic.

- [ ] **Step 2: Create request structs**

Create `create_config_request.go`:
```go
package requests

import "github.com/goravel/framework/contracts/http"

type CreateConfigRequest struct {
    Name string `form:"name" json:"name"`
    Data string `form:"data" json:"data"`
}

func (r *CreateConfigRequest) Authorize(ctx http.Context) error {
    return nil
}

func (r *CreateConfigRequest) Rules(ctx http.Context) map[string]string {
    return map[string]string{
        "name": "required|string",
        "data": "required|string",
    }
}
```

Create `create_secret_request.go` with the same pattern (name + data fields).

- [ ] **Step 3: Update controllers to use ValidateRequest**

In each controller, replace the inline struct + Bind + manual checks with:
```go
var req requests.CreateConfigRequest
errors, err := ctx.Request().ValidateRequest(&req)
if err != nil {
    return ctx.Response().Json(400, map[string]any{"error": err.Error()})
}
if errors != nil {
    return ctx.Response().Json(422, map[string]any{"errors": errors.All()})
}
```

Then use `req.Name`, `req.Data` for the action call.

- [ ] **Step 4: Run `go vet`, commit**

```
git add server/app/http/requests/ server/app/http/controllers/configs/ server/app/http/controllers/secrets/
git commit -m "refactor: add ValidateRequest structs for config and secret endpoints"
```

---

### Task 2: Create request structs for service mutation endpoints

**Files:**
- Create: `server/app/http/requests/scale_service_request.go`
- Create: `server/app/http/requests/update_service_env_request.go`
- Create: `server/app/http/requests/update_service_image_request.go`
- Modify: `server/app/http/controllers/services/scale_service_controller.go`
- Modify: `server/app/http/controllers/services/update_service_env_controller.go`
- Modify: `server/app/http/controllers/services/update_service_image_controller.go`

- [ ] **Step 1: Read each controller to understand exact fields and validation**

- [ ] **Step 2: Create request structs with appropriate Rules()**

For scale: `replicas` required, integer.
For update env: `env` required, array of objects.
For update image: `image` required, string.

- [ ] **Step 3: Update each controller to use ValidateRequest**

Same pattern as Task 1.

- [ ] **Step 4: Run `go vet`, commit**

```
git add server/app/http/requests/ server/app/http/controllers/services/
git commit -m "refactor: add ValidateRequest structs for service mutation endpoints"
```

---

### Task 3: Create request structs for node and settings endpoints

**Files:**
- Create: `server/app/http/requests/update_node_availability_request.go`
- Create: `server/app/http/requests/update_node_role_request.go`
- Create: `server/app/http/requests/update_settings_request.go`
- Modify: `server/app/http/controllers/nodes/update_node_availability_controller.go`
- Modify: `server/app/http/controllers/nodes/update_node_role_controller.go`
- Modify: `server/app/http/controllers/settings/update_settings_controller.go`

- [ ] **Step 1: Read each controller**

Node availability: `availability` field must be one of `active`, `pause`, `drain`.
Node role: `role` field must be one of `manager`, `worker`.
Settings: `settings` field is a `map[string]string`.

- [ ] **Step 2: Create request structs**

For node availability, use `"required|in:active,pause,drain"` rule.
For node role, use `"required|in:manager,worker"` rule.
For settings, use `"required"` on the settings map field.

- [ ] **Step 3: Update each controller**

Same pattern as Tasks 1-2. Remove the anonymous struct in `update_settings_controller.go`.

- [ ] **Step 4: Run `go vet`, run `go test -C server ./...`, commit**

```
git add server/app/http/requests/ server/app/http/controllers/nodes/ server/app/http/controllers/settings/
git commit -m "refactor: add ValidateRequest structs for node and settings endpoints"
```

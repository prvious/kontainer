---
name: update-service-resources
description: API endpoint and Tauri command to set/update CPU and memory limits/reservations
depends_on: expose-service-resource-limits
---

## Feature Description

Add a new endpoint `PUT /api/services/{serviceID}/resources` and corresponding Tauri command `update_service_resources` that allows setting or clearing CPU and memory limits/reservations on a Docker Swarm service. Follows the existing inspect → modify → update pattern used by `update_service_env` and `update_service_image`.

## Implementation Plan

### Backend Components

**New file: `agent/services/update_service_resources.go`**

Action struct with `DockerClient` dependency injection:

```go
type UpdateResourcesRequest struct {
    CPULimit          *float64 `json:"cpuLimit"`          // cores, or null to clear
    MemoryLimit       *int64   `json:"memoryLimit"`       // bytes, or null to clear
    CPUReservation    *float64 `json:"cpuReservation"`    // cores, or null to clear
    MemoryReservation *int64   `json:"memoryReservation"` // bytes, or null to clear
}
```

Execute method:
1. `ServiceInspectWithRaw` to get current spec + version
2. Initialize `Resources`, `Limits`, `Reservations` structs if nil
3. Set values (convert CPU cores → nanocpus by multiplying by `1e9`)
4. Clear values when explicitly set to `0` (set nanocpus/bytes to 0, which Docker interprets as "no limit")
5. `ServiceUpdate` with modified spec

Validation:
- CPU limit must be >= 0.001 cores (1,000,000 nanocpus) when set
- Memory limit must be >= 4 MiB (4,194,304 bytes) when set — Docker's minimum
- Reservation should not exceed limit when both are set for the same resource type
- Return 400 with descriptive error for invalid values

Handler: standard pattern — decode JSON body, call action, return updated resource config.

**Route registration: `agent/server/server.go`**

```go
r.Method("PUT", "/services/{serviceID}/resources", updateServiceResourcesHandler)
```

**Dependency injection: `agent/main.go`**

```go
updateServiceResourcesAction := services.NewUpdateServiceResourcesAction(dockerClient)
updateServiceResourcesHandler := services.NewUpdateServiceResourcesHandler(updateServiceResourcesAction)
```

Pass handler to `server.New()`.

**New file: `app/src-tauri/src/actions/agent/update_service_resources.rs`**

Tauri command following the same pattern as `update_service_env.rs`:

```rust
#[tauri::command]
pub async fn update_service_resources(
    url: String,
    token: String,
    service_id: String,
    resources: UpdateResourcesRequest,
) -> Result<ResourceConfig, String>
```

Register in `mod.rs` and `lib.rs` invoke_handler.

### Frontend Components

**New hook: `app/src/hooks/use-update-service-resources.ts`**

```typescript
export function useUpdateServiceResources(serviceId: string) {
    const { agentUrl, token } = useAuth();
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: (resources: UpdateResourcesRequest) =>
            invoke<ResourceConfig>("update_service_resources", {
                url: agentUrl, token, serviceId, resources,
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["service", serviceId] });
            queryClient.invalidateQueries({ queryKey: ["services"] });
        },
    });
}
```

**New type: `app/src/types/service-detail.ts`**

```typescript
export interface UpdateResourcesRequest {
    cpuLimit: number | null;
    memoryLimit: number | null;
    cpuReservation: number | null;
    memoryReservation: number | null;
}
```

## Acceptance Criteria

- [ ] `PUT /api/services/{id}/resources` accepts JSON body and updates service
- [ ] Setting `cpuLimit: 0.5` correctly translates to 500,000,000 nanocpus
- [ ] Setting `memoryLimit: 536870912` sets a 512 MiB limit
- [ ] Setting a field to `0` clears that limit (removes constraint)
- [ ] Returns 400 for CPU < 0.001 or memory < 4 MiB
- [ ] Returns 400 when reservation exceeds limit for same resource
- [ ] Returns 404 for non-existent service
- [ ] Tauri command `update_service_resources` works end-to-end
- [ ] Mutation hook invalidates correct query keys on success
- [ ] All existing tests still pass

## Testing Strategy

### Go Agent Tests

**New file: `agent/services/update_service_resources_test.go`**

- Test successful update with all four fields set
- Test partial update (only CPU limit)
- Test clearing limits (set to 0)
- Test validation: CPU below minimum returns 400
- Test validation: memory below minimum returns 400
- Test validation: reservation exceeds limit returns 400
- Test non-existent service returns 404 (using mock error)

### Frontend Tests

**New file: `app/tests/unit/use-update-service-resources.spec.tsx`**

- Test successful mutation calls invoke with correct params
- Test mutation invalidates query keys on success
- Test error propagation

## Code Formatting

- Go: `gofmt`
- Rust: `cargo fmt`
- TypeScript: `pnpm fmt` (oxfmt)

## Additional Notes

- The "clear limit" UX uses `0` as a sentinel value. The Go agent converts `0` to Docker's "unlimited" (which is also `0` nanocpus / `0` bytes internally). This is consistent with `docker service update --limit-cpu 0`.
- The update is atomic — all four fields are set in a single `ServiceUpdate` call to avoid partial state.
- Version conflicts (another update happened between inspect and update) will return a 500 with Docker's conflict error. The frontend should show a "service was modified, please refresh" message. This is the same behavior as existing update commands.

---
name: expose-service-resource-limits
description: Expose CPU and memory limits/reservations in service inspect API and Tauri IPC
depends_on: null
---

## Feature Description

Extend the service inspect endpoint and Tauri command to include CPU and memory limits/reservations from Docker Swarm's `TaskTemplate.Resources`. This provides the data layer that the frontend needs to display and later edit resource constraints.

Currently `ServiceDetail` has no resource information. Docker Swarm stores resource config at `svc.Spec.TaskTemplate.Resources` with separate `Limits` (hard caps) and `Reservations` (guaranteed minimums) for both CPU (nanocpus) and memory (bytes).

## Implementation Plan

### Backend Components

**Go Agent (`agent/services/inspect_service.go`)**

Add a `ResourceConfig` struct and include it in `ServiceDetail`:

```go
type ResourceConfig struct {
    CPULimit            *float64 `json:"cpuLimit"`            // CPU cores (e.g. 0.5 = half a core)
    MemoryLimit         *int64   `json:"memoryLimit"`         // bytes
    CPUReservation      *float64 `json:"cpuReservation"`      // CPU cores
    MemoryReservation   *int64   `json:"memoryReservation"`   // bytes
}
```

In the `Execute` method, extract from `svc.Spec.TaskTemplate.Resources`:
- Convert nanocpus (`int64`) to CPU cores (`float64`) by dividing by `1e9` for a friendlier API
- Pass memory as raw bytes (frontend handles display formatting)
- Use pointer types so `null` means "not set" (different from zero)

**Rust Tauri (`app/src-tauri/src/actions/agent/inspect_service.rs`)**

Add matching `ResourceConfig` struct to `InspectServiceResponse`/`ServiceDetail`:

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceConfig {
    pub cpu_limit: Option<f64>,
    pub memory_limit: Option<i64>,
    pub cpu_reservation: Option<f64>,
    pub memory_reservation: Option<i64>,
}
```

Add `resources: Option<ResourceConfig>` to the existing `ServiceDetail` struct.

**TypeScript (`app/src/types/service-detail.ts`)**

```typescript
export interface ResourceConfig {
    cpuLimit: number | null;
    memoryLimit: number | null;
    cpuReservation: number | null;
    memoryReservation: number | null;
}
```

Add `resources: ResourceConfig | null` to `ServiceDetail`.

### Key Design Decisions

- **CPU as float64 cores** (not nanocpus): The API returns `0.5` for half a core rather than `500000000` nanocpus. Conversion happens in the Go agent. This is friendlier for display and form inputs.
- **Memory as bytes**: Frontend converts to human-readable (MB/GB) for display. Keeps precision for the update round-trip.
- **Null vs zero**: A `null` CPU limit means "unlimited". A `0` would be invalid. Pointer/Option types preserve this distinction.

## Acceptance Criteria

- [ ] `GET /api/services/{id}` response includes `resources` field
- [ ] `resources` is `null` when no limits or reservations are set
- [ ] `resources.cpuLimit` returns CPU in cores (float, e.g. `0.5`)
- [ ] `resources.memoryLimit` returns memory in bytes
- [ ] `resources.cpuReservation` and `memoryReservation` populated when set
- [ ] Rust struct matches Go response (serde camelCase)
- [ ] TypeScript type matches Rust struct
- [ ] Existing service detail tests still pass
- [ ] New Go test verifies resource extraction for services with and without limits

## Testing Strategy

### Go Agent Tests

**Test file:** `agent/services/inspect_service_test.go` (update existing or create)

- Test inspect returns `resources: null` when service has no resource config
- Test inspect returns correct CPU/memory limits when set
- Test inspect returns correct CPU/memory reservations when set
- Test inspect returns partial config (e.g. only memory limit, no CPU)
- Test nanocpus-to-cores conversion accuracy

### Frontend Tests

**Test file:** `app/tests/unit/service-detail.spec.tsx` (update existing)

- Update mock data to include `resources` field
- Verify existing tests still pass with new field present

## Code Formatting

- Go: `gofmt`
- Rust: `cargo fmt`
- TypeScript: `pnpm fmt` (oxfmt)

## Additional Notes

- The Docker `swarm.Limit` struct has a `Pids` field (max PIDs). We intentionally omit this as it's rarely used and adds UI complexity.
- `GenericResources` on reservations (GPUs etc.) are also omitted for now — can be added later.
- The nanocpus → cores conversion should round to 3 decimal places to avoid floating point noise.

# 0021. Swarm services, convergence, and the quorum state

- Status: accepted (amended by 0034)
- Date: 2026-09-28
- Spec: docs/build/11-services.md

## Context

Spec 11 lists Swarm services from the manager and scales, restarts, and rolls them back as
durable operations that wait for convergence. It also handles the state where the manager can't
answer. That extends spec 10's `ManagerEngine` port, adds public routes, native-client calls,
Tauri commands, and new closed error codes across the boundaries.

## Decision

- **Engine.** `ContainerEngine` gains:
  - `list_services`: mode, replicas, ports, version, stack label, and update status.
  - `list_tasks(service_ids, current_only)`: one call filtered by service and, optionally,
    `desired-state=running`.
  - `scale_service`, `force_update_service`, and `rollback_service`. Each inspects the service,
    refuses a stale `version` as `VersionConflict`, and sends the update based on that version.
    Docker's "update out of sequence" also maps to `VersionConflict`.
  - `swarm_status`.
- **Error kinds.** `EngineErrorKind` gains `QuorumUnavailable` (Swarm's "does not have a leader"
  and quorum wording, used only to classify) and `VersionConflict`. `ManagerError` mirrors both.
  The port gives the new methods defaults, so network-only fakes stay small.
- **Counts.** Running means `state = running` and `desired_state = running`, so a task being
  replaced doesn't count. A global service's desired count is its tasks meant to run, which is one
  per eligible node.
- **Public API.**
  - `GET /api/v1/services[?stack=]`: one list call and one tasks call.
  - `GET /api/v1/services/{id}`: the service, its 50 newest tasks, and the newest task error.
  - `POST /api/v1/services/{id}/actions/{scale|restart|rollback}` `{ version, replicas? }`.
  - Every manager read has a 3 s deadline. When the manager can't answer (no quorum, timeout, or
    paused), reads and writes are 503 `{ error: "quorum_unavailable", retryAfterSeconds: 10 }`
    with `Retry-After`. A write is refused before admission, so no operation is created.
  - A stale version is 409 `version_conflict` at the edge. Scaling a global service is 400
    `replicas: global_service`.
- **Convergence executor.** One `ServiceConvergeExecutor` per action. It applies the change,
  reports `{ desired, running, phase: accepted }`, then polls every 2 s until converged or 60 s
  pass:
  - Scale converges when running equals desired and, going down, no task above the new count
    still runs.
  - Restart converges when every running task changed after the request.
  - Rollback converges when Swarm isn't updating or rolling back and the counts match.
  - A timeout fails `converge_timeout` with the counts and the newest task error. Service kinds
    get a 90 s operation timeout so the 60 s wait ends first.
- **Native client and IPC.** `NativeClientError` gains `QuorumUnavailable { retry_after_seconds }`,
  `NotAManager`, and `VersionConflict`. The inventory error-body parser accepts
  `retryAfterSeconds`, and admissions now use it too, so those refusals keep their codes.
  `InventoryError` and the TypeScript codes gain `quorum_unavailable`, `not_a_manager`, and
  `version_conflict`.
- **Desktop.**
  - STATUS is derived: `scaling` while our scale runs, `updating` while Swarm updates or another
    operation of ours runs, then `stopped`, `partial`, or `running`.
  - The replica cell becomes a stepper (±, arrows, Enter submits with the row's version, Esc
    cancels) and shows a track from the operation detail while scaling.
  - Hovering a partial count fetches the service and shows the newest task error.
  - Operation rows read `{ desired, running }` as "n of m tasks running".
  - Restart and rollback ask for a plain confirmation. They replace tasks but lose no data.
  - The quorum error state counts down from `errorUpdatedAt`, and the list refetches every 10 s
    while it errors. CREATE SERVICE explains that writes are paused.
  - The "⋮" menu has Scale, Restart, Rollback, and Logs, which stays disabled until the Logs
    screen (spec 15) exists to navigate to.

## Consequences

- A paused or partitioned manager looks the same as lost quorum. The message says workloads keep
  running, which is true in both cases.
- The manager engine is still connected once at startup (ADR 0020).

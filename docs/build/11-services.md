# 11. Services

## Goal

Swarm services from the manager: list, inline scaling as a durable operation that tracks
convergence, restart, rollback, and the quorum-lost error state. Create service and the service
detail page are deferred.

Design: Services `LMjiS` (light `q5rBtQ`); states `Y3UuOQ` (04.S3 scaling running and failed),
`M9tCr` (04.S4 quorum lost); captions `G64Ku`, `bneJ4`. Gaps G2, G4, G6.

## Server

### Manager engine (extend spec 10's port)

- `list_services`: id, name, image reference (display without digest, keep the full one), mode
  `replicated | global`, desired replicas (replicated) or node count (global), ports published,
  created, updated, `version` index, labels (`com.docker.stack.namespace`), update status state and
  message.
- `list_tasks(service_id)`: id, slot, node id, state, desired state, error message, timestamps,
  container id. Running count = tasks with state `running`.
- `scale(service_id, replicas, version)`, `force_update(service_id, version)` (restart),
  `rollback(service_id, version)`. Every write reads the spec, checks `version`, and sends the
  update with the same version; a mismatch fails with `version_conflict`.
- `swarm_info`: control available, managers, nodes; and the error mapping: Bollard errors whose
  message contains the Swarm quorum text map to `QuorumUnavailable`.

### Routes (protected)

- `GET /api/v1/services?stack=` → list with `runningReplicas` computed from tasks in one pass
  (one `tasks` call filtered by service ids, not one per service). 503 `quorum_unavailable` with
  `retryAfterSeconds: 10` when the manager cannot answer.
- `GET /api/v1/services/{id}` → the service plus its tasks and the most recent task error, for
  the replica tooltip and the scaling status line.
- `POST /api/v1/services/{id}/actions/scale` `{ replicas, version }` → 202 `service.scale`.
  Executor: apply, then poll tasks every 2 s until `running == desired` (or, when scaling down,
  until no task above the new count remains); progress detail
  `{ desired, running, phase: "accepted" | "converging" | "converged" }`; timeout 60 s →
  `failed/converge_timeout` with the last task error in the message. Accepted is not converged.
- `.../actions/restart` → 202 `service.restart` (force update; converge rule as above).
- `.../actions/rollback` → 202 `service.rollback`.
- While `quorum_unavailable`, mutation routes return 503 before admission so no operation is
  created.

## Desktop

- Route `/services` with `stack` and filter search params. Tauri: `list_services`, `get_service`,
  `scale_service(id, replicas, version, idempotencyKey)`, `restart_service`, `rollback_service`.
  Hooks: `useServices(filters)`, `useService(id)` (fetched on demand for tooltips).
- Header: `SERVICES`, subtitle `N services · a replicated · b global · all tasks running` or
  `· k services degraded`, CREATE SERVICE disabled with tooltip "Not built yet" (G2).
  `<PinnedOperations scope="services" />` under the header.
- TableToolbar: search over name and image; Filters: mode, stack, status; Sort: name, updated.
- DataTable: STATUS (`running` when converged, `updating` while update status is updating or an
  operation is in flight, `partial` when 0 < running < desired, `stopped` when running is 0),
  SERVICE NAME with the stack in the mono subtitle, IMAGE, MODE badge, REPLICAS `running/desired`
  (global shows `running/nodes`), PORTS, UPDATED, actions `⋮`: Scale, Restart, Rollback, Logs
  (navigates to `/logs` with the service preselected).
- Inline scaling (04.S3): clicking the replica count on a replicated service turns it into a
  stepper (−, value, +); Enter submits with the row's `version`, Esc cancels; the row flips to
  SCALING with a track driven by the operation detail until converged. A failed operation stays
  pinned under the header with its reason code and request id. Hovering a partial replica count
  shows a Tooltip Rich with the latest task error ("no suitable node (insufficient memory)").
- Quorum lost (04.S4): the list query's 503 renders the table body as an ErrorState: title
  "Couldn't load services", body stating that workloads keep running and are never shown as down
  while the cluster can't answer, code + request id, TRY AGAIN with a countdown to the next
  automatic retry (the query's `refetchInterval` while errored is 10 s; the countdown derives from
  `errorUpdatedAt` plus a `useNow` clock hook). CREATE SERVICE tooltip explains that writes are
  paused; row actions are disabled.

## Tests

- Manager engine with a fake Bollard-free adapter: version conflict; quorum mapping; running count
  math.
- Executor: converge detection, scale down, timeout with last task error.
- Real cluster: list the seed services; scale `webapp_web` (or the actual seed name) 1 → 2 and
  see convergence; restart; rollback after an image change made through the CLI in the test.
- Desktop: stepper keyboard flow; SCALING row; pinned failure; quorum ErrorState and countdown;
  disabled writes.

## Done when

- Scaling a seed service from the desktop shows the row converging and a success toast; the
  quorum state renders when the manager DinD container is paused (`docker pause
  kontainer-cluster-manager-1`, then unpause); all checks pass; `PROGRESS.md` updated.

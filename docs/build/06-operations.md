# 06. Operations

## Goal

Wire the existing durable operations engine end to end: every mutation the desktop performs is
admitted as an operation, executed by an in-process worker, reported through polling, and surfaced
in the UI with the Operation Status component and toasts. Specs 07 to 12 only add executors.

Design: Operation Status `nBaer` with Queued `AUZ5w`, Succeeded `UoyTS`, Failed `Kij7U`; state
`Y3UuOQ` (04.S3, an operation pinned under the page header, failed one stays pinned with reason
code and request id); state `q5WTPa` (03.S5, bulk operation status row); toast rules in `EAPpo`.
Decisions: ADR 0009.

## Server

### Kinds

One enum in `operations/domain.rs`: `container.start`, `container.stop`, `container.restart`,
`container.pause`, `container.unpause`, `container.remove`, `containers.bulk`, `image.remove`,
`image.pull`, `volume.create`, `volume.remove`, `network.create`, `network.remove`,
`service.scale`, `service.restart`, `service.rollback`, `stack.remove`. Each kind has a timeout
(`image.pull` 10 minutes, everything else 60 seconds) and a request payload type that is validated
at the HTTP edge before admission.

### Admission at the HTTP edge

- An extractor reads `Idempotency-Key` (16–64 printable ASCII). Missing → 400
  `idempotency_key_required`. Every mutation route uses it.
- The handler validates the body, then calls the existing `admit::execute` with principal, scope
  (the route path with ids), kind, payload, and the key. Response 202 `{ operationId, replayed }`.
  `IdempotencyConflict` → 409 `idempotency_conflict`.

### Worker

- One supervised task started from `app.rs` (fill the empty `connect_workers` / `register_tasks`
  seam): loops `claim_next` with a stable worker ref; idle backoff 250 ms → 1 s; a
  `Semaphore(4)` bounds concurrent executions.
- `OperationExecutor` trait: `kind()`, `execute(&self, op, ctx) -> Result<Detail, Failure>` where
  `ctx` gives the database, node clients, the manager engine (spec 11), a cancellation token, and a
  `report_progress(detail)` callback. Executors live with their domain (for example
  `server/src/containers/operations/restart.rs`), one file each, registered in a map by kind.
- Each execution runs under its kind's timeout; timeout → `failed` with code `timeout`.
- Terminal write through a new `finalize` action: exactly one terminal event (`succeeded`,
  `failed` with code and message, or `cancelled`), state update, audit intent row updated to
  the outcome. Refuse if already terminal.
- Startup recovery: before the worker starts, one update marks every `running` operation
  `uncertain` with a `recovered_after_restart` event.
- `operation_events` gains `detail` (jsonb, nullable) and `message` (text, nullable) if not
  present; progress events carry the kind's detail document.

### Routes (protected)

- `GET /api/v1/operations/{id}` adds `progress` (latest detail), `errorMessage`, `startedAt`,
  `finishedAt`, `requestId`.
- `GET /api/v1/operations/{id}/events?after=0&limit=100` → ordered events.
- `POST /api/v1/operations/{id}/cancel` → 202 when `recoverable`, 409 `not_cancellable` otherwise.
- Hourly maintenance in the same supervised task: delete terminal operations older than 7 days and
  expired idempotency records, in batches.

### Bulk

`containers.bulk` payload `{ action, targets: [{ nodeId, containerId }] }` (max 200). The executor
runs targets with concurrency 8 and reports detail
`{ total, done, failed, results: [{ nodeId, containerId, ok, error? }] }` after each target. The
operation succeeds only when every target succeeded; otherwise `failed` with code
`partial_failure` and the results intact.

## Desktop

- Tauri: `get_operation(id)`, `list_operation_events(id, after)`, `cancel_operation(id)`. Every
  mutation command takes an `idempotencyKey` argument and returns `{ operationId, replayed }`.
- React generates the idempotency key with `crypto.randomUUID()` per user gesture and keeps it in
  the mutation variables so a retried mutation reuses it.
- `useOperation(id)`: `useQuery` with `refetchInterval` 1000 while the state is not terminal,
  `false` after. No effects.
- `OperationsProvider` (context, mounted in `_authenticated`): holds the list of tracked operation
  ids with their resource scope. `useStartOperation({ kind, invalidate })` wraps `useMutation`:
  `onSuccess` registers the id; a small `OperationWatcher` component per tracked id renders
  nothing, subscribes through `useOperation`, and when the operation turns terminal calls
  `invalidateQueries(invalidate)` and fires the toast (success: title + summary from `progress`;
  failure: reason code, message, request id, and an action to copy details). Failed operations
  stay in the list until dismissed; succeeded ones leave after the toast.
- `OperationStatus` rows: pages render `<PinnedOperations scope="services" />` under their header
  (04.S3), which lists tracked operations for that scope with the design's status line
  (`RUNNING · 4 of 5 tasks running · 12s`), a track, and CANCEL while cancellable.
- Toast rules: bottom-right, 5 s, success and failure stack; loading toasts for background pulls.

## Tests

- Server (PostgreSQL): admission requires a key; same key + same body replays; same key + different
  body conflicts; a fake `noop` executor runs end to end with progress and a terminal event;
  restart recovery flips `running` to `uncertain`; cancel only while recoverable; timeout produces
  `failed/timeout`; bulk partial failure keeps results; prune deletes only old terminal rows; event
  pagination.
- Desktop: `useOperation` stops polling on terminal; the watcher invalidates and toasts once;
  pinned failed operation stays until dismissed; idempotency key reused across retry.

## Done when

- A `noop` kind admitted through a test route runs on the dev controller and the desktop shows its
  Operation Status row moving to succeeded with a toast.
- Killing the controller mid-operation and restarting shows the operation as `uncertain`.
- All checks pass; `PROGRESS.md` updated; ADR for the events schema change.

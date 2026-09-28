# 0016. Operation execution, events schema, and the desktop operation model

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/06-operations.md

## Context

ADR 0009 decided an in-process worker. Spec 06 wires admission, execution, polling, and the desktop
Operation Status model end to end, and needs a place for payloads, progress, and failure messages.

## Decision

- **Schema.** `operations` gains `payload` (jsonb, default `{}`), `started_at`, `finished_at`, and
  `request_id` (the admitting request's `x-request-id`, printable ASCII, at most 128 bytes);
  `operation_events` gains `detail` (jsonb) and `message` (text). An index on
  `operations(state, updated_at)` serves pruning. SeaORM's JSON support was already enabled
  transitively by Loco, so no dependency changes.
- **Kinds.** `OperationKind` lists the spec's kinds plus `noop`, which exercises the pipeline and is
  admitted only by `POST /api/v1/operations/noop` in development and test (404 elsewhere).
  `image.pull` gets ten minutes, everything else sixty seconds; tests override per kind.
- **Admission.** The `Idempotency-Key` extractor accepts 16–64 printable ASCII characters and
  answers 400 `idempotency_key_required` otherwise. The canonical request hashed for replay is the
  validated payload serialized with sorted keys. Idempotency records live seven days, the same as
  terminal operations, because visibility derives from them.
- **Worker.** Started beside the servers in `serve` (the Loco `connect_workers` hook has no shutdown
  integration). It recovers `running` → `uncertain` (`recovered_after_restart`) before claiming,
  retrying until the database answers; claims with a 250 ms → 1 s idle backoff; runs at most four
  executions; catches executor panics as `internal_error`; maps timeouts to `timeout`; answers
  unknown or unregistered kinds with `unsupported_kind`; prunes hourly. On drain it stops claiming
  and gives in-flight executions the shutdown grace, then cancels them without writing an outcome,
  so they recover as `uncertain` (never a guessed `failed`).
- **Finalize and cancel.** `finalize` writes exactly one terminal event, stamps `finished_at`, and
  sets the audit intent's state to the outcome; it refuses terminal operations. Cancel locks the row
  before checking `recoverable`, so a racing claim resolves to exactly one of them.
- **Prune.** Terminal operations untouched for seven days go with their idempotency records and
  audit intents (the foreign keys restrict), events by cascade; then expired idempotency records of
  terminal operations.
- **Bulk.** `operations::bulk::run_bulk` runs targets with concurrency 8, reports
  `{ total, done, failed, results }` after each, and fails with `partial_failure` carrying the full
  results. Spec 07 registers the `containers.bulk` executor on it.
- **Native client and IPC.** `OperationSnapshot` gains the new fields; `list_operation_events`
  (strictly increasing sequences after the cursor), `cancel_operation`, and a generic `admit`
  (retried once on a pre-response transient: the key makes it a replay). Tauri adds
  `get_operation`, `list_operation_events`, and `cancel_operation`; `SessionManager::admit` is the
  path specs 07–12 give their mutation commands. `InventoryError` gains `not_found` and
  `conflict`; operation reads never report `node_not_found`.
- **Desktop.** `useOperation` polls every second until the state is terminal or `uncertain`;
  each settled snapshot goes to the `OperationsProvider`, which invalidates and toasts once per
  operation whichever observer fetched it (no effects). `useStartOperation` retries a
  `controller_unavailable` start once with the same variables, so the same key. Failed rows show
  `DISMISS` rather than the design's `DETAILS`: the row already carries the reason code and request
  id, and the failure toast's COPY DETAILS copies them; a second action would not fit the row.
  Uncertain operations stay pinned with a warning toast. The kit's queued icon is the design's
  `clock`.

## Consequences

- Every executor added later is a registry entry; nothing else in the worker changes.
- A second controller still needs leases and fencing (ADR 0009).

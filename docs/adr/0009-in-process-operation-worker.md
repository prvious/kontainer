# 0009. Durable operations run on an in-process controller worker

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/06-operations.md

## Context

The durable operations engine exists: admission with idempotency, claim-next with concurrent safety,
mark-uncertain, and polling. Nothing feeds it and nothing runs it. The backlog planned leases with
fencing, abandoned-claim detection, event listing, cancellation, and pruning as separate issues. There
is one controller.

## Decision

- Every mutation the desktop performs is admitted as an operation and returns 202 with the operation
  id. The desktop polls `GET /api/v1/operations/{id}` at one-second intervals until terminal.
- One supervised Tokio task inside the controller claims recoverable operations and executes them
  through an `OperationExecutor` that dispatches on `kind`. Concurrency is bounded (default 4).
- Executors append `operation_events` for progress and write exactly one terminal event and state:
  `succeeded`, `failed`, or `cancelled`. Progress detail is a small JSON document per kind.
- Crash recovery: on startup the controller marks every `running` operation `uncertain` in one
  statement. No leases, no fencing, no claim timeouts while a single controller runs.
- Cancellation is honoured only before execution starts (state `recoverable`); running operations
  finish.
- Terminal operations older than 7 days are pruned hourly by the same supervised task.

## Consequences

- The client gets one consistent progress model for every action, matching the Operation Status
  component in the design.
- A second controller requires leases and fencing before it is safe. This ADR must be superseded
  then.
- Idempotency keys are client-generated UUIDs per user gesture, so a retried request never doubles
  a mutation.

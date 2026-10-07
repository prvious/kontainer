# 0034. Decide service restart and rollback from Swarm's update status

- Status: accepted
- Date: 2026-10-04
- Spec: docs/build/11-services.md
- Amends: ADR 0021, ADR 0022

## Context

ADR 0021's convergence executor waited 60 s and judged the result with its own rules. The PR 135
review found those rules wrong both ways:

- **Scale-down.** Swarm picks which slots go. A correct scale from 3 to 2 that kept slot 3 still
  had a running task above the new count, so it timed out.
- **Restart.** It compared the controller's clock with task timestamps from the manager's clock.
- **Rollback.** A rollback that Swarm paused on a failed task reported success.
- **The deadline.** The service's update settings pace a rolling update (Docker's default monitors
  each task for 5 s), so ordinary updates take longer than 60 s.
- **Poll errors.** They were dropped. A quorum loss after the write read as
  `converge_timeout … 0 of 0 running`.

The owner chose to keep waiting for a verdict rather than end at accept (decision D3 A).

## Decision

- **Scale** converges when the running count equals the desired count and no task Swarm is
  stopping still runs (`state = running`, `desired_state ≠ running`). Slot numbers aren't used.
- **Restart and rollback** decide from the service's `UpdateStatus.State`:
  - Restart succeeds on `completed`. Rollback succeeds on `rollback_completed`, or on `completed`
    when a later update replaced it and finished.
  - `paused` fails `update_paused`, and `rollback_paused` fails `rollback_paused`. A restart that
    ends `rollback_completed`, because the service's failure action rolled it back, fails
    `rolled_back`. The message carries Swarm's `UpdateStatus.Message`.
  - Any other state keeps polling, including no status yet. Swarm clears the status when it
    accepts an update and sets `updating` when its updater starts. A manual rollback sets
    `rollback_started` at once.
  - A restart of a service that runs no task (desired 0) succeeds at once. Swarm starts no updater
    for it, so the status stays empty; the dev cluster confirmed this.
- **Deadline.** The wait is 15 min, and the service kinds' operation timeout is 16 min.
- **Polling** goes through `get_service`, which now keeps every task. `GET /api/v1/services/{id}`
  still returns the 50 newest.
- **Failures at the deadline.**
  - If the last poll failed, the operation fails with the manager's own code, such as
    `quorum_unavailable` or `manager_unavailable`, and says the change was applied.
  - Otherwise it fails `converge_timeout` with the counts and the newest task error, as before.
  - A service that disappears mid-wait fails `not_found` at once.
- **Stacks** (from the same review):
  - The stacks list leaves CPU and memory out when any node's container list fails, not only when
    the whole fan-out fails.
  - A removal that fails after the services are gone names the networks left behind and points to
    the Networks page (decision D8 A).

## Consequences

- A service operation can hold a worker slot for up to 16 min.
- `update_paused`, `rollback_paused` and `rolled_back` are new failure codes. The desktop renders
  failure codes as text, so it needs no change.
- If someone else updates the service while the operation waits, the verdict follows their update.
- The services test fake now moves the update status and removes a slot other than the highest.
  `services::dev_cluster_tests` runs scale 3 → 2, restart and rollback through the executor
  against the dev Swarm. It runs only when `KONTAINER_TEST_DEV_MANAGER_HOST` is set.

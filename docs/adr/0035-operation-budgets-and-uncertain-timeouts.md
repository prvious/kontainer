# 0035. Size operation budgets to the work, and record a timeout as uncertain

- Status: accepted
- Date: 2026-10-04
- Spec: docs/build/06-operations.md
- Amends: ADR 0016

## Context

ADR 0016 gave `image.pull` ten minutes and every other kind sixty seconds, overridable per kind in
the registry for tests. It also mapped a timeout to `failed` with `timeout`, ran at most four
executions, and only logged a finalize that failed. The PR 135 review found these problems:

- **A timeout guessed `failed`.** When the budget runs out, Docker may still finish the stop.
  ADR 0016 already refuses that guess at forced shutdown, which leaves the operation `uncertain`.
- **The budget ignored the work.**
  - A stop with a 600 s grace can't finish in 60 s.
  - Neither can a 200-target bulk stop, which runs in waves of eight. Targets that hadn't started
    when the budget ran out never ran.
- **Pulls.** Multi-GB pulls died at ten minutes. One hung node also held the whole pull until the
  budget ran out.
- **Four slots.** A few long pulls or 16-minute service updates (ADR 0034) queued every short
  action behind them.
- **A failed finalize.** One slow database connection left the row `running` until the next
  restart.
- **An unsupervised worker.** If the worker task exited, admission kept answering 202 and nothing
  ever ran.

## Decision

- **A timeout is uncertain.** When an execution passes its budget, the worker cancels it. Then it
  marks the operation `uncertain` with error code `timeout`, using the existing `mark_uncertain`
  action. That writes no terminal event and no `finished_at`. The desktop already treats
  `uncertain` as settled and keeps it pinned with a warning.
- **The executor sizes the budget.** `OperationExecutor::timeout(&payload)` defaults to the kind's
  budget, `OperationKind::timeout()`. The registry's per-kind override map and `with_timeout` are
  gone; test executors override the trait method instead.
  - **Single stop and restart:** the larger of 60 s and grace + 10 s. The grace is the request's
    `timeoutSeconds`, or Docker's default of 10 s when it names none.
  - **`containers.bulk`:** the larger of 60 s and ceil(targets / 8) × 20 s. Targets run in waves of
    eight. A bulk request names no grace, so each wave gets Docker's default 10 s plus 10 s.
    A 50-target bulk gets 140 s; the 200-target maximum gets 500 s.
  - **`image.pull`:** 60 minutes.
  - **Unchanged:** service kinds 16 minutes, `stack.remove` 150 s, everything else 60 s.
  - The 60 s floor keeps the agent RPC's own action deadline (grace + 22 s) inside the budget for
    ordinary graces, so a slow daemon still gets a definite answer.
- **A silent node fails its pull alone.** If a node's agent sends no pull event for 60 s, that
  node fails with "No progress from the node for 60 s." The other nodes carry on, and the operation
  ends `partial_failure` as before. Docker reports download and extract progress continuously, so
  that long a silence means the daemon or the connection is stuck.
- **Sixteen execution slots.** Executions mostly wait on agents and the manager. They hold a
  database connection only to write progress and the outcome.
- **Recording retries.** A finalize or `mark_uncertain` that fails with a persistence error is
  retried, holding the slot. The wait starts at 250 ms and doubles up to 5 s. Retrying stops when
  any of these happens:
  - the write lands;
  - the error isn't a database failure (`AlreadyTerminal`, `NotFound`, an invalid state);
  - the forced shutdown cancels it. The row then stays `running`, and startup recovery marks it
    `uncertain` (ADR 0016).
- **The worker is supervised.** `serve` waits on the worker alongside the servers and the shutdown
  signal. If the worker exits before the drain, through a bug or a panic, the controller logs an
  error and drains as it would for a signal. It then exits with an error, like a failed server.
  The worker isn't restarted in a loop, because that would hide the cause. An exit during the
  drain is the normal one and changes nothing.

## Consequences

- An operation that times out stays `uncertain` until someone checks it. Pruning keeps `uncertain`
  rows, as it already does for `recovered_after_restart`.
- The controller's agent RPC pull stream deadline (`PULL_DEADLINE` in `rpc/client.rs`) is raised to
  60 minutes with the pull budget, and the two must stay equal. Otherwise a node's pull ends early.
- A worker that stops before the drain takes the controller down with it. The process supervisor
  restarts the controller, and startup recovery marks the operations it was running `uncertain`.
- A database outage longer than the drain grace still leaves outcomes unrecorded. Recovery reports
  them as `uncertain`, never as a guess.

# 07. Containers

## Goal

The cross-node container list with search, filters, sorting, bulk selection, lifecycle actions,
partial results, and empty states, replacing the single-node `/tasks` page.

Design: Containers `2rYBA` (light `tRaXy`); states `VfepO` (03.S4 partial results and row hover),
`q5WTPa` (03.S5 bulk selection and results), `IU3ve` (03.S6 no matches). Captions `aafqy`,
`E6FljT`, `p6em5`. The Overview table (spec 14) reuses this page's table. Gaps: G5, G6, G7.

## Server

### Agent

- `ListContainers` returns every container on the node (all states) with: id, name, image, state,
  status text, created, started, ports (`host:container/proto` strings for published ports),
  labels subset (`com.docker.swarm.service.name`, `com.docker.stack.namespace`,
  `com.docker.swarm.task.id`), and the cached CPU percent, memory used, memory limit from spec 05.
  Pagination stays but the page size cap rises to 500; the controller drains pages.
- New unary RPC `ContainerAction(container_id, action)` with `START | STOP | RESTART | PAUSE |
  UNPAUSE | REMOVE`, `force` for remove, `timeout_seconds` for stop and restart (default 10).
  Engine trait gains the matching methods. Errors map to stable codes: `not_found`,
  `already_in_state`, `engine_error`.

### Controller

- `GET /api/v1/containers` → fan-out to online nodes, merged `items` (each with `nodeId` and
  `nodeName`), the `nodes` outcome list, `snapshotAt`. Cap 2,000 items; beyond that return 413
  `too_many_containers` with a hint to filter by node (`?node=`). No cursor.
- Remove `GET /api/v1/nodes/{nodeId}/containers` and its native-client and Tauri counterparts
  (`list_container_inventory`, `get_configured_node`) once the new route is covered by tests.
- `POST /api/v1/containers/{nodeId}/{containerId}/actions/{action}` → 202 operation (kinds
  `container.*`); body `{ force?, timeoutSeconds? }`.
- `POST /api/v1/containers/actions/bulk` `{ action, targets }` → 202 `containers.bulk`.
- Executors call the node's agent; `remove` on a running container passes `force` only when the
  request said so.

## Desktop

- Route `/containers` replaces `/tasks` (delete the tasks route, hook, and test). Sidebar link and
  count badge (total containers) come from `useContainers()`.
- Tauri: `list_containers()`, `container_action(nodeId, containerId, action, options,
  idempotencyKey)`, `containers_bulk_action(action, targets, idempotencyKey)`.
- Hook: `useContainers()` → `['containers', profileId]`, polling from settings,
  `keepPreviousData`. Derived values (counts by state, per-node presence) are computed in render
  or with `select`, never in effects.

### Page

- Header: `CONTAINERS`, subtitle `managing N containers across M nodes`, REFRESH (outline, spins
  while refetching).
- Container States strip (`DILCb`): tile, `CONTAINER STATES` with legend counts running / paused /
  exited (success / warning / destructive dots); readout: a Ring labelled `RUNNING` showing
  `running / total` (the design's "slot usage" is this real ratio), and a `SparkBars` trend
  `RUNNING · 24h` from cluster samples `containers_running` (24h / 15m).
- TableToolbar: search across name, image, id; Filters: state (running, paused, exited,
  restarting, created, dead), node, ports published; Sort: recent activity (started desc), name,
  CPU, memory. Filters are URL search params validated by a plain function with defaults, so
  reload and back keep them.
- DataTable columns: checkbox, STATUS (`StatusChip`: UP success, PAUSED warning, EXITED
  destructive, RESTARTING warning, CREATED neutral), NAME (bold; when the container belongs to a
  service show `service · stack` in the mono subtitle), IMAGE, CPU (`—` when not running), MEM,
  PORTS (`80, 443` for published), UPTIME (relative `started`; `exited 2m ago` for exited),
  actions.
- Row actions (icons with tooltips): Exec (`terminal`, navigates to `/exec` with the target, spec
  16), Pause or Play depending on state, Restart (`rotate-cw`) for exited; `⋮` menu: Logs
  (navigates to `/logs` with the owning service when there is one, otherwise disabled with
  tooltip "No service"), Exec, Restart, Stop / Start, Pause / Unpause, Remove.
- Remove (G7): ConfirmDialog with the typed name in every state, because removal loses the
  writable layer and logs (ADR 0036); a running, paused, or restarting container also passes
  `force`. On success the row fades and a toast says removal is final.
- Bulk (03.S5): selecting swaps the toolbar for the bulk bar with Restart, Stop, Start, Pause,
  Unpause, Remove; Stop and Remove confirm with the count typed; one `containers.bulk` operation;
  its Operation Status row stays under the bar until dismissed; per-row failures marked on the row
  from the operation detail; toasts stack.
- Partial results (03.S4): when any node outcome is not `ok`, a destructive Alert names the silent
  node(s) and the subtitle reads `N of M nodes answered`. Rows from the last successful snapshot of
  that node stay visible with STATUS `UNKNOWN` (neutral) and disabled actions. Keep the last good
  per-node rows in the query's `select`/merge of previous data, not in effects.
- No matches (03.S6): EmptyState inside the table body naming the active filters, CLEAR FILTERS
  resets search and chips, SHOW ALL keeps the search text. A cluster with zero containers shows the
  Foundations empty copy with DEPLOY STACK disabled (ADR 0010).
- Every icon-only action has a label tooltip after 400 ms.

## Tests

- Agent: action mapping with the fake engine, `already_in_state`, remove without force on a
  running container fails.
- Controller: merged list with partial outcomes; 413 over the cap; action admission and executor
  with a fake agent; bulk detail after partial failure.
- Real cluster: list shows seed stack containers on all three nodes; restart one; stop and start
  one; bulk pause two.
- Desktop: filters as search params; sort; bulk bar and confirm; partial-results alert with
  UNKNOWN rows; empty state actions; remove dialog typed rule; menus per state.

## Done when

- `/containers` on the dev cluster shows every seed container with live CPU and memory, actions
  work end to end with Operation Status rows and toasts, and stopping an agent shows the partial
  results state.
- `/tasks` is gone; all checks pass; `PROGRESS.md` updated; ADR for the container DTO change.

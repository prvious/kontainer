# 0017. Cluster container list, container actions, and the container DTO

- Status: accepted (amended by 0043)
- Date: 2026-09-28
- Spec: docs/build/07-containers.md

## Context

Spec 07 replaces the one-node container page with a cluster list and lifecycle actions. That changes
the protobuf, public HTTP, native-client, and Tauri IPC container contracts and removes the per-node
route.

## Decision

- **Protobuf (additive).** `ContainerSummary` gains `started_at_unix_seconds` and
  `finished_at_unix_seconds` (optional, from inspect), `published_ports` (`host:container/proto`),
  and the three allowlisted Swarm labels as their own optional fields (`service_name`,
  `stack_namespace`, `task_id`); every other label stays on the node. `NodeAgent` gains
  `ContainerAction(container_id, action, force, timeout_seconds)`. Failures are statuses whose
  message is a stable code: `not_found`, `already_in_state` (Docker 304), `container_running`
  (Docker 409, remove without force; the spec's three codes gain this one so the desktop can say
  "use force"), `engine_error`, `invalid_*`. The page cap rises from 100 to 500.
- **Engine.** Adds `container_times` (inspect) and `container_action`; 404/304/409 map to new
  `NotFound`, `NotModified`, `Conflict` kinds (a stale list cursor stays invalid pagination).
  Docker times are parsed without a date library. Stop and restart deadlines extend by their grace.
- **Start times.** The list API has no start time, so the agent inspects each container once and
  keeps the answer while its state is unchanged, at most a minute; actions through the agent drop
  the entry. Inspects past 1.5 s are left out of that page.
- **Public API.** `GET /api/v1/containers[?node=]` fans out to online and degraded members, drains
  their pages, and answers `{ items, nodes, snapshotAt }`; offline members are listed `unavailable`
  without a dial. More than 2,000 items is 413 `too_many_containers` with a hint.
  `POST /api/v1/containers/{nodeId}/{containerId}/actions/{action}` (optional `{ force,
  timeoutSeconds }`) and `POST /api/v1/containers/actions/bulk` (`{ action, targets, force }`, 1–200
  distinct targets) admit `container.*` and `containers.bulk` operations. `force` is kept only for
  remove and the timeout only for stop and restart. `GET /api/v1/nodes/{nodeId}/containers`, the
  native `list_containers(node, page)`, and the Tauri `list_container_inventory` are removed.
- **Executors.** One lifecycle executor type registered for the six kinds (the logic is identical
  per action) and the bulk executor on spec 06's runner, both in `server/src/containers/operations/`.
  Container actions dial with their own channel deadline (RPC timeout + grace + 20 s); the read
  timeout of 2 s killed long stops.
- **Native client and IPC.** `ContainerItem` carries the new fields plus `nodeId`/`nodeName`;
  `ContainerList` adds per-node outcomes. `container_action` takes a `ContainerTarget`;
  `containers_bulk_action` validates targets locally. The collection body ceiling rises to 4 MiB
  for 2,000 rows. `InventoryError` gains `too_many_containers`.
- **Desktop.** `/containers` replaces `/tasks`. A silent node's last rows are carried forward in the
  query function as stale (UNKNOWN, actions disabled), never in an effect. Exec and Logs entries
  render disabled until specs 15 and 16 add their screens. Tracked operations carry the row ids
  they act on (rows fade while running) and an optional success message (removal is final); bulk
  failures mark their rows from the operation detail.

## Consequences

- Manually starting a Swarm task container revives a task Swarm already replaced; the page does not
  prevent it (the dev cluster needed cleanup after testing). Swarm-aware actions arrive with
  services in spec 11.
- The sidebar count and the page share one query, so the container list polls on every page.

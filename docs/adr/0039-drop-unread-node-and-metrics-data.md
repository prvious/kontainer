# 0039. Drop node and metrics data that no screen reads

- Status: accepted
- Date: 2026-10-04
- Spec: docs/build/05-membership-and-node-metrics.md, docs/build/13-nodes.md
- Amends: ADR 0003, ADR 0015, ADR 0023
- Decision record: PR 135 owner decisions D4.1, D4.2, D4.3, D4.4, D4.5 and D4.7; review item PT-67

## Context

Specs 05 and 13 listed fields and one route that the build carried through every layer: the
database, the agent protocol, the controller DTOs, the native client, the Tauri views and the
TypeScript types. The PR 135 review checked each against the React code, and no screen reads them.
Each one was still parsed and validated in three or four layers. CLAUDE.md says API responses carry
only what the frontend needs.

Two of them also cost something at run time:

- `disk_used_bytes` made the controller read the node's last disk value on every check-in, to
  carry it forward between the agent's `docker system df` readings.
- `GET /api/v1/nodes/{nodeId}/samples` took about 100 lines across four layers, plus tests. Every
  screen asks for the cluster series instead.

The migrations haven't shipped, so the columns are removed by editing
`m20260928_000005_add_node_membership` in place.

## Decision

### Removed

- **Disk usage (D4.1):**
  - the `node_samples.disk_used_bytes` column and the carry-forward read in `record_check_in`;
  - `diskUsedBytes` on the node's latest sample and on each series point, in the DTOs, the native
    client, the Tauri views and the TypeScript types;
  - the protobuf `NodeSample.disk_used_bytes`.
- **Agent start and first-seen times (D4.3):**
  - the `nodes.agent_started_at` and `nodes.first_seen_at` columns;
  - `agentStartedAt` and `firstSeenAt` in every layer;
  - the protobuf `CheckInRequest.agent_started_at_unix_ms`. The node agent no longer sends it.
- **Duplicate timestamps (PT-67):** `nodes.created_at` and `nodes.updated_at`. They always equaled
  the first and the latest check-in. The migration builds the table with SeaQuery's `create_table`,
  not a Loco helper that adds timestamps, so nothing requires them.
- **The per-node series (D4.5):**
  - the route `GET /api/v1/nodes/{nodeId}/samples` and its OpenAPI entry;
  - the per-node scope of the `node_samples` action, which now reads the cluster series only;
  - the native client's `node_samples`;
  - the Tauri `get_node_samples` command, the `Controller` trait's node branch and its fake;
  - the hook's `get_node_samples` branch.

  Re-add it with a per-node chart.
- **Unread response fields (D4.7):**
  - `swarmNodeId` and `swarmStatus` on each node, and the `SwarmNodeStatusResponse` schema. The
    controller still reads both to pair agents with Swarm members and to derive `health`;
  - `sampledAt` on the node's latest sample;
  - `snapshotAt` on the cluster summary and on the cluster series;
  - the `window` and `bucket` echo on the cluster series, which is now `{ points }`. The native
    client still checks the range before it sends, and still bounds and orders the points. It no
    longer compares an echo.

### Reserved protobuf tags

| Message | Tag | Name |
|---|---|---|
| `CheckInRequest` | 11 | `agent_started_at_unix_ms` |
| `NodeSample` | 4 | `disk_used_bytes` |

The tags and names are `reserved` in `agent.proto`, so neither can be reused for something else.
The controller, the agents and the desktop app are deployed separately, so both directions still
work:

- **An older agent with a newer controller.** The agent still sends both fields. Protobuf skips
  unknown fields, and a test decodes such a check-in.
- **A newer agent with an older controller.** The agent sends neither field. The older controller
  reads a missing start time as the Unix epoch, which is a valid timestamp, so it doesn't reject
  the check-in. It reads a missing disk value as absent and carries the previous one forward.

### Kept

- **`containers_paused` and `containers_exited` (D4.2).** They're what the sidebar container count
  would read from samples instead of a cluster fan-out every 5 s (S-36).
- **`agent_version` (D4.4).** It's the first thing a node card needs when agents are upgraded
  unevenly.
- **`snapshotAt` on the node list.** The stale banner (PT-25) reads it.
- **`swarm_node_id` in the database and the domain.** The join with Swarm and the departed-node
  rule (ADR 0037) need it.
- **The agent's `docker system df`, every twelfth sample.** It is what fills the volume-size cache
  that `ListVolumes` reads (spec 09), so it stays. It just no longer reports a disk total.

## Consequences

- Each check-in does one fewer database read.
- Node, summary and series responses are smaller, and the native client, Tauri and TypeScript
  models shrink with them.
- A per-node chart, a disk figure, an agent uptime or a "first seen" date each needs its field
  added back with the screen that shows it. For the disk figure, the engine's `DiskUsage` still
  computes `total_bytes`.
- Desktop builds from before this change reject the new node, summary and series responses,
  because they decode strictly. Nothing has shipped yet, and ADR 0037's summary change had the
  same effect.

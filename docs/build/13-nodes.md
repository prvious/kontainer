# 13. Nodes

## Goal

Node cards joining agent membership with the manager's Swarm node list, the cluster load strip,
and the degraded and down states. Add node (enrollment) is deferred.

Design: Nodes `psaVB` (light `LnwF2`); states `SedYE` (04.S5 degraded agent and expiring
certificate), `MjblZ` (04.S6 add node, deferred); captions `mxibR`, `WF3UC`. Gap G17.

## Server

- Extend spec 05's `GET /api/v1/nodes`: join each membership row to the manager's Swarm node by
  `swarm_node_id` (manager engine `list_swarm_nodes` from spec 11): `role` manager/worker,
  `isLeader`, `availability`, `swarmStatus` (ready/down/unknown), `address`. A Swarm node with no
  agent appears with `agentStatus: "no-agent"`; an agent whose Swarm node is unknown appears with
  role `unknown`.
- Departed nodes (ADR 0037): once the manager has answered, an offline agent row whose
  `swarm_node_id` Swarm no longer lists (a removed or replaced node) is left out. An offline row
  that never named a Swarm id stays: an agent that never joined is a real problem. When several
  rows claim one Swarm id, the newest check-in pairs with it and the older claimants count as
  unlisted. A manager that doesn't answer leaves every row in. Stacks apply the same rule to
  their CPU and memory figures; the other fan-out lists still name a departed node unavailable
  until its rows are pruned.
- Derived `health` per node: `active` when the agent is online and Swarm says ready; `degraded`
  when the agent is degraded or Swarm is not ready; `down` when the agent is offline or Swarm says
  down.
- `GET /api/v1/cluster/summary` → nodes total, active, degraded, down; managers total and
  reachable, both null when the manager doesn't answer (unknown is not zero: the Overview then
  says `manager unreachable`, and `no swarm manager` only for zero managers); quorum state
  (`healthy` when the manager answers and every manager is ready, else `degraded`);
  aggregate CPU percent (average over online nodes), memory used and total (sums), containers
  running (sum). Used by this page's strip, the Overview (spec 14), and the status bar.

## Desktop

- Route `/nodes`. Tauri: `list_nodes` (exists), `get_cluster_summary`. Hooks `useNodes()`,
  `useClusterSummary()`.
- Header: `NODES`, subtitle `N nodes · a active · b down · Docker Swarm cluster`, ADD NODE
  disabled with tooltip "Not built yet" (G17; the dialog in `MjblZ` is not built).
- Cluster Load strip (`xtTtS`): tile, `CLUSTER LOAD` with `N nodes · a reachable · C containers`;
  readout: quorum StatusBadge `HEALTHY` / `DEGRADED`, `AGGREGATE CPU` percent, `MEMORY`
  `used GB / total GB`, `LAST 2m` SparkBars from cluster samples (`5m` window, `10s` bucket,
  showing the last 12 whole buckets; the bucket still filling is left out). A 10 s bucket spans two
  5 s check-ins, so check-in jitter doesn't show as "N of M nodes reporting" (G29). Hovering a bar
  pins a crosshair and a ChartTooltip that says the bucket time, CPU, memory, and how many nodes
  contributed (04.S5).
- Node cards grid, three per row: name, role badge `LEADER` / `MANAGER` / `WORKER`, status dot
  and word `active` / `degraded` / `down`; rows IP Address, OS / Arch, Docker, Containers
  `n running`; CPU track `32% · 4 cores`; MEMORY track `5.2 GB / 8 GB`.
- Down card: values `—`, empty tracks, a destructive banner "Node unreachable since 47m ago"
  (relative to `lastCheckInAt`).
- Degraded card (04.S5): amber note "Agent last checked in 3m ago", figures dimmed but visible.
  The expiring-certificate warning and RENEW NOW are not built (no certificate lifecycle yet);
  record that in `design-gaps.md` as G24 "Certificate expiry surfaces".
- Sidebar NODES badge shows `N DOWN` when any node is down. Degraded nodes don't count: degraded
  includes a node that merely has no agent.

## Tests

- Controller: join cases (agent + swarm, agent only, swarm only); health derivation table;
  summary math with a fake engine and fake samples.
- Real cluster: three cards with LEADER on the manager; stop `worker-2`'s agent → degraded then
  down with the banner; `docker node update --availability drain` on a worker shows availability.
- Desktop: cards per health; strip hover tooltip contributor count; badge text.

## Done when

- `/nodes` shows the dev cluster with correct roles, live CPU and memory tracks, and the down state
  when an agent is stopped; all checks pass; `PROGRESS.md` updated.

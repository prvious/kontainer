# 0037. Node status from fresh samples, departed nodes, and unknown manager counts

- Status: accepted
- Date: 2026-10-04
- Spec: docs/build/13-nodes.md, docs/build/05-membership-and-node-metrics.md
- Amends: ADR 0015, ADR 0023
- Decision record: PR 135 review items PT-30, PT-43, PT-32 and PT-27; owner decision D7a

## Context

The PR 135 review found that node health told users things that weren't true:

- A node whose Docker had stopped kept showing its last figures as live, because the newest sample
  counted however old it was (PT-30).
- A replaced node showed "1 DOWN" forever. Its old agent row stays offline under a Swarm id that
  Swarm no longer lists, and nothing ever removes it (PT-43). ADR 0015 left "removing departed
  nodes" to spec 13, and spec 13 never said how.
- When the manager didn't answer, the summary reported zero managers, and the Overview said "no
  swarm manager" during a quorum loss (PT-32).
- On a node running about 30 containers, the sampler counted only some of them, so node CPU and
  memory under-reported (PT-27).

## Decision

### (a) A node is online only with a fresh sample

- A node reads `online` only when it checked in within 15 s **and** has a sample at most 15 s old,
  measured by the database clock. An agent that keeps checking in without one (it can't read its
  Docker) reads `degraded`. A sample older than 15 s is left out of the node list, so old figures
  never read as live.
- **Ceiling:** sample times come from the agent's clock. The controller replaces a sample time only
  when it is more than five minutes from the database clock (ADR 0015). Agent clocks must therefore
  sit within 15 s of the database clock, or a healthy node reads degraded. Swarm already needs
  synced clocks. A `ponytail:` comment at `is_fresh` names this, with stamping samples on arrival as
  the upgrade.
- Each node's newest sample is read through the `(node_id, sampled_at)` primary key with a lateral
  join (owner decision D6). The cost is one index probe per node, however long retention is.

### (b) Departed nodes and the newest claimant

- **Departed rule.** Once the manager has answered, the node list leaves out an agent row that is
  `offline` and whose `swarm_node_id` Swarm no longer lists. An offline row that never named a
  Swarm id stays, because an agent that never joined is a real problem. When the manager doesn't
  answer, nothing is left out.
- **Newest claimant.** When several agent rows claim the same Swarm id (a rebuilt host, or an agent
  with a new identity), the Swarm node pairs with the claimant that checked in most recently. The
  older claimants count as if Swarm didn't list their id: left out if offline, otherwise shown
  unpaired with role `unknown`.
- **Stacks.** A departed node holds no containers, so its failed fan-out outcome doesn't blank the
  stack CPU and memory. Any other failed node still does (PT-31). `list_stacks` reads the manager's
  node list and shares the predicate with the node list.
- **Ceiling.** The other fan-out list pages (containers, images, volumes, networks) are unchanged.
  They still report a replaced node as `unavailable` until its rows are pruned.
- **Quorum** keeps today's rule (owner decision D7a).

### (c) Unknown manager counts are null

- `managersTotal` and `managersReachable` in `GET /api/v1/cluster/summary` are `null` when the
  manager didn't answer. Unknown is not zero. The controller logs a warning when that happens.
- The native client, the Tauri view, and the TypeScript type carry them as optional. The Overview
  says "manager unreachable" for null, and keeps "no swarm manager" for zero.

### (d) One-shot stats reads

- This replaces ADR 0015's `stream=false, one-shot=false`.
- Waiting for the daemon's second reading took a second or more per container, so the sampler's
  4 s deadline cut off the rest. On the dev cluster, a worker with 35 running containers had 9 to 11 of
  them counted before this change. After it, all 35 were counted in about 150 ms.
- The agent now reads stats with `one-shot=true`. The engine returns the raw CPU counters, and the
  sampler derives each container's CPU percent from the previous tick's cached counters, the same
  delta pattern it already uses for network rates. On the dev cluster, a busy-loop container read
  99 %, as `docker stats` did.
- A container without a previous reading (new, or one that missed last tick's deadline) has no CPU
  figure for one tick. So on the agent's first tick the node reads 0 % once.

## Consequences

- Replacing a node no longer leaves "1 DOWN" behind once Swarm forgets the old node. Removing a node
  from Swarm while its agent is still down removes it from the list as well.
- A node whose agent clock drifts more than 15 s from the database reads degraded. That is the price
  of using sample times the agent stamps.
- The cluster summary contract changes: the two manager counts become nullable.
- The containers, images, volumes and networks lists still name a replaced node as unavailable
  until its rows are pruned.

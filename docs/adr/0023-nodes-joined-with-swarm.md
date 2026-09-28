# 0023. Nodes joined with Swarm, and the cluster summary

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/13-nodes.md

## Context

Spec 13 turns the membership list from spec 05 into node cards. Each card joins an agent's
check-in with the manager's view of the same Swarm node. The spec also adds a cluster summary
for the load strip, the Overview, and the status bar. The spec expected `list_swarm_nodes` to
exist from spec 11, but spec 11 only added `swarm_status`, so this spec adds it.

## Decision

- **Engine and port.** `ContainerEngine` and `ManagerEngine` gain `list_swarm_nodes`, which
  returns `SwarmNode` in Kontainer's own types.
  - Each node carries its role, leader flag, and raft reachability (managers only).
  - It also carries availability, state, address, Docker version, platform, and resources.
  - Docker's `disconnected` state counts as down.
- **List contract.** `GET /api/v1/nodes` joins each membership row to the Swarm node its
  check-in names. This changes the contract:
  - `status` becomes `agentStatus`: `online`, `degraded`, `offline`, or `no-agent`.
  - New fields: `health`, `role` (`manager` / `worker` / `unknown`), `isLeader`,
    `availability`, `swarmStatus` (`ready` / `down` / `unknown`), and `address`.
  - A Swarm member with no agent appears as `no-agent`, with the Swarm node id as its `id`.
    Its agent fields (`agentVersion`, the three check-in times, the sample) are null.
  - An agent whose Swarm node the manager doesn't list gets role `unknown`.
  - Rows are ordered leader first, then managers, workers, and unknown, then by name.
  - Membership and the manager are read together. The manager read has the 3 s deadline from
    ADR 0021.
  - If the manager is missing or silent, Swarm fields are unknown and the list still answers
    200. Only persistence failing fails the read.
- **Health.** `down` when the agent is offline or Swarm says down. `active` only when the agent
  is online and Swarm says ready. Anything else is `degraded`, including a `no-agent` member
  and an agent Swarm doesn't know.
- **Summary.** `GET /api/v1/cluster/summary` returns:
  - Node counts by health.
  - `managersTotal`, and `managersReachable` (raft reachability).
  - `quorum`: `healthy` when the manager answered and every manager is ready *and* reachable,
    otherwise `degraded`. The spec asks only for ready. Reachable was added because Swarm can
    list a manager as ready after raft has lost it.
  - `cpuPercent`: the mean over online agents with a sample, or null when there are none.
  - Memory used and total, and running containers, each summed over those same nodes.
  - The status bar reads it. The sidebar's `N DOWN` badge counts degraded plus down, so a node
    whose agent is fine but Swarm isn't also counts.
- **Desktop.**
  - `/nodes` shows the header, the Cluster Load strip, and the node cards.
  - ADD NODE is disabled with "Not built yet" (G17).
  - The strip's bars are CPU from the cluster series (`5m` window, `5s` buckets, last 12).
    - The tooltip shows the bucket time with seconds, CPU, memory, and "n of N nodes
      reporting". That line turns amber when fewer than all nodes reported.
    - The quorum reads as a dot and a word, as the design draws it, not a tinted badge.
  - Cards sit two per row, three from `xl`.
  - Availability other than `active` shows as a chip next to the role. The design has no row
    for it.
  - OS / Arch shows the agent's operating system string.
  - A degraded card dims its figures under an amber note that names the cause: a stale agent
    ("Agent last checked in 42s ago"), no agent, or unknown Swarm state. The design's DETAILS
    button isn't built.
  - A down card keeps its static facts, muted. Its live figures read "—" and its tracks are
    empty, under "Node unreachable since 47m ago". The spec says "values —". The design keeps
    the address and platform, and so does this build.
  - The certificate warning and RENEW NOW aren't built (G24).
- **Pickers.** The pull, create-volume, and create-network node pickers only offer nodes with an
  agent, since only agents act.

## Consequences

- A manager that can't answer marks every node degraded with an unknown role. Each card says
  why, and the strip reads DEGRADED. The dev cluster showed this with the manager container
  paused.
- The dev cluster showed each transition live. Stopping an agent turned its node degraded
  within 15 s and down after a minute. Draining a worker showed its availability.

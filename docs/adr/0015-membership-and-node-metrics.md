# 0015. Membership, node samples, and fan-out

- Status: accepted (amended by 0037)
- Date: 2026-09-28
- Spec: docs/build/05-membership-and-node-metrics.md

## Context

Spec 05 replaces the one statically configured agent with membership built from check-ins, adds
resource samples, and gives later list pages a fan-out primitive. It changes the protobuf, HTTP,
native-client, and Tauri IPC contracts, adds a dependency edge, and touches the mTLS verifier.

## Decision

- **Protobuf (additive).** `CheckInRequest` fields 4–12 (`advertise_address`, `hostname`,
  `swarm_node_id`, `docker_version`, `os`, `cpu_cores`, `memory_total_bytes`,
  `agent_started_at_unix_ms`, `NodeSample sample`); `arch` reuses the existing `architecture`
  field 3. `NodeSample` carries `optional` `disk_used_bytes` and network rates so "absent" is not
  0. `CheckInResponse.check_in_interval_seconds = 3` (5 today; agents accept 1–300 and keep their
  cadence otherwise). `ContainerSummary` gains `optional` `cpu_percent`, `memory_used_bytes`,
  `memory_limit_bytes` (8–10), filled from the agent's stats cache. A pre-sampling check-in (fields
  1–3 only) still decodes and records the node without a sample.
- **Controller identity for agents.** The agent must verify the controller it dials but does not
  know the controller's instance id. `OutboundPeerVerifier::for_controller_of(cluster)` accepts a
  controller identity of that cluster with any instance id; CA, DNS-name, validity, and single-URI
  checks are unchanged, and every existing verifier still matches exactly. The agent's config gains
  `KONTAINER_CONTROLLER_RPC_ENDPOINT` (`https://host:port`) and `KONTAINER_CONTROLLER_SERVER_NAME`
  (both required) plus `KONTAINER_NODE_ADVERTISE_ADDRESS` (defaults to the binding; required when
  the binding is `0.0.0.0`).
- **Agent server names.** The controller dials `https://{advertise_address}` with TLS server name
  `{node_id}.{suffix}`, `suffix` from `settings.agent_rpc.agent_server_name_suffix` (default
  `agent.kontainer.internal`, ADR 0012's convention) and the expected SPIFFE identity from the node
  id. `settings.agent_rpc.client` and `GET /api/v1/nodes/configured` are removed; the containers
  route now requires a membership row and returns 404 for any other node.
- **Sampling.** Container-scoped, as the spec says (a `ponytail:` comment names host-level sampling
  as the upgrade). Stats are read with `stream=false, one-shot=false` so the daemon fills
  `precpu_stats`; CPU is Docker's formula, memory excludes `inactive_file`/`cache`. Network rates
  use per-container counter deltas so a container starting or stopping between samples does not
  produce a spike. A failed `docker info` sends the check-in without a sample.
- **Supervision.** The agent's check-in loop runs on the request `TaskTracker`: draining cancels
  it and the existing grace deadline joins it. The controller's hourly prune runs beside the
  servers in `serve`, stops on drain, and is joined before `serve` returns; the same pass is a
  Loco task, `metrics:prune-samples`.
- **Clock.** `last_check_in_at` and sample status use the database clock (`now()`), read once per
  action, so the controller host's clock never skews status. A sample time more than five minutes
  from the database clock is replaced by it; a repeated sample time is ignored.
- **Cluster series.** Per-node bucket averages first, then per bucket: CPU is the mean over
  contributing nodes (0–100 of the cluster, which is what specs 13 and 14 display) and memory,
  disk, running containers, and network rates are sums. Every bucket in the range is returned;
  empty buckets carry `null` figures and `nodes: 0`. Buckets align to the Unix epoch and end with
  the bucket containing now. `date_bin`'s interval is the one custom SQL fragment
  (`make_interval(secs => $1)`); SeaQuery has no interval literal.
- **Deep status.** `agentRpc` is replaced by `nodesOnline` and `nodesTotal` (null when membership
  cannot be read).
- **Settings.** `controller_settings` lives in its own `settings` domain module (spec 17 adds more
  fields); `PATCH /api/v1/settings` returns 422 `retention_out_of_range` outside 1–365 and 400 for
  malformed bodies or unknown fields.
- **Fan-out.** `nodes::fan_out` returns one `NodeOutcome` per node in input order, concurrency 8,
  a per-node deadline; `FanOutResponse` is the `{ items, nodes, snapshotAt }` shape spec 07 onward
  serves.
- **Native client and IPC.** `discover_node` is replaced by `list_nodes`, `node_samples`,
  `cluster_samples`, `get_settings`, and `update_settings` (422 maps to `InvalidRequest`). Tauri
  replaces `get_configured_node` with `list_nodes`, `get_cluster_samples`, `get_node_samples`,
  `get_settings`, and `update_settings`; every authenticated call now shares one
  refresh-once-on-401 path. `InventoryError` gains `invalid_request`. Until spec 07, the interim
  Containers screen reads the first online node.
- **Dependencies.** `futures-util` (already a workspace dependency) is added to
  `kontainer-engine` (the stats stream) and `kontainer-node-agent` (bounded concurrent stats reads).
  No new external crate.

## Consequences

- Agents must be able to reach the controller's agent RPC port; a controller that is down only
  delays membership, the agent never exits.
- `/api/v1/nodes` lists every node that ever checked in; removing departed nodes is spec 13's
  concern.
- Production certificates must carry the `{node_id}.agent…` DNS name and a controller DNS name the
  agents are configured with.

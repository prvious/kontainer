# 05. Membership and node metrics

## Goal

Replace the single statically configured agent with real membership: agents check in, the
controller knows every node, its status, and its latest resource sample, and can fan a read out to
all online nodes with explicit per-node outcomes. This is the substrate for every list page, the
Nodes page, the Overview cards, and the status bar.

Design consumers: Nodes `psaVB`, Overview `WCBzg`, Containers strip `DILCb`, status bar `KYxT3`,
state `SedYE` (04.S5 degraded agent), state `VfepO` (03.S4 partial results).
Decisions: ADR 0003 (samples, no spool), ADR 0004 (dev TCP).

## Protobuf (`kontainer.agent.v1`, additive only)

- `CheckInRequest` gains: `advertise_address` (host:port the controller dials for `NodeAgent`),
  `hostname`, `swarm_node_id` (from `docker info`, may be empty), `docker_version`, `os`, `arch`,
  `cpu_cores`, `memory_total_bytes`, `agent_started_at`, and `NodeSample sample`.
- `NodeSample`: `sampled_at`, `cpu_percent` (0–100 of the node), `memory_used_bytes`,
  `disk_used_bytes` (optional, see below), `containers_running`, `containers_paused`,
  `containers_exited`, `net_rx_bytes_per_sec`, `net_tx_bytes_per_sec`.
- `CheckInResponse` gains `check_in_interval_seconds` so the controller sets the cadence
  (default 5).
- `ContainerSummary` gains `cpu_percent`, `memory_used_bytes`, `memory_limit_bytes` (spec 07 uses
  them; fill them here from the stats cache).

## Node agent

- New config: `KONTAINER_CONTROLLER_RPC_ENDPOINT` (`https://host:port`) and
  `KONTAINER_CONTROLLER_SERVER_NAME`, both required; the client certificate is the agent's own.
- A supervised `check_in` task: every interval, collect a sample, call `CheckIn`, adopt the
  returned interval. Failures log at warn with backoff capped at 30 s; the agent never exits
  because the controller is away.
- Sampling, all through the engine trait, never Bollard types outside the engine crate:
  - `docker info` for counts, cores, total memory, versions, Swarm node id;
  - one-shot `stats` for every running container with concurrency 8 and a deadline of 80 % of
    the interval; node `cpu_percent` = sum of container CPU percent ÷ cores, `memory_used_bytes` =
    sum of container usage. This is deliberately container-scoped so dev (TCP to DinD) and prod
    (socket) report the same thing. Mark it with a `ponytail:` comment naming host-level sampling
    as the upgrade.
  - `disk_used_bytes` from `docker system df` every twelfth sample only (it is expensive); other
    samples omit it and the controller carries the last value forward.
  - Network bytes per second from the delta of summed container `networks` counters between
    samples.
- The per-container stats from the last tick are kept in an in-memory cache that `ListContainers`
  reads, so the list carries CPU and memory without extra Docker calls.

## Controller

### Schema (builder migrations)

- `nodes`: `id` text primary key (the agent's node id from its certificate), `cluster_id`,
  `hostname`, `swarm_node_id` nullable, `advertise_address`, `agent_version`, `docker_version`,
  `os`, `arch`, `cpu_cores`, `memory_total_bytes`, `agent_started_at`, `first_seen_at`,
  `last_check_in_at`, `created_at`, `updated_at`.
- `node_samples`: `node_id` references nodes, `sampled_at`, `cpu_percent` real,
  `memory_used_bytes` bigint, `disk_used_bytes` bigint nullable, three container counts,
  `net_rx_bytes_per_sec`, `net_tx_bytes_per_sec`; primary key `(node_id, sampled_at)`, index on
  `sampled_at`.
- `controller_settings`: single row `id = 1`, `metrics_retention_days` int default 7,
  `updated_at`. Seeded by the migration. Spec 17 adds the UI; the route lives here.

### Actions (`server/src/nodes/` and `server/src/metrics/`, one file per action)

- `record_check_in`: verify the certificate identity matches `agent_id` (exists), upsert the node
  row, insert the sample carrying forward `disk_used_bytes` when absent, all in one transaction.
- `list_nodes`: node rows joined with the latest sample and a derived status: `online` when the
  last check-in is within three intervals, `degraded` within 60 s, otherwise `offline`. Status is
  computed from `now()` in the query, never stored.
- `node_samples`: `(node_id | cluster, window, bucket)` → averages per `date_bin` bucket; cluster
  scope sums across nodes and returns the count of nodes that contributed per bucket. Missing
  buckets are returned as `null`, never 0.
- `prune_samples`: delete samples older than `metrics_retention_days`, in batches of 10,000.
- `get_settings` / `update_settings` for `metricsRetentionDays` (1–365).

### Agent clients and fan-out

- Replace the static `AGENT_CLIENT_*` settings and `GET /api/v1/nodes/configured` with clients
  built per node from `advertise_address` and the expected identity
  `spiffe://kontainer.internal/swarm/{cluster}/node/{id}/agent`. Keep connect-per-call; add a
  channel cache only if a measurement shows it matters.
- `fan_out<T>(nodes, timeout, f)` in `server/src/nodes/fan_out.rs`: bounded concurrency 8, a
  3-second deadline per node, returns one `NodeOutcome<T>` per node: `Ok(T)` or
  `Err(Unavailable | Timeout | Error(code))`. Never fails the whole call because one node failed.
- Public DTO shape shared by every fan-out route:
  `{ items: [...], nodes: [{ nodeId, status: "ok" | "timeout" | "unavailable" | "error", message? }], snapshotAt }`.

### Routes (protected)

- `GET /api/v1/nodes` → membership rows with status, latest sample, and nullable Swarm fields
  that spec 11 fills.
- `GET /api/v1/nodes/{id}/samples?window=1h&bucket=60s` and
  `GET /api/v1/cluster/samples?window=24h&bucket=15m`. Windows allowed: `5m`, `15m`, `1h`, `24h`,
  `7d`; bucket must divide the window into at most 200 points.
- `GET /api/v1/settings` and `PATCH /api/v1/settings`.
- `GET /api/v1/status/deep`: the agent check becomes "N of M nodes online".
- Supervised background task in `app.rs`: hourly `prune_samples`. Register it where
  `register_tasks` is empty today; run under the existing cancellation and shutdown.

## Desktop

- Tauri: `list_nodes`, `get_cluster_samples(window, bucket)`, `get_node_samples(nodeId, window,
  bucket)`, `get_settings`, `update_settings`. Remove `get_configured_node`.
- Hooks: `nodesQueries` factory (`['nodes', profileId]`, `['nodes', profileId, 'samples', scope,
  window, bucket]`); polling interval from settings; `keepPreviousData`.
- Status bar fields `CPU`, `MEM`, and the sidebar `N DOWN` badge come from `useNodes()`.
- No page in this spec; spec 13 builds the Nodes page.

## Tests

- Proto: new fields round-trip; old clients without a sample still check in.
- Agent: sampling math with a fake engine (two containers, known stats → node percent); disk every
  twelfth tick; the task survives controller downtime.
- Controller: upsert then update; status boundaries at 15 s and 60 s; bucket query with a gap
  returns `null` for that bucket; cluster query counts contributors; prune deletes only old rows;
  settings validation.
- Fan-out with fake agents: one ok, one timeout, one unavailable → three outcomes, items from the
  ok node only, total time bounded by the deadline.
- Real cluster: three dev agents appear `online` within 15 s of start; stopping one flips it to
  `degraded` then `offline`.

## Done when

- `GET /api/v1/nodes` on the dev controller lists manager, worker-1, worker-2 as `online` with
  CPU and memory figures that change over a minute.
- Cluster samples for `24h/15m` return 96 buckets with nulls before the agents started.
- Deep status reports `3 of 3 nodes online`; killing an agent changes it.
- All checks pass; `PROGRESS.md` updated; an ADR records the protobuf and DTO changes.

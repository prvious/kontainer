# Performance and resource protection in Kontainer v2

Status: proposed production contract

Research date: 2026-08-19

Scope: manager and Docker Engine pressure, discovery, resource inventory, node telemetry, images, logs, exec, gRPC, persistence, scheduling, desktop refresh, Rust runtime behavior, overload handling, and performance qualification.

## Executive decision

Kontainer v2 must decouple user activity from Docker Engine activity. A page refresh, a second desktop, or a slow API consumer must not cause another cluster-wide `TaskList`, another per-container inspection pass, or another sequential follower sweep. Controllers should maintain bounded, immutable resource snapshots from one filtered Engine event stream plus periodic reconciliation; node agents should collect node-local telemetry and inventory through one shared Bollard client; public requests should read, filter, and page those snapshots.

The production shape is:

- one controller per supported manager, as specified in [cluster communication](cluster-communication.md), with a cluster-wide scheduled-work lease only for singleton work;
- one node agent per Swarm node, collecting only node-local information;
- one cloneable, API-version-negotiated Bollard client per local Engine, never a new client per operation;
- event-driven invalidation for promptness and jittered full reconciliation for correctness;
- local, one-shot container statistics collected concurrently under per-node permits and pushed in bounded batches;
- bounded fan-out with per-target results, deadlines, cancellation, and partial-completeness metadata;
- shared caches and request singleflight so UI observers do not independently poll Docker;
- bounded item-and-byte queues for every stream, with admission control before Docker work starts;
- a shared transactional database for the production HA profile, with batch writes, retention, rollups, and idempotency; SQLite remains a single-controller compatibility profile;
- explicit small, medium, and large workload envelopes and a load gate for any deployment outside them.

These are release constraints, not merely deployment suggestions. Overload must produce a visible `429`, `RESOURCE_EXHAUSTED`, partial result, gap, or stale observation before it causes an out-of-memory kill, unbounded queue, Docker socket starvation, manager instability, or misleading success.

## Why the manager is a protected dependency

Swarm managers run Raft and serve cluster management. Adding managers increases the acknowledgement and network work required for writes, while losing manager quorum stops management even though existing tasks may continue. Docker therefore recommends an odd manager count and calls out manager resource starvation explicitly ([Docker Swarm administration](https://docs.docker.com/engine/swarm/admin_guide/)). Kontainer must treat manager Engine work as a scarce control-plane budget, not as an unlimited query backend.

Docker events are valuable invalidation hints, not a durable change log. The CLI documents that only 256 past events are returned, and Swarm-scoped events are visible on managers while local events remain local ([Docker system events](https://docs.docker.com/reference/cli/docker/system/events/)). Events can be missed during disconnect, daemon restart, failover, or a burst larger than retained history. The accepted design is therefore **events plus reconciliation**, never events alone and never tight polling alone.

## What v1 does today

The current implementation is useful as a functional inventory, but its load paths cannot be carried into production v2 unchanged. The observations below are from the current code, not inferred future risks.

### Schedules, discovery, and gRPC

The leader schedule in [`server/app/console/kernel.go`](../../server/app/console/kernel.go) runs production discovery every ten seconds, metrics collection every minute, and metric pruning every five minutes. Development discovery runs every five seconds. `SkipIfStillRunning` prevents overlap, but jobs use background contexts and there is no startup or per-instance jitter. The metrics schedule ignores the configured metrics interval.

Every production discovery pass in [`swarm_discovery.go`](../../server/app/services/cluster/swarm_discovery.go):

1. lists desired-running tasks for the follower service from a manager;
2. lists every Swarm node for hostname enrichment;
3. probes each running follower sequentially with a three-second gRPC health timeout;
4. replaces the entire in-memory follower registry.

Worst-case discovery time therefore grows as three seconds times the number of unreachable followers. While it is still running, later schedule ticks are skipped. A transient failure of either manager list clears all followers. There is no jitter, suspect grace, bounded parallelism, or reconnect admission. The connection pool is keyed by address, retains connections indefinitely, and normally evicts a connection only after a health RPC to that same address fails.

Metrics collection in [`collect_and_store.go`](../../server/app/actions/metrics/collect_and_store.go) collects the controller node first and then calls every follower sequentially with a five-second deadline. Image inventory and cluster prune use the same serial fan-out shape. gRPC responses are unary and unpaged, and v1 sets no explicit message, connection, concurrent-stream, queue, or cluster-wide fan-out limits. gRPC does not impose a deadline unless the application sets one ([gRPC deadlines](https://grpc.io/docs/guides/deadlines/)).

The Docker provider also resolves the local node through an Engine `Info` call using a background context during startup, so a stuck local daemon can delay application initialization without an application deadline.

### Docker statistics and metric persistence

Each agent's container metric pass in [`collect_container_metrics.go`](../../server/app/actions/metrics/collect_container_metrics.go) first performs a Swarm task list, filters to the local node, and then calls `ContainerStats(stream=false)` for each running container **sequentially**. A worker does not need a manager-wide task query to discover its own Swarm containers; local containers can be filtered by Swarm labels.

V1 does not request Engine `one-shot=true`. Docker added that option specifically because `stream=false` otherwise waits for two collection cycles, about one second, to calculate CPU use; the option and service-list status counts were added in API 1.41 ([Engine API version history](https://docs.docker.com/reference/api/engine/version-history/), [Moby API changelog](https://github.com/moby/moby/blob/master/api/docs/CHANGELOG.md)). Serial collection can therefore cost roughly one second per container before transport and decoding. A follower has a five-second outer deadline, while local leader collection has no equivalent deadline.

The leader stores one node metric row and one container metric row per container per cycle in a transaction, but issues an individual insert for each row. At 10,000 live containers and a one-minute interval, raw container history alone is 14.4 million rows per day.

Cluster metric reads in [`get_cluster_metrics.go`](../../server/app/actions/metrics/get_cluster_metrics.go) select up to 100 recent node rows, deduplicate them in application code, and then execute one container query per selected node with a limit of 500. Repeated recent rows from a busy node can crowd out other nodes before deduplication, and the second phase is an N+1 database query. History reads return up to 5,000 rows. The dashboard asks for the same one-hour history repeatedly.

Pruning runs one broad delete every five minutes. The current SQLite configuration allows up to 100 open connections and ten idle connections but does not explicitly configure WAL, a busy timeout, checkpoints, or a write-queue policy. The configuration keys for metric retention also disagree: configuration registers `metrics.retention` while the provider reads `metrics.retention_hours`, so the documented setting is not applied.

### Manager list and enrichment pressure

The current Swarm actions make these Engine calls per request:

| Public operation | Current Engine work | Scaling risk |
|---|---|---|
| Services | full `ServiceList` plus desired-running `TaskList` for counts | two manager scans; API 1.41 can return service status counts directly |
| Stacks | full `ServiceList` plus desired-running `TaskList` | repeats equivalent cluster scans |
| Tasks | `TaskList` plus full `ServiceList` and `NodeList` lookup maps | three manager calls for every observer and refresh |
| Task aggregate stats | unfiltered `TaskList` over retained task history | grows with churn and Swarm task-history limit, not only live scale |
| Task detail | `TaskInspect`, `ServiceInspect`, and `NodeInspect` | three calls; repeated opens have no detail cache |
| Nodes | one unpaged `NodeList` | response and render grow with the cluster |
| Networks, volumes, configs, secrets | one unpaged list each | local-versus-Swarm scope can be misleading; config/secret metadata can be large |
| Images | local `ImageList` plus serial unpaged follower `ListImages` calls | latency is the sum of node latency; omissions look complete |

This is not a per-row inspection N+1 in the task list—the service and node maps are correctly built once—but it is still repeated cluster-wide enrichment for every client poll. There is no shared resource snapshot, singleflight, response-page limit, field projection, or stale-while-revalidate behavior. Task aggregate statistics include historical tasks even when only current workload health is needed. Docker's default Swarm task-history limit is five completed tasks per instance and can be changed by operators, so history cardinality is not under Kontainer's control ([`docker swarm init --task-history-limit`](https://docs.docker.com/reference/cli/docker/swarm/init/), [`docker swarm update`](https://docs.docker.com/reference/cli/docker/swarm/update/)).

At the UI's default five-second refresh, the dashboard currently mounts task list, task aggregate stats, nodes, and cluster metrics. That is five manager Engine reads every five seconds per dashboard—about one manager request per second per active dashboard—before other pages, focus refetches, or retries. Twenty dashboard users can therefore induce roughly twenty manager reads per second even when the cluster is unchanged.

### Image and other follower fan-out

Cluster image inventory lists the leader's local images, then calls every registered follower sequentially without a per-node deadline distinct from the outer HTTP request. Each follower returns its full local image inventory in one response. A follower error is logged and omitted; the response does not prove which nodes were attempted or whether the aggregate is complete.

Cluster-wide image prune is also serial. Successful nodes are aggregated, errors are logged and omitted, and the caller receives no operation ledger. Retrying after a timeout may repeat a destructive operation on nodes that already succeeded. Targeted removal uses a single node but still relies on caller-supplied routing in v1. [Swarm resources](swarm-resources.md) defines the v2 authority and partial-result contract; this document adds the admission and resource limits.

### Logs and exec streams

Service and task log handlers correctly use Docker's manager-native log aggregation rather than inspecting every task. However, each request creates its own Engine log source. The v1 mux reader trusts Docker's 32-bit payload length and allocates the announced record before applying a limit; one frame can request gigabytes. It converts and splits records, emits one JSON WebSocket message per line, and has no write deadline, application queue, byte rate, ping/pong liveness, or source/session quota. A slow WebSocket can block the Docker reader indefinitely.

The React log hook caps the visible array at 5,000 records, but its paused buffer is unbounded. It performs a state update and array copy for each record. Reconnect uses 1/2/4/8/16-second delay without jitter and resets after opening, so a flapping endpoint can reconnect aggressively and replay the original tail. Hidden log tabs remain connected, and the DOM renders all retained records without virtualization.

Exec similarly has a UI-only ten-session limit but no server admission limit, frame limit, byte rate, idle deadline, maximum lifetime, or bounded relay queue. Remote sessions add a bidirectional gRPC stream. The detailed limits and lifecycle in [exec](exec.md) are authoritative; the shared runtime, socket, and user budgets in this document apply in addition.

### Desktop and UI refresh

The desktop's default refresh interval is five seconds, with choices of 3, 5, 10, or 30 seconds and off. Most TanStack queries independently use that interval and do not set a nonzero `staleTime`. The default query client also retains library defaults for focus/reconnect refetch and read retries. TanStack Query documents that every active `QueryObserver` owns its polling timer and only simultaneous in-flight fetches for the same query are deduplicated; it also documents the focus behavior and other important defaults ([important defaults](https://tanstack.com/query/latest/docs/framework/react/guides/important-defaults), [polling](https://tanstack.com/query/latest/docs/framework/react/guides/polling)).

Query keys generally do not include the selected agent/profile identity, risking cross-cluster cache reuse. Mutations invalidate several broad keys and immediately refetch every active observer. Every Tauri HTTP action constructs a new Reqwest client rather than using a connection-pooled client per cluster profile. Most calls have a ten-second timeout, image prune has thirty seconds, and there is no shared desktop concurrency budget. Hidden log and exec tabs stay mounted and continue to consume streams.

The status hook separately polls `/api/me` every 15 seconds. V1's HTTP middleware has a 4 MiB body limit and a three-second ordinary request timeout, but no installed per-user or global rate limiter for these heavy routes. Upgraded log/exec handlers use background contexts after the upgrade, so the ordinary timeout does not bound their lifetime.

### Current amplification summary

| Trigger | V1 amplification |
|---|---|
| One dashboard refresh | 5 manager reads plus database work |
| One task-list refresh | 3 manager list calls |
| One task detail | 3 manager inspections |
| One metric cycle | local collection plus N sequential follower RPCs plus N local-container serial stats loops |
| One image inventory | N sequential full image lists and one unbounded aggregate |
| One discovery pass | 2 manager lists plus N sequential health RPCs |
| One log session | one manager Docker log stream and one WebSocket relay |
| One remote exec | one public WebSocket, one gRPC stream, and one Docker hijacked connection |
| One extra active UI user | another set of timers and cache misses; Docker work grows approximately linearly |

## Supported workload envelopes

The profiles are qualification envelopes, not claims about Docker's absolute limits. A cluster uses the smallest profile that covers **every** dimension. A deployment beyond the large envelope is unsupported until the same suite is rerun with an approved profile; it must not silently inherit the large settings.

| Dimension | Small | Medium | Large |
|---|---:|---:|---:|
| Swarm nodes | 3 | 25 | 100 |
| Managers/controllers | 3 / 3 production; 1 compatibility | 3 | 5 |
| Services | 50 | 500 | 2,000 |
| Live tasks/containers | 200 | 2,500 | 10,000 |
| Retained task records | 1,000 | 20,000 | 100,000 |
| Image placements across nodes | 3,000 | 25,000 | 100,000 |
| Live containers on hottest node | 100 | 250 | 500 |
| Task replacements per minute for a 15-minute burst | 20 | 250 | 1,000 |
| Concurrent authenticated users | 5 | 25 | 100 |
| Concurrent log sources per controller | 16 | 64 | 128 |
| Concurrent exec sessions cluster-wide | 20 | 50 | 100 |
| Node metric interval | 30 s | 30 s | 30 s |

A task replacement creates historical task records and events even when live cardinality is flat. Qualification therefore includes both the live and retained counts and the churn burst. Lists for normal UI paths use current desired/running tasks. Historical task browsing is a separately paged and scoped endpoint.

## Production resource budgets

The following are starting defaults and acceptance ceilings. They must be expressed in deployment configuration, exported as metrics, and validated on the supported minimum hardware. A “hard memory” value is the container limit; normal steady state must remain below “steady RSS” so a burst does not immediately OOM.

### Process, cache, and persistent-storage budgets

| Budget item | Small | Medium | Large |
|---|---:|---:|---:|
| Controller CPU reservation / limit | 0.5 / 1 vCPU | 1 / 2 vCPU | 2 / 4 vCPU |
| Controller steady RSS / hard memory | 384 / 512 MiB | 768 MiB / 1 GiB | 1.5 / 2 GiB |
| Controller resource-cache byte cap | 96 MiB | 256 MiB | 512 MiB |
| Node-agent CPU reservation / limit | 0.10 / 0.5 vCPU | 0.25 / 0.75 vCPU | 0.5 / 1 vCPU |
| Node-agent steady RSS / hard memory | 128 / 256 MiB | 192 / 384 MiB | 256 / 512 MiB |
| Node-agent inventory/telemetry cache cap | 32 MiB | 64 MiB | 128 MiB |
| Controller shared-database pool cap | 8 | 16 | 32 |
| Tokio process task ceiling, excluding short request tasks | 2,000 | 8,000 | 20,000 |
| Open-file target / hard minimum | 2,048 / 8,192 | 8,192 / 32,768 | 20,000 / 65,536 |
| Cluster metric/operation persistent-storage budget | 2 GiB | 10 GiB | 30 GiB |

Resource-cache caps cover owned snapshot data and indexes, not allocator headroom, TLS buffers, WebSocket queues, or database client buffers. Cache insertion must reserve bytes first. If a refreshed snapshot would exceed the cap, omit optional detail fields or reject it and continue serving the previous snapshot as stale; do not temporarily hold two unbounded generations. Snapshot construction may use at most 25% of the process hard-memory limit as transient headroom.

For a hot node with more than 500 live containers, increase the node-agent profile only after stats-cycle qualification. Do not merely increase the hard memory limit; Docker call concurrency and daemon impact remain the limiting resources.

### CPU, memory, disk, and network acceptance

Under the profile's steady test:

- controller CPU averaged over five minutes is at most 50% of its reservation and p99 one-minute CPU is below its limit;
- node-agent CPU averaged over five minutes is at most 10% of one CPU on ordinary nodes and 20% on the hottest qualified node;
- process RSS after warm-up is below the steady target, has no positive leak slope greater than 1% of its hard limit per hour, and at the end of an eight-hour stress run is no more than 10% above hour one;
- file descriptors, Tokio tasks, gRPC streams, and semaphore permits return to within 2% or 20 units, whichever is larger, within five minutes after churn stops;
- controller↔agent, database, and Engine background traffic, excluding public responses and log/exec payloads, stays below 0.5/5/25 Mbit/s cluster-wide sustained for small/medium/large and below 2/20/100 Mbit/s for any one-minute window;
- Kontainer logical database writes stay below 0.5/2/8 MiB/s sustained for small/medium/large; retention/checkpoint work uses at most 25% of measured database I/O capacity and yields whenever foreground write p95 exceeds 500 ms;
- telemetry is no more than 1 MiB or 2,000 samples per batch, inventory chunks are no more than 1 MiB or 500 entries, and public JSON pages are no more than 2 MiB;
- disk use triggers downsampling/pruning at 70%, stops optional history at 85%, and rejects nonessential writes at 95%; audit, identity, idempotency, and lease records have protected quota;
- no request path runs `VACUUM`, a full-table compaction, or an unbounded delete.

Log and exec payload traffic has separate per-session and per-principal budgets below because it is user-driven. Overlay encryption and gRPC compression alter CPU/network tradeoffs and must be included in qualification when enabled.

## Protecting managers and Docker sockets

### Shared resource snapshots

Each controller owns one `SwarmSnapshotService` per cluster:

1. open one filtered manager Engine event stream;
2. perform an initial full reconcile before reporting fresh readiness;
3. apply event hints to mark affected resource kinds or IDs dirty;
4. coalesce invalidations for 250 ms;
5. issue a targeted inspect when the event contains a stable ID and the cost is lower than a full list;
6. perform jittered full reconciliation even when events appear healthy;
7. atomically publish an immutable `Arc<Snapshot>` containing compact DTOs and lookup indexes.

The minimum reconciliation schedule is:

| Data | Event-driven target | Full reconcile | Maximum ordinary staleness |
|---|---:|---:|---:|
| Services, nodes, stacks | yes | 60 s ±20% | 60 s |
| Current tasks and service status counts while UI active | indirect hints only | 10 s ±20% | 10 s |
| Current tasks while no consumer is active | indirect hints only | 30 s ±20% | 30 s |
| Networks, volumes, configs, secrets metadata | yes | 60 s ±20% | 60 s |
| Agent membership | yes plus agent stream | 30 s ±20% | 35 s suspect TTL |
| Image placements | node-local events where supported | 60 s while viewed; 5 min idle | response reports observed time |
| Node/container metrics | pushed sample | 30 s ±10% | 45 s |

After an event disconnect, cursor rejection, daemon restart, manager change, decode error, or detected sequence/time gap, mark affected snapshots stale and run one full reconcile. Because Docker retains only a bounded recent event window, `since` is an optimization rather than proof of continuity.

Docker's documented event types include service, node, container, network, secret, and config, but not a Swarm task event type ([Docker system events](https://docs.docker.com/reference/cli/docker/system/events/)). Service/node and node-agent-local container events may dirty the task view, but the 10/30-second filtered `TaskList` reconciliation is the correctness source. Kontainer must not promise two-second task visibility from an event that Engine does not expose.

Every reconcile is singleflight by cluster and resource kind. It has at most one running attempt and one coalesced pending dirty bit; it never queues every event or timer tick. A failed refresh retains the last valid immutable snapshot with `observedAt`, `staleSince`, and an error summary. Cache reads never wait behind an already-running refresh unless the endpoint explicitly requests strong freshness and can fit within its deadline.

### Lists, enrichment, and task churn

Resource responses are built from snapshot indexes, not new Engine calls:

- services use `ServiceList(status=true)` on API 1.41+ for running/desired counts; older supported daemons may join one current-task snapshot rather than make a user-triggered list;
- task enrichment uses service and node maps from the same or explicitly named snapshot generation;
- current workload health lists only current desired/running tasks;
- completed task history requires a service/stack/time filter, defaults to 50 rows, and has an indexed cursor;
- normal pages default to 50 and allow at most 100 rows;
- inventory export is a separate permission and endpoint, with at most 10,000 rows or 10 MiB per request, one concurrent export per principal, and a resumable cursor rather than an in-memory giant response;
- detail inspections are cached by resource ID plus Docker version index in a byte-bounded LRU. A mutation or matching event invalidates the entry;
- secret data and config payloads are never held in a shared list cache; lists retain only allowed metadata and details are short-lived, authorized, and zeroized where practical.

No list implementation may call `inspect` inside an unbounded row loop. A code review or benchmark should fail an implementation whose Docker request count is a function of returned row count unless the endpoint is explicitly a bounded node fan-out. One hundred concurrent dashboard clients must change cache-read CPU and network, but must increase manager Engine request rate by less than 5%.

High churn is handled by compact IDs and versioned maps, not by retaining an event object per transition. The event coalescer retains at most one dirty marker per resource ID and a global dirty-kind fallback. At 100,000 outstanding dirty IDs it discards the ID set, marks the kind fully dirty, and schedules one reconcile. It does not enqueue 100,000 inspections.

### Engine call budgets and priority

Each process uses one shared Bollard `Docker` handle per local daemon. Bollard documents `Docker` as a cloneable asynchronous client, exposes Engine event and stats streams, and supports client timeouts; its stats example uses `stream(false).one_shot(true)` ([Bollard `Docker`](https://docs.rs/bollard/latest/bollard/struct.Docker.html), [`Docker::stats`](https://docs.rs/bollard/latest/bollard/struct.Docker.html#method.stats)). Cloning the handle is acceptable; constructing a new transport/client for each call is not.

Use separate semaphores so low-value background work cannot occupy every Docker connection:

| Engine work class | Per-process permits | Queue/wait policy |
|---|---:|---|
| Health, version, readiness | 2 reserved | wait up to 500 ms, then fail readiness |
| Manager snapshot/reconcile reads | 4 | one running plus one dirty bit per kind; no request queue |
| User detail reads/inspects | 8 | wait up to 250 ms; then `429` |
| User mutations | 4 cluster-wide, 1 per resource | wait up to 1 s; then `429`; durable idempotency |
| Node-local stats | 8 per agent | current cycle only; skip overdue items |
| Node-local inventory | 4 per agent | one singleflight per inventory kind |
| Image prune | 1 per node, 2 cluster-wide | operation ledger; never automatic retry |
| Docker log source | profile cap 16/64/128 per controller | reject before opening Engine stream |
| Docker exec attachment | 20 per node and 100 cluster-wide | limits from [exec](exec.md) also apply |

The cluster-wide manager background target is no more than two short Engine requests per second averaged over five minutes and no more than eight started in any second, independent of UI-user count. User mutations are excluded from the average but remain capped at four concurrent. Qualification fails if Kontainer raises manager-daemon CPU by more than 5 percentage points in medium or 10 in large, or degrades an independent manager API probe's p99 latency by more than 10% relative to the no-Kontainer baseline.

Default time budgets are:

| Operation | Per attempt | Overall |
|---|---:|---:|
| Engine ping/version | 2 s | 2 s |
| Detail inspect | 3 s | 3 s |
| Manager list/reconcile kind | 10 s | 15 s full snapshot |
| Node-local list | 5 s | 5 s |
| One container stats sample | 2 s | 25 s node cycle |
| Internal unary health/control RPC | 3 s | 3 s |
| Read-only fan-out target | 5 s | 8 s small/medium, 15 s large |
| Database read | 1 s | 1 s |
| Database write/transaction | 2 s | 2 s |
| Ordinary public REST request | — | 15 s |

Read-only transient work may retry once with full jitter only when time remains. Mutations, prune, exec open after Docker creation, and streams are never automatically replayed. Timeouts cancel downstream contexts. A timeout is an outcome, not permission to detach a task and let it consume a semaphore indefinitely.

## Metrics collection without a stats storm

### Accepted collection path

Every node agent samples its local node and containers, then pushes a batch to any ready controller:

1. Discover local Swarm containers using a local `ContainerList` filter such as `label=com.docker.swarm.task.id` and `status=running`. Do not call the manager `TaskList` from every worker.
2. Sample host metrics once per cycle.
3. Call Engine stats with `stream=false` and `one_shot=true` under eight permits and a 25-requests/second token bucket.
4. Retain the previous raw CPU counters by full container ID to calculate the interval. The first sample after process/container restart reports CPU percentage as unknown rather than inventing zero.
5. Stop starting samples when the 25-second cycle deadline is reached. Mark missing containers and the batch as partial.
6. Encode no more than 2,000 samples or 1 MiB per message. A node above that bound sends multiple ordered chunks under one batch ID.
7. Push at 30 seconds with stable node-derived ±10% jitter. Do not align all nodes to the minute.

Long-lived Engine stats streams per container are rejected as the default. They keep one HTTP connection and decoding task per container even when nobody views the metric and complicate reconnect and pressure control. A future hot-detail view may open a short, separately admitted stream for one container.

The per-node success target is at least 99% of eligible container samples in each five-minute window under the qualified profile. A cycle finishes in 25 seconds p99, leaving time before the next interval. If Docker is slow, the node skips the unfinished remainder and emits `sample_missing{reason="deadline"}`; it does not overlap another cycle or create a catch-up queue.

Agents buffer at most two batches or 2 MiB, whichever comes first, while controllers/database are unavailable. Telemetry is freshness-oriented: discard the oldest unsent batch, emit a gap counter, and preserve the newest. Blocking Docker collection behind a long network outage is rejected.

### Ingestion, batching, and retention

Controllers validate node identity, batch size, sample count, monotonic batch sequence, timestamp skew, and container/node ownership before enqueueing a batch. The bounded ingest queue is 16 MiB or 100 batches per controller. Admission over the limit returns `RESOURCE_EXHAUSTED`; agents back off with full jitter and retain only their bounded newest batches.

The writer drains up to 5,000 rows or one second of work into one database transaction. It never executes one transaction per sample. Deduplication uses node boot ID plus batch sequence and an immutable sample identity so controller failover or gRPC retry cannot duplicate data.

Store:

- one upserted latest row per live container;
- one latest row per node;
- 30-second node-level raw samples for 24 hours;
- five-minute node/service rollups for 30 days;
- operation, gap, and coverage metadata needed to explain missing data.

Do not retain raw per-container time series by default. At the large envelope it would create tens of millions of rows per day without a product requirement. If per-container history becomes a requirement, qualify and adopt the external time-series tier before enabling it cluster-wide.

Queries select the latest row per node in SQL by key/index, not “latest 100 then deduplicate.” Current container metrics are one indexed query or a join against the latest table, not one query per node. History endpoints select pre-aggregated resolution appropriate to the time range and return bounded cursor pages.

### SQLite compatibility profile

SQLite is acceptable only for the explicitly non-HA, single-controller parity profile. Use one logical writer, bounded read connections, prepared batch upserts, a two-second busy timeout, short read transactions, and WAL mode. SQLite WAL permits readers and a writer concurrently but still has only one writer; long readers can prevent checkpoint progress and allow the WAL to grow ([SQLite WAL](https://sqlite.org/wal.html), [`busy_timeout`](https://sqlite.org/pragma.html#pragma_busy_timeout), [`wal_autocheckpoint`](https://sqlite.org/pragma.html#pragma_wal_autocheckpoint)).

Pin a SQLite version containing the 2026 WAL-reset fix: upstream documents the bug as affecting WAL-mode databases from 3.7.0 through 3.51.2 and fixes it in 3.51.3 with backports 3.44.6 and 3.50.7 ([SQLite WAL recent fix](https://sqlite.org/wal.html#sometimes_queries_return_sqlite_busy_in_wal_mode)). This is a production-safety gate even for the compatibility profile.

Checkpoint outside request latency, alert at a 64 MiB WAL, force an operator-visible degraded state at 128 MiB, and diagnose long readers before truncation. Retention deletes at most 10,000 rows per transaction and yields between chunks. Never run `VACUUM` while serving traffic.

The production HA profile uses the shared transactional database required by [cluster communication](cluster-communication.md). The selected engine must demonstrate the same two-second write deadline, batch throughput, lease semantics, retention cost, and storage high-water behavior. Database-engine selection is outside this performance document; passing these tests is not.

## Discovery, follower fan-out, and gRPC limits

### Membership without an N-node polling loop

Node agents establish an outbound authenticated registration/telemetry stream to a controller service endpoint. They send a lightweight heartbeat every ten seconds with stable ±20% jitter. A registration becomes suspect after 25 seconds and unroutable after 35 seconds unless manager task state proves it has been replaced sooner. Manager service and node events trigger reconciliation; Engine exposes no distinct task-event type, so each controller still performs the 30-second authoritative membership reconcile specified in [cluster communication](cluster-communication.md).

This removes sequential “ping every follower” discovery. Health RPCs remain available for on-demand diagnostics and channel validation, but are not the membership clock. An agent reconnects with exponential full jitter from one to thirty seconds. Each controller admits at most eight new agent handshakes per second and queues none beyond the transport accept backlog; excess agents retry. This turns startup and failover into a controlled ramp instead of a thundering herd.

Keep one Tonic channel per current agent instance, keyed by cluster/node/task/process generation. Close superseded channels when membership drains them; close non-control idle channels after five minutes. The number of cached channels must not exceed current qualified node count plus 10% rollout headroom.

### Read fan-out

Image placements and other genuinely node-local inventory use bounded parallel fan-out:

| Profile | Concurrent target RPCs | Concurrent fan-outs per controller | Per-target / overall deadline |
|---|---:|---:|---:|
| Small | 4 | 2 | 5 s / 8 s |
| Medium | 8 | 4 | 5 s / 8 s |
| Large | 16 | 4 | 5 s / 15 s |

Each response names every requested target as `ok`, `stale`, `unreachable`, `timeout`, `forbidden`, or `unsupported` and includes observed time and cursor. Completion order never determines result order. A caller cancellation stops starting new targets and cancels in-flight RPCs.

Inventory pages are at most 500 entries or 1 MiB per agent message. The controller incrementally merges into a byte-reserved builder and returns ordinary UI pages of at most 100 placements. It never waits for or holds every image on every node merely to show the first page. Repeated identical inventory requests share one fan-out for its TTL. Destructive fan-out has lower concurrency, durable per-node operation claims, and no implicit retry.

Sequential fan-out is rejected because latency is the sum of node deadlines. Unbounded parallel fan-out is also rejected because it creates a connection, allocation, Docker, and database burst proportional to node count. Bounded parallelism makes latency roughly the number of waves while protecting the controller and nodes.

### Tonic and protobuf bounds

Tonic exposes per-connection concurrency, load shedding, timeouts, HTTP/2 stream/window limits, and keepalive controls. Without load shedding, its concurrency layer buffers requests; without an explicit maximum, concurrent HTTP/2 streams are not bounded by the application ([Tonic `Server`](https://docs.rs/tonic/latest/tonic/transport/struct.Server.html)). Tonic also documents a default 4 MiB decode limit while encoding is unlimited unless configured, so both directions need per-method limits ([Tonic gRPC message sizing](https://docs.rs/tonic/latest/tonic/server/struct.Grpc.html)).

Production defaults:

- `max_concurrent_streams = 64` per HTTP/2 connection;
- Tower concurrency limit 32 requests per authenticated peer and 128 total per node agent/controller process;
- load shedding enabled, returning `RESOURCE_EXHAUSTED` rather than buffering behind the limit;
- 64 KiB maximum for health, registration, control, and exec input messages;
- 1 MiB maximum for metrics and inventory messages in both decode and encode directions;
- method timeouts from the Engine/deadline table, propagated end to end;
- keepalive only for active control channels, with settings verified against proxies and the overlay; no per-request channel construction;
- compression disabled by default for sub-64-KiB control and interactive messages. Enable gzip or zstd for inventory/metrics only when the benchmark shows at least 30% network reduction without violating CPU budgets.

HTTP/2 flow control is necessary but not sufficient. Every producer-consumer boundary still uses a bounded application channel and acquires byte permits before allocation.

## Streaming, records, and backpressure

### Logs

[Logging](logging.md) owns log semantics. This performance profile adopts its concrete record, queue, batching, and session defaults and adds a line limit and aggregate admission budgets:

- maximum Docker record: 256 KiB;
- maximum displayed/protocol line: 64 KiB; larger records are split or truncated with explicit metadata, never silently;
- per-session outbound queue: 2 MiB and 2,000 records;
- shared source included-buffer allowance: 512 KiB;
- WebSocket batch: 64 KiB or 25 ms;
- compatibility sessions per principal: 10;
- enhanced aggregation fan-in: at most 32 services;
- numeric tail: at most 10,000; `tail=all` requires elevated permission and is still bounded by 32 MiB or 30 seconds of startup history.

Additionally, a controller admits only 16/64/128 concurrent Docker log sources and 25/100/250 client log sessions in small/medium/large. Aggregate queued log bytes are capped at 64/256/512 MiB; queues reserve actual bytes lazily rather than preallocating their maxima. Source sharing is allowed only when authorization and request options are identical; each consumer still has its own bounded queue. One session may deliver 2 MiB/s sustained with a 4 MiB one-second burst, one principal 10 MiB/s sustained, and one controller 10/50/200 Mbit/s for small/medium/large. Those are admission ceilings, not promised throughput.

The mux decoder validates the eight-byte header and payload length **before** allocation. Oversized records are drained safely or terminate the source with an explicit `record_too_large` gap. A queue that remains full for five seconds disconnects that consumer, not every shared consumer. Docker reads have cancellation and no unbounded intermediary.

Paused UI is a presentation state, not another queue: pause freezes the viewport while records continue into the same byte-bounded ring and advances a visible dropped/gap count on overwrite. Render at most 5,000 records and 8 MiB per tab with virtualization. Hidden tabs suspend or close after 60 seconds unless the user explicitly pins background collection.

### Exec

[Exec](exec.md) remains authoritative. Shared performance gates include:

- 8 KiB open/control messages, 64 KiB input messages, and a 16 MiB maximum Docker mux record;
- 64 KiB inbound and 256 KiB outbound application queues;
- input at 1 MiB/s with a bounded burst and resize coalesced to 10/s;
- at most 100 sessions cluster-wide, ten per principal, five per container, and twenty per node;
- five-second slow-consumer deadline, 30-minute idle lifetime, eight-hour absolute lifetime;
- the Docker attachment, gRPC stream, public WebSocket, byte permits, and audit guard are owned by one cancellable session actor.

Log and exec queues acquire byte permits before reading or copying a full message. Tokio bounded MPSC queues provide backpressure by waiting when capacity is reached; unbounded channels have unbounded memory potential ([Tokio `mpsc`](https://docs.rs/tokio/latest/tokio/sync/mpsc/)). Item counts alone are insufficient because one log record may be much larger than another.

## API fairness and noisy users

Admission is applied after cheap authentication parsing but before cache misses, fan-out permits, database transactions, Docker calls, or WebSocket upgrades. Defaults per authenticated principal:

| Class | Sustained | Burst | Concurrent |
|---|---:|---:|---:|
| Cached/paged reads | 30/s | 60 | 16 |
| Strong-refresh or fan-out reads | 2/s | 4 | 2 |
| Mutations | 1/s | 3 | 2, plus cluster write permits |
| Stream opens | 2/s | 4 | log/exec session quotas |
| Export | 1/min | 1 | 1 |

Unauthenticated traffic is limited by source IP to 10 requests/s with burst 20 and stricter login/auth-provider limits. Each controller allows 64/128/256 ordinary public requests in flight for small/medium/large. Distributed production deployments enforce principal and tenant counters in the shared control store or ingress; per-process safety limits remain even when that dependency is unavailable.

Return `429 Too Many Requests` with `Retry-After` for public admission, `RESOURCE_EXHAUSTED` for internal RPC, and a stable error code identifying the exhausted class. Never include resource IDs, user IDs, node IDs, request IDs, stack names, or container IDs as Prometheus labels. Prometheus recommends keeping most metric cardinality below ten and investigating alternate designs at cardinality around 100 or more ([Prometheus instrumentation practices](https://prometheus.io/docs/practices/instrumentation/)).

If multi-tenancy is introduced, every principal budget becomes nested under a tenant budget. A tenant may use at most 25% of controller heavy-read/fan-out permits by default, with unused capacity borrowed fairly. Weighted fair queues may be added only for queued work that is already strictly bounded; streams and mutations should normally reject rather than wait indefinitely.

## Desktop refresh and response cost

The desktop creates one shared Reqwest client per cluster profile with connection pooling, bounded idle connections, and the same TLS/auth configuration. Query keys include cluster/profile identity. The UI never calls `invoke` directly from an effect; its TanStack Query hooks consume public cache-backed endpoints as required by the desktop architecture.

Default refresh behavior:

| UI data | Focused and visible | Unfocused/minimized | Server source |
|---|---:|---:|---|
| Current workload/task health | 10 s | paused; one refresh on focus | 10/30-second task snapshot |
| Node/container current metrics | 30 s | paused | 30-second pushed sample |
| Services/nodes/static inventories | 60 s | paused | event-invalidated snapshot |
| Images/local inventory while page open | 60 s | paused | cached bounded fan-out |
| Auth/session status | 60 s | paused except expiry timer | control database |
| Log/exec | explicit live session | user-pinned only | bounded stream |

The existing 3/5/10/30-second setting may remain for parity as a **UI cache refresh preference**, but it must never shorten Engine reconciliation or stats intervals below the server floors. “Refresh now” invalidates the public snapshot only if no refresh is running and the principal has a strong-refresh token. It does not perform one Docker scan per click.

Set one read retry with exponential full jitter for retryable network failures and zero automatic mutation retries. Disable duplicate focus/reconnect refetch; use Tauri native focus/minimize state because WebView browser heuristics are not the product lifecycle. On focus, coalesce all due queries into a 0–1-second stagger and let the server cache deduplicate them.

Normal tables page at 50/max 100 and virtualize beyond 200 rendered rows. Dashboards use a consolidated overview DTO so task, node, status-count, and current metric cards share one snapshot generation and one request. Detail panels fetch details only when opened. Mutations invalidate exact resource keys plus the overview generation rather than every list indiscriminately.

Public-response targets measured at the controller:

- cached normal list p95 below 250 ms and p99 below 1 s in small/medium;
- cached normal list p95 below 500 ms and p99 below 2 s in large;
- response serialization consumes no more than 50 ms CPU p99 for a 100-row page;
- first 100-row fan-out inventory page p95 below 5 s, always with completeness;
- adding users up to the profile limit does not change manager Engine request rate by more than 5%.

## Scheduler, startup, and leader failover

The controller topology, shared database, and lease/fencing semantics in [cluster communication](cluster-communication.md) are authoritative. Public requests may be served by every ready controller; only singleton schedules need a lease. Performance behavior is:

- schedules use monotonic deadlines, ±20% stable instance jitter, and at most one running plus one coalesced pending occurrence;
- an overdue or overlapping telemetry, reconcile, checkpoint, or prune tick is skipped and counted; missed intervals are not replayed in a catch-up burst;
- only the lease holder creates scheduled-work claims, and each occurrence has a unique natural key;
- a controller that cannot renew stops claiming immediately; uncertain edge-triggered work such as prune is not retried without an idempotency result;
- a starting controller waits a random 0–5 seconds, opens the event stream, performs one singleflight full reconcile, and becomes fresh-ready only after it publishes the snapshot;
- agents reconnect with one-to-thirty-second full jitter and controller handshakes are capped at eight per second;
- desktops reconnect with one-to-thirty-second full jitter and do not reset backoff merely because a short-lived socket opened;
- telemetry agents retain only two batches/2 MiB during failover; log/exec sessions end visibly rather than being replayed.

In the production profile, controller loss must move public readiness and cached reads to another controller p95 within 10 seconds. Scheduled-work ownership transfers p95 within 30 seconds and p99 within 60 seconds, without duplicate claims. A cold replacement snapshot is fresh p95 within 30 seconds and p99 within 60 seconds at the large envelope. Manager quorum loss is a dependency failure, not a Kontainer availability failure; the API may serve explicitly stale cached reads but rejects mutations.

The single-controller SQLite profile is v1 parity and explicitly non-HA. It may target restart/reschedule recovery within 60 seconds but cannot claim durable failover because its local database does not follow the task.

## Rust and Tokio runtime protections

Use one Tokio multi-thread runtime per process. Start with worker threads equal to available parallelism clamped to 2–8; override only from a validated profile. Tokio documents that worker-thread defaulting follows available cores, that blocking-thread default maximum is 512, and that the blocking queue can grow without backpressure ([Tokio runtime builder](https://docs.rs/tokio/latest/tokio/runtime/struct.Builder.html)). Kontainer must lower and bound that surface:

| Runtime setting | Small | Medium | Large |
|---|---:|---:|---:|
| Tokio worker threads | clamp 2–4 | clamp 2–8 | clamp 4–8 |
| `max_blocking_threads` | 16 | 32 | 64 |
| Simultaneous blocking database calls | 4 | 8 | 16 |
| Simultaneous CPU-heavy encode/compress jobs | 2 | 4 | 8 |
| Simultaneous blocking filesystem/OS calls | 4 | 8 | 16 |

Acquire a semaphore **before** `spawn_blocking`. Tokio notes that blocking tasks cannot generally be aborted after starting and specifically recommends semaphores for many CPU-bound operations ([`spawn_blocking`](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)). Native async database/network clients should be preferred, but an async wrapper does not make a synchronous call nonblocking.

Do not spawn one detached task per metric row, event, log line, or list entry. Process chunks, reuse buffers, and yield during CPU-heavy loops. Every spawned background task belongs to a cancellation tree and a task tracker. On shutdown, stop admission, cancel polling/events, drain bounded writes, close Docker/gRPC/WebSocket streams, then enforce a 20-second process grace period.

Runtime telemetry includes worker busy ratio, poll/scheduling latency, live task count, blocking tasks/queue, semaphore wait, channel item/byte occupancy, allocation rate, and long polls. Acceptance requires p99 runnable-to-first-poll latency below 50 ms under steady load and below 200 ms during the qualified churn burst, worker busy below 80% for five-minute windows, and no unbounded blocking queue growth. Tokio's fairness guarantee assumes a bounded task population and bounded poll duration, exactly the conditions these limits enforce ([Tokio runtime fairness](https://docs.rs/tokio/latest/tokio/runtime/)).

Avoid tuning Tokio's unstable local/global queue intervals or HTTP/2 window sizes from intuition. Change them only with before/after flamegraphs and the complete load suite.

## Cache invalidation and consistency contract

Every returned resource or metric envelope includes:

- cluster ID and controller instance;
- snapshot generation;
- `observedAt` and, where relevant, source-node times;
- `fresh`, `stale`, `partial`, or `reconciling` state;
- requested/attempted/succeeded/failed target counts for fan-out;
- warnings and stable error codes;
- a page cursor tied to the generation.

An event invalidates current state but does not rewrite historical observations. A successful mutation updates no cache optimistically unless the domain operation can construct the exact Engine result; normally it marks the resource dirty, waits for operation observation up to its deadline, and returns the operation plus latest observed generation.

Caches use size and recency bounds:

- current services/tasks/nodes and lookup maps are pinned because core screens depend on them;
- detail, local inventory, and image placement caches use LRU plus TTL;
- negative cache entries last at most five seconds;
- errors are never cached as successful empty lists;
- a failed refresh retains the prior snapshot and explicit staleness;
- cache eviction never removes operation/idempotency/audit truth.

Controllers may independently maintain manager snapshots. The shared database stores operation and telemetry truth but is not used as an event bus for every Docker resource transition. A future cross-controller snapshot bus is deferred until benchmarks show that one event stream plus sub-one-request-per-second reconciliation per controller materially pressures managers.

## Observability and SLOs

### Service objectives

Measured monthly within a healthy supported Swarm, controller quorum, and database:

| Objective | Target |
|---|---:|
| Cached read availability | 99.9% |
| Admitted mutation API availability, excluding Docker rollout outcome | 99.5% |
| Cached normal-list latency | p95 ≤250 ms medium, ≤500 ms large; p99 ≤2 s |
| Resource-event visibility | p95 ≤2 s; periodic correctness ≤60 s |
| Current task freshness while viewed | p99 ≤15 s |
| Metric freshness | p99 ≤45 s |
| Eligible metric sample coverage | ≥99% per five-minute window |
| Node metric cycle duration | p99 ≤25 s |
| Controller failover to ready API | p95 ≤10 s |
| Scheduled-work takeover | p95 ≤30 s, p99 ≤60 s, zero duplicate claims |
| Fan-out result accounting | 100% of requested targets named |
| Overload safety | zero OOM/unbounded queues; explicit shed/gap before budget breach |

Error-budget reports exclude planned maintenance only when maintenance is recorded. Docker manager/quorum/database dependency failures are broken out but still visible; they are not silently removed from user-experience reporting.

### Required telemetry

Use low-cardinality labels such as operation class, resource kind, result class, profile, peer workload kind, and error code. IDs belong in traces/log fields, not metric labels.

Required metrics and spans include:

- Engine requests, latency, in-flight permits, waits, cancellations, timeouts, and response bytes by operation class;
- manager reconcile duration, generation, age, dirty-set size, event disconnects, gaps, targeted versus full refreshes, and singleflight joins;
- task live/retained/churn counts and snapshot/cache bytes;
- fan-out targets attempted/succeeded/failed, waves, time to first page, completeness, and per-error class;
- stats eligible/sampled/missing, cycle duration, one-shot latency, batch bytes/samples, ingest queue, duplicates, clock skew, and gaps;
- database transaction/row batch size, latency, busy/deadlock/retry, pool waits, retained rows/bytes, delete chunks, disk/WAL use, and lease term/renewal;
- API rate-limit decisions, concurrency, request/response bytes, cache hit/stale/negative hit, principal class, and serialization CPU;
- log/exec sessions and sources, bytes/records, queue item/byte high water, drops/gaps, rate limits, slow consumers, and cleanup outcomes;
- Tonic connections/streams, handshake rate, message sizes, load-shed results, reconnect backoff, and channel lifecycle;
- Tokio worker busy/poll latency, tasks, blocking activity, semaphore wait, allocator/RSS, FDs, CPU, disk, and network.

Dashboards show current value, budget, and percentage used. Alerts:

- warning at 70% of any sustained concurrency, memory, cache, queue, disk, FD, or traffic budget for 10 minutes;
- page at 90%, repeated load shedding above 5% of eligible requests, metric coverage below 99%, manager probe p99 regression above 10%, stale core snapshot above 60 seconds, lease uncertainty, WAL/storage 85%, or runtime p99 scheduling above 200 ms;
- immediate page for OOM/restart loop, manager quorum loss, corrupted persistence, duplicate scheduled claim, authentication bypass, or an unbounded-size assertion failure.

Structured logs and traces are sampled under success load but retain all overload, timeout, partial, gap, lease, mutation, and security outcomes. Trace sampling itself has a CPU/network budget and must not attach log/exec payloads, secrets, registry credentials, or high-cardinality metric attributes.

## Benchmarks, load tests, soak tests, and fault tests

All numbers in this document are hypotheses until the corresponding gate passes on supported minimum hardware and pinned Docker/Bollard/Tokio/Tonic/database versions.

### Microbenchmarks

Benchmark with realistic and adversarial sizes:

- decode, compact, index, filter, sort, paginate, and serialize 1k/20k/100k task records;
- apply 1k and 10k Engine events per second, including repeated IDs and global dirty fallback;
- build and atomically swap snapshots without exceeding transient-memory headroom;
- merge 3k/25k/100k image placements incrementally;
- decode Docker log mux headers with 0, 64 KiB, 256 KiB, and malicious 4 GiB announced lengths without allocating the malicious size;
- encode/decode 64 KiB and 1 MiB protobuf messages at configured Tonic limits;
- batch/upsert 100/1k/5k metric rows and execute indexed latest/history queries;
- exercise rate-limit, semaphore, and byte-permit paths under cancellation.

Record throughput, p50/p95/p99 time, allocations/bytes, peak RSS, and CPU profiles. A regression of more than 10% in p95 or allocations on a protected benchmark blocks merge unless approved with evidence.

### Component load harnesses

Provide deterministic fake Engine and gRPC peers that can delay, stall, fail, stream oversized frames, reorder completion, and disconnect. Required cases:

- 500 live containers on one node with one-shot stats, two-second slow calls, removals during sampling, and a stalled Docker socket;
- 100 nodes with 5,000 images each, bounded fan-out, pagination, 10% unavailable peers, and a slow consumer;
- 100 concurrent desktops refreshing every three seconds, plus focus/reconnect bursts, proving manager request-rate independence;
- 128 log sources with huge records and slow/paused clients;
- 100 exec sessions with large paste/output, abandoned clients, and permit cleanup;
- 100,000 retained tasks, 1,000 replacements/minute, and 10,000 events/second bursts;
- database lock/deadlock, two-second write timeout, 10,000-row retention chunks, and 70/85/95% disk transitions;
- Tonic maximum message, one byte over maximum, 64 concurrent streams, handshake flood, and load shedding.

### End-to-end profile runs

For each profile:

1. establish a no-Kontainer manager/daemon CPU baseline while an independent harness probes `/_ping` at 1/s and representative filtered service/task lists at 0.2/s;
2. warm Kontainer for 15 minutes;
3. run 60 minutes steady with the profile's users, live objects, metrics, logs, and exec;
4. run the 15-minute churn burst;
5. repeat with 10% slow/unreachable agents;
6. compare every resource and SLO threshold.

Run a 24-hour medium soak and an eight-hour large stress. RSS leak slope, FDs/tasks/permits after recovery, database/storage growth, cache generation, event reconnects, metrics coverage, dropped telemetry, and manager impact are release artifacts.

### Chaos and failover

During load:

- restart and demote the manager hosting a controller;
- kill the schedule lease holder before, during, and after a claim;
- lose manager quorum and restore it;
- restart Docker on a manager and a hot worker;
- partition 10% of agents for 5/30/120 seconds;
- terminate the shared database, hold a long read, and induce writer contention;
- fill storage through every high-water mark;
- rotate node-agent tasks and certificates simultaneously;
- disconnect event streams longer than Docker's retained event window;
- fail a controller with active log/exec sessions and a telemetry batch in flight.

Acceptance requires bounded recovery, explicit stale/partial/gap outcomes, no duplicate scheduled claims, no unsafe mutation replay, no corrupted snapshot, and all transient resources returning to budget.

## Accepted, rejected, and deferred approaches

| Approach | Decision | Reason |
|---|---|---|
| Filtered Engine events plus jittered full reconcile | **Accepted** | Events give prompt invalidation; reconciliation repairs bounded history, reconnect, and decode gaps. |
| Poll-only resource reads | **Rejected** | Cost grows with users and refresh rate, and unchanged clusters still pressure managers. |
| Event-only cache | **Rejected** | Docker event history is bounded and not a transactional/durable resource state. |
| Immutable shared snapshots, indexes, and singleflight | **Accepted** | Makes enrichment CPU/memory bounded and disconnects UI concurrency from Engine request count. |
| Inspect every returned task/container | **Rejected** | Docker requests scale with row count and create classic N+1 socket pressure. |
| `ServiceList(status=true)` with current-task fallback | **Accepted** | Uses Engine-computed counts on API 1.41+ and avoids a redundant task scan. |
| Unfiltered all-history task lists for dashboards | **Rejected** | Cardinality grows with task churn and operator history settings. |
| Agent local container list plus one-shot stats | **Accepted** | Avoids manager scans and the two-sample wait while preserving explicit CPU-delta semantics. |
| One permanent stats stream per container | **Rejected as baseline** | Holds O(containers) connections/tasks and keeps work alive without a viewer. |
| Controller sequentially pulls every node's metrics | **Rejected** | Total latency and failure exposure grow linearly with nodes. |
| Agent push in bounded newest-first batches | **Accepted** | Spreads load with jitter, permits failover, and bounds outage memory. |
| Raw per-container history for all containers | **Rejected** | Tens of millions of rows/day at large scale without a current product need. |
| External time-series store | **Deferred** | Required if per-container history/long retention becomes a product goal; unnecessary for parity/latest metrics. |
| Shared cloneable Bollard client | **Accepted** | Reuses transport/version negotiation and follows Bollard's client model. |
| New Docker or Reqwest client per request | **Rejected** | Causes socket, connection, handshake, and allocation churn. |
| Bounded concurrent fan-out | **Accepted** | Limits pressure while avoiding serial N-times latency. |
| Serial or unbounded parallel fan-out | **Rejected** | One is too slow; the other creates node-count-sized bursts. |
| Bounded item-and-byte channels | **Accepted** | Protects against high variance and huge log/protobuf records. |
| Unbounded channels or item-only limits | **Rejected** | Cannot bound memory when messages vary in size. |
| SQLite WAL for single-controller parity | **Accepted with patched version and limits** | Simple and sufficient for compatibility, but still one writer and not HA. |
| SQLite/local volume as production HA state | **Rejected** | State does not follow a rescheduled task and cannot coordinate controllers safely. |
| Shared transactional database and short schedule lease | **Accepted for production** | Matches multi-controller architecture and supports batch ingest, claims, and fencing terms. |
| Multi-controller duplicate background snapshots | **Accepted initially within manager budget** | Keeps controllers independently ready; request rate remains low with 30/60-second reconcile. |
| Cross-controller snapshot/event bus | **Deferred** | Adds another transport/state dependency; adopt only if manager-impact benchmarks fail. |
| Tonic load shedding and per-method size limits | **Accepted** | Prevents hidden buffering and unlimited encoding. |
| gRPC compression everywhere | **Rejected** | Small interactive/control messages pay CPU/latency without enough network savings. |
| Conditional metrics/inventory compression | **Deferred to benchmark** | Valuable only when measured network reduction exceeds 30% within CPU SLO. |
| UI three-second refresh directly against Engine | **Rejected** | A preference cannot override control-plane safety floors. |
| UI three-second refresh against shared cache | **Accepted for parity** | Preserves feel while manager/stats cadence remains bounded. |
| Transparent exec replay across failover | **Rejected** | Replaying process creation is unsafe and not semantically resumable. |
| Service/task logs through manager-native API | **Accepted** | Avoids per-task inspect/follower fan-out when Docker's driver supports it. |
| eBPF, cAdvisor, or direct cgroup scraping | **Deferred** | Adds privilege, platform, and compatibility surface; Engine one-shot stats is the parity path. |
| Tokio unstable scheduler tuning | **Deferred** | Runtime queue knobs require evidence; bounds and blocking isolation address the known risks first. |

## V1 parity versus v2 production

“Parity” means users can perform the current workflows with honest completeness and safe resource bounds. It does not mean preserving the implementation's expensive polling, silent omissions, or unlimited inputs.

| Area | V1-parity release gate | V2 production release gate |
|---|---|---|
| Topology | one controller and SQLite allowed, explicitly non-HA | controller on managers, shared transactional DB, readiness routing |
| Lists | same resource fields/workflows; page/cache; no per-row inspections | event-invalidated snapshots, 100-user independence, staleness SLO |
| UI refresh | 3/5/10/30/off may refresh cache | adaptive focus-aware defaults and consolidated overview |
| Discovery | authoritative task/node mapping; bounded reconcile and health | agent registration stream, handshake admission, 30-second recovery |
| Metrics | latest node/container view at 60 seconds acceptable | 30-second sample, ≥99% coverage, batching/rollups/storage thresholds |
| Images/local inventory | node provenance, partial targets, bounded serial-or-parallel implementation | profile fan-out concurrency, cached pages, p95 first page ≤5 s |
| Logs | current service/task flows, but bounded record/queue/tail/source and visible gaps | throughput/fairness/soak targets and enhanced fan-in qualification |
| Exec | current workflows plus all safety limits in [exec](exec.md) | multi-controller admission/audit and full load/failover suite |
| Scheduling | one process, skip overlap, explicit timeouts/jitter | database lease, fencing term, idempotent claims, takeover SLO |
| Persistence | fixed retention config, batch writes, bounded SQLite | shared DB, protected quotas, 24 h/30 d retention and HA tests |
| gRPC | deadlines, sizes, bounded concurrency, partial errors | mTLS, load shedding, N/N-1 and failure qualification |
| Operations | resource metrics and obvious overload errors | SLO dashboard, alerts, capacity profile, 24 h/large soak evidence |

The following fixes are mandatory even for parity: `one_shot` stats, no worker manager-task scan, bounded records/messages/queues, fan-out deadlines and completeness, pagination, shared Docker/HTTP clients, no N+1 metric queries, correct interval/retention configuration, explicit stale state, and admission before streams or destructive work.

## Release checklist

- [ ] Every UI/API resource path is mapped to cache, database, manager Engine, node Engine, or follower RPC work.
- [ ] A second through hundredth UI client changes manager Engine request rate by less than 5%.
- [ ] Manager background traffic stays below two requests/second average and eight starts/second burst.
- [ ] No normal list or metric query performs a per-row Engine call or per-node database query.
- [ ] Current tasks are separated from bounded historical browsing.
- [ ] Docker events are filtered, reconnectable, and backed by 30/60-second jittered reconciliation.
- [ ] Every Docker call class has a semaphore, wait policy, cancellation, and deadline.
- [ ] Stats use local container discovery, `one_shot`, previous-counter CPU math, eight permits, 25 requests/second, and a 25-second cycle.
- [ ] Metrics batches, queues, writes, retention, rollups, and disk thresholds pass the profile test.
- [ ] SQLite compatibility uses a fixed WAL-safe version; production uses shared transactional state.
- [ ] All fan-outs are bounded, stable-ordered, cancellable, paged, and explicitly partial.
- [ ] Tonic has decode and encode limits, load shedding, per-peer/global concurrency, and method deadlines.
- [ ] Logs and exec acquire source/session/byte permits before opening Docker.
- [ ] Oversized Docker, WebSocket, protobuf, JSON, and log records fail without proportional allocation.
- [ ] Public per-principal/global rate and concurrency limits return stable overload responses.
- [ ] UI query keys include cluster identity, one pooled client is reused, focus/minimize pauses polling, and retries are bounded.
- [ ] All schedules and reconnects have stable jitter, no catch-up queue, and bounded failover buffering.
- [ ] Tokio blocking work is admitted before `spawn_blocking` and runtime scheduling p99 passes.
- [ ] Resource, SLO, manager-impact, soak, and chaos artifacts exist for every supported profile.
- [ ] Unsupported scale is detected at startup/runtime and reported with the exceeded dimensions.

Kontainer v2 is performance-ready when the largest declared profile can run the complete steady, churn, stream, failure, and recovery workload inside these budgets; manager management latency remains within 10% of baseline; every queue, message, request, fan-out, and database operation is bounded; and overload degrades into explicit stale, partial, gap, or admission outcomes rather than hidden pressure or process failure.

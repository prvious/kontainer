# Metrics collection and storage for Kontainer v2

Status: proposed architecture and implementation plan

## Decision summary

Kontainer v2 should use SQLite as its default embedded engine, but it should not put control-plane state and metrics in the same physical database:

- `control.sqlite3` stores durable relational state: agents, clusters, credentials, settings, resource metadata, migrations, and audit/control records.
- `metrics.sqlite3` stores replaceable telemetry: ingest batches, entity metadata needed to interpret samples, raw samples, rollups, and retention watermarks.
- Followers use a bounded `metrics-spool.sqlite3` while disconnected. The leader has one bounded metrics writer and acknowledges a batch only after it is durable.
- Collection is push-based, batched, idempotent, and independent on each node. A slow or unreachable follower must not delay any other node.
- Current snapshots and historical series are different APIs. Historical queries are bounded by explicit series and point limits and automatically select a raw or rollup resolution.

This is the MVP default, not a claim that SQLite is an unlimited time-series database. The supported envelope must be established by the benchmark in this document. Requiring automatic control-plane failover, multi-writer ingestion, longer high-resolution retention, or exceeding the tested envelope triggers an external profile:

- PostgreSQL for HA control-plane data.
- PostgreSQL with TimescaleDB when one external SQL system is preferred for moderate telemetry scale.
- PostgreSQL plus VictoriaMetrics for a PromQL-compatible metrics platform, or PostgreSQL plus ClickHouse for very large analytical workloads.
- Export to an operator-owned OpenTelemetry/Prometheus-compatible destination when Kontainer should not own metrics retention at all.

The storage and transport interfaces must make these profiles possible, but v2 must not ship several half-supported backends in its first release.

## Scope and principles

This document covers host and Docker Swarm task/container metrics. It also defines the boundary between high-churn telemetry and relational control-plane data. Logs, traces, event history, and arbitrary user-defined metrics are separate concerns.

The design follows these principles:

1. Preserve the intended v1 experience before adding new metric families.
2. Treat samples as lossy, bounded telemetry; treat configuration and identity metadata as durable control-plane state.
3. Keep stable identity separate from display names and other mutable labels.
4. Store cumulative counters and base-unit gauges where possible; derive rates and percentages over a stated interval.
5. Make gaps, staleness, partial collection, resets, and clock uncertainty visible instead of silently manufacturing a complete snapshot.
6. Bound cardinality, query cost, disk use, queues, and network payloads.
7. Batch writes and retention work. Never perform one durable transaction per sample or an unbounded delete.
8. Keep the default useful with one machine and no network service, while defining measurable gates for external storage.

## What v1 does today

### Collection and fan-out

The current path is:

```text
leader scheduler (once per minute, hard-coded)
  -> collect leader host metrics
  -> list local running Swarm tasks
  -> call Docker ContainerStats sequentially for each local container
  -> write one node row and one row per container
  -> for each registered follower, serially:
       unary gRPC GetMetrics (five-second timeout)
       -> follower repeats local collection
       -> leader writes that node's response
```

Relevant implementation files are `server/app/console/kernel.go`, `server/app/actions/metrics/collect_and_store.go`, `collect_node_metrics.go`, `collect_container_metrics.go`, `server/app/grpc/controllers/metrics_controller.go`, and `server/proto/agent/agent.proto`.

The leader runs the collection job with `SkipIfStillRunning`. Followers are queried serially. Therefore each unreachable follower can add approximately five seconds to a cycle, and enough slow followers cause the next scheduled run to be skipped. The job has no cluster-wide deadline, no pagination, no compression, no batch identity, no replay protocol, and no partial-result envelope. A follower timestamp is trusted from its wall clock; a parse failure is replaced by the leader's current time. The response node ID is not checked against the registered/authenticated follower identity, and two active leaders could collect duplicate snapshots because the table has no deduplication key.

Node collection uses gopsutil for virtual memory, `cpu.Percent(0, false)`, disk usage at `/`, and logical CPU count. With interval zero, gopsutil documents CPU percent as a comparison with the previous call, which makes the value dependent on process-global call history rather than the batch interval ([gopsutil CPU documentation](https://pkg.go.dev/github.com/shirou/gopsutil/v4/cpu)). Scheduler and follower-RPC calls can also invoke that process-global calculation at unrelated times.

Container collection lists desired-running Swarm tasks, keeps running tasks assigned to the local node, and calls Docker's non-streaming stats API one container at a time. The CPU calculation subtracts unsigned counters before checking for a reset, and it does not use Docker's fallback from `online_cpus` to the per-CPU array. Memory is raw cgroup usage rather than Docker CLI working set. Docker's API documentation and CLI implementation describe the guarded deltas, CPU fallback, and cache subtraction required for consistent values ([Engine API stats](https://docs.docker.com/reference/api/engine/version/v1.46/), [CLI memory-cache behavior](https://docs.docker.com/reference/cli/docker/container/stats/), [Docker CLI calculation source](https://github.com/docker/cli/blob/master/cli/command/container/stats_helpers.go)). An individual container error is logged and skipped, but a top-level task-list/collection error discards the node's otherwise valid snapshot. Network, block I/O, PIDs, counter resets, and collection errors are not represented in storage.

### SQLite schema and writes

Goravel uses `storage/kontainer.db` for both control-plane records and metrics. The configured pool permits up to 100 open connections. There is no application-level SQLite journal, synchronous, checkpoint, busy-timeout, or foreign-key policy; the inspected development database uses the default rollback journal rather than WAL.

Migration `server/database/migrations/20260320000000_create_metrics_table.go` drops the previous `node_metrics` table and creates one wide `metrics` table:

- identity: `id`, `timestamp`, `kind`, `node_id`, `subject_id`, `subject_name`, `service_id`;
- node/container measurements: CPU percent, memory used/total/percent, CPU count, disk used/total/percent;
- timestamps: `created_at`, `updated_at`;
- indexes: `(kind, node_id, timestamp)` and `(kind, timestamp)`.

The migration does not copy the old history before dropping its table. The union-style table also stores irrelevant nullable or zero-valued columns for each entity kind and has no uniqueness key with which to deduplicate a replay. Follower protobuf fields are signed, converted to unsigned values without rejecting negatives, and later written back to signed SQLite columns; malformed or overflowing values can therefore wrap across boundaries.

`CollectAndStoreAction` opens one transaction per node, inserts the node, and then performs an ORM `Create` for every container. It commits nodes independently, so a cluster snapshot is not atomic, although an individual node and its containers are. This produces row-at-a-time statement overhead and causes metrics writes, checkpoints, retention deletes, control operations, and backups to contend on the same database file.

### Configuration, scheduling, and retention

`server/config/metrics.go` defines:

- `METRICS_INTERVAL`, documented and defaulted to 30 seconds;
- `METRICS_RETENTION`, documented and defaulted to 1,440 minutes.

The scheduler ignores the interval and runs once per minute. The service provider reads a different, nonexistent key, `metrics.retention_hours`, defaults it to 24, and multiplies it as hours. Consequently both advertised environment variables are currently ineffective; the observed behavior is a one-minute collection interval and a fallback retention of 24 hours.

Pruning runs every five minutes as a single `DELETE WHERE timestamp < cutoff`. It is not chunked and has no rollup dependency or watermark. Deleting SQLite rows does not ordinarily shrink the database file, so this is not a disk-cap mechanism. It may also create a long write lock and substantial WAL or rollback-journal activity at scale.

### Query behavior

`server/app/actions/metrics/get_cluster_metrics.go` combines current state and history in one response:

- It reads at most 100 recent node rows across the whole cluster, with a fixed two-minute freshness cutoff, then chooses the first row per node in memory. At roughly two rows per node, clusters around 50 nodes can be truncated.
- It executes one container query per selected node at the node's exact timestamp, with a limit of 500 containers. This is an N+1 query pattern and silently clips large nodes.
- It reads at most 5,000 node-history rows for the entire cluster ordered oldest first. A 24-hour one-minute history exceeds that limit at four nodes, and the limit preserves old rather than recent samples.
- It builds the final agent list by iterating a Go map, so node ordering is nondeterministic.
- Only `1h`, `6h`, and `24h` are recognized; unknown values silently become one hour.
- Every dashboard refresh returns full node history and all latest containers even though current React consumers use current values, not the history.

The database-backed query was a useful v1 improvement over live collection on every HTTP request, but global row limits are not correctness-preserving query budgets.

### Frontend and Tauri contract

`app/src-tauri/src/actions/agent/list_cluster_metrics.rs` calls `/api/metrics` without a range and models a container with required `serviceName`. The Go response emits `serviceId`, not `serviceName`. `app/src/types/metrics.ts` repeats the required `serviceName` contract. A nonempty container array can therefore fail Rust deserialization before React receives it. The server does not send `createdAt`; Tauri silently defaults it to zero while TypeScript presents it as an ordinary required value.

`app/src/hooks/use-cluster-metrics.ts` uses the global query key `['cluster-metrics']` and polls every five seconds. The key does not include the selected agent/profile, filters, range, or resolution. Dashboard CPU is a simple arithmetic mean of node percentages, which is not a capacity-weighted cluster CPU calculation when nodes have different core counts. Integer byte/counter values cross Rust's `u64` boundary into JavaScript numbers without a precision policy.

The current tests in `server/tests/metrics/get_cluster_metrics_test.go` cover small query fixtures. There are no end-to-end tests for collection math, fan-out failure, duplicate delivery, configuration wiring, pruning, a destructive migration, database failure, high node/container counts, or the Rust/JSON/TypeScript contract.

## V1 parity versus v2 requirements

Parity means preserving intended user-visible capability, not reproducing known bugs.

| Concern | V1 intended capability | V1-parity milestone | V2 requirement |
|---|---|---|---|
| Current state | Latest node and running-container CPU/memory; node disk | Return the same fields with a working contract and explicit freshness | Separate snapshot API, partial/stale status, stable task/container identity, configurable filters |
| History | Node CPU/memory/disk over 1h/6h/24h | Preserve all requested nodes without global silent truncation | Arbitrary bounded ranges, chosen step, rollups, null gaps, service aggregates |
| Collection | All leader and follower nodes | One-minute effective cadence, but concurrent bounded fan-out and correct calculations | Independent push collection; 30-second default after benchmark; offline spool and idempotent replay |
| Retention | Advertised 24 hours | Wire one retention unit and keep at least 24 hours of node history | Separate task/node raw tiers, long service/node rollups, byte cap, low-water behavior |
| Metrics | CPU, memory, node disk | Correct those definitions first | Add network, block I/O, PIDs, capacity/load only after semantics and budgets are specified |
| Topology | Swarm node/task/service association | Preserve service ID and task/container association | Handle rolling updates, jobs, restarts, and historical names without identity collisions |
| Storage | Embedded SQLite | Non-destructive schema transition and a tested local profile | Separate failure domains and pluggable metrics backend |
| Availability | Leader is required | Data resumes cleanly after restart; followers do not block each other | Bounded follower spool; explicit HA profile and failover semantics |

The initial Rust port may ship at the one-minute effective cadence for strict rollout parity. Changing the documented/default cadence to 30 seconds is an explicit v2 feature and must pass the resource benchmark rather than being smuggled into a porting change.

## Workload and cardinality model

Storage selection must be driven by entity churn and query shapes, not only by current row counts.

Let:

- `N` be nodes;
- `C` be concurrently running containers/tasks;
- `I` be collection interval in seconds;
- `M_n` and `M_c` be stored node and container metric families;
- `T` be new container incarnations per day.

Relational sample rate is approximately `(N + C) / I`. At 50 nodes, 2,000 containers, and 30 seconds this is about 68 entity rows/second, 5.9 million rows/day before rollups and pruning. A row width of 250 bytes including indexes would be about 1.5 GB/day; actual bytes per sample must be measured because SQLite page layout and index choice dominate the estimate.

In a Prometheus-compatible representation, each metric family and unique label set is a series. Ten container families over 2,000 active containers are already roughly 20,000 active series, while rolling updates and jobs continuously create series that remain in the index until retention expires. Container/task ID, image digest, name, stack, arbitrary Docker labels, and exit reason must not all become metric labels. Prometheus explicitly warns that every unique label combination creates a new time series and recommends avoiding unbounded high-cardinality labels ([Prometheus naming guidance](https://prometheus.io/docs/practices/naming/)).

The primary v2 queries are:

1. Current snapshot for all nodes and optionally the containers on selected nodes/services.
2. A time range for one metric across a bounded set of nodes or services.
3. Cluster/service aggregate over a time range and step.
4. Collector/storage health: freshness, missed batches, errors, queue/spool pressure, series/entities, and disk use.
5. Retention/rollup maintenance by closed time buckets.

Kontainer is not initially a general metrics explorer. Arbitrary cross-label ad hoc analysis is valuable in external backends, but it does not justify imposing their operational cost on the embedded profile.

## Considered storage approaches

### Comparison

| Approach | Control-plane fit | Metrics fit | Offline/operations | HA and backup | Rust and license | Decision |
|---|---|---|---|---|---|---|
| One SQLite file for everything | Good relational semantics | Adequate only inside a bounded envelope | Simplest packaging, but telemetry competes with control writes, disk, checkpoints, migrations, and backups | One writer; local-disk WAL only; online backup exists | SQLx/rusqlite are mature; SQLite is public domain | **Rejected** as a physical layout |
| Two SQLite files, one embedded engine | Good, isolated control DB | Adequate for bounded batched raw data and rollups | Zero external service; excellent single-node/offline profile | Single leader; separate backup and failure policies; agent spool limits loss | SQLx supports SQLite and migrations; public domain | **Accepted for MVP** |
| SQLite plus embedded DuckDB/Parquet | SQLite remains good | Strong scans/compression, weak hot trickle-ingest/control transaction fit | More formats, compaction, and recovery code owned by Kontainer | Application must invent replication/retention/backup behavior | DuckDB has an official Rust client; MIT | **Rejected as hot store; deferred for export/offline analysis** |
| PostgreSQL alone | Excellent | Viable at moderate scale with careful partitioning and batched inserts; more row/index/WAL cost than a TS extension | Requires an external service and DBA discipline | Mature streaming replication, PITR tooling; failover/fencing still external | SQLx/tokio-postgres mature; PostgreSQL license | **Deferred external control profile** |
| PostgreSQL + TimescaleDB | Excellent | Strong SQL time buckets, hypertables, compression, retention, continuous aggregates | One external operational system, but extension/version management | Inherits PostgreSQL HA/backup model | PostgreSQL protocol; Apache core plus Timescale License features | **Deferred external all-in-one profile** |
| SQLite/PostgreSQL + ClickHouse | Keep control elsewhere | Excellent very-large append/aggregate/compression fit | Highest operational burden here; batch-friendly, not control OLTP | Replication requires ClickHouse Keeper; backup tooling is mature | Official `clickhouse-rs`; Apache 2.0 | **Deferred hyperscale profile** |
| SQLite/PostgreSQL + VictoriaMetrics | Keep control elsewhere | Strong PromQL-compatible ingestion/query and compression | Single-node or cluster service; cardinality policy required | Cluster is AP-oriented and needs multiple components for HA; snapshot backup | HTTP/Prom protocols from Rust; OSS single/cluster Apache 2.0, some lifecycle features Enterprise | **Deferred Prometheus profile** |
| Delegate to operator OTLP/Prometheus destination | Keep control local/PG | Operator owns capacity, retention, HA, and cost | Lowest Kontainer storage burden; requires a reachable, compatible service | Determined by destination; local spool still necessary | OTel Rust and HTTP/protobuf ecosystem; destination-specific license | **Accepted later as opt-in export mode** |
| Bundle Prometheus local TSDB | Keep control elsewhere | Efficient local TSDB and PromQL | A second process with its own lifecycle; not an embeddable Rust library/API | Local storage is explicitly not clustered or replicated | Apache 2.0 | **Rejected as the default app store** |
| Custom RocksDB/Parquet/DataFusion TSDB | Custom work for both | Potentially excellent after years of engineering | Kontainer would own indexing, compaction, tombstones, recovery, query planning, and migration | All replication and restore behavior is custom | Components exist, product-quality system does not | **Rejected** |

### Why dual-file SQLite is the MVP

SQLite is embedded, transactional, zero-configuration, and public domain ([SQLite overview](https://sqlite.org/about.html), [license](https://sqlite.org/copyright.html)). WAL mode lets readers proceed while a writer commits, but it still permits only one writer and requires all participants to be on the same host; it is not suitable on a network filesystem ([SQLite WAL documentation](https://sqlite.org/wal.html)). These constraints match one Kontainer leader if writes are deliberately serialized and bounded.

Two files materially improve failure containment:

- Metrics can use `synchronous=NORMAL`, aggressive retention, and replaceable migrations while control state uses `synchronous=FULL` and conservative migrations. In WAL mode, NORMAL remains consistent across application crashes but may lose the newest transaction after power loss, while FULL adds durability at each commit ([SQLite synchronous pragma](https://sqlite.org/pragma.html#pragma_synchronous)).
- Metrics can be excluded from the default control backup and reclaimed/rebuilt without locking or inflating the control database.
- A telemetry disk cap can stop or degrade collection without preventing login, configuration, reconciliation, or audit writes.
- Read pools and maintenance schedules can be tuned independently.

Use a bundled SQLite version pinned to at least 3.51.3, or an official backport that contains the 2026 WAL-reset fix documented on the WAL page. Do not rely on an unknown system library. SQLx is the preferred async query/migration layer; its current SQLite driver, compile-time checked queries, migrations, and offline metadata are described by the [SQLx project](https://github.com/launchbadge/sqlx). [rusqlite](https://github.com/rusqlite/rusqlite) is also mature and exposes SQLite's native backup API, but adding both clients needs a concrete benefit. A future PostgreSQL implementation can share domain models and SQL concepts, but should have explicit backend queries and migrations rather than relying on lowest-common-denominator SQL or runtime `Any` dispatch.

SQLite has no native columnar time-series compression. Short high-cardinality raw retention and coarse long-lived rollups are therefore part of the default's capacity design, and every additional raw-table index must justify its write amplification and bytes/sample in the benchmark.

The metrics database should use:

- WAL on local durable storage only;
- one application writer task and a small read pool;
- a measured busy timeout;
- foreign keys and `STRICT` tables where compatible;
- control `synchronous=FULL`, metrics/spool `synchronous=NORMAL` by default;
- bounded transactions and an explicit checkpoint policy informed by WAL size and reader duration;
- page-size and `auto_vacuum=INCREMENTAL` decisions before tables are populated, followed by measured incremental vacuum work rather than surprise full vacuums.

SQLite's online backup API and `VACUUM INTO` can create consistent snapshots, but copying only the main database file while WAL is active is unsafe ([SQLite backup API](https://www.sqlite.org/backup.html)).

### Why DuckDB is not the hot embedded store

DuckDB is excellent for vectorized scans, Parquet interchange, compression, and offline diagnostics, and it provides an official core Rust client ([DuckDB Rust client](https://duckdb.org/docs/current/clients/rust)). Its own concurrency guide says multi-process writes are not a primary design goal and describes optimization for bulk operations rather than many small transactions ([DuckDB concurrency](https://duckdb.org/docs/lts/connect/concurrency)). It is MIT licensed ([DuckDB license](https://github.com/duckdb/duckdb/blob/main/LICENSE)). Using it for the hot path would require Kontainer to design the buffering, compaction, crash boundary, retention, and current-snapshot index that a TSDB normally supplies.

DuckDB/Parquet remains useful later for a support-bundle export or offline long-term archive. That role should consume immutable, versioned exports and must not become a second authoritative database.

### PostgreSQL and TimescaleDB

PostgreSQL is the first control-plane upgrade when automatic leader failover or multiple API replicas become a requirement. Plain PostgreSQL can also hold metrics when inserts are batched and samples are range-partitioned by time. Dropping or detaching an old partition is much faster and avoids the vacuum overhead of a bulk row delete ([PostgreSQL partitioning](https://www.postgresql.org/docs/current/ddl-partitioning.html)). It is nevertheless an external service with WAL, indexes, partitions, autovacuum, connection limits, and capacity planning. It is not the right mandatory dependency for an offline desktop/server installation.

PostgreSQL provides streaming replication and base backups/PITR building blocks, but its documentation is clear that failure detection, failover orchestration, and fencing must be supplied by the deployment ([high availability](https://www.postgresql.org/docs/current/high-availability.html), [`pg_basebackup`](https://www.postgresql.org/docs/current/app-pgbasebackup.html)). SQLx or the async pipelined [tokio-postgres](https://docs.rs/tokio-postgres/latest/tokio_postgres/) client are mature Rust choices. PostgreSQL uses its permissive [PostgreSQL License](https://www.postgresql.org/about/licence/).

TimescaleDB makes the all-in-one external profile more compelling: hypertables partition time automatically, retention policies drop chunks, continuous aggregates maintain downsampled views, and Hypercore applies time-series compression techniques ([hypertables](https://docs.timescale.com/use-timescale/latest/hypertables/), [retention](https://docs.timescale.com/use-timescale/latest/data-retention/create-a-retention-policy/), [continuous aggregates and retention](https://docs.timescale.com/use-timescale/latest/data-retention/data-retention-with-continuous-aggregates/), [compression](https://docs.timescale.com/use-timescale/latest/hypercore/compression-methods/)). Refresh windows must precede raw deletion or aggregates can lose source data.

TimescaleDB is deferred until the team accepts PostgreSQL operations and completes a licensing/product-distribution review. Its repository contains both Apache-2.0 code and Timescale License code, and available features/terms differ by edition ([TimescaleDB license](https://github.com/timescale/timescaledb/blob/main/LICENSE), [editions](https://github.com/timescale/docs/blob/latest/about/timescaledb-editions.md)).

### ClickHouse

ClickHouse is a strong destination for very high ingest volume, long analytical retention, and many cross-entity aggregates. MergeTree TTL can delete, move, or roll up old data; aligning partitions with retention allows efficient whole-partition removal. TTL actions are merge-driven and therefore eventual, not an immediate disk-limit mechanism ([ClickHouse TTL](https://clickhouse.com/docs/guides/developer/ttl)). Incremental materialized views shift aggregation cost to ingestion ([materialized views](https://clickhouse.com/docs/materialized-view/incremental-materialized-view)).

It should not hold Kontainer control-plane OLTP data. HA replication adds ClickHouse Keeper and asynchronous replication/quorum choices; ClickHouse recommends batching inserts and documents deduplication at block granularity ([replication](https://clickhouse.com/docs/engines/table-engines/mergetree-family/replication)). Backup supports full/incremental and local/object destinations ([backup and restore](https://clickhouse.com/docs/operations/backup)). The official pure-Rust async client is [clickhouse-rs](https://github.com/ClickHouse/clickhouse-rs), and the server is Apache 2.0 ([ClickHouse repository](https://github.com/ClickHouse/ClickHouse)).

ClickHouse is therefore a later, separately operated hyperscale metrics backend, selected by benchmark and query need rather than by popularity.

### VictoriaMetrics, Prometheus-compatible storage, and external delegation

VictoriaMetrics is attractive when PromQL and the wider Prometheus ecosystem are product requirements. Its single-node form has one storage path and retention setting; its cluster splits ingestion, storage, and query across `vminsert`, `vmstorage`, and `vmselect` ([single-node](https://docs.victoriametrics.com/victoriametrics/index.html), [cluster architecture](https://docs.victoriametrics.com/victoriametrics/cluster-victoriametrics/)). Cluster mode favors availability over strict consistency and may return partial responses. Replication increases storage and component requirements, so it is not a zero-cost HA switch.

High task churn needs explicit protection. `vmagent` offers hourly/daily cardinality limiters that drop new series over a limit, and stream aggregation can downsample or remove labels before storage ([cardinality limiter](https://docs.victoriametrics.com/victoriametrics/vmagent/#cardinality-limiter), [stream aggregation](https://docs.victoriametrics.com/victoriametrics/stream-aggregation/)). Stream-aggregation state is in memory by default and can be lost on restart. Built-in multi-level historical downsampling and some automated lifecycle capabilities are Enterprise features, so the OSS profile must not be designed on the assumption they are available ([downsampling](https://docs.victoriametrics.com/victoriametrics/index.html#downsampling), [Enterprise features](https://docs.victoriametrics.com/operator/enterprise/)). Single-node and cluster OSS code is Apache 2.0 ([VictoriaMetrics repository](https://github.com/VictoriaMetrics/VictoriaMetrics)). Backups use storage snapshots and every cluster storage node must be handled consistently ([vmbackup](https://docs.victoriametrics.com/victoriametrics/vmbackup/)).

Prometheus's own local TSDB is efficient but explicitly local, not replicated, and not clustered; its storage guidance recommends remote storage for those needs and does not support NFS ([Prometheus storage](https://prometheus.io/docs/prometheus/latest/storage/)). Bundling it would add a supervised service without solving Kontainer relational storage. It is better treated as an external-compatible protocol. Prometheus is Apache 2.0 licensed ([Prometheus license](https://github.com/prometheus/prometheus/blob/main/LICENSE)).

An opt-in exporter may send bounded, low-cardinality node and service series to an operator-owned destination. Per-task series must be disabled or explicitly capped by default. Prometheus Remote Write defines Snappy/protobuf transport, per-series ordering, retries, and the need for a persistent sender buffer, but notes that it was not designed as a generic application push protocol ([Remote Write 1.0](https://prometheus.io/docs/specs/prw/remote_write_spec/)). Prefer OTLP through an OpenTelemetry Collector when broad destination portability is the goal; the OpenTelemetry metric data model supports cumulative/delta temporality and aggregation transformations ([OTel metrics data model](https://opentelemetry.io/docs/specs/otel/metrics/data-model/), [OTLP reliability](https://opentelemetry.io/docs/specs/otlp/)). The Rust metrics API/SDK are stable, while exporter maturity must be checked at implementation time in [opentelemetry-rust](https://github.com/open-telemetry/opentelemetry-rust).

The internal agent-to-leader protocol should remain a Kontainer domain protocol. It needs batch identity, task metadata, partial errors, and acknowledgements that OTLP/Remote Write do not express as the control-plane contract. Export adapters translate committed batches or rollups later.

## Proposed v2 architecture

### Collection protocol

Every agent collects locally on a monotonic fixed cadence. It does not wait for the leader to poll it. A collection cycle:

1. Captures a node counter/gauge snapshot once.
2. Lists local running Swarm tasks once.
3. Fetches Docker stats with bounded concurrency (initial maximum 16) and a cycle deadline of 80% of the interval.
4. Records per-entity errors without discarding successful node or container results.
5. Builds one versioned `MetricBatch` and places it in a bounded local spool.
6. Streams queued batches to the leader, retransmitting until it receives a durable acknowledgement.

The protobuf envelope should contain at least:

```text
MetricBatch {
  schema_version
  cluster_id
  node_id
  agent_instance_id       // regenerated on agent data reset/reinstall
  boot_id                 // detects host reboot/counter reset
  batch_sequence          // monotonic within agent_instance_id
  chunk_index
  chunk_count             // bounded transport chunks for one collection
  observed_at_unix_ms
  monotonic_elapsed_ms    // interval basis; not comparable across agents
  collection_duration_ms
  node_sample?            // missing is representable
  repeated container_samples
  repeated collection_errors { scope, entity_id, code }
  truncated
}
```

The durable collection idempotency key is `(agent_instance_id, batch_sequence)`; chunks add `(chunk_index, chunk_count)` to their transport identity. A complete collection may be split into chunks of at most 1,000 entities/1 MiB. The leader assembles them in its bounded queue and writes/acknowledges only after every chunk is present; losing an uncommitted assembly on restart is safe because the follower has not received an acknowledgement and retransmits it. The leader validates that the authenticated registration maps to `cluster_id` and `node_id`; response identity is not trusted. Replays return the same acknowledgement and do not create samples. A schema version and capability negotiation allow rolling upgrades.

Use a tonic client stream or bidirectional stream with keepalive and optional compression. Tonic supports send/accept compression configuration ([tonic API](https://docs.rs/tonic/latest/tonic/)). Unary batches are an acceptable first implementation only if they retain the same batch/ack/replay semantics and connections are reused. Do not restore serial leader fan-out.

The spool is capped at the earlier of six hours and 256 MiB by default. On overflow it drops the oldest unacknowledged telemetry, increments a durable dropped-batch count, and marks the next delivered batch with the lost sequence interval. Metrics loss must never fill the disk or block control/reconciliation work. A reconnect includes recent batches in sequence order; the leader may accept late samples inside the raw-retention horizon but exposes `received_at` and late/gap status.

### Correct metric semantics

Store base values in canonical units and calculate presentation values centrally:

- Node CPU: cumulative busy and total CPU nanoseconds/ticks plus logical capacity; derive core-seconds and utilization over two batches. A reset/reboot creates a gap, never unsigned underflow.
- Container CPU: cumulative usage nanoseconds, system/elapsed basis, and CPU limit/capacity when known; derive used cores and utilization with guarded positive deltas.
- Node memory: used/available/total bytes with a documented gopsutil/platform definition.
- Container memory: working-set bytes and limit bytes using Docker's cgroup-v1/v2 cache rules; optionally preserve raw usage for diagnostics.
- Node disk: used/total bytes and a stable filesystem/mount identity. `/` alone is a deliberate MVP limitation, not “all disk.”
- Network and block I/O: cumulative byte counters, not instantaneous rates; reset and interface/container incarnation are part of rate validity.
- PIDs and load: gauges.

Ratios in the HTTP API are `0.0..1.0` and named `*_ratio`; UI code decides whether to render percent. Stored byte and counter values are signed 64-bit only after checked conversion. Negative or overflowing source values reject that field and set a quality flag. JSON uses decimal strings for raw counters that can exceed JavaScript's exact integer range, while chart-ready rates/gauges use finite `f64` values.

No arbitrary Docker label is part of a sample primary key. Mutable display metadata belongs in relational entity history. Service rollups aggregate task samples for a bucket and survive task churn; long-term task-level rollups are not a default.

### Write path

The leader's ingestion service validates size, identity, version, timestamp skew, entity count, and numeric bounds. Accepted batches enter a bounded in-memory channel drained by exactly one metrics writer. The writer groups complete collections up to the first reached threshold:

- 250 milliseconds since the oldest pending item;
- 1,000 entity samples;
- 1 MiB of decoded sample payload.

These are starting values for the benchmark, not immutable constants. A single collection larger than an aggregation threshold is committed alone rather than split across database transactions. One transaction inserts the batch record, dimension changes, node/container rows, and the acknowledgement watermark. Multi-row prepared inserts replace per-row ORM creates. The current-snapshot cache is updated only after commit. It is held in memory for fast polling and reconstructed at startup from `(entity, observed_at DESC)` indexes; v2 should not double every write solely to maintain a durable “latest” table unless the benchmark proves reconstruction or query cost unacceptable.

Backpressure propagates to the agent spool rather than allocating an unbounded leader queue. A collection is split deterministically into transport chunks before sending; a collection above the negotiated total-entity limit is rejected with a non-retryable capability error and an explicit degraded status. Partial chunk delivery must never create a partial durable collection.

### Logical data model

Control-plane tables keep authoritative clusters, nodes, agents, Swarm resource snapshots, credentials, and configuration in `control.sqlite3`. The metrics database holds denormalized identity sufficient to read samples even if a Swarm resource has since disappeared.

| Table | Important columns | Key/index purpose |
|---|---|---|
| `ingest_batches` | agent instance, sequence, cluster/node, observed/received times, duration, counts, error/truncated flags | Primary key `(agent_instance_id, batch_sequence)` provides replay idempotency; index received time for maintenance |
| `metric_nodes` | cluster/node IDs, hostname history reference, first/last seen | Stable node identity; no mutable name in every sample |
| `metric_tasks` | cluster, task, container incarnation, service, node, slot, first/last seen, image digest reference | Distinguishes task replacement and container restart; indexes service/node/last seen |
| `node_samples_raw` | batch key, node, observed/received times, CPU counters/capacity, memory gauges, disk gauges, network counters, quality flags | Unique batch/entity; indexes `(cluster_id,node_id,observed_at)` and `(cluster_id,observed_at)` |
| `container_samples_raw` | batch key, task/container IDs, observed/received times, CPU counter/capacity, memory working set/limit, network/block counters, PIDs, quality flags | Unique batch/entity; indexes entity/time plus service/time and node/time only when justified by query plans |
| `node_rollup_5m` / `node_rollup_1h` | bucket, count/expected, min/max/sum/last per metric, counter first/last/reset count, quality | Primary key cluster/node/bucket |
| `service_rollup_5m` / `service_rollup_1h` | bucket, service, contributing tasks/nodes, aggregate core/memory/I/O values, completeness | Primary key cluster/service/bucket; long-lived view unaffected by task IDs |
| `rollup_watermarks` | source table, resolution, completed-through time, version | Makes rollup then delete restartable and migration-aware |
| `retention_runs` | policy version, start/end, rows/buckets/bytes, result | Diagnoses maintenance and prevents silent retention failure |

Do not use an EAV row per `(sample, metric_name)`: it multiplies rows and indexes for a fixed built-in metric set and makes snapshot reconstruction expensive. Do not repeat a single nullable union table from v1: node and container lifecycles, columns, indexes, and retention differ. Additive metric columns are cheaper and clearer within each entity table; a schema-versioned side table or opaque extension payload is acceptable only for experimental metrics that the primary API does not query.

Timestamps are UTC Unix milliseconds. `observed_at` is the agent wall time used for graph alignment; `received_at` is the trusted leader time used for freshness and operations. Rate calculations use monotonic elapsed time and adjacent sequence where available. Clock-skew estimates and late-data thresholds are explicit quality metadata.

### Retention, rollups, and disk pressure

Recommended initial policy after the 30-second cadence is validated:

| Data | Default retention | Reason |
|---|---:|---|
| Ingest metadata and collection errors | 7 days | Debug replay/gaps without preserving payload forever |
| Raw node samples | 7 days | Short incident analysis is inexpensive at node cardinality |
| Raw task/container samples | 24 hours | Highest volume and churn; matches v1's useful horizon |
| Five-minute node/service rollups | 30 days | Operational trend view |
| One-hour node/service rollups | 365 days | Capacity trend with bounded size |
| Task identity metadata | 30 days after last seen | Explains recent raw data without permanent churn |

For strict v1 parity before rollups ship, use one 24-hour raw policy for the implemented fields. Retention durations, not ambiguous hours/minutes keys, are parsed once into a typed configuration and exposed through `/metrics/status`.

A rollup worker processes only closed buckets plus an allowed-lateness delay. Within a transaction it upserts an aggregate and advances a versioned watermark. Raw deletion for a time interval is allowed only when all required rollup watermarks cover it. Late data after a finalized bucket is either included by a bounded repair window or marked late and retained only at raw resolution; the choice must be consistent and tested.

Pruning uses time/index-bounded chunks with a transaction time budget, yields to ingestion, checkpoints deliberately, and runs incremental vacuum work when useful. Retention is time-based but safety is byte-based:

- soft budget: 10 GiB or 25% of the configured metrics volume, whichever is smaller after reserving control-plane free space;
- low-water reserve: the larger of 2 GiB or 10% of the volume;
- emergency reserve: 5% free space.

Exact defaults require installer/platform validation. At soft pressure, accelerate completed raw pruning and pause optional export/backfill. At low water, stop detailed container history, retain node and service aggregates/current cache, and emit a persistent degraded status. At emergency reserve, stop metrics writes entirely. Never delete control-plane data, violate rollup dependencies, or claim a complete series. Time-series TTL systems also need this independent low-water guard because merge-driven deletion is not instantaneous.

### Storage and query interfaces

Keep control and metrics repositories separate in Rust. A conceptual metrics interface is:

```rust
trait MetricsStore {
    async fn ingest(&self, batch: ValidatedMetricBatch) -> Result<IngestAck>;
    async fn snapshot(&self, query: SnapshotQuery) -> Result<MetricSnapshot>;
    async fn series(&self, query: SeriesQuery) -> Result<MetricSeriesPage>;
    async fn status(&self) -> Result<MetricsStatus>;
    async fn roll_up(&self, through: Timestamp) -> Result<RollupReport>;
    async fn prune(&self, policy: RetentionPolicy) -> Result<RetentionReport>;
}
```

The trait expresses domain semantics; it must not expose SQL, PromQL, ClickHouse syntax, or backend-specific consistency. SQLite is the only MVP implementation. Backend contract tests run unchanged against every later implementation. Control repositories should similarly avoid a “SQLite everywhere” assumption so PostgreSQL can be introduced without coupling it to a metrics migration.

### HTTP APIs

Current and historical data have separate endpoints:

`GET /api/v2/metrics/snapshot`

- filters: `clusterId`, repeated `nodeId`, optional `serviceId`, `include=nodes,containers`;
- defaults to nodes only; containers require an explicit include/filter for large clusters;
- returns `generatedAt`, storage/collector status, and per-node `observedAt`, `receivedAt`, `ageMs`, `partial`, `stale`, and missing/error scopes;
- stable entity fields use `nodeId`, `serviceId`, `taskId`, and `containerId`; a resolved `serviceName` is optional display metadata, never a substitute for ID;
- no history is embedded.

`GET /api/v2/metrics/series`

- required: metric name, entity kind/IDs or a bounded aggregate selector, `from`, `to`;
- optional: `step`, aggregation (`avg`, `min`, `max`, `sum`, `last`, rate where valid), pagination token;
- the server chooses a minimum safe step and raw/5m/1h source, returns the effective resolution, and emits `null` for gaps;
- rejects unknown metrics/ranges instead of silently falling back;
- reports partial/late/reset quality and sample count per point.

`GET /api/v2/metrics/status`

- collection interval and policy version;
- per-agent last observed/received/sequence, skew estimate, spool/drop/gap status;
- writer queue, last commit, backend, database/WAL/allocated/free bytes;
- rollup/prune watermarks and last errors;
- active/recent entity and optional exported-series cardinality;
- backend capabilities and enforced query limits.

Initial query limits are 500 series, 2,000 points per series, 100,000 total points, 5 MiB encoded response, and 30 days per request unless a coarser retained rollup permits more. The server returns a structured `limit_exceeded` response with a suggested step/filter; it never returns a plausible truncated chart. Prometheus-compatible backends map this contract to range queries resembling the standard `query_range` start/end/step model ([Prometheus HTTP API](https://prometheus.io/docs/prometheus/3.5/querying/api/)).

The Tauri/Rust/TypeScript DTOs should be generated from one versioned schema where practical, or validated by shared JSON fixtures at minimum. TanStack Query keys include agent/profile ID, endpoint kind, metric, filters, range, step, and aggregation. Snapshot polling and historical series caching are independent: a five-second UI refresh must not refetch a day of history. Cluster CPU is computed from used core-seconds divided by available core-seconds, not an unweighted mean of node percentages.

## Resource budgets and overload behavior

The default embedded profile is provisionally targeted at:

- 50 nodes;
- 2,000 concurrently running containers;
- 10% of containers replaced per hour, with a separate burst test at 100%;
- 30-second collection;
- policies in the retention table above;
- one leader on local SSD-class storage.

This is a test target, not a supported claim until the benchmark passes. Initial acceptance budgets are:

| Resource/SLO | Budget |
|---|---:|
| Agent collection CPU | <2% of one core averaged over 15 minutes at 100 local containers |
| Agent metrics RSS above baseline | <128 MiB, including spool queue metadata |
| Leader metrics CPU | <1 full core averaged at the reference workload; bursts <2 cores during rollup/prune |
| Leader metrics RSS above baseline | <256 MiB with 2,000 current containers and bounded query concurrency |
| Decoded transport batch | <=1 MiB and <=1,000 entity samples |
| Leader in-memory ingest queue | <=64 MiB; backpressure before allocation beyond it |
| Agent durable spool | <=256 MiB and <=6 hours |
| Durable ingest acknowledgement | p95 <250 ms, p99 <1 s on reference hardware |
| Current node snapshot | p95 <250 ms for 50 nodes; container snapshot p95 <500 ms for 2,000 containers |
| Historical query | p95 <1 s for <=100,000 returned points; hard cancellation at 5 s |
| Collection completeness | >=99.9% expected node batches when connected and healthy; missing container samples explicitly counted |
| Bytes per raw sample | Measured gate: <=200 bytes/node and <=250 bytes/container including amortized indexes at steady state |
| Writer/checkpoint stall | No control API operation delayed by metrics, because databases and pools are separate |

If the bytes/sample, latency, CPU, or disk budgets do not pass, first reduce indexes, batch/transaction overhead, raw task retention, and response shape. Do not hide the miss behind silent sampling. If the required production envelope still exceeds the embedded gate, select an external backend profile.

Cardinality overload is explicit. Node and service-level metrics have priority. Detailed task/container retention may be deterministically sampled or rotated only after the product decides how that degraded view is communicated; arbitrary “first 500” clipping is forbidden. Exports have separate hourly/daily new-series limits and must report dropped series. Names, image tags/digests, arbitrary labels, and error messages are metadata, not dimensions by default.

## Backups, migrations, HA, and operations

### Backups

Control-plane backup is mandatory and telemetry backup is optional by default:

- Create a consistent online `control.sqlite3` snapshot using SQLite's backup API or `VACUUM INTO`, encrypt it according to the deployment's secret policy, and verify integrity.
- Do not copy a live `.db` file independently from its WAL/SHM files.
- Record backup schema version, checksum, created time, and application version; perform automated restore drills.
- A leader backup/restore runbook defines RPO/RTO and handles agent credentials and encryption keys, not only rows.
- Metrics can normally be lost and recollected; export a metrics snapshot only when the operator opts into its storage/cost. A metrics backup must include rollup watermarks and entity metadata consistently.

External profiles use native tooling: PostgreSQL base backup plus WAL archiving/PITR, ClickHouse backup, or VictoriaMetrics snapshots. “The vendor supports backups” is not acceptance; Kontainer needs a restore test for the exact deployment topology.

### Migrations

Never repeat v1's drop-first history migration. Each control migration follows expand, backfill, verify, then contract, with a compatible application window and a pre-migration backup. Metrics migrations may choose one of:

1. additive in-place schema change;
2. dual-write old/new tables, backfill bounded time ranges, verify counts/checksums, switch reads, then retire;
3. archive/export old metrics and rebuild the replaceable database after explicit operator notice.

The v1-to-v2 migration must detect both `node_metrics` and unified `metrics`, copy all supported rows within configured retention, map `service_id` without inventing a service name, and report rejected/ambiguous rows. If the product chooses not to migrate telemetry, it must preserve an optional read-only archive and say so; silently dropping history is not acceptable.

Every backend records schema, rollup-definition, and retention-policy versions. Changing an aggregate definition creates a new rollup version and backfill/switch process; it does not mix old and new meanings in one series.

### HA

SQLite WAL is a single-host, single-writer profile. A shared NFS volume does not make it HA. Followers' spools narrow collection loss, and a replacement leader can restore `control.sqlite3` and accept replay, but automated zero/low-downtime failover requires:

- PostgreSQL (or another agreed relational system) for authoritative control state;
- a leader lease plus fencing so two leaders cannot both schedule/control the cluster;
- a metrics backend that accepts multi-writer or routed ingestion, or a single fenced metrics writer;
- replay/dedupe keys that survive leader changes;
- tested DNS/service discovery, certificate, backup, and restore behavior.

PostgreSQL streaming replication alone does not implement those application semantics. VictoriaMetrics cluster may return partial results by design, while ClickHouse replicas are asynchronous unless stronger quorum choices are made. The API's completeness metadata must represent the selected backend's consistency model.

### Security and privacy

Metric ingestion uses the existing authenticated agent trust model plus node/cluster binding and transport encryption. Apply maximum batch/entity/field lengths before allocation or decompression. Never accept an arbitrary metric name or label map from an agent in the built-in path.

Container names, image references, node hostnames, service labels, and error strings may contain tenant or secret-adjacent information. Keep them out of high-cardinality exported labels by default, authorize snapshot/series filters consistently with resource APIs, redact collection errors, and define deletion behavior when a cluster is removed. Backups and external exporters inherit the same sensitivity classification as control-plane metadata even if numeric samples are low sensitivity.

## Evolution plan and decision gates

### Phase 0: corrected v1 parity in Rust

- Port only existing node/container CPU, memory, and node-disk capability.
- Correct Docker counter/reset/cache semantics and the service ID contract.
- Use concurrent bounded node collection, typed interval/retention settings, deterministic queries, and non-destructive migration.
- Keep the one-minute effective cadence and 24-hour raw horizon while compatibility fixtures pass.

Exit gate: all parity and failure-path acceptance tests below pass; no global silent limits; v1 data migration decision is implemented.

### Phase 1: embedded v2 default

- Split control, metrics, and follower spool databases.
- Introduce versioned push batches, durable acknowledgement, current cache, gap/freshness status, rollups, byte limits, and separate snapshot/series APIs.
- Move to a 30-second default only after the reference benchmark passes.
- Add cumulative network/block counters and service rollups behind capability/version fields.

Exit gate: 72-hour soak and fault tests pass at the documented embedded envelope, backup/restore is exercised, and overload degradation is visible.

### Phase 2: export and HA control profiles

- Stabilize the `MetricsStore` backend contract and exporter boundary.
- Add operator-owned OTLP export first unless PromQL is an explicit product requirement; otherwise implement a tested Remote Write adapter.
- Add PostgreSQL control storage, leader lease/fencing, and deployment/restore automation.
- Evaluate TimescaleDB as the first external metrics implementation if users want one SQL service.

Exit gate: conformance tests produce equivalent API semantics across backends; license/security/upgrade runbooks are approved.

### Phase 3: scale-specific metrics backends

- Choose VictoriaMetrics when PromQL/ecosystem interoperability dominates.
- Choose ClickHouse when long retention and analytical aggregation dominate.
- Do not ship both until actual user workloads require both and each has an owner, benchmark, migration path, backup/restore test, and operational guide.

External-backend triggers include any of: required automatic metrics HA, sustained ingest above the embedded tested limit, raw retention whose measured size exceeds the local disk budget, query concurrency/latency beyond the SQLite SLO, or existing operator infrastructure that is explicitly preferred.

## Acceptance tests

### Contract and parity

- Shared JSON fixtures decode identically in the HTTP server, Tauri Rust types, and TypeScript; fixtures include nonempty containers and `serviceId`.
- Snapshot/history results cover 1, 6, and 24 hours for more than 100 nodes and more than 500 containers per node without silent loss.
- Query keys isolate two configured agents and different range/filter/step combinations.
- Heterogeneous-node cluster CPU uses capacity-weighted deltas.
- Unknown range/metric/aggregation and limit violations return structured errors.

### Collector math and identity

- Golden Docker stats fixtures cover cgroup v1/v2 cache subtraction, zero `online_cpus`, zero/negative/reset deltas, host reboot, container restart, counter wrap/corruption, paused container, and no memory limit.
- Rolling update, replicated/global service, replicated job, task replacement, and container restart never merge two incarnations.
- A missing container stats call creates a scoped partial error while node and other containers remain usable.
- Host disk mount and platform-specific memory definitions are documented and tested on every supported OS/architecture.

### Transport and failure

- Duplicate, delayed, out-of-order, split, unsupported-version, oversized, and corrupt batches are rejected or idempotently committed as specified.
- One slow/offline follower cannot delay collection or ingestion from another node.
- Disconnect for six hours, reconnect, replay, acknowledgement loss, agent restart, leader restart, and leader failover preserve sequence/gap semantics.
- Clock skew forward/backward and daylight-saving changes do not corrupt monotonic rate calculation or UTC buckets.
- Bounded queues/spools stay within limits under a slow database; the reported lost interval matches deliberately dropped batches.

### Database, retention, and migration

- Kill the leader before/during/after commit and checkpoint; after restart each acknowledged batch exists exactly once and no unacknowledged batch is falsely acknowledged.
- Fill the metrics volume to soft/low/emergency thresholds; control authentication/configuration/reconciliation remain writable and telemetry status is degraded explicitly.
- Rollup, crash, restart, late arrival, repair, and prune preserve aggregate counts and never delete raw data ahead of the correct watermark.
- Long readers and concurrent snapshot/range queries do not cause unbounded WAL growth or writer starvation.
- v1 schemas migrate or archive according to policy, with row counts and representative values verified; rollback/restore is tested.
- Backup taken during writes restores with a valid integrity check, schema versions, credentials, and expected RPO.

### API and authorization

- Results are stable-ordered and paginated; gaps are null, stale values are marked, and partial backends cannot claim completeness.
- Series queries enforce series, point, byte, time, and concurrency limits before expensive work.
- Cluster/node/service authorization applies to both IDs and resolved display metadata; deletion/export policies are tested.
- Raw counters outside JavaScript's exact range round-trip as decimal strings; chart values reject NaN/infinity.

## Benchmark plan

Use a deterministic generator that produces the same semantic trace for SQLite and every future backend. Avoid comparing a tuned vendor demo with an untuned application schema.

Matrix:

- nodes: 1, 10, 50, and 100;
- concurrent containers: 10, 100, 500, 2,000, and 10,000;
- interval: 10, 30, and 60 seconds;
- churn: 0%, 10%, and 100% task replacement per hour;
- history: 1 hour, 24 hours, 7 days, 30 days, and one year of rollups;
- storage: reference local SSD plus the slowest supported local-disk class; never benchmark SQLite WAL on NFS;
- workload mixes: ingest only, UI snapshot polling, bounded range queries, rollup, prune, backup, and all concurrently.

Measure:

- Docker daemon request count/latency and agent collection duration/completeness;
- serialization/compression ratio, bytes/batch, reconnect throughput, spool growth;
- accepted rows/s, transaction p50/p95/p99, fsync time, WAL size/checkpoint stalls, lock waits;
- bytes per node/container sample including indexes, retained/allocated bytes, reclaim rate, write amplification;
- process CPU/RSS/file descriptors and database connection counts;
- snapshot/range p50/p95/p99/maximum, scanned rows/bytes, cancellation latency, response bytes;
- rollup/prune duration, ingestion interference, late repair cost, and backup/restore throughput;
- startup latest-cache reconstruction and migration time;
- cardinality/index growth under churn for Prometheus-compatible candidates.

Run a 72-hour steady/churn soak with injected process kills, temporary disk stalls, network partitions, acknowledgement loss, and ENOSPC thresholds. Publish hardware, filesystem, SQLite/backend versions, schema/index definitions, raw results, and flame/query plans. The embedded profile is supported only at the largest matrix point that meets every budget with at least 30% headroom.

## GitHub issues / open decisions

Each item should become a decision issue with an owner, evidence, and explicit exit criterion.

1. **Define the supported embedded envelope and 30-second default.** Run the benchmark, choose the advertised node/container/churn limits, and decide whether Phase 1 defaults to 30 or remains at 60 seconds.
2. **Choose v1 telemetry migration behavior.** Decide copy versus read-only archive versus explicit discard; include both legacy schemas, downtime, rollback, and UI notice.
3. **Finalize push transport and batching.** Choose bidi stream versus idempotent unary, compression threshold, maximum batch, acknowledgements, schema negotiation, and certificate/identity binding.
4. **Finalize metric definitions.** Approve node CPU denominator, container CPU relative to host versus limit, cgroup working set, disk mount scope, network aggregation, reset handling, and supported platforms.
5. **Set retention tiers and disk safety values.** Validate raw/5m/1h durations, allowed lateness, repair, byte budget, reserves, and install-volume discovery.
6. **Choose cardinality degradation behavior.** Decide per-task cap, deterministic rotation/sampling, priority by entity kind, exported-series limits, and exact API warning semantics.
7. **Stabilize snapshot and series contracts.** Confirm UI chart needs, series/point limits, counter serialization, pagination, stale threshold, error format, and generated/shared DTO strategy.
8. **Specify agent time semantics.** Define acceptable skew, monotonic interval transport, boot/instance identity, late-data window, and UI representation of uncertain time.
9. **Specify rollup math.** Define weighted CPU/service aggregation, gauge min/max/mean, counter delta/reset, missing samples, expected count, completeness, and versioned reprocessing.
10. **Implement backup and disaster-recovery policy.** Set control RPO/RTO, encryption/key handling, destination, restore drill cadence, and whether metrics are ever backed up by default.
11. **Design PostgreSQL HA control profile.** Include leader lease/fencing, schema migrations during rolling upgrades, connection pooling, failover discovery, and backup/PITR automation.
12. **Select the first external metrics path.** Based on user evidence, choose OTLP export, Prometheus Remote Write/VictoriaMetrics, or TimescaleDB; implement one before considering another.
13. **Complete licensing and distribution review.** Record Timescale License boundaries, VictoriaMetrics Enterprise dependencies, container redistribution, hosted-service implications, and upgrade support.
14. **Define metrics security/privacy.** Classify names/images/labels, authorize queries/exports, set redaction/deletion rules, and test decompression/allocation limits.
15. **Create backend conformance and benchmark harnesses.** Make idempotency, gaps, query limits, rollups, retention, and status semantics portable before adding a second store.

Until these issues are decided, the safe implementation default is dual-file SQLite with a one-minute parity cadence, a 24-hour raw horizon, no arbitrary labels, no silent query truncation, and an explicit path to the benchmark-gated 30-second/rollup profile.

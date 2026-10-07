# 0003. Node metrics are check-in samples in PostgreSQL, no spool or rollups

- Status: accepted (amended by 0039)
- Date: 2026-09-28
- Spec: docs/build/05-membership-and-node-metrics.md

## Context

The reset doc and `docs/v2/metrics-storage.md` describe an HA-grade pipeline: agents spool batches in
checksummed disk segments, push idempotent batches, the controller acknowledges after durable commit,
and rollups feed series queries. The desktop design needs far less: four stat cards, per-node CPU and
memory bars, a status bar, and sparklines over the last 60 seconds to 24 hours. One user runs one
controller.

## Decision

- Every agent check-in (default every 5 seconds) carries one `NodeSample`: CPU percent, memory used
  and total, disk used and total for the Docker data root, container counts by state, and network
  bytes per second.
- The controller upserts the node row and inserts the sample into a `node_samples` table in the same
  transaction. No spool, no batch acknowledgement, no rollups.
- Retention is one typed controller setting, `metricsRetentionDays`, default 7. A supervised task
  prunes samples older than that once an hour.
- Reads are two shapes only: the latest sample per node (snapshot) and samples for one node or the
  cluster within a window, bucketed by the caller's bucket size in SQL. No series API beyond that.
- A missed check-in is a gap. Nothing is replayed.

## Consequences

- Sparklines survive controller restarts and cover the design's 24-hour windows for free.
- At 5-second cadence and three nodes the table grows by about 52,000 rows a day, trivially pruned.
- When a cluster grows past what one insert per node per 5 seconds can carry, revisit with the spool
  design from `docs/v2/metrics-storage.md`. Until then it is speculative.

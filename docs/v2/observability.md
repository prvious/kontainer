# Observability and production diagnosis in Kontainer v2

Status: proposed production contract

Research date: 2026-08-19

Scope: the Rust controller and node-agent processes, their HTTP, gRPC, Docker Engine, database, scheduler, operation, lifecycle, export, profiling, support, alerting, and incident-response surfaces. Workload log viewing and exec transport are covered only at their diagnostic boundary; their content contracts remain in [logging](./logging.md) and [exec](./exec.md).

## Executive decision

Kontainer v2 must be diagnosable from the outside without turning observability into another path for exporting customer workload data. It will use one typed instrumentation layer, built around Rust's `tracing` ecosystem, but five deliberately separate records:

| Plane | Purpose | Authority and durability | May contain |
|---|---|---|---|
| Application diagnostic events | Explain what this process observed or attempted | Best effort and bounded; JSON to stdout/stderr by default | Safe error code, component, timing, request/trace/operation correlation, bounded control-plane metadata |
| Distributed traces | Explain latency and causality across controller, agent, Engine, and database | Sampled and expendable; OTLP to an operator-owned Collector | Safe span attributes and events; no bodies, statements, workload output, or arbitrary names |
| Internal metrics | Detect symptoms, saturation, freshness, and error budgets | Low-cardinality current/process history in the operator's monitoring system | Counts, gauges, histograms, coarse enumerations; never object or user identifiers |
| Operation event history | Tell the user the durable truth about accepted asynchronous work | Transactional product state, queryable through the authorized operations API | Per-step state, safe target reference, attempts, uncertainty, partial outcomes, stable error codes |
| Security audit | Establish who did what and preserve tamper-evident security evidence | Append-only, hash-chained, externally anchorable; fail-closed where [security](./security.md) requires it | Actor, authorization, intent, safe target, result, and content-free log/exec metadata |

No plane may be reconstructed by scraping another. In particular, application logs are not an audit trail, traces are not operation state, the workload metrics database is not Kontainer's monitoring backend, and audit availability is not allowed to depend on a lossy log exporter.

The production defaults are:

- newline-delimited structured JSON on stdout/stderr; the container runtime or host owns collection, rotation, compression, and forwarding;
- an internal/private Prometheus-compatible `/metrics` endpoint on each binary, separate from the public product API and from workload metrics;
- W3C Trace Context across public HTTP and internal gRPC, with bounded OTLP trace export to a local or nearby OpenTelemetry Collector when configured;
- canonical `/livez`, `/startupz`, and `/readyz` probes plus an authenticated deep status document; lifecycle code owns all four views of the same state;
- a small, safe crash marker and panic event, no core dumps by default, and separately published symbols/build IDs for operator-side profiling;
- an allowlisted, size- and time-bounded support bundle generated locally first, with cluster fan-out added only after the authorization and partial-result path is proven;
- shipped dashboards, recording rules, alerts, and runbooks that are release artifacts and are fault-tested against the same signals production consumes.

The observability path is never on the success-critical path. A full exporter queue drops diagnostic telemetry according to policy, increments a self-observation counter, and marks telemetry export degraded; it does not block Docker control work, make liveness fail, or consume the audit reserve. Audit and operation persistence keep their separate fail-closed/fail-safe contracts.

## Goals, non-goals, and invariants

An operator must be able to answer, without enabling global debug logging:

1. Is the process alive, has it completed startup, can it admit this class of work, and which capability is degraded?
2. Where did request `R`, trace `T`, or operation `O` spend time, retry, partially succeed, or become uncertain?
3. Is pressure in HTTP, gRPC, Docker, the database, a queue, the scheduler, the runtime, disk, memory, or an exporter?
4. Did a controller fail over, an agent lose identity, a schedule lose its lease, or a snapshot become stale?
5. What evidence can safely be attached to a support case, and what must remain on the customer system?

The following are invariants, including during an incident:

- Workload log lines, exec stdin/stdout/stderr, Docker config and secret contents, container or service environment, registry authorization, tokens, private keys, provider credentials, HTTP or gRPC bodies, SQL text, and raw upstream response bodies never enter application events, traces, metrics, panic reports, profiles, or support bundles.
- A log level change changes event selection, not the data-classification policy. `TRACE` is not permission to serialize prohibited data.
- Client-controlled strings do not become metric label values, span names, logger targets, event field names, or filesystem paths.
- Public health responses do not reveal topology, dependency addresses, database type, certificate material, filesystem paths, or internal errors.
- Diagnostic telemetry has hard item and byte limits and cannot borrow the protected quotas for audit, identity, idempotency, leases, or operation state.
- Every diagnostic surface has an owner, authorization rule, retention rule, resource budget, and failure mode before it is enabled in production.

This document does not create a public query language over diagnostic events or traces. Operators use their platform's log, trace, and monitoring systems; Kontainer exposes only the bounded health, internal scrape, operation, audit, and support surfaces specified here.

## What v1 actually provides

The current Go/Goravel server has useful log statements, but it does not have an observability system that can be carried into v2 unchanged.

### Logging and errors

- [`server/config/logging.go`](../../server/config/logging.go) defaults to a daily text file at `storage/logs/goravel.log`, retains seven files, and does not print to stdout. This conflicts with the immutable-container/stdout lifecycle contract and has no byte cap, exporter, or dropped-event signal.
- Logging is free-form `Infof`, `Warningf`, and `Errorf`. There are no stable event names, structured fields, request IDs, trace IDs, operation IDs, data classifications, or field bounds.
- [`server/routes/setup.go`](../../server/routes/setup.go) logs the complete setup token. Controllers also interpolate object names, container/node IDs, follower addresses, shell names, and raw Docker, gRPC, database, and OAuth errors. This is direct evidence that redaction cannot be a final string replacement step.
- Framework/database errors in the current log file include stack traces, SQL/table information, source paths, and filesystem paths. Some HTTP and WebSocket paths return raw `err.Error()` strings. Other paths collapse unrelated failures to generic `500`, `503`, or gRPC errors, so neither operators nor clients receive a stable cross-layer taxonomy.
- There is no installed general request/access log or correlation middleware. A controller-to-agent call, Engine error, and resulting HTTP response cannot be joined reliably.

### Health, scheduling, and operations

- [`health_controller.go`](../../server/app/http/controllers/health/health_controller.go) always returns `{"status":"ok"}`. `/ping` always returns HTTP 200 and makes only a setup-related database query. The custom gRPC health method also reports `ok` after construction and is not the standard gRPC health protocol.
- The schedule in [`kernel.go`](../../server/app/console/kernel.go) runs discovery every five or ten seconds, collection every minute, and pruning every five minutes with `SkipIfStillRunning`. Jobs use background contexts and expose no last start, last success, skip count, deadline, lease owner, progress, or history. Their failures are only log lines.
- Discovery and collection perform sequential fan-out. A transient manager list failure clears the follower registry; unreachable followers and raw addresses appear in logs. There is no state transition or causal event that distinguishes stale, suspect, degraded, and removed.
- Mutations, image fan-out, log sessions, and exec sessions do not share a durable operation ledger. Partial remote success is often logged and omitted from the response.
- `main` calls `app.Start()` without an application-owned panic, signal, drain, telemetry flush, or crash-marker policy.

V2 parity therefore means preserving useful operator visibility while replacing unsafe content and false health. It does not mean preserving v1's text format, file location, token exposure, raw errors, or always-healthy responses.

## One instrumentation model, separate sinks

### Component structure

Both binaries should use a shared telemetry crate that owns:

- the field catalog and stable event names;
- safe wrappers for identifiers, errors, secrets, URLs, and durations;
- HTTP and Tonic interceptors, Docker and database adapters, operation/schedule context, and task instrumentation helpers;
- Prometheus metric registration and cardinality tests;
- `tracing_subscriber` layers for JSON events and OpenTelemetry spans;
- reloadable filters with an expiry timer;
- exporter queues, self-observation, and bounded shutdown flush;
- compile-time and test-time forbidden-field checks.

`tracing` spans describe periods of work and events describe points within those periods. The `#[instrument]` macro records all function arguments by default, using `Debug` for most compound types; Kontainer must therefore require `#[instrument(skip_all, fields(...))]` and an explicit allowlist in production code ([`tracing::instrument`](https://docs.rs/tracing/latest/tracing/attr.instrument.html)). Its `ret` and `err` options are also forbidden unless the value is an explicitly safe wrapper. Secret-bearing domain types implement redacted `Debug` and `Display` as defense in depth, not as the primary filter.

For asynchronous work, instrument the future or spawned task so the span is entered on each poll; do not hold a synchronous span-enter guard across `.await`. The `Instrument` trait is specifically designed to attach a span to a future and propagate the current span into spawned work ([`tracing::Instrument`](https://docs.rs/tracing/latest/tracing/trait.Instrument.html)).

### Resource identity

Every emitted event, span, and metric target carries bounded process resource identity:

- `service.name`: `kontainer-controller` or `kontainer-node-agent`;
- `service.version`: product SemVer;
- `service.instance.id`: a random boot-scoped UUID, not a hostname or node ID;
- `kontainer.release.commit`, protocol version, schema version, target architecture, and build profile as resource metadata;
- `deployment.environment.name` only from an operator-defined allowlisted value;
- process start time and a short boot ID suffix in logs/status.

Raw cluster, node, tenant, user, service, task, container, and operation IDs are not resource attributes because they either change per record or create unsafe aggregation dimensions. A controller may attach a stable operator-configured deployment label to OTLP resource metadata after validation, but `/metrics` relies on scrape target labels rather than duplicating it on every series. OpenTelemetry resources describe the entity producing telemetry; semantic-convention versions still need to be pinned as part of Kontainer's schema rather than floated with dependency upgrades ([OpenTelemetry resources](https://opentelemetry.io/docs/specs/semconv/resource/), [semantic conventions](https://opentelemetry.io/docs/specs/semconv/)).

### Event levels

Levels have operational meaning:

| Level | Use | Examples |
|---|---|---|
| `ERROR` | This process failed an admitted unit of work or violated an invariant and an operator may need to act | critical supervisor panic, durable operation becomes uncertain, database corruption signal |
| `WARN` | Work continued with reduced quality or a bounded exceptional condition | stale snapshot served, exporter dropping, capability degraded, retry budget exhausted |
| `INFO` | Low-volume lifecycle, security-safe operation boundary, and state change | ready/unready transition, agent enrolled, operation admitted/completed, filter changed |
| `DEBUG` | Bounded troubleshooting detail with no payload | dependency attempt, cache decision, retry timing, safe upstream classification |
| `TRACE` | Short-lived development detail, disabled in production by default | internal state-machine edges and queue decisions; still no bodies or workload content |

Normal successful reads do not produce one `INFO` line per lower-level call. Their request summary can be sampled or emitted at `DEBUG`; counters and histograms carry the aggregate. State-change logs are edge-triggered and include the previous/new state, not repeated on every probe.

### Stable event schema

JSON events are newline-delimited objects. Required and optional fields are fixed in code; callers cannot add arbitrary keys.

```json
{
  "timestamp": "2026-08-19T16:42:31.381Z",
  "level": "WARN",
  "event": "operation.target.completed",
  "message": "operation target completed with a retryable failure",
  "component": "fanout",
  "service": "kontainer-controller",
  "version": "2.0.0-beta.3",
  "bootId": "8b2d8c4f",
  "requestId": "0198c53b-5ad2-7de2-bc2b-c45ff0cd8c1e",
  "traceId": "71f3e3b9a6e14c5461419d674be1dbd5",
  "spanId": "8d3fbdb14cd5f06a",
  "operationId": "op_0198c53b5b0875b1",
  "operationKind": "image.prune",
  "targetKind": "node",
  "targetRef": "node:7f1c2a9e",
  "attempt": 2,
  "outcome": "failed",
  "errorCode": "engine_unavailable",
  "retryable": true,
  "durationMs": 3012
}
```

`message` is a static human description selected by event name. It is not string interpolation of an upstream error. Fields use JSON numbers/booleans where applicable, strings are valid UTF-8 with control characters escaped, and every field has a maximum encoded length.

Baseline bounds:

- event name, component, enum, and error code: 64 bytes;
- route template or RPC method: 128 bytes;
- safe message: 256 bytes;
- safe error detail in protected diagnostic logs: 1 KiB;
- one serialized event: 16 KiB, after which optional fields are dropped and `truncated=true` is added;
- at most 32 fields per span and 16 fields per span event;
- at most 32 events per span, with excess represented by `span_events_dropped_total{span_kind}`.

These are initial implementation ceilings and must be load-tested. An event that cannot be encoded safely is replaced by `telemetry.event_rejected` with only its static event name and rejection code.

## Data classification and redaction

### Classes

| Class | Examples | Diagnostic treatment |
|---|---|---|
| Public | component, version, protocol compatibility, coarse health code | Allowed in public probes and all protected sinks |
| Internal | route template, operation kind, dependency kind, timing, queue depth, stable domain error | Allowed in authenticated diagnostics and protected telemetry |
| Restricted | principal reference, opaque resource ID, node identity, peer certificate subject, internal path/address, sanitized stack | Excluded by default; allowed only in a named field and protected sink when needed; pseudonymized in support bundles |
| Prohibited | credentials, secret/config values, environment, request/response bodies, query strings, SQL, registry auth, workload logs, terminal I/O, raw Docker/provider error body, private key, memory contents | Never collected or emitted |

Classification is attached to types/fields, not inferred from key-name regexes. A `Secret<T>` cannot be recorded. A `SafeError` contains a stable code, dependency, retryability, and allowlisted cause class; it does not expose the source error's arbitrary `Display`. A `SafeResourceRef` contains kind plus a bounded opaque identifier only when correlation requires it. A `PublicProblem` is safe for the API. Conversions from raw errors to any of these are explicit and unit-tested.

### Identifier policy

- `requestId`, `traceId`, `spanId`, and `operationId` are intentionally usable for correlation in protected logs and traces. The operations API already authorizes access to operation state.
- Principal IDs are HMAC-pseudonymized in diagnostic events. Audit retains the canonical authorized actor reference under its stronger access policy.
- Resource IDs may appear as opaque, bounded restricted fields only for mutation/operation diagnosis. Names, labels, image references, and user-provided stack names do not appear in routine diagnostics.
- Support bundles replace all restricted identifiers with stable bundle-local HMAC aliases such as `node-01` or `resource-a82f`; the random bundle key and raw mapping are not included.
- Metric labels never contain any of these identifiers, including request, trace, operation, user, node, task, container, service, stack, image, hostname, address, URL, or error-message values.

### Redaction pipeline

The pipeline is **construct safe type → validate/bound → serialize → route to sink**. It is not format an arbitrary string and then apply regex replacements. Regex/token scanning is retained as a final tripwire in tests and support-bundle generation.

Required defenses are:

1. secret and credential wrappers with constant redacted `Debug`/`Display`;
2. `skip_all` instrumentation and explicit safe fields;
3. domain error mapping at the adapter boundary;
4. schema-based field allowlists per event/span type;
5. length, character, and enumeration validation before allocation/serialization;
6. sink-specific policy—public health is a strict subset of protected logs, and support bundles are stricter again;
7. canary-secret tests through HTTP, gRPC, Docker fake errors, SQL errors, panic payloads, workload streams, filters, exporters, and interrupted bundle generation.

No dynamic filter may bypass these steps. Any debug-only raw dump helper is prohibited from production builds.

## Context and causality

### Correlation identifiers

The following identifiers are distinct:

| Identifier | Created by | Lifetime | Propagation |
|---|---|---|---|
| `request_id` | Controller at every incoming HTTP request, UUIDv7 | One HTTP exchange or upgraded-stream handshake | Returned as `X-Request-Id`; included in operation admission and safe errors |
| `trace_id`/`span_id` | Tracer, honoring a valid trusted-format remote parent | Distributed causal work | W3C `traceparent`/`tracestate` over HTTP and gRPC metadata |
| `operation_id` | Durable operation store | Admission through terminal/expired history | API response, operation rows/events, audit, traces, diagnostic events |
| `audit_event_id` | Audit store | One audit record | Audit chain and optional reference from operation history; not a metric label |
| `schedule_run_id` | Scheduler/claim owner | One claimed occurrence | Schedule record, operation if created, diagnostic spans/events |
| `boot_id` | Process at startup | One process incarnation | Logs, status, crash marker, resource metadata |

`X-Request-Id` supplied by a client is captured only as a separately named, syntax/length-validated `client_request_id`; the server still creates its own UUIDv7 and never relies on client uniqueness. Invalid trace context is ignored and a fresh root is created. OpenTelemetry requires W3C Trace Context propagators to parse and validate `traceparent` and `tracestate` and to propagate valid context ([propagators API](https://opentelemetry.io/docs/specs/otel/context/api-propagators/)).

Kontainer accepts and propagates `traceparent` and bounded `tracestate`, but drops all inbound `baggage`. Baggage is arbitrary application-defined key/value context that can cross trust boundaries and is visible in transport headers; the API also explicitly supports clearing it before an untrusted boundary ([OpenTelemetry baggage](https://opentelemetry.io/docs/concepts/signals/baggage/), [Baggage API](https://opentelemetry.io/docs/specs/otel/baggage/api/)). Kontainer has no safe need for client-selected telemetry attributes.

### Trace topology

The common trace is:

```text
HTTP SERVER <METHOD route-template>
└── operation.admit / cached_read
    ├── db.client <safe-operation>
    └── operation.dispatch or fanout.coordinator
        ├── gRPC CLIENT <package.Service/Method>
        │   └── gRPC SERVER <package.Service/Method>
        │       └── docker.engine <operation-class>
        └── operation.persist_transition
```

When durable asynchronous execution outlives the admission request, the worker starts a new trace and adds a span link to the admission span context stored with the operation. It does not pretend an hours-long parent span stayed open or require a sampled parent to exist. Retry attempts are child spans only where the call has meaningful duration; retry decisions are bounded events on the owning span. Fan-out has one coordinator span and one target-attempt span per admitted target, subject to the supported 100-node envelope—not one span per returned resource.

### Layer conventions

OpenTelemetry HTTP, RPC, and database semantic conventions are used where stable, with the convention version pinned in the telemetry schema. Changing semantic-convention versions is a schema migration and dashboard compatibility review, not an incidental crate update. The database conventions in particular have explicit migration guidance and mixed stability, so Kontainer must never enable automatic SQL statement capture ([HTTP spans](https://opentelemetry.io/docs/specs/semconv/http/http-spans/), [RPC spans](https://opentelemetry.io/docs/specs/semconv/rpc/rpc-spans/), [database spans](https://opentelemetry.io/docs/specs/semconv/db/)).

| Boundary | Span/event policy | Explicitly excluded |
|---|---|---|
| HTTP server | Method, normalized route template, status code, response byte bucket, request/trace IDs, auth outcome, duration | Raw path for unmatched routes, query, headers except validated context, body, user agent, client IP by default |
| Tonic gRPC | Standard service/method, client/server role, status, deadline bucket, attempt, peer workload kind | Message body/size detail beyond bounded byte counters, peer address in normal logs, certificate bytes |
| Docker Engine | `docker.engine` span with adapter operation class (`service.list`, `task.inspect`, `container.stats`, etc.), local/manager scope, outcome, stable Engine error class | Socket path, raw API URI/query, request/response JSON, environment, registry auth, Docker error string |
| Database | Pinned database-system semantic attribute, logical store (`control`, `audit`, `metrics` only where applicable), operation class (`select`, `insert`, `transaction`), pool wait and duration | Connection string, host, user, SQL text, bind values, row contents, table/object IDs in metrics |
| Operation | Kind, state transition, target count, claimed/uncertain/partial outcome, attempt, operation ID in trace/log only | Submitted payload, target names, secret/config content |
| Scheduler | Job enum, planned/start/end times, lease result, skipped/coalesced count, duration, safe result | Arbitrary job names, lease row/raw owner, payload |
| Reconcile/cache | Resource-kind enum, trigger enum, generation, item count, age, completeness, duration | Resource documents, user labels/names, per-item spans |
| Log/exec stream | Handshake, source/session count, byte counts, duration, gap/drop/close reason | Every workload log line and all terminal bytes, shell command text, terminal title |

Long-lived WebSocket and gRPC streams have one lifecycle span from authenticated admission to close, plus aggregate events at material state changes. Per-frame and per-log-line spans/events are rejected. Metrics carry bytes, queue occupancy, gaps, and close reasons.

## Internal metrics

### Storage and exposure

Kontainer's own monitoring metrics are not written to `metrics.sqlite3` or the product's shared workload-metrics tables. [Metrics storage](./metrics-storage.md) owns customer-visible node/container history; mixing process diagnostics into it would make control-plane diagnosis depend on the system under diagnosis and blur authorization/retention.

Each process exposes Prometheus text/OpenMetrics on `/metrics` on the private diagnostics listener. That listener binds loopback by default for a native service or the private control network for Swarm, is never published by the public ingress service, and uses mTLS or an equivalent operator-only network/auth boundary. The public `/api/v2/metrics/*` namespace remains workload data and must not proxy this scrape.

Prometheus scrape is the beta default because it is simple, pull-based, and works when the OTLP exporter is broken. OTLP metrics export is optional after equivalence tests, but it consumes the same registered instruments rather than creating a second schema. Operators may relabel scrape targets with cluster/deployment identity; the application does not add raw cluster or node IDs to every series.

Prometheus recommends request count, errors, latency, and in-progress work for online services; items, in-progress work, last processed time, and stage flow for offline processors. It also recommends an attempt total alongside failures and timestamps rather than an ever-updated "seconds since" gauge ([instrumentation guidance](https://prometheus.io/docs/practices/instrumentation/)). Kontainer follows those patterns.

### Naming and labels

All product-owned names use the `kontainer_` prefix, base units, `_seconds`, `_bytes`, `_ratio`, or `_total` suffixes as appropriate. A metric represents one logical quantity across its label dimensions, following Prometheus naming guidance ([metric and label naming](https://prometheus.io/docs/practices/naming/)). Histograms, not client-side summaries, are the default where aggregation across controllers or nodes is required.

Allowed labels are closed enums with reviewed cardinality. The catalog includes:

- `component` and `workload_kind`: `controller|node_agent` where a shared dashboard needs them;
- `version` and `protocol` only on `kontainer_build_info`, with one value of each per process;
- `method`: a compiled list of supported HTTP methods plus `OTHER`, never an arbitrary method token;
- `route`: normalized route template from a compiled list;
- `rpc_service`, `rpc_method`: compiled protocol values;
- `operation_kind`, `resource_kind`, `job`, `dependency`, `capability`, `queue`, `traffic_class`, `store`, `operation`, `scope`, `purpose`, `phase`, `stage`, `class`, `kind`, and `event_class`;
- `peer_component` and `result` from the compiled protocol compatibility matrix;
- `signal`, `level`, `boundary`, `action`, `stream_kind`, `direction`, `surface`, `trigger`, and `completeness`;
- `outcome`, `state`, `from`, `to`, `reason`, `error_code`, `grpc_code`, or coarse `status_class` from bounded enums;
- `attempt_bucket` or `deadline_bucket` only where aggregate diagnosis needs it.
- `worker`: a configured process-local Tokio worker ordinal in the fixed range `0..7`.

Forbidden labels include every opaque ID, name, address, URL, path, certificate subject, user-supplied string, raw HTTP status message, raw error, image reference, and numeric value formatted into a label other than the reviewed worker ordinal. HTTP status uses a small class or bounded code set; unmatched routes use `route="unmatched"`. A CI test enumerates possible values and calculates the maximum series count of every vector.

Prometheus notes that every label combination creates another time series, recommends cardinality below 10 for most metrics, and calls for investigation when a dimension can reach 100 ([cardinality guidance](https://prometheus.io/docs/practices/instrumentation/#do-not-overuse-labels)). Kontainer is stricter: any single application label must have at most 32 possible values, any family at most 200 series per process, and exceptions require a written query/use case and load evidence.

### Required families

The exact Rust instrument names are versioned in a generated registry. The following is the minimum schema; labels shown are the only application labels beyond scrape-target labels.

| Metric | Type | Labels | Purpose |
|---|---|---|---|
| `kontainer_build_info` | gauge fixed at 1 | `component`, `version`, `protocol` | Join deployment/build state; commit remains target metadata if cardinality policy requires |
| `kontainer_lifecycle_state` | one-hot gauge | `state` | Starting, bootstrap, ready, degraded, draining; initialize all values to zero |
| `kontainer_uptime_seconds` | gauge | none | Process incarnation context |
| `kontainer_startup_duration_seconds` / `shutdown_duration_seconds` | histogram | `phase`, `outcome` | Lifecycle phase timing and timeout diagnosis |
| `kontainer_dependency_state` | one-hot gauge | `dependency`, `state` | Authenticated status in aggregate monitoring form |
| `kontainer_capability_state` | one-hot gauge | `capability`, `state` | Which traffic/work class is impaired |
| `kontainer_peer_compatibility` | one-hot gauge | `peer_component`, `result` | Adjacent protocol/build skew without exact peer versions |
| `kontainer_schema_version` | gauge | `store` | Numeric active schema version |
| `kontainer_migration_state` | one-hot gauge | `store`, `state` | Clean/running/failed/dirty migration state |
| `kontainer_backup_last_success_timestamp_seconds` | gauge | `class` | Backup staleness calculated at query time |
| `kontainer_restore_drill_last_success_timestamp_seconds` | gauge | none | Restore-drill staleness calculated at query time |
| `kontainer_update_state` | one-hot gauge | `component`, `state` | Canary/rolling update progress |
| `kontainer_http_server_requests_total` | counter | `route`, `method`, `status_class`, `outcome` | API rate/error budget |
| `kontainer_http_server_request_duration_seconds` | histogram | `route`, `method`, `outcome` | Request latency SLO |
| `kontainer_http_server_in_flight` | gauge | `traffic_class` | Admission/saturation |
| `kontainer_grpc_server_requests_total` / `client_requests_total` | counter | `rpc_service`, `rpc_method`, `grpc_code` | Both sides of internal RPC |
| `kontainer_grpc_server_duration_seconds` / `client_duration_seconds` | histogram | `rpc_service`, `rpc_method`, `outcome` | RPC latency |
| `kontainer_grpc_in_flight` | gauge | `rpc_method`, `traffic_class` | Unary and streaming RPC admission pressure |
| `kontainer_grpc_bytes_total` | counter | `rpc_method`, `direction` | Aggregate internal transport volume |
| `kontainer_grpc_streams` | gauge | `rpc_method`, `state` | Long-lived stream ownership |
| `kontainer_docker_requests_total` | counter | `operation_kind`, `scope`, `outcome`, `error_code` | Engine pressure and failures |
| `kontainer_docker_request_duration_seconds` | histogram | `operation_kind`, `scope`, `outcome` | Engine latency |
| `kontainer_docker_in_flight` | gauge | `traffic_class` | Manager/local permits |
| `kontainer_database_operations_total` | counter | `store`, `operation`, `outcome` | DB failures without SQL labels |
| `kontainer_database_operation_duration_seconds` | histogram | `store`, `operation`, `outcome` | DB latency |
| `kontainer_database_pool_connections` | gauge | `store`, `state` | Used/idle/waiting |
| `kontainer_database_pool_wait_duration_seconds` | histogram | `store` | Pool saturation |
| `kontainer_audit_events_total` | counter | `event_class`, `outcome` | Accepted/persisted/rejected audit flow without actor/action IDs |
| `kontainer_audit_writer_state` | one-hot gauge | `state` | Whether required audit intent can be persisted |
| `kontainer_audit_chain_failures_total` | counter | `reason` | Gap, hash, signature, and checkpoint verification failures |
| `kontainer_audit_checkpoint_last_success_timestamp_seconds` | gauge | `kind` | Local signed checkpoint and external anchor freshness |
| `kontainer_audit_unanchored_events` | gauge | none | Backlog awaiting external integrity anchor |
| `kontainer_queue_items` / `queue_bytes` | gauge | `queue` | Current bounded occupancy |
| `kontainer_queue_dropped_total` | counter | `queue`, `reason` | Shedding, including telemetry |
| `kontainer_operations_total` | counter | `operation_kind`, `outcome` | Admitted/terminal aggregate |
| `kontainer_operations_in_progress` | gauge | `operation_kind`, `state` | Stalled operation visibility |
| `kontainer_operation_duration_seconds` | histogram | `operation_kind`, `outcome` | End-to-end durable operation time |
| `kontainer_fanout_targets_total` | counter | `operation_kind`, `outcome` | Complete per-target accounting |
| `kontainer_fanout_duration_seconds` | histogram | `operation_kind`, `completeness` | Cluster fan-out latency |
| `kontainer_schedule_runs_total` | counter | `job`, `outcome` | Success/failure/skipped/lease-lost |
| `kontainer_schedule_run_duration_seconds` | histogram | `job`, `outcome` | Scheduled work latency |
| `kontainer_schedule_last_success_timestamp_seconds` | gauge | `job` | Stall detection |
| `kontainer_schedule_lease_state` | one-hot gauge | `job`, `state` | Ownership/uncertainty without owner ID |
| `kontainer_reconcile_runs_total` | counter | `resource_kind`, `trigger`, `outcome` | Event/timer correction health |
| `kontainer_reconcile_duration_seconds` | histogram | `resource_kind`, `outcome` | Reconcile latency |
| `kontainer_snapshot_age_seconds` | gauge | `resource_kind` | Cached view freshness |
| `kontainer_snapshot_items` | gauge | `resource_kind` | Aggregate scale, no per-object label |
| `kontainer_snapshot_complete` | gauge 0/1 | `resource_kind` | Explicit partial/stale view |
| `kontainer_agent_registry_entries` | gauge | `state`, `capability` | Ready/degraded/suspect/quarantined counts |
| `kontainer_agent_heartbeat_age_seconds` | histogram | `state` | Distribution, not one series per node |
| `kontainer_agent_endpoint_changes_total` / `channel_reconnects_total` | counter | `reason` | Membership/network churn without endpoint labels |
| `kontainer_certificate_not_after_timestamp_seconds` | gauge | `workload_kind`, `purpose` | Earliest expiry timestamp across each bounded class |
| `kontainer_certificate_rotations_total` | counter | `workload_kind`, `outcome` | Rotation success/failure |
| `kontainer_log_sessions` / `exec_sessions` | gauge | `state` | Session admission and drain |
| `kontainer_stream_bytes_total` | counter | `stream_kind`, `direction` | Content-free traffic accounting |
| `kontainer_stream_gaps_total` | counter | `stream_kind`, `reason` | Honest loss signal |
| `kontainer_telemetry_events_total` | counter | `signal`, `level`, `outcome` | Emitted/dropped/rejected self-observation |
| `kontainer_telemetry_export_duration_seconds` | histogram | `signal`, `outcome` | Exporter health |
| `kontainer_telemetry_export_queue_items` | gauge | `signal` | Bounded exporter pressure |
| `kontainer_health_transitions_total` | counter | `surface`, `from`, `to`, `reason` | Edge-triggered lifecycle changes |
| `kontainer_schedule_claims_total` | counter | `job`, `outcome` | Claim, takeover, conflict, and lease-loss accounting |
| `kontainer_idempotency_replays_total` | counter | `operation_kind`, `outcome` | Safe replay/conflict behavior |
| `kontainer_workload_telemetry_lag_seconds` | histogram | `stage`, `outcome` | Aggregate collection pipeline lag, distinct from stored workload samples |
| `kontainer_workload_telemetry_dropped_samples_total` | counter | `stage`, `reason` | Honest loss before product persistence |
| `kontainer_panics_total` | counter | `boundary`, `action` | Request/task/fatal panic signal |
| `kontainer_support_bundle_total` | counter | `scope`, `outcome` | Bundle attempts without requester/object IDs |
| `process_*` | standard process metrics | implementation standard | CPU, RSS, virtual memory, FDs, start time |

For collection coverage and freshness, the product workload-metrics pipeline additionally exposes aggregate `expected`, `attempted`, `succeeded`, `missing`, `late`, and `dropped` counts by bounded stage/reason. Missing dependencies remain in denominators; a failed node does not disappear and improve the ratio.

### Tokio and task metrics

The stable baseline reads `tokio::runtime::RuntimeMetrics` for worker count, alive tasks, global queue depth, and per-worker cumulative busy duration where supported; Tokio documents that some values are only eventually consistent and many deeper scheduler metrics remain under `tokio_unstable` ([Tokio `RuntimeMetrics`](https://docs.rs/tokio/latest/tokio/runtime/struct.RuntimeMetrics.html)). Kontainer exports:

- `kontainer_tokio_alive_tasks` and `kontainer_tokio_global_queue_depth` gauges;
- `kontainer_tokio_worker_busy_seconds_total` by bounded worker index only, because workers are a configured 2–8 process-local set;
- application-owned task counts, ages, last progress timestamps, restarts, and failures by supervisor/task-kind enum;
- blocking permits used/limit, queue time, work duration, timeout, and rejection from the application semaphore that must be acquired before `spawn_blocking`.

Beta may enable a reviewed subset of stable additional runtime metrics after benchmark. `tokio_unstable`, scheduler poll-time histograms, and `tokio-metrics` are not GA dependencies until their build/runtime overhead and compatibility are qualified. The runtime SLO from [performance](./performance.md)—scheduling p99 below 50 ms steady and 200 ms during an allowed burst, worker busy below 80% for five minutes, and no growing blocking queue—is derived from stable/app-owned signals in the supported build. If stable APIs cannot measure scheduling latency directly, a low-frequency application sentinel task measures wake-to-run delay; it uses fixed buckets and is validated not to create a self-fulfilling load problem.

## Health, readiness, and degraded operation

### One state model, four views

[Agent lifecycle](./agent-lifecycle.md) owns the canonical surfaces. This resolves the earlier `/health/live` and `/health/ready` spelling in [API protocol](./api-protocol.md):

| Surface | Exposure | Meaning | Failure response |
|---|---|---|---|
| `/livez` | local/orchestrator, unauthenticated but unexposed | Event loop is responsive and no local fatal invariant has latched | Non-2xx only for a wedged/fatal process that should restart |
| `/startupz` | local/orchestrator, unauthenticated but unexposed | Static config, schema compatibility, identity/cert material, local Engine contract, and minimum initial reconcile completed | Non-2xx until startup succeeds; terminal startup errors cause process exit |
| `/readyz` | local/orchestrator/load balancer, unauthenticated but unexposed | This instance can admit its core responsibility now and is not draining | Non-2xx with only coarse code/component |
| `/api/v2/system/status` | authenticated `diagnostics.read` | Deep component, dependency, capability, freshness, capacity, and lifecycle state | `200` with explicit overall state if status itself is available |

If compatibility aliases `/health/live` and `/health/ready` are retained, they call the exact same functions and are deprecated; there are not two health implementations. V1 `/health` may alias `/livez` for one migration window. `/ping` is removed after compatibility because setup state is not liveness.

Internal gRPC implements the standard `grpc.health.v1.Health` service. Each compiled service name reports `SERVING` only when that service's capability is ready and `NOT_SERVING` while starting, draining, quarantined, or unavailable; those are the standard states gRPC assigns to whether a service can accept work ([gRPC health checking](https://grpc.io/docs/guides/health-checking/)). The endpoint is reachable only after the mTLS transport boundary; richer dependency/capability details come from the authenticated, versioned internal describe RPC rather than an ad hoc health payload. HTTP, gRPC, and lifecycle transitions all read the same registry.

Container health latches startup completion and then checks only local event-loop progress and fatal invariants. It must not flap because a shared database, remote manager, exporter, or another controller is unavailable; otherwise a dependency outage becomes a restart storm. Startup never flips back to false. Readiness and deep status may change in both directions.

### Health state and dependency document

Internal state uses `starting|ready|degraded|unready|draining|failed`. Dependency/capability observations use `ok|degraded|unavailable|stale|disabled|unknown`, a stable `code`, `observedAt`, optional `lastSuccessAt`, age/duration, and an `impact` enum. No raw error or address appears.

```json
{
  "status": "degraded",
  "component": "kontainer-controller",
  "version": "2.0.0-beta.3",
  "bootId": "8b2d8c4f",
  "protocol": { "min": 2, "max": 3 },
  "startedAt": "2026-08-19T16:11:04Z",
  "ready": true,
  "dependencies": [
    {
      "name": "controlDatabase",
      "state": "ok",
      "observedAt": "2026-08-19T16:42:30Z",
      "latencyMs": 8,
      "impact": "none"
    },
    {
      "name": "telemetryExport",
      "state": "degraded",
      "code": "export_queue_dropping",
      "observedAt": "2026-08-19T16:42:29Z",
      "lastSuccessAt": "2026-08-19T16:40:01Z",
      "impact": "diagnostics_only"
    }
  ],
  "capabilities": [
    { "name": "cachedReads", "state": "ok" },
    { "name": "mutations", "state": "ok" },
    { "name": "auditExport", "state": "degraded", "code": "anchor_delayed" }
  ]
}
```

Every probe is a snapshot of state maintained by dependency owners; health handlers do not initiate an N-node poll, a Docker list, a migration, an audit checkpoint, or a blocking remote check. Active checks have their own schedule, deadline, jitter, and metrics. State transitions are logged and audited only where security-relevant.

### Controller readiness

Controller startup requires valid static configuration, compatible schema/migration state, controller identity and certificates, a usable manager-local Engine API, successful initialization of audit/operation stores, and one minimum resource reconcile. Bootstrap/unclaimed mode is a separate, explicitly restricted state and is never reported as ordinary ready.

Controller core readiness requires:

- HTTP admission/runtime healthy and not draining;
- shared control database writable for the production HA profile;
- operation and idempotency persistence usable;
- audit intent persistence usable for mutations and other fail-closed actions;
- current controller identity/certificate valid;
- manager-local Engine available for routes this instance advertises;
- active manager role and compatible schema/protocol.

It does not require every node agent, the OTLP Collector, Prometheus, optional OAuth provider, optional audit anchor, workload-metrics history, or every cached resource kind to be healthy. Those make their associated capability degraded. Cached reads may remain ready with an explicit `stale` age within the contract while mutation admission is unready. Readiness is an admission decision, not a claim that all cluster resources are healthy.

### Node-agent readiness

Node-agent startup requires valid identity/enrollment state, compatible protocol/config, local Engine API negotiation, safe node identity, and minimum local inventory. Readiness requires a valid non-expired agent certificate, responsive internal gRPC server, local Engine availability, identity agreement, and advertised capability initialization. A quarantined, identity-mismatched, or draining agent is unready. Metrics export and controller connection loss make it degraded for observability/remote-control capabilities but do not make its local process liveness fail.

### Capability status

Capabilities are evaluated independently: `cached_reads`, `mutations`, `agent_fanout`, `workload_metrics`, `log_streaming`, `exec`, `scheduling`, `audit_query`, `audit_anchor`, `support_bundle`, and `telemetry_export`. Each reports state, stable reason, freshness, and whether admission is open. API errors return the same stable reason code and `requestId`, while raw causes remain only in protected, sanitized diagnostics.

Global `degraded` means at least one supported capability has reduced quality while the process can still serve its core role. It is not an HTTP failure code and not an excuse to hide which capability is affected.

## Durable operation history, audit, and diagnostics

### Operation event history

Every accepted asynchronous mutation or cluster fan-out has a durable operation row and ordered transition events. The minimum event shape is:

```json
{
  "sequence": 7,
  "at": "2026-08-19T16:42:31.381Z",
  "state": "running",
  "step": "dispatchTargets",
  "targetRef": "node:7f1c2a9e",
  "attempt": 2,
  "outcome": "failed",
  "error": {
    "code": "engine_unavailable",
    "retryable": true
  },
  "requestId": "0198c53b-5ad2-7de2-bc2b-c45ff0cd8c1e",
  "traceId": "71f3e3b9a6e14c5461419d674be1dbd5"
}
```

Transitions are written transactionally with operation state. They express requested, authorized, admitted, claimed, dispatched, per-target result, retry scheduled, compensation, uncertain, partial, succeeded, failed, canceled, and expired as applicable. Every targeted node is accounted for; an omitted or unreachable node is an explicit outcome. An `INFO` event may announce the transition, but losing that log cannot lose the transition.

Operation history uses the API's authorization, pagination, cursor, retention, and redaction contract. Initial online retention is at least 24 hours, with seven days the preferred release default from the API report; discovery reports the effective value. Related security audit records follow the longer security retention. Operation expiry removes user-facing workflow detail, not required audit evidence.

### Audit boundary

[Security](./security.md) remains authoritative:

- audit records authentication/security lifecycle, authorization, mutations, and content-free log/exec open/close metadata;
- high-risk mutation audit intent and operation admission are durable before Docker execution, and those actions fail closed when required persistence is unavailable;
- audit is append-only, hash-chained, periodically signed/checkpointed, and externally anchored/WORM-exportable for production;
- audit access/export has dedicated capability and is itself audited.

Initial retention remains 365 days for authentication, security policy, recovery, enrollment/key, mutation/operation, and exec metadata; audit reads/exports and successful resource reads use 90 days or the approved sampling policy; high-volume diagnostics remain a separate 30-day class. Operator/legal policy may increase these values, while reductions follow the audited cooling-period rules in the security report.

Application events may say that audit persistence is unavailable or an anchor is delayed, using a safe code. They do not duplicate the actor/action record, expose the hash-chain material, or satisfy audit retention. Conversely, audit does not ingest high-volume diagnostic retries, durations, queue depths, or stack traces.

### Diagnostic retention

Kontainer itself does not promise durable application-log or trace retention. The operator configures the platform. Recommended production policy is 30 days for sanitized application events, 7–14 days for sampled traces, and sufficient Prometheus retention to evaluate the longest SLO window, subject to organizational policy. A compatibility local-file sink may default to seven files but is not the production archive.

Retention and deletion are per plane. A support bundle copies a bounded recent slice after applying its own redaction; it never bypasses a plane's authorization or restores expired audit/operation data from telemetry.

## Export, buffering, and rotation

### Application events

JSON stdout/stderr is accepted. `tracing_subscriber`'s JSON formatter produces newline-delimited JSON intended for production log collection ([JSON formatter](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/format/struct.Json.html)). Human pretty output is allowed only for local development and tests.

The writer is asynchronous and bounded. `tracing-appender` provides a dedicated writer thread and a fixed-capacity non-blocking queue that can be lossy or apply backpressure; it also exposes an error counter and requires its worker guard to be retained/flushed ([non-blocking writer](https://docs.rs/tracing-appender/latest/tracing_appender/non_blocking/struct.NonBlocking.html), [implementation behavior](https://docs.rs/tracing-appender/latest/src/tracing_appender/non_blocking.rs.html)). Kontainer chooses lossy behavior for diagnostics so a blocked collector cannot stop the control plane:

- reserve capacity for `WARN`/`ERROR` and lifecycle edges before accepting `DEBUG`/`TRACE`;
- drop lower levels first; if all diagnostic queues are exhausted, aggregate a synchronous minimal drop counter rather than block;
- report queue items/bytes, dropped events by level/reason, encoder failures, writer errors, and last successful write;
- flush for a bounded interval during graceful shutdown, after durable audit/operation flush and before the runtime closes;
- never send audit records through this queue.

The concrete implementation may use an alternative writer if the selected crate cannot enforce item and byte limits, priority reserve, a write deadline, and a shutdown deadline. Dropping a worker guard must not hold the lifecycle owner past its telemetry flush budget. Crate defaults are not a production budget.

### Rotation

In OCI deployments, the application writes stdout/stderr only. Docker's configured logging driver, journald, or the host collector owns size/time rotation, compression, retention, disk quota, and forwarding. This avoids two independent filesystems and makes rotation observable at the platform layer.

For the deferred native-service/compatibility profile, journald is preferred. An optional local file sink is accepted only with a configured directory, restrictive permissions, time rotation, maximum retained files, and a hard aggregate byte quota enforced by the surrounding service/platform. `tracing-appender` supports time-based rolling and a maximum retained file count, but not by itself the full production disk budget ([rolling appender builder](https://docs.rs/tracing-appender/latest/tracing_appender/rolling/struct.Builder.html)). Synchronous duplicate writes to stdout and a local file are rejected.

### OpenTelemetry export

OTLP traces go to one local/nearby Collector endpoint using TLS and explicit authentication where the network is not loopback/private. The application uses a bounded batch processor and one destination. Multi-backend routing, retry persistence, vendor authentication, filtering, and tail sampling belong in the Collector. Direct export to every supported vendor is rejected.

The exporter has a timeout, exponential backoff with jitter for transient failures, a maximum retry age, and a hard item/byte queue. The OTLP specification requires retryable and non-retryable errors to be distinguished and recommends counters for dropped non-retryable data; the exporter specification requires exponential backoff with jitter for transient errors ([OTLP](https://opentelemetry.io/docs/specs/otlp/), [OTLP exporter](https://opentelemetry.io/docs/specs/otel/protocol/exporter/)). Kontainer additionally exposes drops, last success, and queue pressure locally through Prometheus and health status so the telemetry system can diagnose itself.

OTel Logs export is deferred through beta. JSON stdout already integrates reliably with container platforms, while adding a second log exporter introduces ordering, duplicate, queue, and maturity work without improving the security model. A GA implementation may add OTLP Logs as an optional sink only if it shares the exact safe event schema, does not double-buffer without an accounted budget, and passes loss/duplicate/shutdown tests.

Exporter failure is `telemetry_export=degraded`, never `/livez` failure and normally not `/readyz` failure. Failure of every local observability surface, including `/metrics` rendering or diagnostic writer initialization, is a startup/deep-status error but still cannot weaken audit enforcement.

### Runtime filters

The startup filter is strict configuration and defaults to `INFO`, with noisy library targets pinned higher. Filters are reloadable locally without restart, but:

- production changes require `diagnostics.configure`, an audit record, actor/reason, scope, and a maximum expiry of 30 minutes;
- scope is a compiled component/target and optionally one safe operation ID, never a user-provided regex over data;
- `TRACE` for high-volume stream/frame targets is not offered;
- expiration atomically restores the previous baseline even if the requester disconnects;
- active override and expiry appear in deep status and a metric;
- filters affect selection only, not fields or redaction.

The `tracing_subscriber` filtering model can filter per layer, which allows a more selective local JSON sink without accidentally changing trace sampling ([subscriber filters](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/index.html)).

## Sampling, cardinality, and resource budgets

### Trace sampling

Sampling decisions are made when a span is created; attributes and events on non-recording spans are not available later. OpenTelemetry's SDK defines parent-based and probability sampling but cannot retroactively recover an unsampled successful root just because it later failed ([Trace SDK sampling](https://opentelemetry.io/docs/specs/otel/trace/sdk/)). Therefore:

- always sample security-sensitive control operations, admitted mutations, operation workers, schedule/lease changes, readiness transitions, panics, and explicit incident probes, subject to an emergency hard cap;
- probability-sample successful read-only API roots, health excluded, initially at 1%;
- sample successful high-frequency reconcile/collection roots at an initially low fixed rate and always record aggregate metrics;
- respect a valid remote parent only within the configured trust/sampling ceiling—an external client cannot force unlimited recording with the sampled flag;
- use Collector tail sampling for error/latency enrichment only when its own memory/cardinality budget is deployed and tested;
- never rely on trace sampling to preserve errors: stable error counters and sanitized `WARN`/`ERROR` events remain unsampled, subject only to duplicate-rate aggregation and the global safety cap.

Repeated identical warnings use token-bucket rate limiting per static event/error code, with `suppressedCount` emitted in the next event and an unsuppressed metric counter. There is no per-resource limiter map.

### Starting budgets

These are implementation acceptance ceilings, not undocumented library defaults:

| Budget | Controller | Node agent | Exhaustion behavior |
|---|---:|---:|---|
| Diagnostic event queue | 8 MiB and 8,192 items | 4 MiB and 4,096 items | Drop `TRACE/DEBUG`, then sampled `INFO`; preserve bounded high-level reserve |
| OTLP trace queue | 16 MiB and 4,096 spans | 8 MiB and 2,048 spans | Drop oldest low-priority batches; increment local metric |
| Export batch | 1 MiB or 512 spans, 5 s max age | same | Export with deadline; retry within queue/age cap only |
| Metric families | at most 200 | at most 150 | Registration/CI fails beyond catalog |
| Active application series | at most 5,000 | at most 2,000 | CI cardinality test fails; runtime overflow uses aggregate series |
| Application event | 16 KiB | 16 KiB | Truncate optional fields or replace with rejection event |
| Trace attributes/events | 32 attributes, 32 events | same | Count dropped additions |
| Telemetry CPU | at most 2% of one core steady / 5% for a ten-minute incident burst | at most 1% steady / 3% burst | Reduce sampled diagnostics before product work |
| Telemetry steady RSS | at most 32 MiB | at most 16 MiB | Hard queues prevent growth; alert/degrade before cap |
| Shutdown telemetry flush | at most 2 s | at most 2 s | Drop remaining diagnostics; never overrun audit/drain deadline |

The CPU/RSS budgets include encoding, queues, trace SDK, metric registry, and exporter but exclude the operator's Collector. Steady ceilings are measured in every small/medium/large performance profile. A 10× diagnostic burst with the exporter disconnected or slow must remain within the burst CPU and RSS ceilings by shedding optional signals; it need not preserve the configured success-trace sample rate. If these ceilings are not achievable, reduce instrumentation/sampling; do not silently increase process hard memory.

Histograms use reviewed fixed buckets tied to an SLO or timeout. Route × method × status × outcome dimensions are multiplied in CI. Exemplars may carry trace IDs only in backends that keep them out of ordinary metric labels and honor the same access boundary.

## Panic, crash, and fatal-error handling

### Panic policy

Install one process-global panic hook at the earliest safe point. Rust invokes the hook before either unwind or abort, and exposes the panic payload and source location ([`std::panic::set_hook`](https://doc.rust-lang.org/std/panic/fn.set_hook.html)). Because that payload may contain secrets or arbitrary upstream text, the hook records only:

- static event `runtime.panic`;
- component/version/commit/boot ID;
- boundary/task-kind and current trace/request/operation IDs if already safe and available;
- source module plus line/column only for Kontainer-owned compiled source, otherwise crate/module classification;
- thread class, not arbitrary thread name;
- a generated crash ID and action `request_failed|task_restarted|process_exit`.

It never formats the panic payload. The hook performs no async work, lock-heavy export, allocation-dependent bundle, Docker call, database call, or network request. It writes a small best-effort crash marker through a pre-established restricted path/handle and a minimal stderr record, then lets the boundary policy proceed.

Backtraces are disabled by default in routine production logs. Rust notes that capture can be memory-intensive and slow ([`std::backtrace::Backtrace`](https://doc.rust-lang.org/std/backtrace/struct.Backtrace.html)). A sanitized symbolic backtrace may be enabled for a bounded incident window, stored only in protected local diagnostics, and excludes variable values. Release symbols/build IDs are published separately so addresses can be symbolicated without shipping source/full debug data in the runtime image.

There is no embedded third-party crash-reporting SDK or automatic crash upload in the production baseline. Sanitized panic/crash events may reach the operator's existing log/OTLP pipeline under its access policy; a new SaaS destination would require an explicit data-processing, credential, egress, queue, and redaction decision.

### Boundaries and exit

- Expected dependency and validation failures are `Result`, never panic control flow.
- An HTTP request or noncritical isolated task panic may be caught at the framework/supervisor boundary under unwind builds, returned as the standard internal problem with `requestId`, counted, and followed by resource cleanup.
- A critical supervisor, lifecycle owner, lease owner, audit writer, operation writer, or runtime-driver panic marks readiness false, attempts the bounded durable flush/drain that remains safe, and exits nonzero for orchestrator replacement.
- Repeated noncritical task panics trip a restart-rate circuit and escalate to process exit rather than loop forever.
- OOM, allocator abort, stack overflow, SIGKILL, or undefined behavior cannot be made recoverable by the hook. Startup writes a tiny boot-in-progress marker; clean shutdown replaces it with the terminal marker. The next boot detects a prior in-progress marker without a matching clean shutdown, enriches it with a panic crash marker if one exists, emits `previous_process_unclean`, and increments restart/crash signals without claiming a cause it cannot prove.

`panic=abort` is deferred. Rust's abort mode skips stack unwinding/cleanup ([Rust panic behavior](https://doc.rust-lang.org/book/ch09-01-unrecoverable-errors-with-panic.html)); Kontainer must first prove the consequences for audit checkpoints, operation uncertainty, stream termination, temporary bundles, and profile capture. Core dumps remain disabled by default because they can contain credentials, workload content, and decrypted control data. An operator enabling host-level cores owns protected storage, encryption, access, and deletion; cores are never included in a support bundle.

## CPU, heap, and task inspection

There is no public `/debug/pprof`, heap dump, task dump, or remote shell. Profiling is an operator-local, time-bounded, audited action with a single concurrent session, explicit disk/CPU limit, protected output, and automatic cleanup. A profile contains stack/control-flow information and is Restricted even when no payload is intended.

| Option | Decision | Rationale and constraints |
|---|---|---|
| Linux `perf`/eBPF sampling against the normal process | **Accepted GA baseline** | No permanently listening application endpoint or in-process signal profiler. Publish build IDs and separate line-table symbols. Operator runs a bounded host-local command under existing host privilege; profiles are not auto-uploaded. Rust's Performance Book lists `perf`, flamegraph, samply, heaptrack, and other proven external options and recommends release debug line tables for useful profiles ([Rust profiling guidance](https://nnethercote.github.io/perf-book/profiling.html)). |
| Stable Tokio `RuntimeMetrics` plus application task supervisors/sentinels | **Accepted beta/GA** | Always-on low-cost aggregate diagnosis without task stack contents; qualify every enabled metric. |
| `pprof-rs` in-process CPU samples | **Deferred experimental build** | Convenient protobuf/flamegraph output, but it uses `SIGPROF`; its own documentation warns that libgcc unwind from a signal handler is not signal-safe and discusses deadlock/safety constraints ([pprof-rs](https://docs.rs/crate/pprof/latest)). It must not become an unauthenticated HTTP route or default dependency. |
| `tokio-console` | **Accepted development/staging only; rejected in production image** | It requires experimental Tokio tracing via `tokio_unstable` and `TRACE` runtime targets and serves a console endpoint ([console-subscriber](https://docs.rs/console-subscriber/latest/console_subscriber/)). That broad, high-volume task instrumentation does not fit the production budget/trust surface. |
| Tokio `Handle::dump` | **Deferred diagnostic build** | It requires `tokio_unstable`, Linux and selected architectures, pauses/re-polls tasks, is expensive, and can fail to terminate when a worker is blocked; Tokio recommends an explicit timeout ([Tokio task dump](https://docs.rs/tokio/latest/tokio/runtime/struct.Handle.html#method.dump)). If later accepted, it is local-only, two-second timeout, one dump, restricted artifact, and never automatic during shutdown. |
| RSS, allocator-independent process metrics, and application cache byte accounting | **Accepted GA baseline** | Safe always-on memory symptom/saturation signals; sufficient for initial detection, not allocation attribution. |
| jemalloc stats/heap profiling feature | **Deferred production experiment** | Changes the global allocator and build/platform behavior. The Rust wrapper exposes jemalloc control/introspection and optional profiling/stats features ([tikv-jemallocator](https://github.com/tikv/jemallocator)); compare memory/latency before selecting it. Heap dumps are restricted local artifacts and may expose allocation structure. |
| `dhat-rs` allocator | **Accepted CI/performance-lab only** | Useful for allocation counts/high-water tests, but its allocator is slower and the docs warn runtime slowdown can be large ([dhat-rs](https://docs.rs/dhat/latest/dhat/)). It does not ship in normal images. |
| Raw process memory dump/core in app or bundle | **Rejected** | It defeats the secret boundary and has no safe automated redaction. |

The local support CLI should initially print exact safe commands for `perf` attachment and how to match the release build ID to symbols. If later profile orchestration is implemented, `kontainer diagnostics profile cpu --duration <=30s --output <restricted-dir>` is local-only, rejects insufficient disk/duplicate sessions, and produces a manifest/checksum. It never accepts an arbitrary output path outside the diagnostics directory.

## Support bundles

[Agent lifecycle](./agent-lifecycle.md) defines the packaging contract; this section makes the observability content and workflow explicit.

### Workflow

1. `kontainer diagnostics bundle preview` performs no collection and reports planned sections, node scope, redaction classes, estimated upper size, and unavailable sections.
2. The operator selects local controller or local node-agent scope. Cluster fan-out is a later authorized operation with explicit target and partial-result accounting.
3. Generation reserves disk and a single bundle slot, uses a restricted temporary directory, applies per-section deadlines and byte limits while collecting, and rejects symlink/path traversal.
4. The result is a deterministic compressed archive containing `manifest.json`, per-file classification/source/time/truncation/error, SHA-256 checksums, product/build/schema/protocol versions, and redaction policy version.
5. Temporary material is removed on success, failure, cancellation, and next startup cleanup. The archive is mode `0600` or stricter and expires locally by policy.
6. Nothing uploads automatically. Optional encryption to a configured support public key occurs locally before the operator moves the archive.

Default ceilings are 100 MiB compressed, five minutes, one concurrent bundle, and 100 explicitly accounted nodes for beta cluster fan-out. A smaller free-space reserve can reduce/deny generation before it threatens the controller database or audit partition.

### Included by allowlist

- build/release/protocol/schema/feature manifest and boot/lifecycle timeline;
- redacted effective configuration showing source and whether a secret is set, never its value;
- public and deep dependency/capability status snapshots with safe error codes/timestamps;
- Engine API version/capability and safe aggregate inventory counts, not object specs;
- database reachability, pool/latency, migration/checkpoint/WAL/file-size metadata, not rows, SQL, URLs, or database files;
- pseudonymized agent membership state, heartbeat/freshness distributions, certificate purpose/issuer ID/expiry/fingerprint suffix, not certificate/private-key bytes;
- bounded recent application events re-parsed through the current bundle schema; malformed/unclassified lines are omitted, not copied raw;
- bounded internal Prometheus snapshot with forbidden-label validation;
- queue/task/permit counts, process CPU/RSS/FD/disk, scheduler last-run/lease state, operation aggregate counts, and selected redacted recent operation transitions;
- service/container deployment spec for Kontainer's own tasks after allowlist redaction;
- optional, separately consented external CPU profile; never automatic and never a heap/core dump.

### Excluded even on debug

- environment dumps, command lines with values, config/secret contents, provider/registry credentials, JWT/enrollment/setup/recovery material, database URLs, keys, certificates containing private material;
- container/service environment or full Docker specs, customer labels/names unless explicitly safe and pseudonymized;
- workload logs and all exec/terminal content;
- request/response bodies, raw upstream errors, SQL text/binds/rows, database files;
- process memory, core dumps, heap object contents, raw task locals, unrestricted stack traces;
- arbitrary filesystem paths, hostnames, IPs, peer addresses, user IDs, email, and URL query strings.

Bundle generation and download require `diagnostics.bundle`, are audited, rate limited, and use one-time/short-lived retrieval where remote API orchestration exists. Preview, generation, download, expiry, and deletion are distinct audit events. A bundle failure must not make readiness fail.

## Dashboards, SLOs, alerts, and runbooks

### Shipped dashboards

At beta, release artifacts include dashboard/recording-rule source for:

1. **Fleet overview:** ready/startup/degraded instances, version/protocol skew, agent state, certificate minima, scrape/export gaps.
2. **API and operations:** traffic/admission, latency/error SLO, operations by state/outcome/age, target completeness, idempotency conflicts.
3. **Docker and fan-out:** manager/local calls, latency/error, permits, reconciles, snapshot age/completeness, registry heartbeat.
4. **Database and scheduler:** pool/wait/transactions, write errors, disk/WAL/checkpoints, leases, last success, duplicate/uncertain claims.
5. **Runtime and capacity:** CPU/RSS/FD/tasks, Tokio busy/queue/sentinel delay, blocking permits, every bounded queue in items and bytes, stream/session limits.
6. **Telemetry health:** scrape success, event/trace drops, exporter queue/retries/last success, filter overrides, bundle attempts.

Every alert links to a versioned runbook ID and dashboard panel. Prometheus recommends alerting on symptoms/user pain, keeping alerts actionable, and using lower-severity notifications for predicted resource exhaustion rather than paging on every possible cause ([alerting practices](https://prometheus.io/docs/practices/alerting/)). Kontainer pages on SLO exhaustion or integrity/safety threats; dependency and capacity alerts normally warn/ticket unless they cause that symptom.

### SLO alignment

The [performance](./performance.md) targets are authoritative and become recording rules:

- cached reads successful/valid at least 99.9%; admitted mutation API at least 99.5%;
- list p95 at most 250 ms medium/500 ms large and p99 at most 2 s;
- resource-event visibility p95 at most 2 s with reconciliation correction at most 60 s;
- task snapshot freshness p99 at most 15 s;
- workload metric freshness p99 at most 45 s and five-minute sample coverage at least 99%;
- node collection cycle p99 at most 25 s;
- controller failover p95 at most 10 s;
- scheduled occurrence takeover p95 at most 30 s/p99 at most 60 s with zero duplicate claims;
- fan-out target accounting exactly 100%; zero OOM and zero unbounded queue assertions.

Availability calculations exclude valid user errors such as authentication denial, validation failure, and conflict, but include internal, dependency, timeout, overload, and false-success failures according to the API taxonomy. A request rejected by intentional capacity protection remains visible and counts against the appropriate admission SLO when the service promised that workload envelope. Scheduled/reconcile SLOs use expected occurrences/targets, not only observed successes.

### Alert policy

Initial multi-window burn-rate alerts are implementation hypotheses to validate with production traffic:

- page when an availability/freshness SLO burns at least 14.4× over both 5 minutes and 1 hour, or at least 6× over both 30 minutes and 6 hours;
- create a high-priority ticket at at least 3× over 24 hours;
- page immediately for audit/operation corruption, duplicate schedule claim/effect, controller quorum loss, authentication bypass indication, certificate expiry threatening all control traffic, OOM/restart loop, or impossible/unbounded resource assertion;
- warn at a sustained 70% of a hard queue/FD/task/memory/disk budget for ten minutes and page at 90% or active shedding above 5%;
- warn on any telemetry drop in steady state and page only if all independent diagnostic signals are absent during a concurrent service symptom;
- warn when exporter last-success age exceeds five minutes, but do not page if local metrics/logs and product service remain healthy;
- use absent-series rules for expected controller/agent scrape targets, with deployment/update inhibition to prevent noise.

Alerts carry component, environment, bounded reason, SLO window, current/threshold value, and runbook URL. They do not carry raw resource/user IDs. Rules are unit-tested for firing, inhibition, and clearing; game days confirm notification delivery.

### Minimum runbooks

Runbooks are versioned with releases and contain safe commands, expected outputs, escalation thresholds, evidence preservation, remediation, and verification:

- `OBS-001` controller unready/startup failure;
- `OBS-002` node-agent degraded, suspect, or quarantined;
- `OBS-003` Docker manager/local Engine latency or exhaustion;
- `OBS-004` control database unavailable/slow/schema incompatible;
- `OBS-005` operation stalled, partial, or uncertain;
- `OBS-006` schedule lease loss, takeover, or duplicate effect;
- `OBS-007` resource/metric freshness and coverage breach;
- `OBS-008` runtime CPU/RSS/FD/Tokio/blocking saturation;
- `OBS-009` telemetry exporter/log queue/cardinality failure;
- `OBS-010` audit chain/checkpoint/anchor failure;
- `OBS-011` certificate expiry/rotation failure;
- `OBS-012` panic, crash loop, OOM, and safe profiling;
- `OBS-013` support bundle generation/redaction failure;
- `OBS-014` runaway log/exec sessions without inspecting their content.

## Incident workflow

1. **Detect and scope.** Page from a user symptom or integrity alert. Record incident ID, component/version/deployment, start time, affected capabilities, and relevant request/operation IDs. Do not paste workload output or secrets into the incident channel.
2. **Confirm admission state.** Check `/readyz`, authenticated system status, fleet/SLO dashboard, and operation state. Distinguish one instance from shared database, manager quorum, or fleet-wide failure before restarting anything.
3. **Preserve safe evidence.** Snapshot dashboards/alerts, audit checkpoint status, relevant durable operation history, and a local support-bundle preview. Generate the bundle before restart only when it will not worsen pressure; never collect a core or raw database by habit.
4. **Control blast radius.** Stop or shed the affected traffic class, mark the instance unready/draining, pause schedules through their lease-aware control, or quarantine an agent using existing authorized operations. Do not globally enable debug or repeatedly retry uncertain mutations.
5. **Deepen selectively.** Apply a time-limited component filter, follow a known request/operation trace, or use an operator-local bounded CPU profile. Audit the diagnostic change and monitor telemetry overhead/drop signals.
6. **Remediate through product/lifecycle controls.** Fail over, roll back a signed image, restore a dependency, rotate identity, or reconcile an uncertain operation according to its runbook. A restart is not evidence that a partially applied mutation is safe to replay.
7. **Verify.** Confirm readiness/capability recovery, SLO burn stops, queues drain, snapshots/metrics become fresh, leases have one owner, operation targets are fully accounted, alerts clear, and no new audit/telemetry gap appears.
8. **Close and learn.** Revoke temporary filters/access, delete or transfer bundles under policy, anchor audit evidence, record the stable root cause and missing signal, and add a regression/fault test plus runbook/dashboard change before closing.

Incident IDs may be added locally to audit/operation annotations through a validated bounded field. They are not accepted from public request baggage and are not metric labels.

## Architecture options and decisions

| Route | Decision | Reason |
|---|---|---|
| Typed `tracing` spans/events with explicit fields | **Accepted** | One correlation model across async Rust and multiple sinks; supports structured layers without string parsing. |
| Free-form logger calls with post-hoc regex redaction | **Rejected** | Cannot reliably classify nested errors, `Debug` output, bodies, or new secrets. |
| JSON stdout/stderr, platform collection/rotation | **Accepted default** | Matches OCI lifecycle, avoids mutable image storage, and delegates durable log operations to the operator platform. |
| Daily application file as primary log | **Rejected production; compatibility only** | Weak byte/disk control, difficult forwarding, and inconsistent container behavior. |
| Prometheus pull on private diagnostics listener | **Accepted beta default** | Independent local symptom path, low operational coupling, and established alert/query model. |
| Store Kontainer's process metrics in workload `metrics.sqlite3` | **Rejected** | Creates self-dependency and mixes product data authorization/retention with internal operation. |
| OTLP traces through an operator Collector | **Accepted optional beta, GA-qualified** | Vendor-neutral correlation and central policy with bounded process-side queue. |
| Direct in-process exporter for every vendor/multiple destinations | **Rejected** | Multiplies credentials, queues, retries, failure modes, and dependencies in privileged processes. |
| OTLP metrics | **Deferred optional equivalence path** | Useful for some operators, but must reuse one schema and must not remove the independent scrape path prematurely. |
| OTLP Logs | **Deferred to GA consideration** | Stdout is sufficient for beta; avoid duplicate queues/order semantics until a demonstrated need. |
| Client W3C `traceparent`/`tracestate` | **Accepted after validation and local sampling cap** | Standard cross-process correlation without accepting arbitrary fields. |
| Client W3C baggage | **Rejected** | Arbitrary propagated data is unnecessary and creates confidentiality/cardinality risk. |
| Canonical `/livez`, `/startupz`, `/readyz` | **Accepted** | Matches lifecycle semantics; API spellings may be exact temporary aliases only. |
| Deep unauthenticated health | **Rejected** | Leaks topology/dependency/security state. |
| Public `/debug/pprof`, task dump, heap dump | **Rejected** | Expensive, abusable, and sensitive on a Docker-privileged service. |
| External host-local profiling plus separate symbols | **Accepted GA** | Bounded operator action without permanent public runtime surface. |
| In-process pprof/Tokio task dump/jemalloc profile | **Deferred experiments** | Valuable in specific incidents but requires unstable/signal/allocator/security qualification. |
| Automatic support-bundle upload | **Rejected** | Customer/operator must preview, consent, and control transfer. |
| Embedded third-party crash reporter | **Rejected baseline; separate integration decision** | Adds privileged-process credentials/egress and risks uploading panic payload, stack, or local context outside the operator's telemetry boundary. |
| Local support bundle | **Accepted parity/beta** | Provides bounded evidence even when cluster/database/exporters are impaired. |
| Controller-orchestrated multi-node bundle | **Deferred to beta completion/GA** | Requires durable operation, authorization, fan-out limits, pseudonymization, and honest omissions. |
| Core dumps by default | **Rejected** | Memory contents violate the secret/workload boundary. |

## Reconciliation with other v2 reports

### Security

Audit remains durable and tamper-evident; diagnostics remain bounded/best effort. The forbidden-data list, pseudonymization, public/deep health split, setup/enrollment protection, RBAC, fail-closed mutation audit intent, and no-core default are inherited from [security](./security.md). Observability adds safe typed implementation mechanics but does not weaken them. Audit query/export and diagnostic bundle/filter permissions are separate.

### Performance and resource protection

Instrumentation observes every semaphore, queue, cache, fan-out, Engine class, runtime task class, and shedding decision required by [performance](./performance.md), but never emits per-resource metric series or per-frame spans. It has independent CPU/RSS/queue budgets and degrades before product work. The performance report's SLOs and hard resource thresholds are the alert source of truth.

### API and errors

[API protocol](./api-protocol.md) owns `X-Request-Id`, W3C trace acceptance, safe problem details, status/error mappings, operation admission, pagination, SSE, and WebSocket contracts. This report defines their telemetry. Route templates, not raw URLs, are recorded. The lifecycle probe paths take precedence over draft API spelling; aliases cannot diverge.

### Metrics storage

Workload node/container metrics, rollups, gaps, and history remain in the product data plane defined by [metrics storage](./metrics-storage.md). Kontainer's own `kontainer_*`, process, and Tokio metrics are scrape/export signals and are never persisted there. Both planes expose freshness/coverage truth but have separate schemas and access.

### Cluster communication

[Cluster communication](./cluster-communication.md) owns mTLS identity, registration streams, membership states, deadlines/retries, capabilities, protocol skew, fan-out, and leases. Observability propagates validated trace context over Tonic, records both client/server sides, and reports aggregate registry/cert/capability state without peer addresses or node labels. Health consumes the registration/reconcile state; it never adds active N-node polling.

Its metric block is an illustrative requirements list; the canonical registry above normalizes `rpc_*` to `grpc_*`, `agent_registry` to `agent_registry_entries`, gauges/counters to correct suffixes such as `fanout_targets_total`, and certificate remaining-age to `certificate_not_after_timestamp_seconds`. This is one instrument rename/mapping, not two simultaneously exported compatibility families. Recording rules may provide temporary dashboard aliases outside the process.

### Lifecycle

[Agent lifecycle](./agent-lifecycle.md) owns startup/readiness/drain state, process signals, build metadata, support bundle packaging, installation, upgrade, rollback, and DR. Observability supplies the state views and evidence. Shutdown order remains: become unready; stop admission/schedule ownership; drain and terminate streams honestly; flush audit/control/operation state; flush bounded diagnostics; close database/gRPC/Docker/exporters/task tracker; write clean terminal marker.

Its illustrative `kontainer_process_info` is canonicalized as `kontainer_build_info`; `kontainer_readiness{dependency}` becomes the explicit one-hot `kontainer_dependency_state` plus `kontainer_capability_state`; and backup/restore "age" gauges become last-success timestamps so query time computes age even if the updater stalls. Exact peer versions remain in deep status/build info, while metrics expose bounded compatibility results. These decisions follow the Prometheus timestamp and cardinality rules cited above.

### Logging and exec

[Logging](./logging.md) and [exec](./exec.md) own user workload transport. Their payloads never cross into this system. Observability records only authenticated session lifecycle, byte/item counters, queue pressure, gaps, duration, target kind, and safe close/error code. Support bundles never include session content or command/shell text.

### Stack deployment and Swarm resources

[Stack deployment](./stack-deployment.md) and [Swarm resources](./swarm-resources.md) own desired/observed revision, authorization, target resolution, confirmation, idempotency, per-target outcomes, reconciliation, and rollback/compensation semantics. Observability records their bounded operation kind, counts, timing, state, and stable outcome; it does not serialize stack files, service specs, config/secret content, environment, registry auth, labels, names, or raw Engine representations. Durable operation events—not traces—remain the convergence and partial-success truth.

### Reliability testing

[Reliability testing](./reliability-testing.md) owns release gates and game days. Observability tests use the exact production registry/export/status path, not test-only counters. Alerts must demonstrably fire and clear. Secret canaries, exporter loss, corrupt events, slow scrape, bundle interruption, task panic, clock skew, lease loss, and cardinality attacks join the fault matrix.

## Delivery scope

### Parity gate

- structured safe JSON events on stdout/stderr for both binaries, with lifecycle, request summary, Docker/database safe errors, and content-free log/exec session boundaries;
- server request IDs and propagation into errors, controller-agent calls, and initial operation records;
- typed safe error taxonomy; no raw `err` serialization; remove setup-token logging;
- `/livez`, `/startupz`, `/readyz` with truthful local/startup/core readiness and v1 compatibility aliases where required;
- authenticated local deep status for controller and agent dependency/capability state;
- private `/metrics` with standard process, API, Engine, database, queue, schedule, stream, lifecycle, panic, and telemetry-self metrics;
- scheduler last run/success/failure/skipped/duration visibility even before HA leasing;
- panic hook, clean/unclean shutdown marker, bounded diagnostic flush, core dumps disabled;
- local support-bundle preview/generation with allowlist, limits, pseudonymization, secret-canary tests, and no upload;
- platform stdout rotation documentation and an optional bounded compatibility file profile;
- no workload logs, exec contents, environment, SQL, secrets, raw bodies/errors, or arbitrary paths in any artifact.

### Beta gate

- W3C trace context across HTTP, Tonic gRPC, Docker, database, operation workers, scheduler, and fan-out; asynchronous operations use span links;
- bounded OTLP trace export to a Collector, sampling policy, exporter self-observation, and disconnected/slow-collector fault tests;
- production Prometheus recording rules, six dashboards, actionable alerts, and versioned `OBS-*` runbooks;
- durable operation transition history with exact target accounting, audit references, and authorized API/SSE correlation;
- multi-controller readiness, lease/claim and failover signals, registry/certificate/capability metrics;
- stable Tokio runtime/application task metrics and scheduler-delay sentinel qualified against the telemetry resource budget;
- audited, time-limited runtime filters with expiry;
- local bundle behavior fault-tested for full disk, interruption, symlink/path traversal, canary secrets, malformed logs, and process crash;
- external Linux CPU profiling runbook, build-ID/symbol publication, and staging proof on amd64/arm64;
- SLO calculations proven under small/medium/large load profiles, including dependency failures in denominators.

### GA gate

- observability overhead, queue/cardinality ceilings, and long-run leak behavior pass every supported profile with exporters healthy, slow, and absent;
- production backend interoperability and semantic-convention/schema upgrade process are validated; dashboards/rules stay compatible across one release rollback;
- alert delivery, inhibition, and clearing are game-day proven; on-call can diagnose DB, manager, agent, lease, freshness, exporter, panic/OOM, and runaway-stream failures from shipped signals/runbooks;
- support bundles are judged useful by a blind support exercise and contain zero canary/prohibited content; cluster fan-out is enabled only if authorization/partial-result/limits pass, otherwise remains deferred;
- safe `perf`/eBPF production procedure and symbol retention are release policy; any in-process CPU/heap/task profiler remains feature-gated until separately approved;
- audit, operation history, traces, diagnostic logs, and metrics can be cross-correlated without treating any as a substitute for another;
- retention/deletion, incident evidence, support transfer, and diagnostic-access policies are documented for production operators.

## Acceptance checklist

### Schema and confidentiality

- [ ] Every event, span, metric, status field, operation event, and bundle section is in a versioned catalog with owner, type, class, bound, and retention/sink.
- [ ] Production instrumentation uses `skip_all` plus explicit fields; CI rejects unreviewed `#[instrument]`, raw `Debug`/`Display` errors, and dynamic event field names.
- [ ] Canary values placed in setup/enrollment/JWT/OAuth/registry/database credentials, Docker secrets/configs/environment, request bodies, workload logs, exec I/O, URLs, and errors occur zero times in logs, traces, metrics, panic/crash records, and bundles.
- [ ] Public probes disclose only component/version/protocol/boot suffix/coarse code; authenticated status contains no raw address/path/provider/certificate/error.
- [ ] Metric label enumeration proves family and process series ceilings at the large profile; fuzzed paths/IDs/errors do not create new labels.
- [ ] Control characters, invalid UTF-8, oversized errors/IDs, and malformed upstream data are bounded before serialization.

### Correlation and truth

- [ ] One test follows an HTTP mutation through auth, operation admission, database, gRPC, agent, Docker, per-target transition, response/SSE, diagnostic event, trace, and audit using the documented IDs.
- [ ] Invalid/oversized trace context starts a safe new root; inbound baggage never propagates.
- [ ] An asynchronous operation survives controller restart and links its new execution trace to admission without requiring the old span/exporter.
- [ ] Every fan-out target has a terminal/uncertain operation outcome; metrics totals equal durable target accounting.
- [ ] Losing diagnostic events/traces changes neither operation nor audit truth. Losing required audit persistence prevents the defined high-risk action.

### Health and failure behavior

- [ ] Live, startup, ready, degraded, draining, bootstrap, quarantined, and failed transitions are table-tested for both binaries.
- [ ] Database/Engine/certificate/schema failures affect exactly the documented readiness/capabilities; Collector/Prometheus/remote-agent failure never causes restart-loop liveness.
- [ ] Probe handlers perform no remote fleet poll, Docker list, migration, or unbounded allocation and respond within their local deadline under saturation.
- [ ] Slow/absent log writer, Collector, and scraper create visible drops/degraded export while admitted control work and audit reserves remain available.
- [ ] Panic tests cover request, noncritical task, critical supervisor, repeated panic, and next-boot unclean marker; no panic payload leaks.

### Resource and operations

- [ ] Healthy, slow, disconnected, and rejecting exporters pass the item/byte/CPU/RSS budgets at 10× expected telemetry rate.
- [ ] All queues export items, bytes, capacity, drops, and oldest age where useful; no queue is unbounded.
- [ ] Metric scrape, JSON encode, filter reload, trace sampling, exporter flush, and bundle generation have benchmarks and fault tests.
- [ ] Dashboards and recording/alert rules are generated/tested from the metric registry; absent-series and counter-reset cases are covered.
- [ ] Alerts link to versioned runbooks, fire and clear in a game day, and do not include restricted identifiers.
- [ ] A blind incident drill locates a stalled/uncertain operation, failed dependency, hot runtime, and telemetry outage without reading workload output.
- [ ] Bundle preview is accurate; generated archive respects time/byte/node/disk limits, records truncation/omission, cleans temp files, and never uploads automatically.
- [ ] Release symbols/build IDs can symbolize a bounded external profile for both supported Linux architectures without adding full debug/source material to runtime images.

## Issue-ready decisions

The following can be copied into implementation issues. Dependency selection/change still requires the repository's normal approval.

1. **OBS-CORE-001 — typed telemetry crate.** Create shared event/span field catalog, safe wrappers, error taxonomy integration, `skip_all` lint/test, JSON layer, resource identity, and golden schema tests. Acceptance: confidentiality/schema checklist passes.
2. **OBS-CTX-002 — request and trace context.** Add UUIDv7 server request IDs, validated W3C Trace Context, drop baggage, Tonic propagation, task instrumentation helpers, and async operation links. Acceptance: cross-layer correlation test passes.
3. **OBS-MET-003 — internal registry and private scrape.** Implement the approved families, standard process metrics, series enumerator, `/metrics` diagnostics listener, scrape auth/network policy, and product-metrics separation. Acceptance: large-profile series ceilings and malformed-label tests pass.
4. **OBS-HLT-004 — lifecycle health registry.** Implement one state owner serving `/livez`, `/startupz`, `/readyz`, compatibility aliases, gRPC health, and authenticated dependency/capability status. Acceptance: failure matrix proves no restart storm or false readiness.
5. **OBS-OP-005 — durable operation events.** Persist ordered transitions, request/trace/audit references, retry/uncertain/partial states, per-target accounting, retention, API pagination, and SSE. Acceptance: crash/failover and exact-accounting tests pass.
6. **OBS-EXP-006 — bounded sinks and OTLP.** Add item+byte queues, priority/drop policy, self-metrics, bounded shutdown, Collector OTLP traces, TLS/auth, retry-age cap, and exporter fault tests. Keep OTLP Logs out of scope. Acceptance: telemetry budgets pass with exporter disconnected.
7. **OBS-TOK-007 — runtime and task supervision metrics.** Export stable Tokio/process/application task/blocking metrics and scheduling sentinel; qualify worker-index series and overhead. Acceptance: runtime SLO alerts detect injected starvation without unstable Tokio.
8. **OBS-CRASH-008 — safe panic/crash handling.** Install payload-free hook, boundary classification, critical exit policy, restart circuit, safe crash/clean marker, symbol/build-ID release artifacts, and disabled-core policy. Acceptance: panic matrix and secret canaries pass.
9. **OBS-BUNDLE-009 — local support bundle.** Build preview, deterministic manifest/checksum archive, allowlisted collectors, pseudonymization, quotas, protected temp/output, cleanup, audit, and optional local public-key encryption. Acceptance: bundle fault/security suite passes.
10. **OBS-DASH-010 — SLO operations pack.** Ship registry-derived recording rules, dashboards, alert rules/unit tests, and `OBS-001` through `OBS-014` runbooks. Acceptance: beta game day fires/clears each critical path against production signals.
11. **OBS-FILTER-011 — temporary diagnostic controls.** Add locally/auditably scoped filters with permission, reason, maximum 30-minute expiry, visible status, and automatic rollback. Acceptance: crash/restart/disconnect cannot leave a permanent override.
12. **OBS-PROFILE-012 — external profiling support.** Publish separate symbols/build IDs and safe `perf`/eBPF runbook/CLI hints for amd64/arm64. Benchmark frame-pointer/line-table choices before changing release profile. Keep pprof-rs, task dumps, tokio-console, and jemalloc out of production baseline.
13. **OBS-GATE-013 — release observability qualification.** Add schema diff, prohibited-field scan, series calculator, exporter chaos, overhead/leak, probe, operation/audit independence, alert, bundle, and blind-incident gates to [reliability testing](./reliability-testing.md).
14. **OBS-DEFER-014 — post-beta experiments.** Separately evaluate OTLP metrics/logs, Collector tail sampling, cluster bundle fan-out, in-process CPU profile, Tokio dump, and allocator profiling. Each proposal must include threat model, platform matrix, measured overhead, artifact classification, access/cleanup, and removal/rollback path.

## Definition of done

Observability is complete for GA only when an on-call operator can move from an alert or user-supplied request/operation ID to the failing capability and safe causal evidence; can distinguish local, dependency, saturation, stale-data, partial-operation, audit, and exporter failures; can collect a useful bounded bundle or external profile without workload content; and can remediate and verify recovery using shipped runbooks. The same release must demonstrate that hostile inputs, maximum supported cardinality, unavailable exporters, panics, and support tooling cannot leak prohibited data, exhaust the process, weaken audit/operation guarantees, or cause false liveness/readiness.

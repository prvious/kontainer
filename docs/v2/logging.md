# Logging in the Kontainer v2 Rust Agent

Status: research and implementation design  
Last verified: 2026-08-19  
Scope: Docker Swarm service, task, stack, and multi-service log streaming

## Executive decision

Build the first Rust implementation on Docker's native manager-side service and task log endpoints through Bollard. Normalize each Docker record into an internal `LogRecord`, put bounded queues between every asynchronous stage, and expose the existing WebSocket routes and JSON shape as a compatibility adapter.

The production path should be:

```text
React/Tauri client
    -> authenticated Kontainer WebSocket
    -> session supervisor and bounded writer queue
    -> optional fair fan-in (one source per selected service)
    -> normalizer (timestamp/details/UTF-8/line fragments)
    -> Bollard service_logs or task_logs stream
    -> Docker Engine on a Swarm manager
    -> Swarm log broker
    -> node-local task logger
```

This is the closest match to v1 and lets Docker perform service-to-task fan-out across nodes. Kontainer must not read Docker log files directly and does not need to proxy task logs through follower agents in the normal case.

The first release should promise **best-effort, at-least-once delivery during reconnect**, not exact resume. Docker exposes timestamps but no durable log record ID, and the public service/task endpoints have `since` but no `until` or resume cursor. Reconnect must deliberately overlap and may repeat records. The protocol and UI must never imply lossless delivery.

Strict v1 parity and v2 enhancements are separated below. The compatibility surface can ship before stack fan-in, resumable sessions, metadata filters, and the unified protocol.

## Source of truth and version caveat

Docker's [Engine API v1.52 reference](https://docs.docker.com/reference/api/engine/version/v1.52/) and Moby source are the protocol authority. The CLI documentation is useful but is not always synchronized with the Engine API: for example, the current [service logs CLI page](https://docs.docker.com/reference/cli/docker/service/logs/) names `json-file` and `journald`, while the current Engine schema also names `local`. Tests must therefore exercise the actual minimum and maximum supported daemon versions.

Bollard's current documentation describes an asynchronous Docker client with Unix socket, Windows named-pipe, HTTP, HTTPS, SSH, and API-version negotiation support; it generates models from a recent Moby schema ([Bollard repository](https://github.com/fussybeaver/bollard), [Bollard API documentation](https://docs.rs/bollard/latest/bollard/)). Pin an exact reviewed release rather than using `*`, and re-run the conformance matrix on upgrades. The registry release observed during this research was `0.21.1`; the documented generated Engine schema and the Moby tip can advance independently.

## v1 behavior found in this repository

### Backend

The current routes are:

- `GET /api/services/{serviceID}/logs`
- `GET /api/tasks/{taskID}/logs`

They are WebSocket upgrades outside the normal JWT middleware. The controllers validate the full JWT from `?token=...`; the upgrader accepts every origin. See [`service_logs_controller.go`](../../server/app/http/controllers/websocket/service_logs_controller.go), [`task_logs_controller.go`](../../server/app/http/controllers/websocket/task_logs_controller.go), [`auth.go`](../../server/app/http/controllers/websocket/auth.go), and [`upgrader.go`](../../server/app/services/websocket/upgrader.go).

Both handlers force `follow=true`, default `tail` to `100`, allow `tail=all`, clamp a numeric tail to `1..10000`, and default timestamps on. They use `context.Background()` after upgrade and rely on a WebSocket read goroutine closing the Docker response body when the peer disconnects. See [`service_logs_handler.go`](../../server/app/services/websocket/service_logs_handler.go) and [`task_logs_handler.go`](../../server/app/services/websocket/task_logs_handler.go).

The orchestrator uses Docker's native `ServiceLogs` and `TaskLogs` methods. Although the browser sends `stdout` and `stderr` query parameters, the handlers never read them and [`swarm/logs.go`](../../server/app/services/docker/swarm/logs.go) hard-codes both streams on. `LogStreamOptions` has `Since`, but neither public handler accepts it. There is no `until` or `details` support.

[`log_stream.go`](../../server/app/services/websocket/log_stream.go) manually reads an eight-byte mux header, allocates the declared payload, maps type `2` to stderr and every other type to stdout, splits payloads on newline, drops empty lines, optionally removes an RFC3339 timestamp prefix, and writes one JSON text message per non-empty line:

```json
{"stream":"stdout","timestamp":"2026-08-19T12:00:00.000000000Z","line":"ready"}
```

Consequences to preserve or correct explicitly:

- It works for the common non-TTY stdout/stderr path.
- It cannot decode a raw TTY response.
- It misclassifies stdin, system-error, and unknown frame types as stdout.
- A declared frame length can trigger an allocation approaching 4 GiB.
- It treats a Docker record as interchangeable with a text line, discards blank records, and changes invalid UTF-8.
- It has no service/task/node provenance because `details=false`.
- It has no write deadline, ping policy, server-shutdown cancellation, or per-session resource quota.
- Errors after the Docker response begins cannot be represented distinctly from records.

### Frontend

[`use-log-stream.ts`](../../app/src/hooks/use-log-stream.ts) connects directly from the WebView, appends records to a 5,000-line state buffer, and reconnects after 1, 2, 4, 8, then 16 seconds. The service/task viewer supports pause, clear, reconnect, tail selection, and stream/timestamp toggles. The logs page mounts one connection per service tab and permits ten tabs; hidden tabs remain connected. See [`log-viewer.tsx`](../../app/src/components/log-viewer.tsx), [`log-panel.tsx`](../../app/src/components/logs/log-panel.tsx), and [`logs/index.tsx`](../../app/src/routes/_authenticated/logs/index.tsx).

Important observed gaps:

- Automatic reconnect requests the original tail again with no cursor, so it duplicates history.
- A manual reconnect clears visible logs, while an automatic reconnect retains them.
- Option changes update a ref but do not reconnect; they take effect only on a later reconnect. Server-side stdout/stderr selection is ignored regardless.
- The normal list is capped, but the paused buffer is unbounded.
- Every record causes a React state update and each visible record is rendered as a DOM row.
- A connection that opens and immediately flaps resets backoff to one second; there is no jitter or terminal-error classification.
- Closing an old socket as part of reconnect can race its `onclose` retry timer.
- There is no resume/gap/dropped-record indication, heartbeat, source identity, stack aggregation, or actual log-level parsing.

The existing UI tests verify basic URL creation, stdout/stderr display, pause/resume, clear, task routing, tab creation, and unmount close. They do not exercise the failure and resource cases above.

## Docker Engine behavior that shapes the design

### Service logs versus task logs

`GET /services/{id}/logs` accepts a service ID or name and aggregates the service's tasks. `GET /tasks/{id}/logs` selects one task ID. Both are Swarm manager APIs; Docker documents `docker service logs` as a cluster-management command that must run on a manager ([CLI reference](https://docs.docker.com/reference/cli/docker/service/logs/)). A non-manager or non-Swarm daemon can return `503`.

Moby converts either route to a Swarm log selector, opens the SwarmKit `SubscribeLogs` stream, and adds reserved node, service, and task IDs to every message's attributes. The manager receives messages produced by the executor on the node that owns each task; it is not reading every node's filesystem itself. See Moby's [Swarm route helper](https://github.com/moby/moby/blob/master/daemon/server/router/swarm/helpers.go), [cluster service log implementation](https://github.com/moby/moby/blob/master/daemon/cluster/services.go), and [node-local container adapter](https://github.com/moby/moby/blob/master/daemon/cluster/executor/container/adapter.go).

Implications:

- Native `service_logs` is already task fan-in and follows task movement without Kontainer opening one container stream per replica.
- Native `task_logs` still belongs on a manager even when the task executes remotely.
- Worker reachability, task-container retention, logger rotation, manager leadership, and the Swarm log broker all affect availability. Docker is not a durable centralized log store.
- A direct `container_logs` fallback must execute against the node that owns the container. It cannot be sent blindly to the leader's Engine.
- Service tail/order semantics are Docker/Swarm subscription semantics, not a guaranteed globally sorted last-N result. Kontainer should not label it as such.

### Options

The service and task endpoints support:

- `follow`: keep the response open after history is sent.
- `stdout` and `stderr`: at least one must be selected.
- `tail`: a non-negative integer or `all`. Moby's Swarm implementation rejects invalid and negative values. `tail=0` is useful for “new records only.”
- `since`: a Unix timestamp. Moby accepts a seconds value with optional nanoseconds and converts it to a timestamp before subscribing. The Docker CLI also accepts RFC3339 and relative input, but that convenience parsing is client-side ([CLI reference](https://docs.docker.com/reference/cli/docker/service/logs/)).
- `timestamps`: prepend a fixed-width RFC3339Nano timestamp to each record.
- `details`: prepend URL-query-escaped log attributes, including reserved Swarm node/service/task IDs.

The public service/task Engine schema does **not** define `until`; only container logs do. Moby's current Swarm route reads `since` but never reads `until` ([route implementation](https://github.com/moby/moby/blob/master/daemon/server/router/swarm/helpers.go)). Bollard reuses its container `LogsOptions` for `service_logs` and `task_logs`, so its builder exposes `until`, but that does not make the daemon endpoints support it ([Bollard `LogsOptionsBuilder`](https://docs.rs/bollard/latest/bollard/query_parameters/struct.LogsOptionsBuilder.html), [`service_logs`/`task_logs`](https://docs.rs/bollard/latest/bollard/struct.Docker.html)). Do not silently accept and ignore this field.

Recommended public semantics:

- Parse RFC3339Nano or Unix input in Kontainer and validate `since <= until`.
- Always request Docker timestamps internally, even if the UI hides them. Resume and fan-in need source timestamps.
- For service/task streams, report `untilCapability: "client-filtered"`, suppress records after the bound, and cancel a live stream when wall clock reaches `until`. This is useful but not an exact historical `tail-at-until` query.
- Reject combinations that imply exact historical `tail` relative to `until`, or label them explicitly as approximate.
- Track native service/task `until` as unavailable until Docker adds it. Per-container node fan-out is the only current native-`until` route and is deferred.

### Multiplexed and raw responses

With TTY disabled, Docker uses a sequence of frames:

```text
byte 0       stream: 0 stdin, 1 stdout, 2 stderr, 3 daemon/system error
bytes 1..3   reserved
bytes 4..7   payload length, unsigned 32-bit big endian
next N bytes payload
```

Moby's canonical decoder treats stream `0` as stdout for compatibility, stream `3` as a terminal daemon error, and unknown values as protocol errors ([`stdcopy` source](https://github.com/moby/moby/blob/master/api/pkg/stdcopy/stdcopy.go)). For Engine API 1.42 and later the response advertises `application/vnd.docker.multiplexed-stream`; TTY streams advertise `application/vnd.docker.raw-stream`. A TTY response has no headers and combines output as raw console bytes. See the Engine's [attach stream format](https://docs.docker.com/reference/api/engine/version/v1.52/#tag/Container/operation/ContainerAttach) and the [Swarm route's TTY/content-type logic](https://github.com/moby/moby/blob/master/daemon/server/router/swarm/helpers.go).

Use Bollard's `LogOutput::{StdOut, StdErr, StdIn, Console}` rather than duplicating the normal mux decoder. However, make daemon system-error frames a release gate: Moby can emit type `3` after HTTP 200, while Bollard's currently documented `LogOutput` has no system-error variant ([enum](https://docs.rs/bollard/latest/bollard/container/enum.LogOutput.html)). Current Bollard source also deserves a conformance test for this value. If the selected release panics or treats it as unreachable, patch/upstream the decoder or use a small reviewed raw-body decoder before production. A spawned-task panic may contain process damage, but it is not an acceptable error protocol.

Normalization rules:

1. Inspect the service/task TTY setting before opening the stream and also honor `Console` output. Never infer stdout from arbitrary raw bytes.
2. Preserve Docker record boundaries internally. A frame may contain multiple newlines or no final newline.
3. For v1 output, split a record into logical lines, preserving blank lines if the UI can render them. Give every fragment the record timestamp and add an internal `record_id`/`fragment_index`.
4. Cap accepted record and line sizes. For example, begin with 256 KiB per Docker record and make the value configurable. Oversize records should be truncated or split with an explicit flag, never allocated or dropped silently.
5. Treat malformed headers, truncated payloads, invalid reserved bytes, unknown stream types, and system-error frames as typed source errors.
6. Choose an invalid-UTF-8 policy explicitly. Recommended: send lossy text for display with `encoding: "utf8-lossy"` and optionally retain base64 bytes only in protocol v2. Do not pretend conversion was lossless.
7. When `details=true`, parse the comma-separated, URL-query-escaped `key=value` prefix and extract only `com.docker.swarm.node.id`, `.service.id`, and `.task.id` by default. Do not expose configured environment/label attributes without a separate permission and request flag.

## Proposed internal design

### Domain types

Keep Docker, transport, and UI concerns separate:

```rust
enum LogTarget {
    Service { id: String },
    Task { id: String },
    Stack { namespace: String },
    Services { ids: Vec<String> },
}

struct LogRequest {
    target: LogTarget,
    follow: bool,
    streams: LogStreams,
    tail: Tail,
    since: Option<OffsetDateTime>,
    until: Option<OffsetDateTime>,
    ordering: OrderingMode,
}

struct LogRecord {
    source_timestamp: Option<OffsetDateTime>,
    observed_at: OffsetDateTime,
    stream: LogStream, // stdout, stderr, console
    payload: Bytes,
    target_id: String,
    service_id: Option<String>,
    task_id: Option<String>,
    node_id: Option<String>,
    record_id: u64,
}

trait LogSource {
    fn open(
        &self,
        request: SourceRequest,
        cancel: CancellationToken,
    ) -> BoxStream<'static, Result<LogRecord, LogSourceError>>;
}
```

Use action-per-file operations consistent with the repository: resolve a target, detect capabilities, open one Docker source, fan sources in, and run a client session should be discrete actions. Keep the Docker adapter behind `LogSource` so fixtures do not need a daemon.

Never use a timestamp as the only in-process identity. `record_id` is session-local and monotonic; it aids ordering and diagnostics but is not a durable Docker cursor.

### Session task structure and cancellation

One supervisor owns the WebSocket and all source tasks:

```text
session cancellation token
  |- peer reader: close/control/pong
  |- source task 1: Docker -> normalized per-source queue
  |- source task N: Docker -> normalized per-source queue
  |- fair merger: source queues -> bounded writer queue
  `- writer: batches -> WebSocket
```

Use `tokio_util::sync::CancellationToken` and a `JoinSet` (or equivalent owned task set). Tokio documents `CancellationToken` as a way to signal one or more tasks ([docs](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html)). Cancel on peer close/read error, WebSocket write error, shutdown, authorization/session expiry, `until`, source terminal failure when no usable sources remain, or overload. Drop every Bollard stream, close the socket, then await all children with a bounded shutdown timeout. There must be no detached task holding a Docker response after the client disappears.

Do not use a background context analogue. A connection test must prove that closing the browser causes the Docker body and all fan-in tasks to disappear promptly.

### Backpressure and bounded buffering

Tokio's bounded `mpsc` channel waits when full and therefore provides backpressure; its unbounded variant has effectively infinite capacity ([Tokio `mpsc` documentation](https://docs.rs/tokio/latest/tokio/sync/mpsc/)). Use only bounded channels on log data paths.

An item limit alone is insufficient because a log record can be large. Pair bounded channels with a byte-budget semaphore, or place records in a byte-accounted bounded deque. Recommended initial, configurable limits:

- 256 KiB maximum normalized Docker record.
- 2 MiB and 2,000 records queued per client session, whichever is reached first.
- 512 KiB per source before fan-in, included in the session total.
- 64 KiB or 25 ms maximum WebSocket batch, whichever occurs first.
- 10 concurrent compatibility streams per user to match the v1 UI; separately cap global sessions and total Docker sources.
- 32 services in one enhanced fan-in session.
- 10,000 tail lines per source. Treat `all` as privileged/configurable because it is not a resource bound.

These are starting values, not protocol constants. Load tests should tune them.

When the client is slow:

1. Allow the bounded queue to absorb a short burst.
2. Apply backpressure for a short configured grace period.
3. If still full, cancel the session and close with WebSocket `1013` (try again later), including a last resume hint if possible.
4. Increment dropped/overload metrics and show a visible gap after reconnect.

Do not block a Docker stream forever, grow memory, or silently discard. A configurable “drop oldest” mode may be useful for dashboards, but it must emit a `gap` control event with record/byte counts. It should not be the default for forensic logs.

The frontend must use both line and byte bounds. Pausing should freeze rendering or auto-scroll, not create an unbounded second buffer. Batch state updates and virtualize long lists; keep the visible v1 limit of 5,000 lines initially, add a byte limit, and display dropped/gap counts.

### Fair fan-in for services and stacks

One Docker `service_logs` source already merges that service's tasks. For a stack or explicit multi-service request:

1. Resolve stack membership using the `com.docker.stack.namespace` service label and authorize every resolved service.
2. Open one `service_logs` stream per service, subject to the per-session/global source semaphore.
3. Give every source its own small bounded queue so one noisy service cannot occupy the entire shared queue.
4. Poll sources fairly and add stable target/service IDs before emitting records.
5. Treat one source failure as a scoped `source_error`; continue healthy sources. End only when the request is complete, canceled, or every source is terminal.

Default ordering should be `arrival`: preserve order within each Docker source and make no cross-node/global ordering promise. An optional `timestamp-window` mode may hold a min-heap for a small lateness window (for example 250 ms), then order by source timestamp, stable source key, and source sequence. This improves readability but adds latency and cannot fix clock skew. Report late records rather than reordering already emitted data.

For `follow=false`, a bounded snapshot may be collected and timestamp-sorted only when its maximum record/byte budget is known. Never collect `tail=all` into memory to produce a perfect sort.

Define stack tail as **per service**, matching the fact that each service is a separate Engine request. A “global last N” feature requires a bounded snapshot and is deferred. Also decide whether membership is snapshotted at connection time (recommended first) or follows service create/remove events. Dynamic membership is an enhancement and needs deterministic authorization and reconnect behavior.

Resolve names and task slots asynchronously from cached service/task data; IDs are authoritative. Do not inspect a task per log record. Moby already supplies reserved context IDs when details are requested.

## Reconnect and resume semantics

Docker provides no stable log ID, offset, or acknowledgement. `since` is timestamp-based and records from different nodes can arrive late or have skewed clocks. Exact once-only resume cannot be built on this API.

### Compatibility behavior

For the strict compatibility routes, retain reconnecting behavior but fix the duplicate storm:

- On the first connection use the selected tail (default 100).
- Track the greatest **committed** source timestamp delivered to the UI.
- On reconnect use `tail=0` plus `since=floor(last_timestamp)-1s`; the one-second overlap compensates for Bollard's currently integer-shaped log option and timestamp-boundary behavior.
- Preserve existing records and label the session `reconnecting`; manual reconnect may offer a separate “clear and reconnect” action.
- Use exponential backoff with full jitter, a cap (for example 30 seconds), and reset only after a stability interval, not immediately on `open`.
- Do not retry invalid request, forbidden, not found, unsupported driver, or incompatible-target errors until the user/request changes. Retry transport, manager failover, node unreachable, and overload errors.

This is at-least-once. Duplicate records may appear. A bounded fingerprint cache can suppress exact replays within the overlap, keyed by target/task/node, stream, timestamp, payload hash, and per-source occurrence. Because identical records can legitimately share a timestamp and payload, prefer a visible duplicate over an uncertain drop.

### Enhanced resume token

Protocol v2 should emit an opaque resume token on `ready`, periodically, and before a graceful overload/restart close. It can refer to short-lived server state containing:

- normalized target set and authorized subject;
- per-source high-water timestamps and a small replay fingerprint window;
- protocol version and expiry;
- stream filters and ordering mode.

On reconnect, re-authorize the target set, open every source from the minimum safe timestamp with overlap, and deduplicate only confirmed replay-window entries. If the token expired or source history rotated away, emit a `gap` event and continue according to policy. A token must not claim exact continuity.

A completely stateless token grows with task/service count and exposes more parsing/cryptographic surface; a random opaque ID backed by bounded TTL state is preferred unless restart-resume is a firm requirement. Durable cursor state or guaranteed history would require a real log store and is outside Engine-API logging.

## WebSocket and public API design

### Strict v1 compatibility adapter

Keep the two existing paths and current record JSON while the frontend migrates:

```text
GET /api/services/{id}/logs?ticket=...&tail=100&timestamps=true&stdout=true&stderr=true
GET /api/tasks/{id}/logs?ticket=...&tail=100&timestamps=true&stdout=true&stderr=true
```

Compatibility defaults:

- `follow=true`
- `tail=100`
- `timestamps=true`
- stdout and stderr enabled
- one JSON WebSocket message per logical line
- maximum tail 10,000
- `all` retained only if policy permits it

Honor stdout/stderr, support TTY as `stream: "stdout"` in the compatibility shape, and preserve empty lines unless frontend compatibility testing shows a blocker. Translate typed errors to the existing `{"error":"..."}` only for the old protocol.

For a short migration period, accept `token` as an alias to `ticket` if necessary, but redact it everywhere and remove it on a scheduled issue. A JWT in a URL is not an acceptable steady state.

### Enhanced protocol

Prefer a single request that can represent one or many targets. Because browser WebSockets cannot add an arbitrary `Authorization` header, create a one-use ticket through normal authenticated HTTPS first:

```http
POST /api/v2/log-stream-tickets
Authorization: Bearer <access token>
Content-Type: application/json

{
  "targets": [{"kind":"stack","id":"payments"}],
  "follow": true,
  "tail": {"lines": 100},
  "since": null,
  "until": null,
  "streams": ["stdout", "stderr"],
  "ordering": "arrival"
}
```

The response contains a `wss:` URL with a random, single-use, target-bound ticket expiring in roughly 30–60 seconds. The server consumes it atomically during upgrade. The ticket is not a reusable API credential.

Use a versioned envelope so records, state, gaps, and source errors are distinct:

```json
{"v":2,"type":"ready","sessionId":"...","resume":"...","capabilities":{"resume":"at-least-once","until":"client-filtered"}}
{"v":2,"type":"record","seq":42,"timestamp":"2026-08-19T12:00:00.123456789Z","observedAt":"...","stream":"stderr","line":"failed","target":{"kind":"service","id":"..."},"serviceId":"...","taskId":"...","nodeId":"..."}
{"v":2,"type":"gap","reason":"client_slow","records":318,"bytes":90122,"resume":"..."}
{"v":2,"type":"source_error","targetId":"...","code":"node_unreachable","retryable":true,"message":"..."}
{"v":2,"type":"end","reason":"until_reached","resume":"..."}
```

Batch records in a JSON array or a `records` envelope to reduce WebSocket and React overhead, while bounding encoded bytes. Keep control messages unbatched. Do not send engine error strings or internal addresses verbatim; map them to stable codes and a safe message.

An HTTP `fetch()` streaming NDJSON endpoint is a credible future replacement because it can send an Authorization header and naturally supports one-way logs. It is deferred to avoid changing the v1 WebSocket UI and because browser/WebView streaming behavior must be proven across supported platforms. SSE is less attractive: `EventSource` cannot set a bearer header and multi-line/binary handling adds encoding rules.

## Authentication, authorization, and transport security

There are two separate trust boundaries.

### Client to Kontainer

- Authenticate the HTTPS ticket request through normal middleware.
- Authorize `logs:read` for the service/task/stack and every expanded service. For a task, authorize through its parent service/stack rather than treating a guessed task ID as sufficient.
- Bind the ticket to subject, normalized targets/options, protocol version, expiry, and a random nonce; consume it once.
- Validate `Origin` against the configured Tauri/Web origin allowlist. Token-based auth reduces CSRF risk but does not justify accepting every origin.
- Require WSS outside an explicitly detected loopback/local development connection.
- Never log access JWTs, tickets, complete upgrade URLs, user-supplied log contents, or unfiltered Docker `details` attributes.
- Decide whether a long stream ends when the user's access session expires. Recommended first behavior: ticket only authenticates setup, but impose a configurable maximum stream lifetime and re-authorize on reconnect.
- Rate-limit ticket creation and concurrent sessions per subject and target.

### Kontainer to Docker

Prefer the local Unix socket/named pipe with least-privilege OS ownership. Docker warns that control of its daemon is effectively host-root authority and recommends mutual TLS for remote access ([Docker Engine security](https://docs.docker.com/engine/security/), [protect the daemon socket](https://docs.docker.com/engine/security/protect-access/)). Never use unauthenticated `tcp://...:2375` in production.

For a remote Engine, configure Bollard's rustls/mTLS transport with an explicit CA, client certificate, hostname verification, and secret-file permissions. Do not expose Docker credentials through the public API. Negotiate the Engine API version at startup and record the result; Bollard provides `negotiate_version()` for downgrade compatibility ([docs](https://docs.rs/bollard/latest/bollard/struct.Docker.html#method.negotiate_version)).

Kontainer remains an application API, not a generic Docker socket proxy. Validate IDs/options and expose only log operations.

## Log-driver capability detection

The Engine service/task reference limits readable service logs to `local`, `json-file`, or `journald`. Docker's broader logging documentation explains that remote drivers can remain readable through a local dual-logging cache; the default cache rotates five 20 MiB files, and cache writes can be lost when the remote driver write fails ([dual logging](https://docs.docker.com/engine/logging/dual-logging/)). Logging plugins may advertise a `ReadLogs` capability ([plugin protocol](https://docs.docker.com/engine/extend/plugins_logging/)).

Driver names are therefore hints, not a definitive gate:

1. At startup, call ping/version/info. Record Swarm state, manager control availability, daemon default logging driver, and advertised log plugins.
2. At request time, inspect the service or task. Reject non-container runtimes; record TTY and explicit `TaskTemplate.LogDriver` when present.
3. If no explicit service driver exists, remember that tasks can inherit different node defaults. Do not infer cluster-wide readability from the leader's `/info`.
4. Definitively test capability by opening the requested log stream and classify “configured logging driver does not support reading,” `none`, not found, non-manager, and transport errors. Moby defines `ErrReadLogsNotSupported` and uses the log driver's `LogReader` capability ([logger source](https://github.com/moby/moby/blob/master/daemon/logger/logger.go)).
5. Cache only hints and recent outcomes with a short TTL keyed by service version/task ID/node. Never permanently reject a remote driver name: dual logging or a plugin may make it readable.

If native service/task logs are unsupported, return a clear remediation message mentioning a readable driver or dual logging. Do not silently fall back to the leader's container endpoint. A node-local per-container follower fallback is a later feature with separate authorization, resource, and consistency tests.

Also communicate retention honestly: Docker rotation, dual-cache loss, task garbage collection, node loss, and remote-driver delivery failures can remove history before Kontainer asks for it.

## Failure model

Validate everything possible before upgrade so normal HTTP status codes remain available:

| Condition | Before upgrade | After upgrade / multi-source behavior | Retryable |
|---|---:|---|---:|
| Invalid tail/time/zero streams | 400 | `end: invalid_request` | No |
| Missing/expired auth | 401 | close 1008 | After re-auth |
| Unauthorized target/origin | 403 | close 1008 | No |
| Service/task absent | 404 | target-scoped `source_error` | Usually no |
| Unsupported runtime/log driver | 422 | target-scoped `source_error` | After config change |
| Per-user/global limit | 429 | close 1013 with retry hint | Yes |
| Not a Swarm manager / Engine unavailable | 503 | `source_error` or terminal end | Yes |
| Node unreachable/task log gone | 503 or 502 | target-scoped `source_error`; other sources continue | Yes/maybe |
| Malformed/oversize Docker stream | 502 | terminal source protocol error | Maybe |
| Slow browser | n/a | gap then close 1013, according to policy | Yes |
| Agent shutdown | n/a | `end: server_restart`, close 1012 | Yes |
| Clean finite EOF | n/a | `end: complete`, close 1000 | No |

Docker can report an error inside stream type `3` after sending HTTP 200. Map this to `docker_stream_error`, not stdout. EOF on `follow=true` is usually reconnectable unless cancellation or a known finite condition caused it. Preserve per-target partial success in stack/multi-service streams.

Avoid returning raw daemon errors to users. Internal logs may retain a sanitized cause chain and engine status, but remove paths, credentials, internal follower addresses, and log content.

## Observability

Create one tracing span per client session and one child span per Docker source. Useful fields are session ID, hashed/auth subject ID, target kind and safe target ID, source count, follow/tail/since/until presence, negotiated Engine API, TTY, driver hint, queue limits, and terminal reason. Do not attach record payloads or tickets.

Recommended metrics:

- `log_sessions_active` and `log_sources_active` gauges.
- `log_sessions_started_total{target_kind,result}`.
- `log_records_in_total`, `log_bytes_in_total`, `log_records_out_total`, `log_bytes_out_total`.
- `log_queue_records` and `log_queue_bytes` gauges or sampled histograms.
- `log_records_dropped_total{reason}` and `log_gaps_total{reason}`.
- `log_source_errors_total{code,retryable}`.
- `log_reconnects_total{reason}` and resume overlap/dedup counters.
- session duration, Engine-open latency, time-to-first-record, WebSocket send latency, and cancellation latency histograms.
- source timestamp lag (`observed_at - source_timestamp`) with protection against clock-skew outliers.
- fan-in source count and late-record count for timestamp-window ordering.

`tracing`/`tracing-subscriber` should be the base. Add `tracing-opentelemetry` only if v2 standardizes OpenTelemetry. The `metrics` facade plus the repository's selected exporter is adequate for counters and gauges. `tower-http` supplies HTTP request tracing, but the long-lived upgraded session needs its own span because request middleware only sees the upgrade ([Tower HTTP trace docs](https://docs.rs/tower-http/latest/tower_http/trace/)).

Operational health should distinguish “agent can reach Engine,” “node is active manager,” and “a particular service's driver is readable.” Do not make readiness depend on every workload's logging driver.

## Recommended Rust crates

No dependency change is authorized by this document. When implementation begins, use the smallest set already selected by the broader v2 architecture and request approval for additions.

| Need | Recommendation | Reason / caveat |
|---|---|---|
| Docker Engine | `bollard` 0.21.x, exact pin | Native async service/task streams, transports, typed models, version negotiation. Gate on TTY and system-error conformance. |
| Runtime | `tokio` | Async tasks, bounded channels, semaphores, timers, `select!`, `JoinSet`. |
| Cancellation/codecs | `tokio-util` | `CancellationToken`; codecs only if a raw mux fallback is required. |
| Stream combinators | `futures-util` | `StreamExt`, fair source composition, sink handling. |
| HTTP/WebSocket | `axum` with `ws` if it is the v2 server framework | Split read/write halves and configure frame/message/write-buffer bounds. Axum exposes these limits ([WebSocket upgrade docs](https://docs.rs/axum/latest/axum/extract/struct.WebSocketUpgrade.html)). |
| Bytes/serialization | `bytes`, `serde`, `serde_json` | Avoid needless payload copies; stable JSON compatibility and v2 envelopes. |
| Time | `time` **or** the repository-standard `chrono`, not both | RFC3339Nano parsing/formatting. Bollard documents these feature choices as mutually exclusive. |
| Errors | `thiserror` | Stable internal error taxonomy without string matching at API boundaries. |
| IDs/fingerprints | existing UUID crate; optionally `blake3` | Session IDs and bounded replay fingerprints. A hash is dedup help, not a durable cursor. |
| Telemetry | `tracing`, `tracing-subscriber`, `metrics` | Structured spans and low-cardinality metrics. |
| Property tests | `proptest` | Mux/details/timestamp/parser and merge invariants. |

Do not enable Bollard's WebSocket feature for Engine logs; `service_logs` and `task_logs` are streaming HTTP responses. Kontainer's browser-facing WebSocket is separate.

## Approaches considered

| Approach | Decision | Why |
|---|---|---|
| Bollard `service_logs`/`task_logs` on a manager | **Accepted baseline** | Matches Docker and v1 semantics, handles Unix/named-pipe/TLS transport, and delegates task/node fan-out to Swarm. |
| Small raw Engine client/decoder over Hyper | **Deferred escape hatch** | Gives exact query strings, content-type control, and safe type-3/frame-size handling, but recreates transport, version negotiation, status decoding, and security-sensitive client code. Use only if the Bollard release cannot be fixed. |
| Fork/patch Bollard decoder | **Conditionally accepted** | Narrower than a full client. Required if type-3/size behavior fails conformance and an upstream fix is not released in time. Keep the patch minimal and linked to an upstream issue. |
| Docker CLI subprocess | **Rejected** | External binary/version dependency, text formatting and name resolution, awkward cancellation/backpressure, weaker structured errors, and additional credential/process surface. |
| Read `/var/lib/docker/containers/...` | **Rejected** | Unsupported internal storage, root/node locality, driver/rotation differences, and bypasses Engine authorization/capabilities. |
| One container stream per task via followers | **Deferred fallback** | Can use native container `until` and diagnose a single node, but multiplies streams, must follow rescheduling/history, requires node-local routing, and changes service-tail semantics. |
| One native service stream per selected service | **Accepted for stack/multi-service** | Keeps Docker's per-service task aggregation and makes source count proportional to services rather than replicas. Needs fair bounded fan-in. |
| Arrival-order fan-in | **Accepted default** | Minimal latency and honest semantics; preserves each source's order without pretending clocks provide a total order. |
| Timestamp-window ordering | **Deferred optional mode** | Better visual chronology at the cost of delay, heap memory, and clock-skew/late-record complexity. |
| Persist logs in Kontainer/SQLite | **Rejected for v2 baseline** | Turns the agent into a log database, creates retention/search/security work, and SQLite is not suited to high-volume cluster logs. Integrate a real backend later. |
| Loki/Elastic/Cloud provider adapter | **Deferred enhancement** | Correct long-term path for durable history/search and drivers that cannot read back through Docker, but not universal or v1 parity. |
| One WebSocket per v1 tab | **Accepted compatibility only** | Matches existing UI and isolates failures, but consumes many sockets and Docker sources. |
| One multiplexed WebSocket for many targets | **Accepted v2 direction** | Centralizes authentication, batching, fan-in, quotas, and gaps; requires a versioned protocol/UI migration. |
| Authenticated fetch/NDJSON | **Deferred candidate** | Supports bearer headers and one-way streaming well, but needs cross-WebView validation and changes the current UI contract. |
| SSE/EventSource | **Rejected for now** | No arbitrary Authorization header in native EventSource, plus extra multiline/binary encoding constraints. |
| Silent drop-oldest buffering | **Rejected default** | Keeps dashboards moving but hides evidence. Only offer with explicit gap events and policy. |
| Block slow clients indefinitely | **Rejected** | Pins Docker streams/tasks and allows one client to consume unbounded operational capacity. |

## Strict v1 parity versus v2 enhancement

| Behavior | Strict parity requirement | v2 enhancement |
|---|---|---|
| Routes | Existing service/task WebSocket paths | Unified ticket + multi-target session API |
| Wire record | `{stream,timestamp,line}` | Versioned records, control events, provenance, batching, resume hints |
| Default query | follow, tail 100, timestamps, both streams | since/until, tail 0, ordering, metadata policy |
| TTY | Must work without breaking old shape | Explicit `console` stream and byte encoding |
| Filters | Honor stdout/stderr immediately | Server-side task/node/service/text/level filters |
| Buffers | Visible 5,000-line behavior | Byte bounds, virtualization, gap indicators, global quotas |
| Pause/clear | Preserve controls | Pause rendering without unbounded buffering; optional server control |
| Reconnect | Automatic bounded-jitter retry | At-least-once resume token and replay-window dedup |
| Multi-stream page | Up to ten service tabs/connections | One session with fair stack/multi-service fan-in |
| Context | None required for old rows | IDs/names/slot/node metadata and filters |
| Ordering | Arrival | Optional timestamp lateness window |
| Log level | OUT/ERR remains valid | Optional structured parser/provider, never infer silently |
| Driver errors | Clear terminal message | Capability hints, remediation, provider fallback |

Bug fixes are required even in parity mode: honor stream toggles, cancel promptly, bound every buffer/allocation, support raw TTY, classify daemon/system errors, add write/heartbeat timeouts, and redact URL credentials. “Parity” does not mean reproducing unsafe behavior.

## Implementation sequence

1. **Engine capability and adapter foundation**
   - Create negotiated Bollard client configuration for local socket/named pipe and approved mTLS endpoints.
   - Add typed service/task inspect and `LogSource` adapters.
   - Prove non-TTY mux, TTY raw, stdout/stderr, timestamps/details, EOF, and daemon system-error behavior against supported Engines.
2. **Normalized bounded pipeline**
   - Add domain request/record/error types, details/timestamp parsing, size limits, bounded byte accounting, cancellation tree, and source fixtures.
   - Add session/source telemetry and quotas.
3. **v1 WebSocket adapter**
   - Preserve routes/JSON/defaults, honor filters, add Origin/auth hardening, heartbeat/write deadlines, and frontend reconnect race fixes.
   - Keep the 5,000-line UI behavior but bound paused bytes and batch updates.
4. **Resume semantics**
   - Add timestamp overlap reconnect and visible gaps; test manager restart/network interruption.
   - Add ticket and resume-token APIs after the compatibility path is stable.
5. **Stack/multi-service fan-in**
   - Resolve and authorize stack services, add fair per-source queues, scoped failures, color/source metadata, and explicit per-service tail semantics.
6. **Enhancements**
   - Optional ordering window, dynamic stack membership, structured level parsing, remote log-provider adapters, or node-local container fallback.

## Detailed implementation and acceptance checklist

### Docker client and capability setup

- [ ] Pin an exact reviewed Bollard release and record enabled features.
- [ ] Connect through Unix socket/named pipe by default; permit remote Engine only with verified mTLS/approved SSH policy.
- [ ] Call API-version negotiation and expose the negotiated version in diagnostics.
- [ ] Validate Swarm active/manager state without assuming the agent's role name proves it.
- [ ] Inspect service/task before streaming; validate container runtime, TTY, parent service, and authorization.
- [ ] Record explicit driver when present and node-default uncertainty when absent.
- [ ] Attempt the real stream before declaring a driver readable.
- [ ] Classify not found, not manager, unsupported driver, `none`, node unreachable, bad request, timeout, and transport failure without relying solely on English strings.
- [ ] Verify service logs include replicas on at least two worker nodes.
- [ ] Verify task logs contain only the selected task.
- [ ] Verify a service update/restart and new replacement task behavior under `follow=true`.

### Stream decoding and normalization

- [ ] Test mux headers split across arbitrary HTTP chunks and multiple frames in one chunk.
- [ ] Test stream types 0, 1, 2, 3, unknown values, zero-length payload, truncated header/payload, and oversize length.
- [ ] Confirm type 3 becomes a terminal `docker_stream_error`; no panic and no stdout misclassification.
- [ ] Confirm raw TTY output works, including first byte 0/1/2 and a final line without newline.
- [ ] Confirm stdout/stderr selection reaches Docker and is also enforced after normalization.
- [ ] Request timestamps internally and parse RFC3339Nano with 0–9 fractional digits and offsets.
- [ ] Parse URL-escaped detail attributes and extract reserved Swarm IDs.
- [ ] Do not expose arbitrary detail/env attributes without explicit permission.
- [ ] Preserve record boundaries and define multiline/blank-line compatibility fragments.
- [ ] Mark invalid UTF-8 conversion and oversize truncation/splitting explicitly.
- [ ] Enforce both record-byte and line-byte limits before public serialization.

### Query semantics

- [ ] Default compatibility tail to 100; validate `0`, positive integers, `all`, and a 10,000 numeric ceiling.
- [ ] Require at least one of stdout/stderr.
- [ ] Parse RFC3339Nano and Unix `since`/`until`; reject invalid ranges.
- [ ] Document that tail is per Docker source/service, not globally sorted across a stack.
- [ ] Never pass Bollard `until` to service/task and assume it worked.
- [ ] Expose service/task `until` as client-filtered/best-effort or reject exact-only combinations.
- [ ] Ensure `follow=false` ends cleanly and `follow=true` EOF is classified for retry.

### Cancellation and resource safety

- [ ] One session cancellation token reaches reader, every Docker source, merger, and writer.
- [ ] Peer close, failed send, shutdown, auth expiry policy, `until`, and overload all cancel sources.
- [ ] Drop Docker response streams and await every child task on exit.
- [ ] No task survives a test connection close beyond the shutdown grace period.
- [ ] Use bounded record and byte budgets at every queue; no unbounded data channels/deques.
- [ ] Enforce per-user, per-session, per-target, global-source, message-size, tail, and stream-lifetime limits.
- [ ] Configure WebSocket inbound message/frame/write-buffer limits and reject unexpected client binary/data messages.
- [ ] Add ping/pong or application heartbeat and idle/write timeouts appropriate to browser behavior.
- [ ] Slow consumers receive a gap/1013 policy outcome; they cannot pin resources indefinitely.
- [ ] `tail=all` is disabled, privileged, or protected by a documented transfer budget.
- [ ] Load test high-rate, long-line, ten-tab, and 32-service fan-in cases under a fixed memory ceiling.

### Reconnect and delivery semantics

- [ ] Initial connect uses requested tail; reconnect uses `tail=0` and timestamp overlap.
- [ ] Backoff uses jitter, a cap, stability-based reset, and a single owned timer/state machine.
- [ ] Terminal errors do not retry automatically; retryable errors do.
- [ ] Manual reconnect does not race an old socket's close callback.
- [ ] The UI distinguishes connecting, live, reconnecting, partial, ended, and error.
- [ ] Resume contract says at-least-once and tests permit duplicates but detect missing records within retained history.
- [ ] Gap events are visible after overload, expired cursor, rotated history, or failed sources.
- [ ] Resume token is opaque, expiring, target/subject-bound, size-limited, and replay-tested.
- [ ] Manager failover, agent restart, worker disconnect/rejoin, and log rotation are exercised.

### Fan-in and UI

- [ ] Stack membership expansion uses authoritative labels and authorizes every service.
- [ ] One Docker source is opened per service, not per task, for normal fan-in.
- [ ] Source concurrency is semaphore-limited before tasks are spawned.
- [ ] Per-source queues and fair polling prevent a noisy service from starving others.
- [ ] Source failure is target-scoped and healthy sources continue.
- [ ] Arrival order preserves per-source order; no total-order promise appears in UI/docs.
- [ ] Optional timestamp ordering has a fixed lateness/memory bound and reports late records.
- [ ] IDs are emitted immediately; name/slot/node resolution uses caches and never performs per-line inspection.
- [ ] Frontend buffers are bounded by lines and bytes, including pause mode.
- [ ] Incoming records are batched into state updates and rendered with list virtualization.
- [ ] Closing a tab/stream cancels its server session; hidden compatibility tabs count toward the limit.
- [ ] Filters indicate whether they are client-only or reduce server/source traffic.
- [ ] The LEVEL column remains stream (`OUT`/`ERR`) unless a tested explicit parser is selected.

### Security and observability

- [ ] Replace reusable JWT query auth with short-lived, single-use stream tickets.
- [ ] During migration, redact both `token` and `ticket` query values from all logs/traces/proxies.
- [ ] Enforce an Origin allowlist and WSS outside loopback development.
- [ ] Audit log access by subject and target without storing payloads.
- [ ] Do not expose Docker socket operations, daemon paths, credentials, or raw internal errors.
- [ ] Session/source spans contain safe identifiers and terminal reasons.
- [ ] Active sessions/sources, bytes/records, queue occupancy, gaps/drops, errors, reconnects, and latency metrics exist with low-cardinality labels.
- [ ] A status endpoint distinguishes Engine reachability, manager capability, and request-specific driver readability.
- [ ] Resource-limit and cancellation failures are observable without logging log content.

### Test matrix

- [ ] Unit/property tests cover parser, timestamp/details, validation, error mapping, merge fairness, byte permits, dedup, ticket replay, and cancellation races.
- [ ] WebSocket tests cover pre-upgrade status, control/data envelopes, close codes, heartbeat, slow reader, abrupt disconnect, and shutdown.
- [ ] Frontend tests cover option changes, bounded pause, reconnect state, duplicates/gaps, partial stack failure, batching, and unmount cancellation.
- [ ] Integration tests run `local`, `json-file`, `journald`, `none`, and at least one remote driver with dual cache enabled/disabled where CI supports it.
- [ ] Integration tests run TTY and non-TTY, stdout only, stderr only, multiline, blank, invalid UTF-8, long-line, high-volume, finite job, replicated, and global services.
- [ ] A three-node Swarm test covers cross-node service aggregation, per-task selection, worker loss, manager failover, task replacement, and completed-task/history cleanup.
- [ ] Compatibility contract snapshots match v1 JSON and routes.
- [ ] The selected Bollard version's own Swarm/log integration behavior is reproduced; its repository documents an integration setup that initializes Swarm ([Bollard repository](https://github.com/fussybeaver/bollard#integration-tests)).
- [ ] Soak tests show stable memory/task/socket counts after repeated connects, reconnects, pauses, and disconnects.

## Open decisions suitable for GitHub issues

1. **Bollard type-3 and frame-size conformance**  
   Reproduce daemon system-error and malicious/oversize frame behavior on the pinned Bollard release. Upstream a fix, carry a minimal patch, or approve a raw decoder. This blocks production release.

2. **Minimum/maximum Docker Engine versions**  
   Select the supported API range, including whether API 1.42 content-type signaling is the minimum. Add CI jobs for both ends and define upgrade messaging.

3. **Stream authentication migration**  
   Design the single-use ticket store/claims, expiry, Origin allowlist, WSS requirements, JWT-query deprecation window, and long-session expiry policy.

4. **Unified protocol and batching**  
   Finalize v2 envelope fields, stable error codes, record batch limits, binary/invalid-UTF-8 representation, close codes, and compatibility negotiation.

5. **Reconnect/resume guarantee**  
   Choose global versus per-source high-water state, replay overlap, fingerprint rules, server-side TTL storage, gap conditions, and wording of the at-least-once guarantee.

6. **`until` product semantics**  
   Decide whether service/task requests expose best-effort client filtering, reject `until`, or invest in node-local per-container fan-out for exact native filtering. Specify `tail + until` behavior.

7. **Slow-consumer policy and default budgets**  
   Load-test and set record/byte queues, line size, batching, grace period, close-versus-drop policy, per-user/global limits, and `tail=all` permissions.

8. **Stack fan-in semantics**  
   Decide snapshot versus dynamic membership, per-service versus global tail, partial-failure UI, maximum services, reconnect behavior when membership changes, and authorization rechecks.

9. **Ordering contract**  
   Confirm arrival order as default. If timestamp-window mode is wanted, set lateness, clock-skew handling, deterministic tie breaks, and late-record UI.

10. **Metadata and privacy policy**  
    Decide which node/task/service names, labels, and log-driver details viewers may see. Keep arbitrary Docker log details off by default and review environment-label secret risk.

11. **Node-local container fallback**  
    Separately design follower routing, internal authentication, task-to-container mapping, task churn, concurrency, native `until`, and behavior when a node is offline. Do not make this an implicit fallback.

12. **External log-provider interface**  
    Define when Loki/Elastic/cloud adapters should replace Engine logs for durable search/history, how capabilities are advertised, and how provider cursors map to the public protocol.

13. **Frontend storage/rendering architecture**  
    Select byte/line limits, virtualization library or in-house windowing, batching cadence, pause semantics, searchable buffer strategy, and whether compatibility tabs migrate to one v2 session.

14. **Structured log-level parsing**  
    Decide explicit JSON/logfmt/configured regex parsers and fallback behavior. Stdout/stderr is a stream, not an application severity; do not relabel it automatically.

15. **Operational defaults and administrator controls**  
    Add settings for maximum sessions/sources/tail/bytes/lifetime, readable-driver remediation, audit retention, and telemetry exporter without permitting unbounded values.

## Definition of done

Logging is production-ready when the compatibility service/task routes work on the supported Engine matrix; TTY, mux, and system-error behavior cannot panic or misclassify; every queue/allocation/session has an enforced bound; disconnect and shutdown cancel all Docker work; authentication no longer exposes a reusable access token in routine URLs; retries are jittered and visibly at-least-once; driver/node failures are actionable; and the three-node high-volume soak test holds stable memory, task, and socket counts.

Stack/multi-service logs are done only after fair fan-in, explicit per-service tail and ordering semantics, partial failure, target authorization, and source/global quotas pass the same soak tests. Exact historical resume, exact service/task `until`, and durable search are not part of this Engine-backed definition of done.

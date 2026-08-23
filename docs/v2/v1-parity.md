# Kontainer v1 behavior and parity inventory

Status: archaeology baseline for the v2 Rust agent rewrite  
Snapshot: repository `HEAD` `2c91413` on 2026-08-19, plus named historical commits  
Scope: observable v1 behavior only; this document classifies behavior but does not design the replacement

This is the canonical, mechanically checkable inventory of the Go/Goravel agent, its Tauri adapter, and the React client that exist in this repository. The focused v2 reports remain authoritative for proposed v2 designs; this document only cross-references them. If prose elsewhere conflicts with executable code, the source and tests win. If the Rust adapter conflicts with the Go server, both facts are recorded and the mismatch is classified as a defect rather than treated as a contract.

## Classification key

| Classification | Meaning in this inventory |
|---|---|
| **Useful parity** | Working or intended v1 capability whose user-visible outcome should be considered for parity. Exact implementation quirks are not automatically required. |
| **Bug to fix — do not port** | Current behavior is unsafe, inconsistent, broken, or accidental. Preserve the capability, not the defect. |
| **Obsolete/deleted historical behavior to reconsider** | Removed, unmerged, framework-specific, or superseded behavior. It is evidence for product intent, not automatic parity. |
| **Aspirational only** | Documented product direction with no complete executable v1 implementation. |
| **Frontend-only / no backend** | UI affordance, placeholder, or client behavior without an agent capability behind it. |
| **Backend-only / no UI** | Agent capability exists but the shipped desktop UI cannot reach it. |
| **Intentionally deferred** | The repository explicitly presents the behavior as future work or out of the current target. |

## Runtime boundary and source precedence

- Leaders expose setup, authentication, `/api/*`, HTTP health, WebSockets, the scheduler, and gRPC. Followers expose HTTP health and gRPC, but not the leader API. Invalid `AGENT_ROLE` values silently become `leader`; see [`server/config/cluster.go`](../../server/config/cluster.go#L6) and [`server/routes/*.go`](../../server/routes).
- The desktop normally calls the leader through Tauri `invoke`; logs and exec deliberately use WebView WebSockets directly. New Tauri commands are registered in [`app/src-tauri/src/lib.rs:9`](../../app/src-tauri/src/lib.rs#L9).
- The agent is an API over Docker Engine/Swarm, not a Docker-socket proxy. The implemented Docker interface is deliberately narrow in [`server/app/services/docker/swarm/client.go:19`](../../server/app/services/docker/swarm/client.go#L19) and [`server/app/services/docker/orchestrator.go:247`](../../server/app/services/docker/orchestrator.go#L247).
- Current source describes present behavior. `docs/features.md` is primarily a vision document. Pre-Goravel behavior is taken from `d509d6d^`; removed service-resource work is taken from the non-ancestor `feat/service-resources` commits `e2053df`, `b9e73f4`, and `e8bfbfa`.
- Focused evidence and proposed v2 treatment are in [API protocol](api-protocol.md), [cluster communication](cluster-communication.md), [security](security.md), [logging](logging.md), [exec](exec.md), [Swarm resources](swarm-resources.md), [stack deployment](stack-deployment.md), [metrics storage](metrics-storage.md), [agent lifecycle](agent-lifecycle.md), [performance](performance.md), and [reliability/testing](reliability-testing.md). Those designs are not repeated here.

## Public HTTP route inventory

All non-WebSocket leader `/api/*` routes pass through JWT middleware. The three `/api/*` WebSockets are registered outside that group and validate a query token themselves; setup/auth/health exceptions are also listed separately. Router registration is the route-level source of truth: [`server/routes/api.go:46`](../../server/routes/api.go#L46), [`auth.go:13`](../../server/routes/auth.go#L13), [`setup.go:14`](../../server/routes/setup.go#L14), and [`web.go:9`](../../server/routes/web.go#L9).

| ID | Method and path | Request → observable response/behavior | Classification |
|---|---|---|---|
| V1-HTTP-001 | `GET /health` | All roles; always `200 {"status":"ok"}` and does not test Docker, database, scheduler, gRPC, or readiness. | Useful parity; shallow semantics are V1-BUG-001 |
| V1-HTTP-002 | `GET /ping` | All roles; `200 {app:"dev.kontainer.app",status:"ok"}` or `status:"setup_required"` from persisted setup state. | Useful parity |
| V1-HTTP-003 | `GET /setup/status` | Leader/public; returns `complete`, `github_connected`, or `pending` by checking config keys. It does not authenticate the supplied setup token. | Useful parity; verification defect is V1-BUG-002 |
| V1-HTTP-004 | `POST /setup/github/manifest` | Leader/setup-token protected; optional body `{org}`; replaces a single in-memory manifest state and returns `{setupUrl}` pointing at `/setup/github/start?token=…`. | Useful parity; singleton/state defects are V1-BUG-003 |
| V1-HTTP-005 | `GET /setup/github/start` | Leader/public route with one-time `token` query; consumes it, then auto-submits a GitHub App manifest form. | Useful parity |
| V1-HTTP-006 | `GET /setup/github/callback` | Leader/public GitHub manifest callback; converts `code`, stores GitHub App credentials/JWT secret, then redirects to GitHub installation. A non-empty `setup_action` returns installed HTML. | Useful parity; missing state validation is V1-BUG-004 |
| V1-HTTP-007 | `POST /setup/complete` | Leader/setup-token protected; sets setup complete, reloads JWT config, and registers the GitHub provider; returns `{status:"complete"}`. | Useful parity; non-atomic completion is V1-BUG-005 |
| V1-HTTP-008 | `GET /auth/providers` | Leader/public; `{providers:[provider names]}` from the in-memory OAuth registry. | Useful parity |
| V1-HTTP-009 | `GET /auth/listen` | Leader/public WebSocket; sends a newly generated session ID and waits up to ten minutes for an OAuth token bridge. | Useful parity |
| V1-HTTP-010 | `GET /auth/{provider}` | Leader/public; requires `session` query, creates one-time OAuth state bound to that string, then redirects to the provider. | Useful parity |
| V1-HTTP-011 | `GET /auth/{provider}/callback` | Leader/public; requires `code` and `state`, validates/consumes state, exchanges code, upserts the user, signs JWT, publishes it to the listener, and returns close-window HTML. | Useful parity |
| V1-HTTP-012 | `GET /api/me` | Authenticated; `{user:{id,name,email,avatarUrl,provider},status:"ok"}`. `id` crosses to clients as a string. | Useful parity |
| V1-HTTP-013 | `GET /api/agents` | Authenticated; `{agents}` from the in-memory discovery registry, including `nodeId`, `hostname`, internal `address`, `role`, `status`, `lastSeen`. No desktop command/UI. | Backend-only / no UI |
| V1-HTTP-014 | `GET /api/services` | Optional `stack`; returns `{services}`. | Useful parity |
| V1-HTTP-015 | `GET /api/services/{serviceID}` | Returns the service-detail object directly or 404. | Useful parity |
| V1-HTTP-016 | `POST /api/services/{serviceID}/scale` | Body `{replicas:uint64}`; normal replicated service only; response `{replicas}`. | Useful parity |
| V1-HTTP-017 | `POST /api/services/{serviceID}/restart` | Increments `ForceUpdate`; `{status:"restarting"}`. | Useful parity |
| V1-HTTP-018 | `POST /api/services/{serviceID}/rollback` | Sets rollback to `previous`; `{status:"rolling_back"}`. | Useful parity |
| V1-HTTP-019 | `PUT /api/services/{serviceID}/env` | Body `{env:[{key,value}]}` replaces the complete environment list; `{status:"updated"}`. | Useful parity |
| V1-HTTP-020 | `PUT /api/services/{serviceID}/image` | Body `{image}` replaces container image; `{status:"updated"}`. | Useful parity |
| V1-HTTP-021 | `GET /api/services/{serviceID}/logs` | Authenticated by query token, then upgraded; query options are in V1-WS-001. | Useful parity |
| V1-HTTP-022 | `GET /api/services/{serviceID}/tasks` | Returns `{tasks}` filtered by route ID. | Useful parity |
| V1-HTTP-023 | `GET /api/tasks` | Optional `serviceID` and `nodeID`; returns `{tasks}`. | Useful parity |
| V1-HTTP-024 | `GET /api/tasks/stats` | Returns `{stats:[container stats]}` for local-node running tasks. No Tauri command/UI calls this exact endpoint. | Backend-only / no UI |
| V1-HTTP-025 | `GET /api/tasks/{taskID}` | Returns the task-detail object directly or 404. | Useful parity |
| V1-HTTP-026 | `GET /api/task-stats` | Returns aggregate `{taskStats:{total,running,failed,preparing,complete,shutdown,desiredRunning}}`. | Useful parity |
| V1-HTTP-027 | `GET /api/tasks/{taskID}/logs` | Query-token WebSocket; calls Docker `TaskLogs` by task ID through the leader's Engine API. | Useful parity |
| V1-HTTP-028 | `GET /api/nodes/{nodeID}/tasks` | Returns `{tasks}` filtered by route ID. | Useful parity |
| V1-HTTP-029 | `GET /api/nodes` | Returns `{nodes}`. | Useful parity |
| V1-HTTP-030 | `PATCH /api/nodes/{nodeID}/availability` | Body availability enum `active\|drain\|pause`; returns `{availability}`. | Useful parity |
| V1-HTTP-031 | `PATCH /api/nodes/{nodeID}/role` | Body role enum `manager\|worker`; returns `{role}`. | Useful parity |
| V1-HTTP-032 | `GET /api/stacks` | Groups labeled services; returns `{stacks}`. | Useful parity |
| V1-HTTP-033 | `DELETE /api/stacks/{stackName}` | Removes matching services, networks, secrets, then configs; `{status:"removed"}`. Volumes are retained. | Useful parity; partial-success behavior is V1-BUG-017 |
| V1-HTTP-034 | `GET /api/networks` | Returns `{networks}`. `services` is always an empty placeholder. | Useful parity; incomplete field is V1-BUG-018 |
| V1-HTTP-035 | `GET /api/volumes` | Returns local-engine `{volumes}` only. | Useful parity |
| V1-HTTP-036 | `GET /api/secrets` | Returns metadata-only `{secrets}`; Docker secrets are never readable. | Useful parity |
| V1-HTTP-037 | `POST /api/secrets` | Body `{name,data}`; creates secret; `201 {name}`. | Useful parity |
| V1-HTTP-038 | `DELETE /api/secrets/{secretID}` | Deletes by ID; `{status:"removed"}`. | Useful parity |
| V1-HTTP-039 | `GET /api/configs` | Returns metadata-only `{configs}`. | Useful parity |
| V1-HTTP-040 | `POST /api/configs` | Body `{name,data}`; creates config; `201 {name}`. | Useful parity |
| V1-HTTP-041 | `GET /api/configs/{configID}` | Returns the config-detail object directly, including base64-encoded `data`. | Useful parity |
| V1-HTTP-042 | `DELETE /api/configs/{configID}` | Deletes by ID; `{status:"removed"}`. | Useful parity |
| V1-HTTP-043 | `GET /api/metrics` | Optional `range`; only `6h` and `24h` are recognized, otherwise one hour. Returns latest cluster snapshot plus history. | Useful parity |
| V1-HTTP-044 | `GET /api/metrics/{nodeID}` | Latest node metric not older than two minutes, otherwise 404; includes recent container rows. | Useful parity |
| V1-HTTP-045 | `GET /api/images` | Fanout across leader and healthy discovered followers; returns `{nodes:[{nodeId,hostname,images}]}`. | Useful parity |
| V1-HTTP-046 | `DELETE /api/images/{imageID}` | Query `node` and optional `force`; blank/leader is local, named follower is remote. | Useful parity |
| V1-HTTP-047 | `POST /api/images/prune` | Query `nodeId`; blank prunes leader and all registered followers, otherwise one node; returns `{imagesDeleted,spaceReclaimed}`. | Useful parity |
| V1-HTTP-048 | `GET /api/settings` | Returns all persisted arbitrary settings as `{settings:{key:value}}`. Hooks exist, but no screen uses them. | Backend-only / no UI |
| V1-HTTP-049 | `PUT /api/settings` | Body `{settings:{key:value}}`; arbitrary string-map partial upsert, then returns merged `{settings}`. There is no reset/delete operation. Hooks exist, but no screen uses them. | Backend-only / no UI |
| V1-HTTP-050 | `GET /api/exec/{containerID}` | Query-token WebSocket; local Docker exec or gRPC proxy by optional `nodeID`. | Useful parity |

Request validation lives in [`server/app/http/requests`](../../server/app/http/requests). The concrete controller response shapes and status handling are summarized in [API protocol § Public HTTP routes](api-protocol.md#public-http-routes).

## WebSocket contracts

| ID | Surface | Exact v1 behavior | Classification | Evidence |
|---|---|---|---|---|
| V1-WS-001 | Service logs | Queries: `token`, `tail`, `timestamps`, `stdout`, `stderr`. `tail` defaults to `100`, accepts `all`, clamps numeric values to `1..10000`; timestamps default on unless literal `false`; follow is forced on. `stdout`/`stderr` are transmitted by the client but ignored by the handler; `since` exists in the service option type but is never wired. | Useful parity; ignored controls are V1-BUG-007 | [`service_logs_handler.go:24`](../../server/app/services/websocket/service_logs_handler.go#L24) |
| V1-WS-002 | Task logs | Same output protocol and tail/timestamp rules as service logs; uses Docker's task-log endpoint through the leader client, with no Kontainer follower proxy. | Useful parity | [`task_logs_handler.go:23`](../../server/app/services/websocket/task_logs_handler.go#L23) |
| V1-WS-003 | Log server frames | Docker 8-byte multiplex header; byte `2` is `stderr`, everything else `stdout`. Frames are emitted as JSON `{stream,timestamp?,line}`. Trailing newlines are trimmed, multi-line payloads split, blank lines dropped, and leading RFC3339Nano timestamps parsed when enabled. | Useful parity | [`log_stream.go:15`](../../server/app/services/websocket/log_stream.go#L15) |
| V1-WS-004 | Log client lifecycle | Client keeps at most 5,000 visible entries; paused messages accumulate in an unbounded buffer. Automatic reconnect delays are 1/2/4/8/16 seconds without jitter and re-request the current configured tail without a cursor, so entries can duplicate. Option changes wait for a reconnect; manual reconnect clears visible logs. | Useful parity; unbounded/duplicate behavior is V1-BUG-008 | [`use-log-stream.ts:20`](../../app/src/hooks/use-log-stream.ts#L20) |
| V1-WS-005 | Exec handshake | Queries: `token`, `nodeID`, `shell`, `user`, `cols` (80), `rows` (24). Invalid dimensions fall back, but literal zero is accepted. Container ID/name must match `[A-Za-z0-9][A-Za-z0-9_.-]*` and be at most 128 bytes. No node/leader selects local exec. | Useful parity | [`exec_handler.go:21`](../../server/app/services/websocket/exec_handler.go#L21) |
| V1-WS-006 | Exec client frames | Binary frames are raw stdin. Text JSON control frames contain resize dimensions; malformed controls are ignored. | Useful parity | [`exec_session.go:15`](../../server/app/services/websocket/exec_session.go#L15) |
| V1-WS-007 | Exec server frames | Binary frames are terminal output. Text JSON is `{type,message?,code}`, used for error/exit signalling. Local success closes without a success exit frame; follower Docker output EOF sends exit code 0 without inspecting the exec result. | Useful parity; inconsistent exit semantics are V1-BUG-009 | [`exec_session.go:21`](../../server/app/services/websocket/exec_session.go#L21) |
| V1-WS-008 | Local shell selection | `auto` tries `/bin/sh`, `/bin/bash`, `/bin/ash`; an explicitly requested missing shell silently falls back through that same list. Docker exec uses TTY, stdin/stdout/stderr, requested user, and resizes. | Useful parity | [`server/app/services/docker/swarm/exec.go:42`](../../server/app/services/docker/swarm/exec.go#L42) |
| V1-WS-009 | Remote exec | Leader proxies WebSocket↔gRPC. Unknown node upgrades first, then emits an in-band error. Follower always launches `/bin/sh`, ignores requested `user`, and has no shell fallback. | Bug to fix — do not port | [`grpc/controllers/exec_controller.go:30`](../../server/app/grpc/controllers/exec_controller.go#L30) |
| V1-WS-010 | WebSocket authentication/origin | `token` query is required and parsed as JWT; missing/invalid gives HTTP 401 before upgrade. All origins are accepted. Tokens therefore appear in URLs/logs/history. | Bug to fix — do not port | [`websocket/auth.go:8`](../../server/app/http/controllers/websocket/auth.go#L8), [`websocket/upgrader.go`](../../server/app/services/websocket/upgrader.go) |
| V1-WS-011 | OAuth bridge | Server sends JSON `{sessionId}` immediately and later `{token}`. Bridge session and OAuth state are in memory; both identifiers are 128-bit random, state is one-use with ten-minute expiry, and listener wait is ten minutes. | Useful parity | [`bridge.go`](../../server/app/services/auth/bridge.go), [`session_store.go`](../../server/app/services/auth/session_store.go) |

See [logging](logging.md) and [exec](exec.md) for framing and v2 design decisions.

## Internal gRPC inventory

The sole schema is [`server/proto/agent/agent.proto:1`](../../server/proto/agent/agent.proto#L1). Services are registered only when the orchestrator, node ID, and hostname resolve ([`server/routes/grpc.go:11`](../../server/routes/grpc.go#L11)).

| ID | RPC | Request → response/stream | Classification |
|---|---|---|---|
| V1-RPC-001 | `Health(HealthRequest)` | Empty request → `HealthResponse{status,node_id,hostname}`; always reports `ok`. | Useful parity; shallow semantics are V1-BUG-001 |
| V1-RPC-002 | `Info(InfoRequest)` | Empty request → `InfoResponse{node_id,hostname,role}`. Registered but no current leader caller was found. | Backend-only / no UI |
| V1-RPC-003 | `Metrics(MetricsRequest)` | Empty request → timestamp/node identity, one `NodeMetrics`, repeated `ContainerMetrics`. | Useful parity |
| V1-RPC-004 | `ListImages(ListImagesRequest)` | Empty request → repeated image metadata plus responding node identity. | Useful parity |
| V1-RPC-005 | `RemoveImage(RemoveImageRequest)` | `{image_id,force}` → repeated `{deleted,untagged}` string identifiers. | Useful parity |
| V1-RPC-006 | `PruneImages(PruneImagesRequest)` | Empty request → `{images_deleted,space_reclaimed}`. | Useful parity |
| V1-RPC-007 | `Exec(stream ExecMessage)` | Bidirectional stream. INIT supplies container/shell/user/size, DATA carries bytes, RESIZE carries size, EXIT carries code, ERROR carries text. | Useful parity; follower divergence is V1-WS-009 |
| V1-RPC-008 | `HealthRequest`, `InfoRequest`, `MetricsRequest`, `ListImagesRequest`, `PruneImagesRequest` | Five distinct empty protobuf request messages. | Useful parity |
| V1-RPC-009 | `HealthResponse` | `status`, `node_id`, `hostname`. | Useful parity |
| V1-RPC-010 | `InfoResponse` | `node_id`, `hostname`, `role`. | Useful parity |
| V1-RPC-011 | `MetricsResponse` | RFC3339 `timestamp`, `node_id`, `hostname`, one `NodeMetrics node`, repeated `ContainerMetrics containers`. | Useful parity |
| V1-RPC-012 | `NodeMetrics` | `cpus`, `cpu_percent`, signed `memory_used`, `memory_total`, `memory_percent`, signed `disk_used`, `disk_total`, `disk_percent`. No network counters exist in the protobuf. | Useful parity |
| V1-RPC-013 | `ContainerMetrics` | `container_id`, `name`, `service_id`, `cpu_percent`, signed `memory_usage`, `memory_limit`, `memory_percent`. | Useful parity |
| V1-RPC-014 | `ListImagesResponse` | `node_id`, `hostname`, repeated `ImageItem images`. | Useful parity |
| V1-RPC-015 | `ImageItem` | `id`, repeated `tags`, signed `size`, signed Unix `created`. | Useful parity |
| V1-RPC-016 | `RemoveImageRequest` / `RemoveImageResponse` | Request `image_id`, `force`; response repeated `deleted` and `untagged`. | Useful parity |
| V1-RPC-017 | `PruneImagesResponse` | Unsigned `space_reclaimed`, signed `images_deleted`. | Useful parity |
| V1-RPC-018 | `ExecMessage` | Enum values: `INIT=0`, `DATA=1`, `RESIZE=2`, `EXIT=3`, `ERROR=4`; fields `type`, `data`, `cols`, `rows`, `exit_code`, `error`, `shell`, `user`. INIT overloads `data` with container ID. | Useful parity |

Internal transport behavior:

| ID | Behavior | Classification | Evidence |
|---|---|---|---|
| V1-RPC-019 | Unary calls require plaintext metadata `x-kontainer-secret`; stream auth is checked manually. Empty configured secret yields `Internal`; absent/wrong value yields `Unauthenticated` using constant-time compare. Transport credentials are insecure. | Bug to fix — do not port | [`secret_auth.go:15`](../../server/app/grpc/interceptors/secret_auth.go#L15), [`grpc_pool.go:15`](../../server/app/services/cluster/grpc_pool.go#L15) |
| V1-RPC-020 | Connections are cached indefinitely by address. Failed discovery health removes one pool entry; a pool-wide `CloseAll` exists but has no lifecycle caller. | Bug to fix — do not port | [`grpc_pool.go:15`](../../server/app/services/cluster/grpc_pool.go#L15) |
| V1-RPC-021 | Discovery scans desired-running tasks for `CLUSTER_FOLLOWER_SERVICE`, selects the first address on the first network attachment, appends `GRPC_PORT`, and performs sequential three-second health calls. Only healthy entries survive each replacement. | Useful parity | [`swarm_discovery.go:41`](../../server/app/services/cluster/swarm_discovery.go#L41) |
| V1-RPC-022 | A task-list or node-list error replaces the registry with empty state. Leader/follower node-ID collisions are possible; registry ordering is nondeterministic. | Bug to fix — do not port | [`registry.go:13`](../../server/app/services/cluster/registry.go#L13) |
| V1-RPC-023 | Static development discovery uses comma-separated `CLUSTER_DEV_FOLLOWERS`; when non-empty it is scheduled every 5s, otherwise Swarm task discovery is every 10s. `APP_ENV` does not select this. | Useful parity |
| V1-RPC-024 | Metrics and image fanout are sequential. Follower errors are logged and omitted; local list-image failure aborts. Calls do not consistently derive deadlines from the action context. | Bug to fix — do not port | [`cluster-communication.md`](cluster-communication.md) |

## Docker and Swarm operation inventory

| ID | Operation | Exact implemented scope/edge behavior | Classification | Evidence |
|---|---|---|---|---|
| V1-DKR-001 | List services | Stack-label filter; counts desired-running tasks and actual running tasks; strips image digest; global services report desired replicas `-1`. | Useful parity | [`services.go:25`](../../server/app/services/docker/swarm/services.go#L25) |
| V1-DKR-002 | Inspect service | Normalizes mode, replicas, env, ports, mounts, constraints, labels, update status, previous image. `rawSpec` is defined but never populated; absent previous image becomes empty string. | Useful parity; shape quirks are V1-BUG-010 | [`services.go`](../../server/app/services/docker/swarm/services.go) |
| V1-DKR-003 | Scale service | Rejects global services; sets replicated count. | Useful parity |
| V1-DKR-004 | Restart service | Increments Swarm task-template `ForceUpdate`. | Useful parity |
| V1-DKR-005 | Roll back service | Requests rollback to `previous` without verifying a previous spec exists. | Useful parity; missing precondition is V1-BUG-011 |
| V1-DKR-006 | Replace environment | Replaces all env entries by concatenating `key=value`; no key/value validation or merge semantics. | Useful parity; validation semantics need reconsideration |
| V1-DKR-007 | Replace image | Replaces image and updates service; no registry-auth forwarding. | Useful parity; private image limitation is V1-BUG-012 |
| V1-DKR-008 | List/inspect tasks | Optional service/node filters; best-effort service/node names with ID fallback; image digest stripped; exit code included only for terminal states. Detail adds PID, env, mounts, and networks. | Useful parity | [`tasks.go:16`](../../server/app/services/docker/swarm/tasks.go#L16) |
| V1-DKR-009 | Container stats | Only desired/running tasks on the leader's local Docker engine; sequential per-container stats; individual failures skipped/logged. Uses raw memory usage and can unsigned-underflow CPU deltas. | Useful parity; math/scope defects are V1-BUG-013 | [`tasks.go`](../../server/app/services/docker/swarm/tasks.go) |
| V1-DKR-010 | Aggregate task states | Counts total, running, failed, preparing, complete, shutdown, desired-running; several Swarm states collapse into these buckets. | Useful parity |
| V1-DKR-011 | List/update nodes | Lists node identity/status/availability/role/engine/platform/IP/leader/time; raw Swarm updates change availability or role. | Useful parity | [`nodes.go`](../../server/app/services/docker/swarm/nodes.go) |
| V1-DKR-012 | List stacks | Synthesizes stacks only from `com.docker.stack.namespace`; unlabeled services are omitted; global desired count is zero; ordering is nondeterministic. | Useful parity | [`stacks.go:15`](../../server/app/services/docker/swarm/stacks.go#L15) |
| V1-DKR-013 | Remove stack | Sequentially removes matching services, networks, secrets, configs; stops on first error; leaves volumes; zero matches is reported as success. | Bug to fix — do not port | [`stacks.go`](../../server/app/services/docker/swarm/stacks.go) |
| V1-DKR-014 | List networks | Lists driver/scope/flags/timestamps; `services` is always empty and IPAM/subnet is absent. | Useful parity; placeholder fields are V1-BUG-018 | [`networks.go:11`](../../server/app/services/docker/swarm/networks.go#L11) |
| V1-DKR-015 | List volumes | Leader's local engine only; size/ref-count comes from optional usage data; absence uses `-1`; stack label is projected. | Useful parity | [`volumes.go:11`](../../server/app/services/docker/swarm/volumes.go#L11) |
| V1-DKR-016 | Secrets | List metadata, create plaintext payload, remove. Secret contents are intentionally unavailable after creation. | Useful parity | [`secrets.go`](../../server/app/services/docker/swarm/secrets.go) |
| V1-DKR-017 | Configs | List metadata/size, inspect with base64 data, create, remove. | Useful parity | [`configs.go`](../../server/app/services/docker/swarm/configs.go) |
| V1-DKR-018 | Images | Local list/remove/prune plus leader fanout to followers. Image list associates node ID/hostname. | Useful parity | [`images.go`](../../server/app/services/docker/swarm/images.go) |
| V1-DKR-019 | Logs | Service/task Docker log streaming with multiplex decoding described in V1-WS-001–004. | Useful parity |
| V1-DKR-020 | Exec | Docker TTY create/attach/start/resize with stdin/output streaming. `InspectExec` is implemented but unused and absent from the public orchestrator interface. | Useful parity | [`exec.go:42`](../../server/app/services/docker/swarm/exec.go#L42) |
| V1-DKR-021 | Agent-owned resource mutations | No create/deploy/update/remove implementation for services beyond the five mutations above, networks, volumes, or stack deployment. | Intentionally deferred | [`docs/missing.md`](../missing.md) |

See [Swarm resources](swarm-resources.md) and [stack deployment](stack-deployment.md) for resource semantics and proposed v2 boundaries.

## Scheduled work and metric persistence

| ID | Item | Exact v1 behavior | Classification | Evidence |
|---|---|---|---|---|
| V1-JOB-001 | Cluster discovery | Leader only; every 5s when `CLUSTER_DEV_FOLLOWERS` is non-empty, otherwise every 10s; `SkipIfStillRunning`, background context. | Useful parity | [`console/kernel.go:27`](../../server/app/console/kernel.go#L27) |
| V1-JOB-002 | Metrics collection | Leader only, hard-coded every minute, `SkipIfStillRunning`, background context. Local sample first, then followers sequentially with five-second calls. | Useful parity | [`console/kernel.go`](../../server/app/console/kernel.go) |
| V1-JOB-003 | Metrics prune | Leader only, every five minutes, one cutoff delete from unified metrics table. | Useful parity |
| V1-JOB-004 | Scheduler registration | Failure to resolve an earlier dependency returns from `Schedule` and suppresses registration of later jobs. | Bug to fix — do not port |
| V1-JOB-005 | Per-node collection transaction | Inserts node row then container rows in one transaction. Bad follower timestamps become now; signed protobuf byte values are cast to unsigned model fields; a container collection error can discard the node sample. | Bug to fix — do not port | [`collect_and_store.go`](../../server/app/actions/metrics/collect_and_store.go) |
| V1-JOB-006 | Cluster latest query | Node rows newer than two minutes, limit 100, deduplicated through an unordered map; then N+1 container queries limit 500 per node. History limit is 5,000. | Useful parity; scale/order behavior is V1-BUG-014 | [`get_cluster_metrics.go`](../../server/app/actions/metrics/get_cluster_metrics.go) |
| V1-JOB-007 | Node latest query | Exact node ID, freshness cutoff two minutes, otherwise not found; recent related container samples returned. | Useful parity |
| V1-JOB-008 | Metrics interval config | `METRICS_INTERVAL` defaults 30 but the scheduler ignores it and always runs once/minute. | Bug to fix — do not port | [`config/metrics.go:7`](../../server/config/metrics.go#L7) |
| V1-JOB-009 | Metrics retention config | Config defines `metrics.retention`; provider reads nonexistent `metrics.retention_hours` and falls back to 24 hours, so `METRICS_RETENTION` is ineffective. | Bug to fix — do not port | [`metrics_service_provider.go`](../../server/app/providers/metrics_service_provider.go), [`config/metrics.go`](../../server/config/metrics.go) |

## Persistence inventory

The configured store is SQLite at `storage/kontainer.db` by default. SQL migrations are not automatically run on ordinary process startup; see [agent lifecycle](agent-lifecycle.md).

| ID | Table/store | Persisted shape and usage | Classification | Evidence |
|---|---|---|---|---|
| V1-DATA-001 | `users` | Numeric ID; `provider`, `provider_id` unique pair/identity, name, nullable email/avatar columns, timestamps. The Go model and all desktop/wire DTOs normalize email/avatar to non-null strings. OAuth login upserts by provider identity. | Useful parity | [`app/models/user.go`](../../server/app/models/user.go), [`20260312062757_create_users_table.go`](../../server/database/migrations/20260312062757_create_users_table.go) |
| V1-DATA-002 | `app_configs` | Primary string key and text value. Stores GitHub App credentials, JWT secret, and setup state in plaintext. | Useful parity; plaintext secrets are V1-BUG-015 | [`app/models/app_config.go`](../../server/app/models/app_config.go) |
| V1-DATA-003 | App-config keys | `github_app_id`, `github_client_id`, `github_client_secret`, `github_private_key`, `github_webhook_secret`, `github_org`, `jwt_secret`, `setup_complete`. | Useful parity | [`setup`](../../server/app/services/setup) |
| V1-DATA-004 | `settings` | Arbitrary string `key` primary key and `value`, timestamps; exposed wholesale through the agent settings API. | Backend-only / no UI | [`app/models/setting.go`](../../server/app/models/setting.go) |
| V1-DATA-005 | `metrics` | Wide table keyed by row ID and `kind` (`node`/`container`): timestamp, node/subject identity, CPU/memory, CPU count, disk, service ID, framework timestamps, and indexes on `(kind,node_id,timestamp)` and `(kind,timestamp)`. Irrelevant node/container dimensions are stored as zero/empty values rather than normalized child tables. | Useful parity | [`20260320000000_create_metrics_table.go:15`](../../server/database/migrations/20260320000000_create_metrics_table.go#L15) |
| V1-DATA-006 | Destructive metrics migration | Unified migration drops old `node_metrics` without copying it. | Bug to fix — do not port | [`20260320000000_create_metrics_table.go`](../../server/database/migrations/20260320000000_create_metrics_table.go) |
| V1-DATA-007 | `jobs`, `failed_jobs` | Goravel queue scaffold; default queue is synchronous and no domain job dispatch was found. | Obsolete/deleted historical behavior to reconsider | [`20210101000001_create_jobs_table.go`](../../server/database/migrations/20210101000001_create_jobs_table.go) |
| V1-DATA-008 | `profiles.json` | Tauri store: `profiles[]` and `activeProfileId`; each profile contains ID, label, agent URL, user identity/provider, full JWT, and expiry in plaintext. | Useful parity; storage risk is V1-BUG-016 | [`load_profiles.rs`](../../app/src-tauri/src/actions/auth/load_profiles.rs) |
| V1-DATA-009 | `session.json` | Legacy single-session Tauri store read only for migration/compatibility; converted to a default profile then cleared. | Obsolete/deleted historical behavior to reconsider | [`load_profiles.rs:25`](../../app/src-tauri/src/actions/auth/load_profiles.rs#L25) |
| V1-DATA-010 | `settings.json` | Tauri app settings: refresh interval; terminal shell/font size/font family/scrollback; font; color scheme. Defaults include 5s refresh. | Useful parity | [`app/src-tauri/src/actions/app`](../../app/src-tauri/src/actions/app) |
| V1-DATA-011 | Migration/DB operation | SQL migrations, SQLite connection, maximum 100 open connections; no backup/restore or schema compatibility protocol. | Useful parity; lifecycle risk belongs to v2 lifecycle report | [`config/database.go`](../../server/config/database.go) |
| V1-DATA-012 | Removed `node_metrics` | Historical current-tree migration created timestamp, node/hostname, CPUs, CPU/memory/disk totals/percentages plus timestamp/node indexes. The later unified migration deletes it without copying data. | Obsolete/deleted historical behavior to reconsider | [`20260312062903_create_node_metrics_table.go`](../../server/database/migrations/20260312062903_create_node_metrics_table.go) |
| V1-DATA-013 | Tauri window state | `tauri-plugin-window-state` is registered and persists native window geometry/state in plugin-owned storage; no Kontainer schema or migration code controls it. | Useful parity only at desktop-shell outcome level | [`app/src-tauri/src/lib.rs:8`](../../app/src-tauri/src/lib.rs#L8) |

## Configuration and environment inventory

Defaults below are code defaults, not promises in prose. Values in `.env.example`, deployment docs, and runtime code do not always agree.

| ID | Variable/config | Current consumption/default | Classification |
|---|---|---|---|
| V1-CFG-001 | `APP_NAME` | Goravel app name, default `Goravel`; not the `/ping` app identifier. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-002 | `APP_ENV` | Framework environment label, default `production`; no current Kontainer domain branch reads it (`.env.example` uses `local`). | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-003 | `APP_DEBUG` | Framework debug flag, default false. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-004 | `APP_KEY` | Framework key, default empty; does not encrypt the domain secrets in `app_configs`. | Bug to fix — do not port |
| V1-CFG-005 | `APP_URL` | Default `http://localhost`; used by framework/filesystem URL generation, not cluster discovery. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-006 | `APP_HOST` | HTTP bind host, default `127.0.0.1`. | Useful parity |
| V1-CFG-007 | `APP_PORT` | HTTP port, default `3000`. | Useful parity |
| V1-CFG-008 | `AGENT_ROLE` | `leader` or `follower`; any other value coerces to leader. | Useful parity; coercion is a bug |
| V1-CFG-009 | `AGENT_URL` | Externally reachable base used to generate setup and OAuth callback URLs; default `http://localhost:3000`. | Useful parity |
| V1-CFG-010 | `CLUSTER_SECRET` | Shared plaintext gRPC secret, default empty. | Useful parity; security mechanism is V1-RPC-019 |
| V1-CFG-011 | `CLUSTER_FOLLOWER_SERVICE` | Swarm service name used for follower task discovery, default `kontainer-agent`. | Useful parity |
| V1-CFG-012 | `CLUSTER_DEV_FOLLOWERS` | Comma-separated development follower addresses. | Useful parity |
| V1-CFG-013 | `GRPC_HOST` | gRPC bind host, default empty. | Useful parity |
| V1-CFG-014 | `GRPC_PORT` | gRPC bind/discovery port, code default empty even though deployment prose says `9000`. Empty values can produce malformed advertised/discovered addresses. | Bug to fix — do not port |
| V1-CFG-015 | `DB_CONNECTION` | Database driver, default `sqlite`. | Useful parity |
| V1-CFG-016 | `DB_DATABASE` | Database path, default `storage/kontainer.db`. | Useful parity |
| V1-CFG-017 | `JWT_SECRET` | Present in JWT config, but runtime user-guard secret is hot-loaded from `app_configs.jwt_secret`; configuration paths are inconsistent. | Bug to fix — do not port |
| V1-CFG-018 | `JWT_TTL` | JWT lifetime in minutes, default 1440. | Useful parity |
| V1-CFG-019 | `JWT_REFRESH_TTL` | Default 0; no refresh endpoint or desktop refresh flow exists. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-020 | `KONTAINER_SETUP_TOKEN` | Optional setup bootstrap token; otherwise a new `ktsetup_` plus 12 hex digits is generated every boot and logged in full. | Useful parity; entropy/logging are bugs |
| V1-CFG-021 | `METRICS_INTERVAL` | Default 30; currently ineffective (V1-JOB-008). | Bug to fix — do not port |
| V1-CFG-022 | `METRICS_RETENTION` | Default 1440; currently ineffective (V1-JOB-009). | Bug to fix — do not port |
| V1-CFG-023 | `DOCKER_HOST` | Consumed implicitly by Docker SDK; dev fixture points agents at unauthenticated DinD TCP endpoints. | Useful parity |
| V1-CFG-024 | `DOCKER_TLS_VERIFY` | Docker SDK `FromEnv` input when supplied. No Kontainer-specific validation. | Useful parity |
| V1-CFG-025 | `DOCKER_CERT_PATH` | Docker SDK `FromEnv` input when supplied. | Useful parity |
| V1-CFG-026 | `DOCKER_API_VERSION` | Docker SDK environment negotiation input when supplied. | Useful parity |
| V1-CFG-027 | `HOST_PROC` | Automatically set to `/host/proc` if mounted; used for host metric collection. | Useful parity |
| V1-CFG-028 | `HTTP_CLIENT_DEFAULT` | Goravel HTTP-client default connection name. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-029 | `HTTP_CLIENT_BASE_URL` | Goravel outbound client base URL. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-030 | `HTTP_CLIENT_TIMEOUT` | Goravel outbound client timeout. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-031 | `HTTP_CLIENT_MAX_IDLE_CONNS` | Goravel transport tuning. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-032 | `HTTP_CLIENT_MAX_IDLE_CONNS_PER_HOST` | Goravel transport tuning. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-033 | `HTTP_CLIENT_MAX_CONN_PER_HOST` | Goravel transport tuning. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-034 | `HTTP_CLIENT_IDLE_CONN_TIMEOUT` | Goravel transport tuning. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-035 | `SESSION_LIFETIME` | Framework cookie session lifetime; OAuth bridge uses its own in-memory ten-minute lifetime. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-036 | `SESSION_EXPIRE_ON_CLOSE` | Framework cookie session option; not the Tauri profile session. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-037 | `SESSION_GC_INTERVAL` | Framework cookie session option. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-038 | `SESSION_COOKIE` | Framework cookie name. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-039 | `SESSION_PATH` | Framework cookie path. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-040 | `SESSION_DOMAIN` | Framework cookie domain. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-041 | `SESSION_SECURE` | Framework cookie secure flag. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-042 | `SESSION_HTTP_ONLY` | Framework cookie HTTP-only flag. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-043 | `SESSION_SAME_SITE` | Framework cookie same-site flag. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-044 | `MAIL_HOST` | Unused Goravel mail scaffold. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-045 | `MAIL_PORT` | Unused Goravel mail scaffold. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-046 | `MAIL_FROM_ADDRESS` | Unused Goravel mail scaffold. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-047 | `MAIL_FROM_NAME` | Unused Goravel mail scaffold. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-048 | `MAIL_USERNAME` | Unused Goravel mail scaffold. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-049 | `MAIL_PASSWORD` | Unused Goravel mail scaffold. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-050 | `MAIL_TEMPLATE_ENGINE` | Unused Goravel mail scaffold. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-051 | `MAIL_VIEWS_PATH` | Unused Goravel mail scaffold. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-052 | `QUEUE_CONNECTION` | Default synchronous Goravel queue; no product jobs use it. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-053 | `LOG_CHANNEL` | Goravel logging channel. | Useful parity at outcome level |
| V1-CFG-054 | `LOG_LEVEL` | Goravel logging level. | Useful parity at outcome level |
| V1-CFG-055 | CORS/server limits | CORS config has empty paths, while WebSocket origin checks allow all. HTTP body/header limit is 4096 KB and request timeout is 3s. | Bug to fix — do not port |
| V1-CFG-056 | Removed `*_FILE` support | Commit `5d3cae1` added `_FILE` resolution for cluster secret, JWT secret, and DB path; `571b0e4` removed it as deployment-layer responsibility while deployment prose retained the claim. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-057 | App timezone/locales | Static `UTC`, locale `en`, fallback locale `en`. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-058 | Auth guard/provider | Default `user` guard uses JWT and the ORM-backed `user` provider; setup later injects its signing secret at `auth.guards.user.secret`. | Useful parity |
| V1-CFG-059 | Cache | In-memory default store with `<app.name>_cache` prefix; no product cache use was found. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-060 | Database pool/migrations | Idle/open limits 10/100, one-hour idle/lifetime, 200ms slow threshold; SQL migration driver and `migrations` repository table. | Useful parity only as current operational evidence |
| V1-CFG-061 | Filesystems | Framework local disk at `storage/app` and public disk at `storage/app/public`; no product storage action uses them. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-062 | Hashing | Bcrypt default with 12 rounds; Argon2id alternative configured at 64MiB/time 4/thread 1. No product password/hash flow uses either. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-063 | Logging channels | Default stack→daily, seven retained days at `storage/logs/goravel.log`; single channel is also defined, text format, no stdout printing. | Useful parity only at observability outcome level |
| V1-CFG-064 | Queue connections | Sync default; unused database queue is SQLite, queue `default`, concurrency 1; failures target `failed_jobs`. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-065 | Framework sessions | File driver under `storage/framework/sessions`; these cookies are separate from JWT/Tauri profiles and no domain flow relies on them. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-066 | gRPC client config | Framework `grpc.servers` map is empty; follower clients are created ad hoc by `GrpcPool`, not from this map. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-067 | HTTP/TLS | Gin driver is selected; TLS host/port mirror HTTP but certificate/key are empty, so no configured agent TLS termination exists. | Useful parity as a deployment limitation |
| V1-CFG-068 | DinD `DOCKER_TLS_CERTDIR` | Development Docker containers set it to empty, explicitly disabling automatic DinD TLS. It is a fixture variable, not consumed by the agent. | Useful parity as fixture evidence only |
| V1-CFG-069 | Build `GO111MODULE`, `CGO_ENABLED` | Server image build sets `on` and `0`; these are build-time only, not runtime agent knobs. | Obsolete/deleted historical behavior to reconsider |
| V1-CFG-070 | Workflow `REGISTRY`, `IMAGE_NAME` | Stale image workflow targets `ghcr.io/prvious/kontainer-agent`; GitHub event/token variables are workflow inputs, not runtime configuration. | Obsolete/deleted historical behavior to reconsider |

## Tauri desktop invocation inventory

Every registered command is listed. The definitive registration list is [`app/src-tauri/src/lib.rs:9-60`](../../app/src-tauri/src/lib.rs#L9). HTTP actions create a new `reqwest::Client` per call, generally use 5/10/30-second timeouts, and expose raw response bodies in opaque strings on failure.

| ID | Invocation | Role and parity status | Classification |
|---|---|---|---|
| V1-IPC-001 | `auth_listen` | Spawns `/auth/listen` WebSocket and emits `auth://session-id`, `auth://token`, or `auth://error`; React opens the provider URL after the session event. | Useful parity |
| V1-IPC-002 | `parse_auth_token` | Locally decodes JWT claims without server validation. | Useful parity |
| V1-IPC-003 | `save_session` | Legacy compatibility wrapper that now upserts a default profile by matching agent URL (or deterministic user/URL hash) and activates it. | Obsolete/deleted historical behavior to reconsider |
| V1-IPC-004 | `load_session` | Legacy compatibility wrapper that projects the active profile as one session, falling back to old `session.json`. | Obsolete/deleted historical behavior to reconsider |
| V1-IPC-005 | `delete_session` | Deletes the currently active profile; does not revoke JWT. | Useful parity; naming/logout semantics are defective |
| V1-IPC-006 | `load_profiles` | Loads profiles and migrates legacy `session.json`. | Useful parity |
| V1-IPC-007 | `save_profile` | Upserts only by profile ID and makes it active. | Useful parity |
| V1-IPC-008 | `delete_profile` | Removes a profile from local storage. | Useful parity |
| V1-IPC-009 | `set_active_profile` | Changes active ID without revalidation. | Useful parity; validation/cache issues are V1-BUG-020 |
| V1-IPC-010 | `fetch_providers` | Calls `/auth/providers`; React intentionally skips it and hardcodes GitHub. | Backend-only / no UI |
| V1-IPC-011 | `ping_agent` | Calls public `/ping`. | Useful parity |
| V1-IPC-012 | `check_auth` | Calls `/api/me`. | Useful parity |
| V1-IPC-013 | `setup_verify_token` | Calls public `/setup/status`; therefore cannot actually verify token. | Bug to fix — do not port |
| V1-IPC-014 | `setup_github_connect` | Requests and returns `{setupUrl}`; React then opens it externally. | Useful parity |
| V1-IPC-015 | `setup_complete` | Calls protected completion. | Useful parity |
| V1-IPC-016 | `list_services` | Calls service list, optional stack. | Useful parity |
| V1-IPC-017 | `inspect_service` | Calls service detail. | Useful parity |
| V1-IPC-018 | `list_service_tasks` | Calls route-filtered tasks. | Useful parity |
| V1-IPC-019 | `inspect_task` | Calls task detail. | Useful parity |
| V1-IPC-020 | `list_stacks` | Calls stack list. | Useful parity |
| V1-IPC-021 | `remove_stack` | Calls stack delete, but expects incompatible `{removed:number}`. | Bug to fix — do not port |
| V1-IPC-022 | `list_networks` | Calls network list. | Useful parity |
| V1-IPC-023 | `list_volumes` | Calls volume list. | Useful parity |
| V1-IPC-024 | `list_nodes` | Calls node list. | Useful parity |
| V1-IPC-025 | `list_node_tasks` | Calls route-filtered tasks. | Useful parity |
| V1-IPC-026 | `list_task_stats` | Calls aggregate singular `/api/task-stats`, not container `/api/tasks/stats`. | Useful parity |
| V1-IPC-027 | `list_tasks` | Calls task list. | Useful parity |
| V1-IPC-028 | `scale_service` | Uses `PATCH /api/services/{id}` instead of server's `POST …/scale`. | Bug to fix — do not port |
| V1-IPC-029 | `restart_service` | Calls restart. | Useful parity |
| V1-IPC-030 | `rollback_service` | Calls rollback. | Useful parity |
| V1-IPC-031 | `update_service_env` | Mutation succeeds server-side, but client expects an `env` response the server does not return. | Bug to fix — do not port |
| V1-IPC-032 | `update_service_image` | Mutation succeeds server-side, but client expects an `image` response the server does not return. | Bug to fix — do not port |
| V1-IPC-033 | `update_node_availability` | Uses `PUT` while server requires `PATCH`. | Bug to fix — do not port |
| V1-IPC-034 | `update_node_role` | Uses `PUT` while server requires `PATCH`. | Bug to fix — do not port |
| V1-IPC-035 | `list_secrets` | Calls secret list. | Useful parity |
| V1-IPC-036 | `create_secret` | Server creates secret, then client decode fails because it expects `id` plus `name`. | Bug to fix — do not port |
| V1-IPC-037 | `remove_secret` | Server removes secret, then client decode fails because it expects `{removed:bool}` instead of `{status}`. | Bug to fix — do not port |
| V1-IPC-038 | `list_configs` | Calls config list. | Useful parity |
| V1-IPC-039 | `create_config` | Server creates config, then client decode fails because it expects `id` plus `name`. | Bug to fix — do not port |
| V1-IPC-040 | `remove_config` | Server removes config, then client decode fails because it expects `{removed:bool}` instead of `{status}`. | Bug to fix — do not port |
| V1-IPC-041 | `inspect_config` | Calls config detail; UI displays base64 payload as if plain text. | Useful parity; display defect is V1-BUG-022 |
| V1-IPC-042 | `list_cluster_metrics` | Calls cluster metrics but does not expose server `range`. | Useful parity |
| V1-IPC-043 | `list_node_metrics` | Calls node latest metrics. | Useful parity |
| V1-IPC-044 | `list_images` | Calls cluster image fanout. | Useful parity |
| V1-IPC-045 | `remove_image` | Calls targeted image removal. | Useful parity |
| V1-IPC-046 | `prune_images` | Calls prune but does not let the desktop select `nodeId`; always cluster-wide. | Useful parity; missing control is a frontend gap |
| V1-IPC-047 | `get_agent_settings` | Calls agent settings; hooks exist but no screen uses them. | Backend-only / no UI |
| V1-IPC-048 | `update_agent_settings` | Calls arbitrary agent settings upsert; hooks exist but no screen uses them. | Backend-only / no UI |
| V1-IPC-049 | `close_splashscreen` | Shows main window and closes splash. | Useful parity |
| V1-IPC-050 | `load_app_settings` | Deserializes the whole `appSettings` object from `settings.json`; missing/malformed/incompatible data falls back to the whole default object. | Useful parity |
| V1-IPC-051 | `save_app_settings` | Persists app/terminal/appearance settings. | Useful parity; clobber behavior is V1-BUG-021 |

There is no desktop command for V1-HTTP-013 (`/api/agents`) or V1-HTTP-024 (`/api/tasks/stats`). Logs and exec deliberately have no Tauri command and use direct WebSockets.

## Wire DTO and frontend-visible field inventory

Go wire structs originate in [`server/app/services/docker/orchestrator.go`](../../server/app/services/docker/orchestrator.go), Rust response types live beside the individual commands in [`app/src-tauri/src/actions/agent`](../../app/src-tauri/src/actions/agent), and TypeScript-facing types are distributed under [`app/src`](../../app/src). [API protocol](api-protocol.md) has the field-by-field cross-language comparison.

| ID | DTO/family | Canonical v1 server fields | Classification |
|---|---|---|---|
| V1-DTO-001 | Current user | Envelope `{user,status}`; user fields `id`, `name`, `email`, `avatarUrl`, `provider`; status is `ok`. | Useful parity |
| V1-DTO-002 | Agent registry | Envelope `{agents}`; item fields `nodeId`, `hostname`, internal `address`, `role`, `status`, `lastSeen`. | Backend-only / no UI |
| V1-DTO-003 | Service summary | `id`, `name`, `image`, `stack`, `replicasRunning`, `replicasDesired`, `updatedAt`. Global desired is `-1`. | Useful parity |
| V1-DTO-004 | Service detail | `id`, `name`, `image`, `stack`, `mode`, nullable `replicas`, `createdAt`, `updatedAt`, `env[{key,value}]`, `ports`, `mounts`, `constraints`, `labels`, `updateStatus`, `previousImage`, optional `rawSpec`. | Useful parity; `rawSpec`/empty previous mismatch is V1-BUG-010 |
| V1-DTO-005 | Task summary | `id`, `serviceId`, `serviceName`, `nodeId`, `nodeName`, `state`, `desiredState`, `timestamp`, `containerId`, `image`, `slot`, nullable `exitCode`, `error`. | Useful parity |
| V1-DTO-006 | Task detail | Summary fields plus `pid`, `createdAt`, `updatedAt`, `env`, `mounts`, `networks`. | Useful parity |
| V1-DTO-007 | Live container stats | `containerId`, `name`, `serviceId`, `nodeId`, `cpuPercent`, signed `memUsage`, `memLimit`, `memPercent`. No Tauri command or frontend DTO consumes `/api/tasks/stats`. | Backend-only / no UI |
| V1-DTO-008 | Aggregate task stats | `total`, `running`, `failed`, `preparing`, `complete`, `shutdown`, `desiredRunning`; Rust/TS loses `shutdown`. | Bug to fix — do not port |
| V1-DTO-009 | Node | `id`, `hostname`, `status`, `availability`, `role`, `engineVersion`, `os`, `architecture`, `ip`, `leader`, `updatedAt`. | Useful parity |
| V1-DTO-010 | Stack | `name`, `services`, `runningReplicas`, `desiredReplicas`, `updatedAt`. | Useful parity |
| V1-DTO-011 | Network | `id`, `name`, `driver`, `scope`, `internal`, `attachable`, `ingress`, `services[{id,name}]`, `createdAt`. TS additionally anticipates optional `subnet`, never supplied. | Useful parity; placeholder mismatch is V1-BUG-018 |
| V1-DTO-012 | Volume | `name`, `driver`, `mountpoint`, `scope`, `labels`, `options`, `createdAt`, `refCount`, `sizeBytes`, `stack`. | Useful parity |
| V1-DTO-013 | Secret | `id`, `name`, `labels`, `createdAt`, `updatedAt`; create returns only `name`. | Useful parity; create adapter mismatch is V1-IPC-036 |
| V1-DTO-014 | Config | `id`, `name`, `labels`, `dataSize`, `createdAt`, `updatedAt`; detail adds base64 `data`; create returns only `name`. | Useful parity; adapter/display mismatches are V1-IPC-039/V1-BUG-022 |
| V1-DTO-015 | Cluster images | Top-level `nodes[]`; each group has `nodeId`, `hostname`, `images[]`; each image has `id`, `tags`, `size`, Unix `created`, `nodeId`, `hostname`. | Useful parity |
| V1-DTO-016 | Stored metric response/node metric | Response `{agents,history}` of snapshots `{timestamp,nodeId,hostname,node,containers}`. `node` has `cpus`, `cpuPercent`, `memoryUsed`, `memoryTotal`, `memoryPercent`, `diskUsed`, `diskTotal`, `diskPercent`. | Useful parity |
| V1-DTO-017 | Stored container metric | `containerId`, `name`, `serviceId`, `cpuPercent`, signed `memoryUsed`, `memoryLimit`, `memoryPercent`. Rust instead requires `serviceName`, unsigned memory, and adds defaulted `createdAt=0`, causing non-empty container snapshots to fail deserialization before the default matters. | Bug to fix — do not port |
| V1-DTO-018 | Settings | Envelope `{settings:{key:value}}`; values must be strings but keys have no typed schema or domain validation. | Backend-only / no UI |
| V1-DTO-019 | WebSocket log frame | `stream:"stdout"\|"stderr"`, optional timestamp string, `line`. | Useful parity |
| V1-DTO-020 | WebSocket exec frames | Binary raw I/O plus text error/exit/resize controls described in V1-WS-006/007. | Useful parity |
| V1-DTO-021 | Client-only service/task projections | Rust invents a defaulted `user` only for service-task rows; TypeScript drops some server detail fields including service labels/raw spec and task-detail `serviceName`/`timestamp`; these are not server contracts. | Frontend-only / no backend |
| V1-DTO-022 | Update status | `state`, `startedAt`, `completedAt`, `message`; nullable on service detail. | Useful parity |
| V1-DTO-023 | Port configuration | `protocol`, `targetPort`, `publishedPort`, `publishMode`. | Useful parity |
| V1-DTO-024 | Mount | `type`, `source`, `target`, `readOnly`. | Useful parity |
| V1-DTO-025 | Task network attachment | `name`, `addresses[]`. | Useful parity |
| V1-DTO-026 | Image mutation results | Remove: `deleted[]`, `untagged[]`; prune: `spaceReclaimed`, `imagesDeleted`. Cluster prune sums successful nodes. | Useful parity |
| V1-DTO-027 | Discovery/setup/auth responses | Ping `{app,status}`; setup status `{step}`; manifest `{setupUrl}`; completion `{status}`; providers `{providers}`; auth listener `{sessionId}` then `{token}` or `{error}`. | Useful parity |
| V1-DTO-028 | Mutation response families | Scale `{replicas}`; restart/rollback/env/image/stack/secret/config deletion `{status}`; node mutations `{availability}` or `{role}`; secret/config create `{name}`. | Useful parity; adapter mismatches are V1-GAP-004–007 |
| V1-DTO-029 | Envelopes versus direct objects | Lists use named envelopes (`services`, `tasks`, `stats`, `nodes`, `stacks`, `networks`, `volumes`, `secrets`, `configs`); cluster images use grouped `{nodes}` and aggregate stats uses `{taskStats}`. Service, task, and config detail responses are direct objects. Some Tauri commands unwrap selected list envelopes while others forward them. | Useful parity |
| V1-DTO-030 | Error response families | Most pre-upgrade HTTP failures are `{error:string}`; validation failures are `422 {errors:{field:[messages]}}`; setup gating is `503 {status:"setup_required"}`. OAuth/setup success and failure pages can be HTML, while post-upgrade log/exec/auth-listener failures are WebSocket JSON or close behavior rather than HTTP envelopes. | Useful parity at outcome level; inconsistent channels are V1-BUG-024 |

## Setup, authentication, authorization, and profile flows

| ID | Flow/constraint | Exact v1 behavior | Classification | Evidence |
|---|---|---|---|---|
| V1-FLOW-001 | Setup gate | Global middleware lets `/health`, `/ping`, `/setup/*`, `/auth/*` through. Before completion, every other leader request gets `503 {"status":"setup_required"}`. Followers bypass it. | Useful parity | [`setup_guard.go:19`](../../server/app/http/middleware/setup_guard.go#L19) |
| V1-FLOW-002 | Setup token | Environment override or per-boot 48-bit token, logged in full; middleware reads `X-Kontainer-Setup-Token`, and only manifest creation/completion enforce it. Missing/wrong values return distinct 401 errors. | Useful parity; entropy/exposure/partial enforcement are bugs |
| V1-FLOW-003 | GitHub App manifest | One process-wide state/token/org with no TTL; a new request replaces the previous attempt and restart loses it. Callback does not validate returned state. | Bug to fix — do not port | [`manifest_state.go`](../../server/app/services/setup/manifest_state.go) |
| V1-FLOW-004 | Setup persistence | GitHub App ID/client credentials/private key/webhook secret/org and generated 256-bit-hex JWT secret are stored in one transaction. | Useful parity; plaintext storage is V1-BUG-015 |
| V1-FLOW-005 | Setup completion | Marks DB and in-memory guard complete before validating later JWT/provider registration. Failure leaves the installation complete; setup token/routes remain reusable afterward. | Bug to fix — do not port |
| V1-FLOW-006 | Provider lifecycle | GitHub provider is loaded from DB at startup and hot-registered after setup. `/auth/providers` reflects registry content, but desktop hardcodes GitHub. | Useful parity |
| V1-FLOW-007 | OAuth identity | GitHub scopes request profile/email and optional organization membership. Agent uses profile email when present, otherwise requires a verified primary email; when configured, it checks org membership by login. | Useful parity |
| V1-FLOW-008 | OAuth bridge/state | Desktop listener obtains session ID; provider redirect creates a separate one-use state bound to the supplied session string; callback upserts user, creates JWT, and tries to publish it. Process restart loses bridge/state. | Useful parity; missing session binding/delivery checks are V1-BUG-030 |
| V1-FLOW-009 | JWT middleware | `/api/*` requires authorization; missing returns 401 `authorization required`, bad/expired returns 401 `invalid or expired token`. The whole Authorization header is handed to Goravel parsing. | Useful parity |
| V1-FLOW-010 | Authorization | No roles, policy checks, or per-resource grants: every valid user can call every authenticated route. | Bug to fix — do not port |
| V1-FLOW-011 | Logout/revocation | Logout only removes local active profile. JWT remains valid until expiry; no logout, revocation, or refresh endpoint exists. | Bug to fix — do not port |
| V1-PROFILE-001 | Profile identity | Successful login creates a random UUID profile, so repeated login to the same agent/user creates duplicates. `save_profile` only upserts by this UUID. | Useful parity; duplicate rule is a bug |
| V1-PROFILE-002 | Active constraint | Exactly one local `activeProfileId`; other profiles remain saved. No server-side profile concept exists. | Useful parity |
| V1-PROFILE-003 | Legacy migration | Old session becomes a deterministic 64-bit `DefaultHasher(userId+URL)` profile labeled `Default`, then old session is cleared. The hash is implementation-dependent, not a stable protocol ID. | Obsolete/deleted historical behavior to reconsider |
| V1-PROFILE-004 | Startup restore | Only active profile expiry is checked locally; there is no immediate `/me` revalidation. A 15s status poll eventually detects failure. | Bug to fix — do not port |
| V1-PROFILE-005 | Profile switch | Cluster switcher sets active, applies auth, then clears TanStack Query cache. Saved-profile selection on login uses the same auth switch but does **not** clear cache. Query keys generally omit profile/agent identity. | Bug to fix — do not port | [`auth-context.tsx`](../../app/src/contexts/auth-context.tsx), [`cluster-switcher.tsx`](../../app/src/components/cluster-switcher.tsx) |
| V1-PROFILE-006 | Long-lived streams | Terminal/log providers can outlive profile changes; terminal sessions may continue against old target/credentials while active auth changes. | Bug to fix — do not port |
| V1-PROFILE-007 | Profile editing/removal | URL can be edited without reauthentication. Switching does not reject a non-active expired profile before applying it. Programmatically removing the active profile clears persisted active ID but leaves React user/token state populated; UI normally exposes removal only for inactive profiles. | Bug to fix — do not port |
| V1-PROFILE-008 | App settings isolation | App/terminal/appearance settings are global, not per profile. Agent settings are server-global. | Useful parity |

Security-specific implications are analyzed in [security](security.md); this inventory does not select the v2 mechanism.

## Failure, placeholder, and accidental behavior register

This table consolidates defects referred to above so an implementation checklist cannot accidentally treat them as compatibility requirements.

| ID | Behavior to avoid porting | Classification | Evidence/cross-reference |
|---|---|---|---|
| V1-BUG-001 | HTTP/gRPC health always says `ok`; it is liveness only but named/used like health. | Bug to fix — do not port | V1-HTTP-001, V1-RPC-001; [agent lifecycle](agent-lifecycle.md) |
| V1-BUG-002 | Setup “verify token” calls a public status route and accepts any token. | Bug to fix — do not port | V1-HTTP-003, V1-IPC-013 |
| V1-BUG-003 | Only one in-flight setup manifest, no TTL, replacement races, restart loss. | Bug to fix — do not port | V1-FLOW-003 |
| V1-BUG-004 | GitHub App manifest callback never reads or checks returned `state`; it only requires `code`, then clears the stored state after exchange. | Bug to fix — do not port | [`callback_controller.go:23`](../../server/app/http/controllers/setup/callback_controller.go#L23) |
| V1-BUG-005 | Setup completion becomes durable before all completion checks succeed; token remains reusable. | Bug to fix — do not port | V1-FLOW-005 |
| V1-BUG-006 | JWT secret has two configuration sources; stored GitHub/JWT secrets are plaintext; setup token is low-entropy and logged. | Bug to fix — do not port | V1-CFG-017/V1-CFG-020/V1-DATA-002 |
| V1-BUG-007 | Log `stdout`, `stderr`, and `since` controls are exposed or modeled but ignored. | Bug to fix — do not port | V1-WS-001 |
| V1-BUG-008 | Paused log buffer is unbounded; reconnect can duplicate tail and has race-prone old-socket close handling. | Bug to fix — do not port | V1-WS-004 |
| V1-BUG-009 | Local and remote exec disagree on user, shell, resize validation, and success-exit frames; gRPC EOF fabricates success. | Bug to fix — do not port | V1-WS-007–009 |
| V1-BUG-010 | `rawSpec` is never populated; absent previous image crosses Go→Rust as `Some("")`; language models omit different fields. | Bug to fix — do not port | V1-DTO-004/V1-DTO-021 |
| V1-BUG-011 | Rollback is requested without checking that a previous spec exists. | Bug to fix — do not port | V1-DKR-005 |
| V1-BUG-012 | Image updates cannot forward registry authentication. | Bug to fix — do not port | V1-DKR-007 |
| V1-BUG-013 | Live stats are leader-local, sequential, skip failures silently, can underflow CPU delta, and have no safe CPU-count fallback. | Bug to fix — do not port | V1-DKR-009 |
| V1-BUG-014 | Metric reads have nondeterministic ordering, fixed caps, N+1 queries, and ambiguous freshness semantics. | Bug to fix — do not port | V1-JOB-006; [metrics storage](metrics-storage.md) |
| V1-BUG-015 | OAuth private key/client/webhook/JWT secrets are plaintext in SQLite. | Bug to fix — do not port | V1-DATA-002 |
| V1-BUG-016 | Full user JWTs are plaintext in Tauri store. | Bug to fix — do not port | V1-DATA-008 |
| V1-BUG-017 | Stack deletion is non-atomic partial deletion, omits volumes, and reports nonexistent stacks removed; controller 404 path is therefore unreachable. | Bug to fix — do not port | V1-DKR-013 |
| V1-BUG-018 | Network `services` is always empty and subnet is never supplied, despite UI fields. | Bug to fix — do not port | V1-DKR-014/V1-DTO-011 |
| V1-BUG-019 | Node drain UI begins polling after the mutation and treats disappearance as migrated; fast-moving tasks may never enter its baseline. No replacement-task validation or timeout exists. The current HTTP verb mismatch prevents it earlier. | Bug to fix — do not port | [`node-drain-dialog.tsx`](../../app/src/components/node-drain-dialog.tsx) |
| V1-BUG-020 | Multi-profile query keys omit identity; not every switch clears cache; long-lived streams are not rebound. | Bug to fix — do not port | V1-PROFILE-005/006 |
| V1-BUG-021 | Settings provider writes `font:null`/`scheme:null` and can clobber appearance settings; DOM always adds dark even for light while native scheme changes separately. Theme choice is not persistently exposed in settings UI. | Bug to fix — do not port | [`settings-context.tsx`](../../app/src/contexts/settings-context.tsx) |
| V1-BUG-022 | Config detail displays base64-encoded Docker data as plain text. | Bug to fix — do not port | V1-IPC-041 |
| V1-BUG-023 | Rust builds path segments by interpolation without URL encoding; IDs/names containing reserved characters can target the wrong URL. | Bug to fix — do not port | [`app/src-tauri/src/actions/agent`](../../app/src-tauri/src/actions/agent) |
| V1-BUG-024 | Missing orchestrator/fanout failures are inconsistent: selected routes return dedicated 503 strings, many Docker failures become 500, follower fanout failures disappear, and post-upgrade failures become in-band JSON. | Bug to fix — do not port | [`routes/api.go`](../../server/routes/api.go), [API protocol](api-protocol.md) |
| V1-BUG-025 | Docker log decoder trusts an unbounded uint32 frame length before allocating. | Bug to fix — do not port | [`log_stream.go`](../../server/app/services/websocket/log_stream.go) |
| V1-BUG-026 | Discovery error clears all followers; pooled connections have no lifecycle cleanup; all fanout is sequential. | Bug to fix — do not port | V1-RPC-020/022/024 |
| V1-BUG-027 | Route/response mismatches can mutate Docker successfully and then show desktop failure (env, image, stack, secret, config); blind retry can repeat destructive work. | Bug to fix — do not port | V1-IPC-021/031/032/036/037/039/040 |
| V1-BUG-028 | Current build/deployment instructions and workflow still refer to removed `agent/` paths/context; `_FILE`, gRPC default, and metrics env claims disagree with code. | Bug to fix — do not port | [`build-agent.yml`](../../.github/workflows/build-agent.yml), [`docs/deployment.md`](../deployment.md) |
| V1-BUG-029 | Any unrecognized `AGENT_ROLE` silently becomes `leader`, expanding public surface instead of failing configuration. | Bug to fix — do not port | [`config/cluster.go:10`](../../server/config/cluster.go#L10) |
| V1-BUG-030 | OAuth redirect accepts any non-empty `session` without verifying a live listener; callback ignores `SendToken=false` and still renders authentication success after minting an undelivered JWT. | Bug to fix — do not port | [`redirect_controller.go:34`](../../server/app/http/controllers/auth/redirect_controller.go#L34), [`callback_controller.go:92`](../../server/app/http/controllers/auth/callback_controller.go#L92) |
| V1-BUG-031 | Setup callback treats any non-empty `setup_action` as installed and returns success HTML without checking its value, installation identity, or setup state. | Bug to fix — do not port | [`setup/callback_controller.go:23`](../../server/app/http/controllers/setup/callback_controller.go#L23) |

Normal negative behavior that is not itself a parity feature:

- Missing/invalid JWT returns 401; validation usually returns 422; malformed IDs/queries commonly become 400/500 depending on controller. Missing Docker entities generally become 404 only where controllers map that error.
- If orchestrator construction is unavailable, the generic route fallback is `503 {"error":"docker orchestrator is not available on this agent"}`; agents/images/exec have their own 503 text. These strings are not consistent enough to elevate to v2 contract.
- gRPC handlers collapse many underlying failures to `Internal` while preserving error text. Fanout omits follower errors rather than reporting partial results.
- Volume unknown size/ref-count is `-1`; global desired replicas is `-1`; metrics freshness expiration becomes 404. Ordering and pagination are mostly unspecified.

## Frontend surfaces, placeholders, and deferred affordances

Every generated TanStack route has a corresponding source under [`app/src/routes`](../../app/src/routes):

| ID | Desktop route/screen | Current functional surface | Classification |
|---|---|---|---|
| V1-SCR-001 | `/login` | Agent URL discovery, setup-required branch, GitHub login, and saved-profile selection. | Useful parity |
| V1-SCR-002 | `/` dashboard | Cluster metrics/task aggregate, service/container overview, refresh and placeholder status cards. | Useful parity; placeholders are V1-UI-001–005 |
| V1-SCR-003 | `/services` | List services, navigate to detail, and start service log/exec actions through shared UI; visible filter/sort affordances are incomplete. | Useful parity |
| V1-SCR-004 | `/services/$serviceId` | Inspect, scale, restart, rollback, replace env/image, task list/detail, logs/exec. Several mutations are broken by V1-GAP-001/004. | Useful parity |
| V1-SCR-005 | `/nodes` | List, role update, availability/drain workflow, node tasks. Mutations are broken by V1-GAP-002/003. | Useful parity |
| V1-SCR-006 | `/stacks` | List and remove stacks; deploy affordance is no-op. Remove response mismatch is V1-GAP-005. | Useful parity |
| V1-SCR-007 | `/stacks/$stackName` | Stack-filtered service view and aggregate placeholders. | Useful parity |
| V1-SCR-008 | `/tasks` | List all tasks and open task detail/log/exec. | Useful parity |
| V1-SCR-009 | `/networks` | Read-only network list; subnet/services data is incomplete. | Useful parity |
| V1-SCR-010 | `/volumes` | Read-only local volume list. | Useful parity |
| V1-SCR-011 | `/images` | Grouped cluster image list, per-node remove, cluster-wide prune. | Useful parity |
| V1-SCR-012 | `/secrets` | List, create, remove. Create/remove adapters report failure after successful mutation (V1-GAP-006/007). | Useful parity |
| V1-SCR-013 | `/configs` | List, create, remove. Create/remove adapters report failure after successful mutation. | Useful parity |
| V1-SCR-014 | `/configs/$configId` | Inspect and display config metadata/data; base64 display defect V1-GAP-010. | Useful parity |
| V1-SCR-015 | `/logs` | Up to ten concurrent service-log tabs and stream picker. | Useful parity |
| V1-SCR-016 | `/exec` | Container picker and shared multi-session terminal provider. | Useful parity |
| V1-SCR-017 | `/settings` | General refresh, appearance/font, terminal defaults, and connection/profile editing. Does not expose agent settings. | Useful parity |

The query hooks cover services, service detail/tasks/logs and mutations; all/task/node tasks and stats; nodes/mutations; stacks/removal; networks; volumes; secrets/configs CRUD/detail; images/remove/prune; cluster metrics; agent status/settings; auth/appearance/settings/terminal contexts; and direct log/exec streams. Their query keys generally omit agent/profile identity (V1-PROFILE-005). Functional actions correspond to the useful-parity HTTP/IPC rows above. The following visible or documented affordances have no complete backend behavior.

| ID | Affordance/claim | Actual state | Classification | Evidence |
|---|---|---|---|---|
| V1-UI-001 | Dashboard Traefik card | Static fake service/card. | Frontend-only / no backend | [`docs/missing.md`](../missing.md) |
| V1-UI-002 | Deploy button | No-op; no stack deploy endpoint. | Frontend-only / no backend |
| V1-UI-003 | Network I/O and service ports | Network chart/data is placeholder; missing ports render dash. | Frontend-only / no backend |
| V1-UI-004 | Metrics server filter | Visible affordance without implementation. | Frontend-only / no backend |
| V1-UI-005 | Branch/Docker version/WebSocket status/stream count | Static `main`, hardcoded or placeholder values. | Frontend-only / no backend |
| V1-UI-006 | Create service/deploy stack/add node/create network/create volume/pull image | Buttons are disabled or no-op; corresponding backend mutations do not exist. | Intentionally deferred |
| V1-UI-007 | Filters and sort buttons | Several resource table buttons are no-op. | Frontend-only / no backend |
| V1-UI-008 | Stack CPU/memory | Dash; no stack aggregation contract. | Frontend-only / no backend |
| V1-UI-009 | Network subnet | Dash/optional client field; server never returns IPAM. | Frontend-only / no backend |
| V1-UI-010 | Log OUT/ERR levels and text filter | OUT/ERR are stream labels; text filter is deferred. | Intentionally deferred |
| V1-UI-011 | Exec PID/TTY | Placeholder labels; server does not provide session PID metadata. | Frontend-only / no backend |
| V1-UI-012 | Agent version/uptime | Placeholder; no route/DTO. | Frontend-only / no backend |
| V1-UI-013 | Setup token/help copy | Placeholder uses `kst_live...`, not actual `ktsetup_`; footer mentions nonexistent `/github/app-manifest`. | Bug to fix — do not port | [`setup-wizard.tsx`](../../app/src/components/setup-wizard.tsx) |
| V1-UI-014 | Log tab limit | Logs UI limits active service tabs to ten; hidden tabs remain connected. | Useful parity; connection retention should be reconsidered |
| V1-UI-015 | Exec session limit | Terminal provider de-duplicates by container but does not enforce a reliable backend/session cap. | Frontend-only / no backend |
| V1-UI-016 | Agent settings | API and hooks exist; settings screen only exposes app/profile settings. | Backend-only / no UI |

`docs/features.md` must not be read as an implemented contract. Its unimplemented families are recorded once here:

| ID | Aspirational family | Classification |
|---|---|---|
| V1-ASP-001 | Generic OIDC, deployment tokens, RBAC/roles/policies, session/device management. | Aspirational only |
| V1-ASP-002 | GitHub-triggered deployment pipeline, build/GHCR, deployment history/diff/rollout/actions. | Aspirational only |
| V1-ASP-003 | Secret vault encryption/versioning and richer secret/config workflows. | Aspirational only |
| V1-ASP-004 | Host shell and `/etc/passwd` user discovery. Current exec is container-only. | Aspirational only |
| V1-ASP-005 | Full service/network/volume/stack CRUD and declarative stack deployment. | Aspirational only |
| V1-ASP-006 | Rich log search/filter/levels/throttling/storage. Current logs are live only. | Aspirational only |
| V1-ASP-007 | Per-profile refresh/customization, complete theme/settings control, and agent YAML editing. | Aspirational only |
| V1-ASP-008 | Compose and Kubernetes targets. Current primary/implemented target is Docker Swarm. | Intentionally deferred |

## Removed and historical behavior

Historical entries are not current v1 requirements. They prevent silently forgetting an earlier capability while avoiding accidental resurrection of obsolete transport/framework code.

| ID | Historical behavior | Archaeological finding | Classification | Evidence |
|---|---|---|---|---|
| V1-HIST-001 | Pre-Goravel follower HTTP | `d509d6d^:agent/server/follower.go` exposed `/internal/health`, `/internal/metrics`, `/internal/images`, `/internal/info`, image remove/prune, and exec WebSocket behind `X-Kontainer-Secret`. Current gRPC superseded it. | Obsolete/deleted historical behavior to reconsider |
| V1-HIST-002 | Pre-Goravel route verbs | Old service scale was `PATCH /api/services/{id}` and node mutations were `PUT`, explaining the stale current Rust methods. Current server routes changed without adapters/tests. | Obsolete/deleted historical behavior to reconsider; current mismatch is a bug |
| V1-HIST-003 | Pre-Goravel setup router | `d509d6d^:agent/server/setup.go` swapped to a setup-only router and protected status/manifest/complete with setup token; all other requests got 403. Callback still failed to validate GitHub state despite a comment claiming protection. | Obsolete/deleted historical behavior to reconsider |
| V1-HIST-004 | YAML configuration | `d509d6d^:agent/config/config.go` and `kontainer.example.yml` supported `--config`, role/listen/agent URL/cluster/metrics/dev settings and limited `file:`/`${ENV}` secret resolution. Goravel migration removed it; feature/deployment prose still references it. | Obsolete/deleted historical behavior to reconsider |
| V1-HIST-005 | `_FILE` configuration | Added during Goravel (`5d3cae1`) and deliberately removed (`571b0e4`) as deployment-layer work. | Obsolete/deleted historical behavior to reconsider |
| V1-HIST-006 | Explicit shutdown | Pre-Goravel main handled SIGINT/SIGTERM with five-second leader/follower shutdown. Replacement lacks an equivalent owner for gRPC pool, sessions, scheduler, and HTTP/gRPC shutdown. | Obsolete/deleted historical behavior to reconsider | [agent lifecycle](agent-lifecycle.md) |
| V1-HIST-007 | Service resources branch | Non-ancestor commits `e2053df`, `b9e73f4`, `e8bfbfa` added CPU/memory limit/reservation fields, `PUT /api/services/{id}/resources`, validation (minimum CPU/.001, memory/4MiB, reservation≤limit), Tauri action, editor, and tests. Never merged to current main. | Obsolete/deleted historical behavior to reconsider |
| V1-HIST-008 | Removed Go test suite | Immediately before Goravel replacement, `d509d6d^` contained 55 `*_test.go` files and 269 top-level `Test*`/`Example*` entry points across auth/config/discovery/middleware/Docker handlers/metrics/storage/setup. They were deleted rather than migrated. | Obsolete/deleted historical behavior to reconsider |
| V1-HIST-009 | Old metric storage work | Historical time-series work was superseded by current unified `metrics`; it is not a separate parity contract. | Obsolete/deleted historical behavior to reconsider |
| V1-HIST-010 | Compose/Kubernetes | Mentioned as future targets throughout docs; no prior production implementation found in relevant history. | Intentionally deferred |

## Development fixtures, packaging, and test evidence

| ID | Artifact/coverage | What it actually establishes | Classification | Evidence |
|---|---|---|---|---|
| V1-FIX-001 | Three-node DinD Swarm | Three privileged unpinned `docker:dind` engines, manager TCP 2375 and workers 2376/2378 without TLS/auth, persistent named state. Setup polls each daemon with no overall timeout, initializes one manager/two workers, then seeds stacks. Manual dev fixture, not production behavior. | Useful parity as test fixture only | [`dev/docker-compose.yml`](../../dev/docker-compose.yml), [`dev/setup.sh`](../../dev/setup.sh) |
| V1-FIX-002 | Seeded resources | `webapp` (`web`×3, `api`×2, `worker`×1), `monitoring` (`dashboard`×1, global `collector`), and `toolbox` (`nginx`×2, `alpine`×2, `redis`×1), including labels/resources. | Useful parity as fixture only | [`dev/seed-services.sh`](../../dev/seed-services.sh) |
| V1-FIX-003 | Local multi-agent process fixture | Procfile runs one leader plus two followers against DinD, follower HTTP/gRPC 3001/9001 and 3002/9002, shared `dev-secret`. | Useful parity as fixture only | [`dev/Procfile`](../../dev/Procfile) |
| V1-FIX-004 | Agent build workflow | Workflow still uses removed `agent/` build context/paths, so it is historical intent rather than a verified build. | Bug to fix — do not port | [`build-agent.yml`](../../.github/workflows/build-agent.yml) |
| V1-FIX-005 | Current server image recipe | `server/Dockerfile` runs `go mod tidy`, uses unpinned Alpine runtime, and copies ignored `.env`; a clean checkout has no `.env`, so that `COPY` fails even after correcting workflow context. If supplied, the environment file is baked into the image. | Bug to fix — do not port | [`server/Dockerfile`](../../server/Dockerfile), [`server/.gitignore`](../../server/.gitignore) |
| V1-TEST-001 | Current Go suite | Ten `_test.go` files, 43 test entry points plus `TestMain`, one skipped integration-style user-upsert test. Covers WebSocket parsing/demux/bridge, OAuth provider/registry config, setup token, metrics query suite, interface assertions, and an example smoke test. | Useful parity as evidence only | [reliability/testing](reliability-testing.md) |
| V1-TEST-002 | Missing Go coverage | No systematic current route/controller contract, Docker behavior, gRPC/discovery/auth, setup guard/manifest, scheduler, migration, fanout, or real log/exec integration tests. | Bug to fix — do not port |
| V1-TEST-003 | Frontend suite | 39 Vitest files and 300 `it`/`test` declarations in the audited snapshot; mocked IPC/WebSocket tests cover major screens, hooks, profiles/auth, logs/exec, and mutation states. They validate client assumptions, not Go↔Rust wire compatibility. | Useful parity as evidence only | [`app/tests`](../../app/tests), [reliability/testing](reliability-testing.md) |
| V1-TEST-004 | Rust suite | No Rust unit/integration tests were found for the 51 Tauri commands or DTO decoding. | Bug to fix — do not port |
| V1-TEST-005 | System verification | No E2E, contract, compatibility, coverage, fuzz, migration, failure-injection, or fixture-driven CI suite was found. | Bug to fix — do not port |
| V1-TEST-006 | Historical service-resource tests | Tests existed only on the unmerged resource branch and do not establish current behavior. | Obsolete/deleted historical behavior to reconsider |

No test or formatter command was run for this archaeology task; verification was read-only, as required.

## Cross-layer mismatch matrix

These IDs are the shortest mechanical checklist for incompatibilities that a replacement must not canonize.

| ID | Go server | Rust/Tauri or React | Effect | Classification |
|---|---|---|---|---|
| V1-GAP-001 | `POST /api/services/{id}/scale` | `PATCH /api/services/{id}` | Scale cannot reach current handler. | Bug to fix — do not port |
| V1-GAP-002 | `PATCH …/nodes/{id}/availability` | `PUT` | Availability/drain cannot reach handler. | Bug to fix — do not port |
| V1-GAP-003 | `PATCH …/nodes/{id}/role` | `PUT` | Role mutation cannot reach handler. | Bug to fix — do not port |
| V1-GAP-004 | Env/image update returns `{status}` | Rust expects updated `env`/`image` | Server mutates, desktop reports decode failure. | Bug to fix — do not port |
| V1-GAP-005 | Stack delete returns `{status}` | Rust expects `{removed:number}` | Destructive success appears as failure. | Bug to fix — do not port |
| V1-GAP-006 | Create secret/config returns `{name}` | Rust requires `{id,name}` | Creation succeeds, UI reports failure/retry risk. | Bug to fix — do not port |
| V1-GAP-007 | Delete secret/config returns `{status}` | Rust requires `{removed:bool}` | Deletion succeeds, UI reports failure/retry risk. | Bug to fix — do not port |
| V1-GAP-008 | Container metric has `serviceId`, signed memory | Rust requires `serviceName`, unsigned memory | Non-empty container metrics fail deserialization. | Bug to fix — do not port |
| V1-GAP-009 | Aggregate stats includes `shutdown` | Rust/TS omits it | Information silently disappears. | Bug to fix — do not port |
| V1-GAP-010 | Config data is base64 | UI renders as plain text | User sees encoded content. | Bug to fix — do not port |
| V1-GAP-011 | Setup status is public and token-agnostic | Command/UI calls it “verify token” | Any value appears valid. | Bug to fix — do not port |
| V1-GAP-012 | `/api/agents`, `/api/tasks/stats`, agent settings exist | No reachable desktop UI | Backend-only surfaces can drift unnoticed. | Backend-only / no UI |
| V1-GAP-013 | Logs/exec authenticate directly via WS query | Normal data uses Tauri HTTP adapter | Two auth/network/error paths; URL leaks token. | Bug to fix — do not port |
| V1-GAP-014 | Remote exec proto carries `user`/shell | Follower drops user and forces `/bin/sh` | Local/remote behavior differs. | Bug to fix — do not port |
| V1-GAP-015 | Service has labels/rawSpec; task has serviceName/timestamp; network lacks subnet | Rust/TS shapes diverge | Fields are lost, defaulted, or invented. | Bug to fix — do not port |
| V1-GAP-016 | Metrics accepts `range`; prune accepts `nodeId` | Tauri does not expose either selection | UI can only use defaults/cluster-wide action. | Backend-only / no UI |
| V1-GAP-017 | Current code has no YAML config or `_FILE` helper | Features/deployment docs claim them | Operators receive false configuration guidance. | Obsolete/deleted historical behavior to reconsider |
| V1-GAP-018 | Code default `GRPC_PORT` is empty and metrics envs are ineffective | Deployment docs advertise 9000 and working metric knobs | Deployment can misconfigure discovery/retention. | Bug to fix — do not port |
| V1-GAP-019 | Current source root is `server/` | Build workflow targets `agent/` | Agent image workflow is stale. | Bug to fix — do not port |
| V1-GAP-020 | Query cache is agent-agnostic | Only one of multiple profile-switch paths clears it | Cross-profile data leakage/staleness. | Bug to fix — do not port |

## Nothing-forgotten checklist

This checklist is intentionally redundant with the tables. A master feature map can cite the category IDs and tick each line only after accepting, fixing, deferring, or explicitly rejecting it.

- [x] Every leader/follower public HTTP setup, auth, health, API, log, and exec route registered at current `HEAD` is listed (V1-HTTP-001–050).
- [x] Setup guard, JWT, WebSocket query authentication, origin policy, error channels, and public-route exceptions are recorded (V1-FLOW-001, V1-FLOW-009, V1-WS-010, V1-BUG-024).
- [x] Both WebSocket protocols include query options, frame types, parsing, reconnect/buffering, local/remote divergences, and limits (V1-WS-001–011).
- [x] Every protobuf RPC and every request/response/message/enum family is inventoried (V1-RPC-001–018), with discovery/auth/pool/fanout behavior (V1-RPC-019–024).
- [x] Every method on the current orchestrator capability surface is covered: services, tasks/stats, nodes, stacks, networks, volumes, secrets, configs, images, logs, and exec (V1-DKR-001–021).
- [x] Every scheduled job, cadence, overlap policy, metric ingest/query/prune behavior, and broken metric setting is recorded (V1-JOB-001–009).
- [x] Every product persistence model/table plus removed metric table, local/legacy/plugin store, and unused queue scaffold is recorded (V1-DATA-001–013).
- [x] Every runtime, SDK, fixture, image-build, and workflow environment variable is listed, together with operationally relevant static config, framework-only settings, and removed `_FILE` behavior (V1-CFG-001–070).
- [x] Every command in `tauri::generate_handler!` is listed exactly once (V1-IPC-001–051), and direct WebSockets/no-command backend routes are called out.
- [x] Every frontend-visible server DTO family and all fields are inventoried, including nested structs, envelopes, error families, mutation results, and client-only/missing/defaulted fields (V1-DTO-001–030).
- [x] Setup, GitHub App/OAuth, user upsert, JWT, authorization, logout, profile storage/migration/switching, and single-active-profile constraints are recorded (V1-FLOW-001–011; V1-PROFILE-001–008).
- [x] Known destructive-after-success decode errors, method mismatches, config/doc mismatches, and local/remote exec differences are isolated as bugs (V1-GAP-001–020).
- [x] Every placeholder/disabled/no-op item in `docs/missing.md` and the implemented screen set is represented (V1-UI-001–016).
- [x] Every generated desktop route/screen and its reachable read/mutation/stream surface is mapped (V1-SCR-001–017); hook families trace back to HTTP/IPC/WS IDs.
- [x] Aspirational-only feature families in `docs/features.md`, including non-Swarm orchestrators, are not mislabeled as current behavior (V1-ASP-001–008).
- [x] Pre-Goravel internal HTTP/setup/config/shutdown behavior, `_FILE` removal, removed tests, and unmerged service-resource work are recorded (V1-HIST-001–010).
- [x] DinD topology, seeded stacks, process fixture, workflow/image state, current/historical test coverage, and absence of system/contract/Rust tests are recorded (V1-FIX-001–005; V1-TEST-001–006).
- [x] Focused v2 reports are cross-referenced without copying or superseding their proposed designs.

## Unresolved archaeology questions

These require an explicit product/implementation decision or evidence outside the repository. They are not permission to infer a v2 design.

1. **V1-Q-001 — service resources:** Was the unmerged `feat/service-resources` branch intentionally abandoned, or should CPU/memory limit/reservation editing be treated as product parity despite never reaching main?
2. **V1-Q-002 — setup status authentication:** Was the Goravel change from protected setup status to public status intentional discovery behavior, or an accidental regression that made desktop token verification meaningless?
3. **V1-Q-003 — legacy YAML and `_FILE`:** Are these intentionally owned by packaging/deployment now, and if so which current deployment artifact replaces the stale documentation contract?
4. **V1-Q-004 — backend-only endpoints:** Are `/api/agents`, live `/api/tasks/stats`, and arbitrary `/api/settings` supported public surfaces, internal diagnostics, or unfinished UI work?
5. **V1-Q-005 — metric retention:** Which historical/default policy is desired: configured minutes (`1440`), the provider's accidental 24 hours, or something else? Current code cannot answer.
6. **V1-Q-006 — profile uniqueness:** Should a profile be unique by installation/user, URL/user, or remain an arbitrary saved connection? v1's random IDs and legacy deterministic hash disagree.
7. **V1-Q-007 — stack deletion ownership:** Is retention of named stack volumes deliberate data-safety policy, or an omission? Should a nonexistent stack be idempotent success or not found?
8. **V1-Q-008 — raw service data:** Was `rawSpec` intended as a supported escape hatch, or should the never-populated field be deleted? The same question applies to client-only `subnet`, task `user`, and metric `serviceName`.
9. **V1-Q-009 — follower identity:** Should follower discovery key exclusively by Swarm node ID, task/service identity, or an installation identity? Current task-derived addresses and node-ID registry do not resolve duplicate agents cleanly.
10. **V1-Q-010 — exec success semantics:** Does the desktop need authoritative exit codes for local and remote sessions, or is close-without-error the compatibility outcome? v1 disagrees by path.
11. **V1-Q-011 — GitHub dependence:** Is GitHub the sole supported v2 identity/provider for parity, or should the generic provider registry and `docs/features.md` OIDC aspiration remain a first-class extension point?
12. **V1-Q-012 — test baseline provenance:** The reliability report counts reflect the audited working snapshot. If other concurrent v2 work changes tests, record the commit SHA used for final acceptance rather than treating counts as timeless.

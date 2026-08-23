# Kontainer v2 API and protocol architecture

Status: proposed production contract

Audience: server, desktop, CLI, and agent implementers

Research date: 2026-08-19

## Purpose

This document inventories the externally observable v1 contracts and defines the public and internal protocol boundaries for the Rust rewrite. It is deliberately a protocol specification, not an implementation-module design.

The sibling logging, exec, metrics-storage, Swarm-resource, and cluster-communication documents own their domain mechanics. This document owns cross-domain naming, authentication transport, versioning, errors, operations, and generation. Where an earlier domain draft proposed a JWT/ticket in a WebSocket URL, the safer header-or-first-frame rule here is the production target; the transitional URL behavior remains legacy-only.

The primary decisions are:

- The public controller API is resource-oriented HTTPS JSON under `/api/v2`, described by a checked-in OpenAPI document.
- Cluster-internal controller/node-agent calls use versioned protobuf services over Tonic gRPC and mandatory mutual TLS.
- WebSocket is reserved for interactive exec and high-volume multiplexed log streams. Server-sent events (SSE) carry resumable, one-way operation and resource events.
- Public wire DTOs, internal protobuf messages, domain objects, persistence records, and desktop IPC models are separate types with explicit conversions.
- The desktop native layer owns authentication material and network connections. React receives view models and stream events, never an agent bearer token.
- User-visible v1 capability parity is required, but v2 is intentionally not wire-compatible with v1. A bounded legacy adapter, if shipped, preserves v1 separately rather than aliasing `/api` to `/api/v2`.

Normative words such as **MUST**, **SHOULD**, and **MAY** describe the intended v2 contract. All limits in this document are initial production defaults and remain configurable downward by an operator; the discovery document reports effective values.

## Standards and implementation baseline

The design uses current primary specifications and Rust project documentation:

- HTTP semantics, conditional requests, content negotiation, and `202 Accepted` follow [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html). `428 Precondition Required` follows [RFC 6585](https://www.rfc-editor.org/rfc/rfc6585.html).
- Errors use [RFC 9457 Problem Details](https://www.rfc-editor.org/rfc/rfc9457.html).
- Bearer transport follows [RFC 6750](https://www.rfc-editor.org/rfc/rfc6750.html); its URI-query method is specifically discouraged because URLs are commonly logged. OAuth deployments follow the current security practices in [RFC 9700](https://www.rfc-editor.org/rfc/rfc9700.html).
- API discovery uses [RFC 9727 API Catalog](https://www.rfc-editor.org/rfc/rfc9727.html).
- Deprecation and end-of-life signals use [RFC 9745](https://www.rfc-editor.org/rfc/rfc9745.html) and [RFC 8594](https://www.rfc-editor.org/rfc/rfc8594.html).
- Distributed trace propagation uses the W3C [Trace Context Recommendation](https://www.w3.org/TR/trace-context/).
- The checked-in public schema targets OpenAPI 3.1.x because the current [Utoipa 5.5 documentation](https://docs.rs/utoipa/latest/utoipa/) supports OpenAPI 3.1. The latest published OpenAPI specification is [3.2.0](https://spec.openapis.org/oas/v3.2.0.html); adoption is deferred until the complete linting and client-generation chain is qualified.
- Internal RPC uses the official gRPC guidance for [deadlines](https://grpc.io/docs/guides/deadlines/), [status codes](https://grpc.io/docs/guides/status-codes/), [retry](https://grpc.io/docs/guides/retry/), and the standard [health checking protocol](https://grpc.io/docs/guides/health-checking/).
- The implementation must override library defaults intentionally: Axum's JSON/bytes extractors have a [2 MB default body limit](https://docs.rs/axum/latest/axum/extract/struct.DefaultBodyLimit.html), Axum WebSocket defaults permit [64 MB messages and 16 MB frames](https://docs.rs/axum/latest/axum/extract/ws/struct.WebSocketUpgrade.html), and Tonic's codec defaults permit [4 MB encoded and decoded messages](https://docs.rs/tonic/latest/tonic/server/struct.Grpc.html). Those defaults are too broad for several Kontainer routes.

## v1 contract inventory

This section records behavior, including bugs and drift. “Shape” means the JSON response body after Go's normal JSON encoding. Unless stated otherwise, successful reads and mutations return `200 OK`, timestamps are Go/Rust RFC 3339 strings, and failures use `{"error":"..."}`.

### Public HTTP routes

| Domain | Method and v1 path | Authentication | Request | Success contract | Important behavior |
|---|---|---|---|---|---|
| Process | `GET /health` | Public | None | `{"status":"ok"}` | Present on leader and follower; liveness only. |
| Process | `GET /ping` | Public | None | `{"app":"dev.kontainer.app","status":"ok\|setup_required"}` | Product identity and setup state are coupled. |
| OAuth | `GET /auth/providers` | Public | None | `{"providers":[string]}` | Provider names sorted. |
| OAuth | `GET /auth/{provider}` | Public | Query `session` | `302` to provider | `404` unknown provider, `400` missing session, `500` state failure. |
| OAuth | `GET /auth/{provider}/callback` | Public | Query `code`, `state` | HTML success page | `400` invalid state, `403` organization denial, `502` provider failure, `500` persistence/JWT failure. |
| Setup | `GET /setup/status` | Public | None | `{"step":"pending\|github_connected\|complete"}` | The desktop sends a setup header, but this route has no setup-token middleware; it does not verify the token. |
| Setup | `POST /setup/github/manifest` | Setup token | `{"org"?:string}` | `{"setupUrl":string}` | Creates an in-memory, short-lived manifest state. |
| Setup | `GET /setup/github/start` | Possession of query token | Query `token` | HTML auto-submit form | Missing/invalid state returns HTML `401`/`403`. Token appears in a URL. |
| Setup | `GET /setup/github/callback` | Provider state | Query `code` | Redirect or HTML | Stores GitHub credentials and a generated JWT secret. |
| Setup | `POST /setup/complete` | Setup token | None | `{"status":"complete"}` | State is marked complete before all later provider/JWT work succeeds; completion is not atomic. |
| Identity | `GET /api/me` | Bearer JWT | None | `{"user":User,"status":"ok"}` | `User`: `id`, `name`, `email`, `avatarUrl`, `provider`. |
| Agents | `GET /api/agents` | Bearer JWT | None | `{"agents":[Agent]}` | Registry absence is `503`. |
| Services | `GET /api/services` | Bearer JWT | Query `stack` | `{"services":[Service]}` | No pagination or sort. |
| Services | `GET /api/services/{serviceID}` | Bearer JWT | None | Bare `ServiceDetail` | `404` for missing service. |
| Services | `POST /api/services/{serviceID}/scale` | Bearer JWT | `{"replicas":u64}` | `{"replicas":u64}` | Global services reject scaling; validation may be `400` or `422`. |
| Services | `POST /api/services/{serviceID}/restart` | Bearer JWT | None | `{"status":"restarting"}` | Starts an untracked long-running Docker update. |
| Services | `POST /api/services/{serviceID}/rollback` | Bearer JWT | None | `{"status":"rolling_back"}` | Starts an untracked long-running Docker update. |
| Services | `PUT /api/services/{serviceID}/env` | Bearer JWT | `{"env":[EnvVar]}` | `{"status":"updated"}` | Array must be non-null; keys and duplicates have no item validation. |
| Services | `PUT /api/services/{serviceID}/image` | Bearer JWT | `{"image":string}` | `{"status":"updated"}` | Required string; no optimistic concurrency. |
| Tasks | `GET /api/tasks` | Bearer JWT | Query `serviceID`, `nodeID` | `{"tasks":[Task]}` | No pagination; filters are camel-case query names. |
| Tasks | `GET /api/services/{serviceID}/tasks` | Bearer JWT | None | `{"tasks":[Task]}` | Alias of task list handler. |
| Tasks | `GET /api/nodes/{nodeID}/tasks` | Bearer JWT | None | `{"tasks":[Task]}` | Alias of task list handler. |
| Tasks | `GET /api/tasks/{taskID}` | Bearer JWT | None | Bare `TaskDetail` | Detail adds process/environment/network data. |
| Tasks | `GET /api/tasks/stats` | Bearer JWT | None | `{"stats":[ContainerStats]}` | Live, local container samples. |
| Tasks | `GET /api/task-stats` | Bearer JWT | None | `{"taskStats":TaskStats}` | Aggregate counts; route naming differs from `/tasks/stats`. |
| Nodes | `GET /api/nodes` | Bearer JWT | None | `{"nodes":[Node]}` | No pagination. |
| Nodes | `PATCH /api/nodes/{nodeID}/availability` | Bearer JWT | `{"availability":"active\|drain\|pause"}` | `{"availability":string}` | Exact enum validation. |
| Nodes | `PATCH /api/nodes/{nodeID}/role` | Bearer JWT | `{"role":"manager\|worker"}` | `{"role":string}` | Exact enum validation. |
| Stacks | `GET /api/stacks` | Bearer JWT | None | `{"stacks":[Stack]}` | No detail route. |
| Stacks | `DELETE /api/stacks/{name}` | Bearer JWT | None | `{"status":"removed"}` | Synchronous-looking response for multi-resource work. |
| Networks | `GET /api/networks` | Bearer JWT | None | `{"networks":[Network]}` | `services` is present but not populated by the server. |
| Volumes | `GET /api/volumes` | Bearer JWT | None | `{"volumes":[Volume]}` | Unknown usage can be represented by `sizeBytes: -1`. |
| Secrets | `GET /api/secrets` | Bearer JWT | None | `{"secrets":[Secret]}` | Secret data is not returned. |
| Secrets | `POST /api/secrets` | Bearer JWT | `{"name":string,"data":string}` | `201 {"name":string}` | No resource ID or `Location`; desktop expects an ID. |
| Secrets | `DELETE /api/secrets/{secretID}` | Bearer JWT | None | `{"status":"removed"}` | Desktop expects a boolean. |
| Configs | `GET /api/configs` | Bearer JWT | None | `{"configs":[Config]}` | No pagination. |
| Configs | `POST /api/configs` | Bearer JWT | `{"name":string,"data":string}` | `201 {"name":string}` | No resource ID or `Location`; desktop expects an ID. |
| Configs | `GET /api/configs/{configID}` | Bearer JWT | None | Bare `ConfigDetail` | Engine bytes are base64-encoded by Go JSON, while the desktop treats `data` as text. |
| Configs | `DELETE /api/configs/{configID}` | Bearer JWT | None | `{"status":"removed"}` | Desktop expects a boolean. |
| Metrics | `GET /api/metrics` | Bearer JWT | Query `range=1h\|6h\|24h` | Bare `ClusterMetrics` | Unknown values silently become `1h`; data freshness target is two minutes. |
| Metrics | `GET /api/metrics/{nodeID}` | Bearer JWT | None | Bare `MetricsSnapshot` | Cluster fan-out failures are not represented consistently. |
| Images | `GET /api/images` | Bearer JWT | None | `{"nodes":[NodeImages]}` | Cluster fan-out response. |
| Images | `DELETE /api/images/{imageID}` | Bearer JWT | Query `node`, `force` | Bare `ImageRemoveResponse` | Query key is `node`, unlike prune. |
| Images | `POST /api/images/prune` | Bearer JWT | Query `nodeId` | Bare `ImagePruneResponse` | Query key is `nodeId`; desktop cannot select a node. |
| Settings | `GET /api/settings` | Bearer JWT | None | `{"settings":{string:string}}` | Unversioned arbitrary map. |
| Settings | `PUT /api/settings` | Bearer JWT | Same map | Same wrapper | Performs partial upsert although `PUT` implies replacement; unknown keys are accepted. |

Leader setup middleware allows `/health`, `/ping`, `/setup*`, and `/auth*`; any other route before setup returns `503 {"status":"setup_required"}`. The follower exposes health and internal gRPC rather than the leader API.

### v1 public DTOs

The following is the server wire model. Optionality here is wire optionality, not merely optional UI presentation.

| Type | v1 fields and notable semantics |
|---|---|
| `EnvVar` | `key`, `value`. No sensitivity marker. |
| `Service` | `id`, `name`, `image`, `stack`, `replicasRunning`, `replicasDesired`, `updatedAt`. |
| `ServiceDetail` | Service identity plus `mode`, nullable `replicas`, `createdAt`, `updatedAt`, `env`, `ports`, `mounts`, `constraints`, `labels`, nullable `updateStatus`, string `previousImage`, and optional arbitrary `rawSpec`. |
| `Port` / `Mount` | Port: `protocol`, `targetPort`, `publishedPort`, `publishMode`. Mount: `type`, `source`, `target`, `readOnly`. |
| `UpdateStatus` | `state`, `startedAt`, `completedAt`, `message`. |
| `Task` | `id`, `serviceId`, `serviceName`, `nodeId`, `nodeName`, `state`, `desiredState`, `timestamp`, `containerId`, `image`, `slot`, nullable `exitCode`, `error`. |
| `TaskDetail` | Task plus `pid`, `createdAt`, `updatedAt`, `env`, `mounts`, `networks`. |
| `ContainerStats` | `containerId`, `name`, `serviceId`, `nodeId`, `cpuPercent`, `memUsage`, `memLimit`, `memPercent`. |
| `TaskStats` | `total`, `running`, `failed`, `preparing`, `complete`, `shutdown`, `desiredRunning`. |
| `Node` | Docker node identity, hostname, role, availability, status, engine/platform data, resources, manager status, labels, and timestamps as flattened UI data. |
| `Stack` | `name`, `services`, `runningReplicas`, `desiredReplicas`, `updatedAt`. |
| `Network` | `id`, `name`, `driver`, `scope`, `internal`, `attachable`, `ingress`, `services`, `createdAt`. |
| `Volume` | `name`, `driver`, `mountpoint`, `scope`, `labels`, `options`, `createdAt`, `refCount`, `sizeBytes`, `stack`. |
| `Secret` | `id`, `name`, `labels`, `createdAt`, `updatedAt`; data intentionally absent. |
| `Config` / `ConfigDetail` | `id`, `name`, `labels`, `createdAt`, `updatedAt`, `dataSize`; detail adds `data`. |
| `Image` / `NodeImages` | Image: `id`, `tags`, byte `size`, epoch `created`, `nodeId`, `hostname`; node wrapper repeats node identity. |
| Metrics | Node metrics include CPU count/percent, memory used/total/percent, and disk. Container metrics include `containerId`, `name`, `serviceId`, CPU and memory values. Snapshot adds `timestamp`, `nodeId`, `hostname`, `node`, `containers`; cluster adds `agents`, `history`. |
| `Agent` | `nodeId`, `hostname`, `address`, `role`, `status`, `lastSeen`; leaks routable internal address to any authenticated user. |

Important encoding ambiguities include epoch seconds beside RFC 3339 timestamps, signed `-1` sentinel byte counts, empty strings standing in for null, JSON numbers for values that can exceed JavaScript's safe integer range, arbitrary `rawSpec`, and unbounded string maps.

### v1 WebSocket contracts

All v1 upgrades accept every origin. Log and exec bearer tokens are passed in the URL query, contrary to the preferred header transport in [RFC 6750](https://www.rfc-editor.org/rfc/rfc6750.html).

| Route | Frames | Lifecycle and failure behavior |
|---|---|---|
| `GET /auth/listen` | Server text JSON: first `{"sessionId":hex}`, then `{"token":jwt}` or `{"error":string}` | Public, ten-minute in-memory bridge. Client disconnect cancels. OAuth session is sent before authorization. |
| `GET /api/services/{serviceID}/logs` | Server text JSON `{"stream":"stdout\|stderr","timestamp"?:string,"line":string}` | Query: `token`, `tail`, `timestamps`; follows indefinitely. Tail defaults to 100, numeric maximum 10,000, `all` supported. Both output streams are always enabled even though the desktop sends toggles. Upgrade failures use HTTP JSON; post-upgrade failures use ad hoc `{"error":...}`. |
| `GET /api/tasks/{taskID}/logs` | Same as service logs | No stream version, sequence, heartbeat, cursor, gap, or resumable replay. Reconnect fetches the tail again and can duplicate lines. |
| `GET /api/exec/{containerID}` | Client binary is stdin; server binary is terminal output. Client text resize is `{"type":"resize","cols":u16,"rows":u16}`. Server text is `{"type":"error","message":string,"code":0}` or `{"type":"exit","message"?:string,"code":i32}`. | Query: `token`, `nodeID`, `shell`, `user`, `cols`, `rows`. Container ID is restricted to 128 characters from `[A-Za-z0-9_.-]` with an alphanumeric first character. Defaults are 80x24 and shell fallback `/bin/sh`, `/bin/bash`, `/bin/ash`. Invalid dimensions fall back, but zero is accepted. Local exits are unreliable; follower EOF is reported as exit 0. Malformed controls are ignored. |

The v1 Docker log demultiplexer trusts a 32-bit frame length before allocation, has no application frame cap, trims and drops blank lines, and treats any stream identifier other than `2` as stdout. The desktop keeps at most 5,000 visible lines but its paused buffer is unbounded. Reconnect delays are 1, 2, 4, 8, and 16 seconds without jitter.

### v1 internal gRPC contract

The protobuf package is the unversioned `agent`. `AgentService` has these calls:

| RPC | Shape | v1 behavior |
|---|---|---|
| `Health(HealthRequest) -> HealthResponse` | `status`, `node_id`, `hostname` | Protected by the same shared secret as functional calls; does not implement the standard gRPC health service. |
| `Info(InfoRequest) -> InfoResponse` | node identity, hostname, role | Response identity is trusted by registration. |
| `Metrics(MetricsRequest) -> MetricsResponse` | timestamp, node/host, node metrics, repeated container metrics | Errors collapse to `Internal`. |
| `ListImages(ListImagesRequest) -> ListImagesResponse` | node/host and repeated images | Errors collapse to `Internal`. |
| `RemoveImage(RemoveImageRequest) -> RemoveImageResponse` | image ID, force; deleted and untagged lists | Errors collapse to `Internal`. |
| `PruneImages(PruneImagesRequest) -> PruneImagesResponse` | reclaimed bytes and deleted images | Errors collapse to `Internal`. |
| `Exec(stream ExecMessage) <-> stream ExecMessage` | Generic message with enum `INIT`, `DATA`, `RESIZE`, `EXIT`, `ERROR` and fields `data`, `cols`, `rows`, `exit_code`, `error`, `shell`, `user` | Bidi stream manually checks authentication. Follower receives `user` but fails to pass it to Docker. Exec errors are often in-band instead of gRPC statuses. |

Transport uses plaintext gRPC plus static `x-kontainer-secret` metadata. A global unary interceptor authenticates all unary calls; the stream duplicates this check. No key identity, rotation protocol, peer identity, channel binding, or authorization scope exists. Empty configuration can surface as `Internal`. Most clients have no explicit deadline even though gRPC does not set one by default; the official [deadline guide](https://grpc.io/docs/guides/deadlines/) recommends explicit realistic deadlines. Channels are cached indefinitely.

Follower discovery polls Swarm tasks, uses the first network address and configured port, then probes `Health`. The in-memory registry is keyed only by node ID, so leader and follower tasks on the same node can overwrite one another. The public `/api/agents` response exposes the discovered address.

### Desktop/Tauri actions and duplicated types

The Tauri app registers these native actions (complete inventory):

- authentication/profile: `auth_listen`, `parse_auth_token`, `save_session`, `load_session`, `delete_session`, `load_profiles`, `save_profile`, `delete_profile`, and `set_active_profile`;
- agent/auth/setup: `fetch_providers`, `ping_agent`, `check_auth`, `setup_verify_token`, `setup_github_connect`, and `setup_complete`;
- services/tasks: `list_services`, `inspect_service`, `list_service_tasks`, `inspect_task`, `list_node_tasks`, `list_task_stats`, `list_tasks`, `scale_service`, `restart_service`, `rollback_service`, `update_service_env`, and `update_service_image`;
- cluster resources: `list_stacks`, `remove_stack`, `list_networks`, `list_volumes`, `list_nodes`, `update_node_availability`, and `update_node_role`;
- secrets/configs: `list_secrets`, `create_secret`, `remove_secret`, `list_configs`, `create_config`, `remove_config`, and `inspect_config`;
- telemetry/images/settings: `list_cluster_metrics`, `list_node_metrics`, `list_images`, `remove_image`, `prune_images`, `get_agent_settings`, and `update_agent_settings`;
- local application: `close_splashscreen`, `load_app_settings`, and `save_app_settings`.

The native Rust contract structs are also action-local rather than shared. Complete groups are:

| Rust action area | DTOs |
|---|---|
| Auth/setup | `JwtPayload`, `ParsedToken`, `SaveSessionInput`, `StoredSession`, `ConnectionProfile`, `StoredProfiles`, `SaveProfileInput`, `AuthUser`, `CheckAuthResponse`, `ProvidersResponse`, `PingResponse`, `SetupStatus`, `ManifestResponse`, auth-listener `WsMessage`, `SessionIdPayload`, `TokenPayload`, and `ErrorPayload`. |
| Services | `ServiceItem`, `ListServicesResponse`, detail `EnvVar`, `MountInfo`, `PortInfo`, `UpdateStatusInfo`, `ServiceDetail`, `ScaleServiceBody`/`Response`, `RestartServiceResponse`, `RollbackServiceResponse`, update-env `EnvVar`, `UpdateEnvBody`, `UpdateServiceEnvResponse`, `UpdateImageBody`, and `UpdateServiceImageResponse`. |
| Tasks | `ServiceTaskItem`, `ListServiceTasksResponse`, `NodeTaskItem`, `ListNodeTasksResponse`, `TaskItem`, `ListTasksResponse`, task-detail `EnvVar`, `MountInfo`, `TaskNetworkInfo`, `TaskDetail`, `TaskStats`, and `ListTaskStatsResponse`. |
| Nodes/stacks/networks/volumes | `NodeItem`, `ListNodesResponse`, `UpdateAvailabilityBody`/`Response`, `UpdateRoleBody`/`Response`, `StackItem`, `ListStacksResponse`, `RemoveStackResponse`, `NetworkServiceRef`, `NetworkItem`, `ListNetworksResponse`, `VolumeItem`, and `ListVolumesResponse`. |
| Secrets/configs | `SecretItem`, `ListSecretsResponse`, `CreateSecretBody`/`Response`, `RemoveSecretResponse`, `ConfigItem`, `ListConfigsResponse`, `CreateConfigBody`/`Response`, `InspectConfigResponse`, and `RemoveConfigResponse`. |
| Metrics/images/settings | `NodeMetrics`, `ContainerMetrics`, `AgentMetricsSnapshot`, `ClusterMetricsResponse`, `ImageItem`, `NodeImages`, `ClusterImagesResponse`, `RemoveImageResponse`, `PruneImagesResponse`, `AgentSettingsResponse`, `UpdateBody`, and `UpdateAgentSettingsResponse`. |
| Local app | `AppSettings` plus profile/session persistence structs in the auth action modules. |

Rust wire numbers use a mixture of `i32`, `i64`, `u32`, and `u64`; string timestamps coexist with `createdAt: i64`. Most structs opt into `camelCase`, but wrappers whose fields are already lower-case do not consistently declare it. Identically named concepts such as `EnvVar` and `MountInfo` are redefined in multiple action files.

The TypeScript contract/view types are similarly hand-maintained:

| TypeScript file/area | Types and v1 shape |
|---|---|
| `types/auth.ts`, `types/connection.ts` | `AuthUser {id,name,email,avatarUrl,provider}`; `ParsedToken {userId,expiresAt}`; `CheckAuthResponse {user,status}`; `AuthState {user,token,expiresAt,agentUrl,isLoading}`; `StoredSession` holds flattened user, token, expiry, URL; `ConnectionProfile` adds `id`/`label`; `StoredProfiles {profiles,activeProfileId}`. These expose bearer credentials to UI state. |
| `types/services.ts`, `types/service-detail.ts` | `ServiceItem {id,name,image,stack,replicasRunning,replicasDesired,updatedAt}`; detail support `EnvVar {key,value}`, `MountInfo {type,source,target,readOnly}`, `PortInfo {publishedPort,targetPort,protocol,publishMode}`, `UpdateStatusInfo {state,startedAt,completedAt,message}`; `ServiceDetail` adds mode, nullable replicas, timestamps, arrays, nullable update status/previous image but omits server labels/raw spec. |
| `types/tasks.ts`, `types/service-tasks.ts`, `types/node-tasks.ts` | `TaskItem` has IDs/names, slot, state/desired state, nullable exit code, error, timestamp, container ID, image. `ServiceTaskItem` omits service/image, adds `user`. `NodeTaskItem` omits node/image/exit code. |
| `types/task-detail.ts`, `types/task-stats.ts` | Detail support `TaskEnvVar {key,value}`, `TaskMountInfo {type,source,target,readOnly}`, `TaskNetworkInfo {name,addresses}`; `TaskDetail` has task IDs/state/error/image/container/pid/nullable exit code/timestamps and support arrays, but omits server `serviceName`/`timestamp`. `TaskStats {total,running,failed,preparing,complete,desiredRunning}` omits server `shutdown`. |
| `types/nodes.ts`, `types/stacks.ts` | `NodeItem {id,hostname,role,status,availability,engineVersion,os,architecture,ip,leader,updatedAt}`; `StackItem {name,services,runningReplicas,desiredReplicas,updatedAt}`. |
| `types/networks.ts`, `types/volumes.ts` | `NetworkServiceRef {id,name}`; `NetworkItem {id,name,driver,scope,subnet?,internal,attachable,ingress,services,createdAt}`; `VolumeItem {name,driver,scope,mountpoint,labels,options,createdAt,refCount,sizeBytes,stack}`. |
| `types/secrets.ts`, `types/configs.ts` | `SecretItem {id,name,labels,createdAt,updatedAt}`; `ConfigItem` adds `dataSize`; `ConfigDetail {id,name,labels,data,createdAt,updatedAt}`. |
| `types/images.ts` | `ImageItem {id,tags,size,created,nodeId,hostname}`; `NodeImages {nodeId,hostname,images}`; `ClusterImagesResponse {nodes}`; `RemoveImageResponse {deleted,untagged}`; `PruneImagesResponse {spaceReclaimed,imagesDeleted}`. |
| `types/metrics.ts` | `NodeMetrics {cpus,memoryTotal,memoryUsed,memoryPercent,cpuPercent,diskTotal,diskUsed,diskPercent}`; `ContainerMetrics {containerId,name,serviceName,cpuPercent,memoryUsed,memoryLimit,memoryPercent,createdAt}`; `AgentMetricsSnapshot {timestamp,nodeId,hostname,node,containers}`; `ClusterMetricsResponse {agents,history}`. |
| `types/settings.ts` | `AppSettings` has nullable refresh interval, terminal shell/font size/font family/scrollback, application font, and scheme; `AgentSettings {settings: Record<string,string>}`. |
| Hook-local wire types | Create secret/config `{id,name}`; remove secret/config `{removed:boolean}`; remove stack `{removed:number}`; scale `{replicas:number}`; restart/rollback `{status}`; update env `{env}`; update image `{image}`; node role/availability return their named string. |
| Direct streaming hooks | `LogEntry {id,stream,timestamp,line}`, `LogOptions {tail,timestamps,stdout,stderr}`, four-state `ConnectionStatus`, five-state `ExecStatus`, and `ExecControlMsg {type,message?,code?}`; these model ad hoc v1 controls rather than generated protocol types. |

Every HTTP action constructs a new `reqwest::Client`; timeouts vary between 5, 10, and 30 seconds. Native errors are opaque strings such as `Agent responded with status ...: <raw body>`. This loses structured status, field validation, retryability, and request IDs and can expose a raw upstream body.

Rust structs and TypeScript interfaces are maintained manually. The largest confirmed mismatches are:

| Operation/type | Desktop expectation | Server reality | Result |
|---|---|---|---|
| Scale service | `PATCH /api/services/{id}` | `POST /api/services/{id}/scale` | `404`/`405`; action cannot work. |
| Node availability and role | `PUT` subresource | `PATCH` subresource | `404`/`405`; actions cannot work. |
| Create secret/config | `{"id":string,"name":string}` | `{"name":string}` | Mutation succeeds, response decoding fails. |
| Delete secret/config | `{"removed":bool}` | `{"status":"removed"}` | Mutation succeeds, response decoding fails. |
| Delete stack | `{"removed":number}` | `{"status":"removed"}` | Mutation succeeds, response decoding fails. |
| Update service env | `{"env":[...]}` | `{"status":"updated"}` | Mutation succeeds, response decoding fails. |
| Update service image | `{"image":string}` | `{"status":"updated"}` | Mutation succeeds, response decoding fails. |
| Container metrics | Rust/TS require `serviceName` | Go emits `serviceId` | Any non-empty container array can fail native decoding. |
| Metrics timestamp | Rust defaults absent `createdAt` to zero; TS requires it | Go model uses other timestamp fields | Native/TS semantics diverge. |
| Task aggregate | Rust and TS omit `shutdown` | Go emits it | Field is silently lost. |
| Service task | Rust/TS include `user` (Rust defaults empty) | Go task has no `user` | Fabricated empty value. |
| Network | TS permits `subnet` | Go never sends it | Type suggests a capability that does not exist. |
| Service detail | TS omits `labels` and `rawSpec`; `previousImage` is nullable | Go emits labels/raw spec and uses empty string | Data loss and null/empty ambiguity. |
| Setup verification | Sends setup token to `GET /setup/status` | Route ignores the token | Any value appears “verified.” |
| Cluster metrics | No `range` argument | Server supports a range | Desktop always receives the default window. |
| Image prune | No node argument | Server supports `nodeId` | Desktop can invoke cluster-wide prune only. |

Several Tauri actions interpolate resource identifiers and query values directly into URLs. For example, service `stack` is not URL-encoded and `node`/`force` are assembled manually. React also opens log and exec WebSockets directly, exposing the JWT to WebView JavaScript and URL logging and bypassing the intended React → Tauri → agent boundary.

Profiles are currently stored in `profiles.json` with label, agent URL, user fields, token, and expiration; `session.json` is migrated as a legacy source. Tokens are plaintext in the plugin store rather than an OS credential store. Local UI settings live separately in `settings.json` and include refresh interval, terminal shell, font size/family, scrollback, terminal font, and scheme.

### v1 cross-cutting behavior

Authentication and setup:

- `/api/*` uses a JWT in `Authorization: Bearer`. Missing authentication returns `401 {"error":"authorization required"}`; invalid/expired returns `401 {"error":"invalid or expired token"}`.
- OAuth state is one-time, in memory, and expires after ten minutes. GitHub scopes include user profile/email and optionally organization membership.
- The JWT lifetime defaults to 1,440 minutes and there is no refresh-token protocol. The desktop decodes `exp` without signature validation for display/scheduling, then uses `/api/me` as the actual server check.
- A generated bootstrap value uses the `ktsetup_` prefix plus 12 hexadecimal characters and is logged at startup when no configured value exists. The setup header is `X-Kontainer-Setup-Token` and comparison is constant-time.

Validation and errors:

- Binding failures generally return `400`; framework validation often returns `422 {"errors":...}`. Field error shape is framework-owned rather than an API contract.
- Docker not-found/global-service cases are mapped selectively. Most Docker and cluster errors become `500` or `503` with implementation text.
- Setup-required uses `{"status":"setup_required"}` instead of the error envelope. OAuth and setup browser routes use HTML errors. WebSockets introduce a third in-band error shape.
- Collection writes usually return `200`; creates return `201` without `Location`; deletes return `200` with status strings. Long-running actions have no operation ID, progress, durable outcome, retry key, or cancellation.

Runtime constraints:

- Goravel is configured for a 4,096 KB request/header allowance and a three-second request timeout. This conflicts with desktop client timeouts and with operations that can validly take longer.
- HTTP CORS configuration has allow-all-related values but no configured paths, while every WebSocket upgrader independently accepts every origin; browser-origin policy is therefore inconsistent.
- No public pagination, stable ordering, request ID, trace propagation, ETag, precondition, idempotency-key store, rate limit, consistent decompressed-body limit, or partial-result envelope exists.
- Agent settings accept any string key and value, expose all stored values, and implement partial upsert behind `PUT`. There is no schema, revision, sensitivity classification, size bound, or deletion behavior.

## Compatibility policy

### Strict v1 visible parity

“Parity” means users can perform all currently exposed workflows after migration:

- authenticate through the configured provider and enforce organization policy;
- complete first-run setup;
- inspect identity and cluster agents/capabilities;
- list and inspect services/tasks, view stats, and perform scale, restart, rollback, image, and environment changes;
- list nodes and change availability/role;
- list/remove stacks; list networks and volumes;
- list/create/inspect/delete secrets and configs;
- retrieve current and historical metrics;
- list/remove/prune images locally or by node;
- read/write supported controller settings;
- follow service/task logs and open interactive exec sessions.

The v2 desktop must pass parity tests against the v2 server before the legacy UI path is removed. User-visible parity does **not** preserve defects such as fake setup verification, untracked actions, plaintext token storage, inconsistent node query names, or direct WebView access to tokens.

### Intentional v2 wire breaks

The following changes are intentional and must appear in migration notes:

- `/api/v2` path versioning, a new discovery root, and renamed health/setup/auth endpoints;
- consistent collection envelopes instead of domain-specific plural keys;
- explicit string encoding for unsafe 64-bit integers and byte sizes;
- `null` instead of sentinel `-1` or empty-string stand-ins;
- canonical RFC 3339 UTC timestamps and explicit `...Ms` durations;
- Problem Details errors and corrected HTTP status codes;
- operation resources for long-running mutations;
- `ETag`/`If-Match` preconditions for state-changing operations;
- bearer credentials removed from URLs, native-layer token custody, short-lived access tokens, and refresh rotation;
- versioned, framed, bounded stream protocols with tickets/cursors where needed;
- typed/revisioned settings and removal of arbitrary `rawSpec` from the default public model;
- non-secret capability data instead of exposing follower addresses;
- corrected method names, request/response bodies, and response IDs;
- paginated, deterministically ordered lists and explicit partial results.

The server MUST NOT make `/api` an alias for `/api/v2`. If compatibility is required, a `legacy-v1` adapter is compiled/deployed separately, has its own integration fixtures, and translates between frozen v1 wire DTOs and v2 application commands. It preserves actual v1 server shapes, not the contradictory desktop expectations. A v1 desktop repair release may align its broken actions before migration, but that is a v1 maintenance decision rather than a v2 contract.

## Transport and schema choices

| Candidate | Decision | Rationale |
|---|---|---|
| REST/JSON + OpenAPI | **Accept for public control plane** | Natural resource semantics, cache/precondition support, inspectable with `curl`, broad proxy/CLI tooling, and generated clients. OpenAPI is language-neutral and does not couple consumers to Rust. |
| Tonic gRPC + protobuf | **Accept for internal cluster traffic** | Strong streaming and code generation, deadlines/cancellation/status model, compact messages, and mature Rust implementation. Versioned services and mTLS fit a controlled node fleet. |
| Public gRPC | **Reject for v2 baseline** | Adds browser, CLI, HTTP gateway, and proxy friction while still needing a public resource model. It would duplicate rather than replace REST. |
| Connect protocol | **Defer** | Connect supports protobuf/JSON and unary/server/client/bidi streaming with HTTP-friendly semantics ([protocol](https://connectrpc.com/docs/protocol/)), but there is no demonstrated public RPC SDK requirement and internal Tonic is already selected. Revisit if browser/native protobuf interoperability materially reduces complexity. |
| GraphQL | **Reject for baseline** | The current [GraphQL specification](https://spec.graphql.org/September2025/) provides an expressive query language, but Kontainer's bounded screens do not need client-defined graph traversal. Query-cost policy, field authorization, subscriptions, N+1 prevention, persisted queries, and mutation idempotency would be additional protocol systems. Revisit only for proven third-party analytics demand. |
| JSON-RPC 2.0 | **Reject** | The [JSON-RPC 2.0 specification](https://www.jsonrpc.org/specification) defines a compact method/call protocol, but it loses useful HTTP resource, conditional, cache, and status semantics here; discovery, pagination, long-running operations, and idempotency would all become custom conventions. |
| WebSocket | **Accept narrowly** | Required for low-latency full-duplex exec and efficient multiplexed logs. It is not the default request/response transport. |
| SSE | **Accept for one-way events** | HTTP-native reconnect and `Last-Event-ID` semantics are standardized by the [HTML Living Standard](https://html.spec.whatwg.org/multipage/server-sent-events.html). It fits operation/resource notifications, but not terminal input. Native clients can set an Authorization header. |
| Shared server/domain Rust structs | **Reject** | Serde layout would accidentally make domain refactors into wire changes and force client/server crate coupling. |
| Dedicated public DTO crate + generated clients | **Accept** | A dependency-light wire crate can feed OpenAPI while domain conversions remain explicit. TypeScript and external clients are generated from the checked-in schema. |
| Protobuf as the universal public DTO | **Reject for baseline** | It is correct internally, but JSON/OpenAPI remains the better automation and compatibility surface. Do not maintain a hand-written JSON mirror of every internal protobuf message. |

## Protocol boundaries

```text
React UI
  │ typed Tauri commands/channels; no bearer token
  ▼
desktop application layer ─────── GPUI or CLI application layer
  │                                    │
  └──────────── kontainer-client ──────┘
                        │ HTTPS REST / WSS / SSE
                        ▼
                 public API adapters
                        │ explicit DTO ↔ command/query conversion
                        ▼
                 controller domain/services
                        │ Tonic gRPC + protobuf + mTLS
                        ▼
                    node agents
```

The public server must not know whether the client is Tauri, GPUI, or a CLI. `kontainer-client` contains public protocol types and transport behavior only. It must not import controller persistence, Docker adapters, Tauri commands, React models, or internal protobuf service implementations.

## Public HTTP conventions

### Origins, paths, identifiers, and naming

- Production is HTTPS only. Plain HTTP may be enabled explicitly on loopback for development.
- Browser CORS is disabled by default and, when enabled, uses an explicit operator allow-list of exact origins, methods, and headers with a bounded preflight cache. WebSocket handshakes validate `Origin` for browser clients; native clients without `Origin` still require normal authentication. There is no allow-all production mode.
- Public versioned resources live below `/api/v2`. Browser-facing OAuth completion pages and liveness/readiness remain outside bearer-protected resource routing where noted.
- JSON property names are `camelCase`; URI path segments are lower-case plural nouns with hyphens; query parameters are `camelCase`.
- OpenAPI operation IDs are stable lower-camel commands such as `listServices`, `scaleService`, and `getOperation`.
- IDs are opaque strings. Clients MUST percent-encode path segments and use a URL builder, never string interpolation. Names are not IDs unless the resource explicitly declares a name key.
- New server-generated public IDs use UUIDv7 text. Internal Docker IDs remain opaque external IDs and are never parsed by clients.
- Clients must ignore unknown response properties and unknown enum strings. Requests with unknown properties or query parameters are rejected to catch automation mistakes. The schema provides an `unknown` representation where UI behavior depends on exhaustive enum handling.
- Integers whose range can exceed `2^53-1`, including byte counts, sequence numbers, Docker version indices, and nanosecond values, are decimal strings in public JSON. Bounded counts and percentages remain JSON numbers.
- Timestamps use RFC 3339 UTC with explicit offset and fractional seconds where available. Durations use bounded integer fields suffixed `Ms`. Unknown values are `null`, never `-1`, zero, or empty string.

### Media types and compression

- Requests with JSON bodies use `Content-Type: application/json`. Settings patches use `application/merge-patch+json` only where the operation documents merge-patch semantics.
- Successful JSON responses use `application/json`; errors use `application/problem+json`.
- Missing `Accept` or `Accept: */*` selects JSON. An unsupported `Accept` returns `406`; an unsupported request content type returns `415`.
- UTF-8 is mandatory. A byte-order mark and malformed Unicode are rejected.
- The server may return `gzip` or `br` for eligible responses and sets `Vary: Accept, Accept-Encoding`. Authentication/session responses and bodies containing secret/config data are never compressed.
- CBOR and protobuf over public REST are deferred. They require a measured need and a second explicit media type; content negotiation must never silently change JSON encoding.

### Common response headers

Every API response includes:

- `X-Request-Id`: server-generated UUIDv7, even if a client supplies a separate candidate;
- `Kontainer-API-Version`: concrete semantic API build version such as `2.3`;
- `traceparent` when trace propagation is active and safe to expose;
- cache policy appropriate to the representation;
- deprecation headers when the selected endpoint is deprecated.

The server accepts valid `traceparent`/`tracestate` and propagates them through gRPC metadata and operation records. It validates size/syntax and starts a new trace when invalid. The request ID is used in support and audit records; client-supplied IDs are captured only as bounded metadata and never trusted for uniqueness.

## Proposed public route/domain matrix

“Operation” means `202 Accepted` with a persisted operation resource. `If-Match` means the mutation returns `428` if the precondition is omitted.

| Domain | Proposed v2 route | Success | Notes |
|---|---|---|---|
| Process | `GET /health/live` | `200` empty/minimal | Public process liveness; no Docker or cluster dependency. |
| Process | `GET /health/ready` | `200` or `503` Problem | Public readiness; reveals only coarse dependency state. |
| Discovery | `GET /.well-known/api-catalog` | RFC 9727 linkset | Lists supported API roots/OpenAPI descriptions, not cluster topology. |
| Discovery | `GET /api/v2` | `ApiRoot` | Version, links, setup state, auth methods, limits summary. |
| Discovery | `GET /api/v2/openapi.json` | OpenAPI JSON | Public, immutable per build, strong ETag. YAML MAY also be published. |
| Discovery | `GET /api/v2/capabilities` | `Capabilities` | Authenticated resource/action/stream capabilities and effective limits. |
| Setup | `GET /api/v2/setup/status` | coarse `SetupStatus` | Public status only; never claims token validity. |
| Setup | `POST /api/v2/setup/sessions` | `201 SetupSession` | Setup header exchange; returns short-lived scoped session. |
| Setup | `POST /api/v2/setup/github-manifests` | `201 SetupManifest` | Setup-session auth; URL contains only one-time opaque state. |
| Setup | `POST /api/v2/setup/actions/complete` | Operation | Atomic, retryable completion. |
| Auth | `GET /api/v2/auth/providers` | collection | Public provider metadata and supported flows. |
| Auth | `POST /api/v2/auth/sessions` | `201 AuthSession` | Starts native authorization transaction with PKCE. |
| Auth | `GET /api/v2/auth/sessions/{id}` | pending/complete result | Requires transaction verifier; bounded polling; token returned once. |
| Auth | `POST /api/v2/auth/tokens/refresh` | token pair | Rotating refresh credential; no access token in URL. |
| Auth | `POST /api/v2/auth/tokens/revoke` | `204` | Idempotent revocation. |
| OAuth browser | `GET /oauth/{provider}/authorize` | redirect | One-time state; no API bearer token. |
| OAuth browser | `GET /oauth/{provider}/callback` | safe HTML | Exchanges provider response and marks auth session complete. |
| Identity | `GET /api/v2/me` | `User` | Scopes/roles included; email only when allowed. |
| Agents | `GET /api/v2/agents` | collection | Never exposes internal addresses or credentials. |
| Agents | `GET /api/v2/agents/{agentId}` | `Agent` | Health/capability summary. |
| Services | `GET /api/v2/services` | collection | Filters: `stack`, `mode`, `updateState`; stable sort. |
| Services | `GET /api/v2/services/{serviceId}` | `Service` + ETag | `include=spec` is authorized/bounded; no default `rawSpec`. |
| Services | `POST /api/v2/services/{serviceId}/actions/scale` | Operation | Body replicas + `If-Match` + idempotency key. |
| Services | `POST /api/v2/services/{serviceId}/actions/restart` | Operation | Explicit side effect; idempotency key required. |
| Services | `POST /api/v2/services/{serviceId}/actions/rollback` | Operation | Explicit side effect; idempotency key required. |
| Services | `PATCH /api/v2/services/{serviceId}/environment` | Operation | Full typed environment patch, `If-Match`; sensitive values redacted after write. |
| Services | `PATCH /api/v2/services/{serviceId}/image` | Operation | Body `image`, optional registry credential reference, `If-Match`. |
| Tasks | `GET /api/v2/tasks` | collection | Filters: repeated `serviceId`, `nodeId`, `state`; optional snapshot stats include. |
| Tasks | `GET /api/v2/tasks/{taskId}` | `Task` + ETag | Single canonical task detail. |
| Tasks | `GET /api/v2/task-summary` | `TaskSummary` | Includes every state bucket and snapshot timestamp. |
| Nodes | `GET /api/v2/nodes` | collection | Explicit partial/warnings if enrichment fan-out fails. |
| Nodes | `GET /api/v2/nodes/{nodeId}` | `Node` + ETag | Capability and reachability summary. |
| Nodes | `POST /api/v2/nodes/{nodeId}/actions/set-availability` | Operation | Body enum + `If-Match`. |
| Nodes | `POST /api/v2/nodes/{nodeId}/actions/set-role` | Operation | Body enum + `If-Match`. |
| Stacks | `GET /api/v2/stacks` | collection | Stable stack ID/name semantics documented. |
| Stacks | `GET /api/v2/stacks/{stackId}` | `Stack` + ETag | Aggregate detail. |
| Stacks | `DELETE /api/v2/stacks/{stackId}` | Operation | `If-Match`, idempotency key; target snapshot recorded. |
| Networks | `GET /api/v2/networks` | collection | Filters and partial enrichment are explicit. |
| Networks | `GET /api/v2/networks/{networkId}` | `Network` + ETag | Subnets are present only when known and authorized. |
| Volumes | `GET /api/v2/volumes` | collection | Unknown size/reference values are null. |
| Volumes | `GET /api/v2/volumes/{volumeId}` | `Volume` + ETag | Name-based Docker identity is contained behind opaque URI encoding. |
| Secrets | `GET /api/v2/secrets` | collection | Metadata only. |
| Secrets | `POST /api/v2/secrets` | `201 Secret` | Returns ID and `Location`; data is write-only; idempotency key required. |
| Secrets | `DELETE /api/v2/secrets/{secretId}` | Operation | Data never appears in operation/error/audit payloads. |
| Configs | `GET /api/v2/configs` | collection | Metadata only. |
| Configs | `POST /api/v2/configs` | `201 Config` | Explicit UTF-8 text or base64 bytes discriminator; `Location`. |
| Configs | `GET /api/v2/configs/{configId}` | `Config` + ETag | Data returned only with `configs:read-data`; otherwise metadata. |
| Configs | `DELETE /api/v2/configs/{configId}` | Operation | `If-Match`, idempotency key. |
| Metrics | `GET /api/v2/metrics/snapshot` | `MetricsSnapshot` | Filters: `nodeId`, `serviceId`; includes freshness and partial status. |
| Metrics | `GET /api/v2/metrics/series` | `MetricSeries` | Explicit `from`, `to`, `step`, series selectors; bounded points. |
| Metrics | `GET /api/v2/metrics/status` | `MetricsStatus` | Ingestion freshness, gaps, storage health; privileged topology redacted. |
| Images | `GET /api/v2/images` | collection | Filter `nodeId`, `reference`; per-node origin explicit. |
| Images | `DELETE /api/v2/images/{imageId}` | Operation | Body/query uses one canonical `nodeId`; force explicit. |
| Images | `POST /api/v2/image-prune-operations` | Operation | Scope body (`nodeIds` or cluster) and preview/confirmation semantics. |
| Settings | `GET /api/v2/settings` | typed `ControllerSettings` + ETag | Server-owned settings only. |
| Settings | `PATCH /api/v2/settings` | typed result + ETag | Merge patch, unknown keys rejected, `If-Match`. |
| Operations | `GET /api/v2/operations` | collection | Filter `status`, `kind`, `targetId`, time range. |
| Operations | `GET /api/v2/operations/{operationId}` | `Operation` + ETag | Durable terminal error/result and per-node outcomes. |
| Operations | `POST /api/v2/operations/{operationId}/actions/cancel` | `202`/`409` | Best effort, itself idempotent. |
| Events | `GET /api/v2/events` | SSE | Authorized operation/resource events; `Last-Event-ID` resume. |
| Logs | `POST /api/v2/log-stream-tickets` | `201 StreamTicket` | Restricted fallback only; native clients normally use header auth. |
| Logs | `GET /api/v2/streams/logs` | WebSocket | Subprotocol `kontainer.logs.v2`; multiplexed targets and resume. |
| Exec | `POST /api/v2/exec-stream-tickets` | `201 StreamTicket` | Scoped to exact task/container, user, shell, and protocol. |
| Exec | `GET /api/v2/exec` | WebSocket | Fixed path, target in first `Open`; subprotocol `kontainer.exec.v2`; binary protobuf; not resumable in baseline. |

Aliases such as both `/tasks?serviceId=` and `/services/{id}/tasks` are not both first-class in v2. The canonical collection plus filters is the generated-client surface. UI convenience methods live in the client SDK.

Route disposition is explicit:

- **Accepted for the v2 production baseline:** the catalog/root/capability surface; setup/auth/identity; read and v1-parity mutations for services, tasks, nodes, stacks, networks, volumes, secrets, configs, metrics, images, and typed controller settings; durable operations; SSE events; versioned log and exec WebSockets.
- **Rejected:** an arbitrary Docker socket/HTTP/command proxy; default raw Docker specs; bearer tokens in URLs; the OAuth token-delivery WebSocket; direct React-to-agent networking; duplicate nested collection aliases; a generic `/rpc` or GraphQL route; arbitrary key/value settings; public follower addresses; and `/api` aliasing v2.
- **Deferred behind capability/version work:** stack deployment/update, network and volume mutations, secret/config rotation helpers, Kubernetes/Compose backend-specific resources, detachable exec, public protobuf/CBOR media types, public gRPC/Connect, browser-direct stream tickets, exact list totals, and a read-only GraphQL analytics facade. Deferred routes do not appear as empty/stub endpoints; capability absence is authoritative until their contract is accepted.

## Discovery and capabilities

`/.well-known/api-catalog` lists the supported versioned API base and its OpenAPI document using the standard linkset format. `/api/v2` then returns a small runtime root:

```json
{
  "apiVersion": "2.1",
  "serverVersion": "2.4.0",
  "deploymentId": "0199...",
  "setupState": "complete",
  "links": {
    "openapi": "/api/v2/openapi.json",
    "capabilities": "/api/v2/capabilities"
  }
}
```

Authenticated capabilities are machine-readable, cacheable with ETag, and include:

- resources and actions enabled by server build, orchestration backend, license/configuration, and caller authorization;
- internal feature state projected safely, such as `logs.resume`, `exec.resize`, `metrics.history`, and supported service modes;
- supported public API minor range, stream subprotocol versions, maximum page/frame/body limits, and operation retention;
- unavailable capabilities with stable reason codes, not internal addresses or secrets.

Clients use capabilities to hide or disable actions but MUST still handle `403`, `409`, and `501` because authorization and runtime state can change. Unknown capabilities are ignored. Absence means unsupported, not false.

## Collections: pagination, filtering, sorting, and partial results

Every collection uses one envelope:

```json
{
  "items": [],
  "page": {
    "limit": 50,
    "nextCursor": null,
    "snapshotAt": "2026-08-19T17:20:31.123Z"
  },
  "partial": false,
  "warnings": []
}
```

Conventions:

- `limit` defaults to 50 and is capped at 100. `cursor` is an opaque base64url token; clients do not decode or synthesize it.
- A cursor is integrity-protected, expires, and is bound to principal, route, filters, sort, and snapshot. Reusing it with changed inputs returns `400 cursor_invalid`.
- Domain filters are explicit OpenAPI parameters (`stack`, repeated `state`, `nodeId`, `serviceId`, and so on). There is no free-form filter language in v2.
- `sort=updatedAt,-name` uses comma-separated allow-listed fields and `-` for descending. A stable unique ID is always an implicit final tie-breaker. Unsupported sort/filter/query keys return `400`.
- `total` is absent by default because cluster-wide exact counts can be expensive. An endpoint may document `includeTotal=true`; its result can be null/approximate with an explicit accuracy field.
- Cluster fan-out never silently drops a node. A usable subset is `200` with `partial:true` and stable warning objects containing safe node IDs/codes. If no meaningful result exists, return `503` Problem Details. Authorization failures are not converted to partial success.
- Cursors across changing Swarm state are snapshot-consistent where storage permits; otherwise the response declares `consistency:"bestEffort"`, and clients tolerate duplicates/missing items across pages.

## Mutations, idempotency, and operations

### Idempotency keys

Every non-inherently-safe mutation that creates a resource or operation requires `Idempotency-Key`. The active IETF work describes this use for fault-tolerant `POST`/`PATCH`, but the most recent [draft-ietf-httpapi-idempotency-key-header-07](https://datatracker.ietf.org/doc/draft-ietf-httpapi-idempotency-key-header/07/) expired in April 2026 and is not an RFC. Kontainer therefore owns and versions these semantics rather than claiming standards compliance:

- key is 16–64 characters of URL-safe high-entropy text; SDKs generate UUIDv7 values;
- scope is authenticated principal + API major + method + canonical target + key;
- server stores request digest, status, headers, response/operation ID, and expiry for 24 hours;
- exact replay returns the original logical outcome and `Idempotency-Replayed: true`;
- same scoped key with a different canonical request returns `409 idempotency_key_reused`;
- an in-flight duplicate returns the same operation, not a second action;
- the raw key is hashed/redacted in logs and never propagated to Docker;
- clients do not retry mutations without a key. SDK automatic retry is limited to documented transient transport failures and the same key.

HTTP method idempotence alone is insufficient for `restart`, prune, and other action semantics. Explicit action resources make that fact visible.

### Long-running operation resource

`202 Accepted` only means the request was accepted; [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html#name-202-accepted) does not promise completion. Kontainer therefore persists an operation before scheduling work and returns:

```http
HTTP/1.1 202 Accepted
Location: /api/v2/operations/0199...
Retry-After: 1
ETag: "op-7"
```

```json
{
  "id": "0199...",
  "kind": "service.scale",
  "status": "queued",
  "target": { "type": "service", "id": "abc" },
  "submittedAt": "2026-08-19T17:20:31.123Z",
  "startedAt": null,
  "finishedAt": null,
  "progress": null,
  "result": null,
  "error": null,
  "links": { "self": "/api/v2/operations/0199..." }
}
```

Status is `queued`, `running`, `succeeded`, `failed`, `cancelRequested`, or `cancelled`. Terminal results are immutable. Multi-node operations include one outcome per requested node and an aggregate policy. Cancellation is best effort; an already terminal or uncancellable operation returns `409 operation_not_cancellable`. Operations are retained for at least 24 hours and preferably seven days; discovery reports the effective retention.

A very fast resource creation may return `201 Created` with `Location` and the final representation. Any mutation whose Docker convergence, fan-out, retry, or compensation continues after the response is an operation. “Accepted” and “converged” are separate timestamps/states.

## Optimistic concurrency and caching

- Every mutable singleton representation has a strong ETag derived from its canonical resource revision, not a timestamp. Public responses also expose an opaque string `revision` when useful to CLIs.
- `PATCH`, action calls dependent on current state, and destructive calls require `If-Match`. Missing preconditions return `428`; mismatches return `412 resource_revision_mismatch` plus the current ETag when disclosure is allowed.
- `If-Match: *` is allowed only for explicitly documented existence guards, never as a UI default.
- Reads support `If-None-Match` and `304`. Immutable schema/build assets use long-lived caching; identity, secrets/config data, auth, and setup use `Cache-Control: no-store`.
- A semantic state conflict unrelated to a representation revision uses `409`, such as scaling a global service.
- List ETags may be weak. A list cursor captures the snapshot/consistency contract and is not interchangeable with an item ETag.
- For Docker-backed objects, adapters translate Docker's version index into an opaque ETag and re-check it immediately before mutation. A successful operation records both submitted and observed revisions.

## Validation and consistent errors

All machine API errors, including pre-upgrade stream failures, use `application/problem+json`:

```json
{
  "type": "https://docs.kontainer.dev/problems/validation-failed",
  "title": "Request validation failed",
  "status": 422,
  "detail": "One or more fields are invalid.",
  "instance": "urn:kontainer:request:0199...",
  "code": "validation_failed",
  "requestId": "0199...",
  "retryable": false,
  "fields": [
    { "path": "/replicas", "code": "out_of_range", "message": "Must be at most 10000." }
  ]
}
```

`type`, `code`, and field `code` are stable automation contracts. `title` and `message` are display text and may be localized later. `detail` is safe and never contains a raw Docker, database, OAuth provider, filesystem, command, secret/config value, or network-address error. A privileged diagnostic log can be located using `requestId`.

Status mapping:

| HTTP | Use |
|---|---|
| `400` | Malformed JSON/query/cursor, unknown fields or parameters, mutually exclusive inputs. |
| `401` | Missing/invalid/expired access token; include appropriate `WWW-Authenticate: Bearer`. |
| `403` | Authenticated but missing scope/policy or organization membership. |
| `404` | Resource not found or intentionally concealed. |
| `409` | Valid request conflicts with current semantic state or reuses an idempotency key with different input. |
| `412` | `If-Match` failed. |
| `413` | Compressed or decompressed body exceeds the route limit. |
| `415` | Unsupported request media type/encoding. |
| `422` | Syntactically valid body fails field/domain validation. |
| `428` | Required `If-Match` is absent. |
| `429` | Principal/IP concurrency or request rate exceeded; include `Retry-After`. |
| `501` | Capability is known by this API version but unsupported by the selected backend/build. |
| `503` | Required controller/cluster dependency unavailable; include bounded `Retry-After` when known. |
| `504` | A synchronous dependency deadline elapsed before a meaningful response. Long work should have returned an operation instead. |

The current IETF RateLimit header work remains a [May 2026 Internet-Draft](https://datatracker.ietf.org/doc/draft-ietf-httpapi-ratelimit-headers/) rather than an RFC. v2 baseline guarantees `429` and `Retry-After`; additional `RateLimit` fields may be added after pinning a final standard or a versioned documented profile.

Internal errors map to gRPC statuses by meaning, not by blanket `Internal`: invalid input → `InvalidArgument`, auth → `Unauthenticated`/`PermissionDenied`, missing → `NotFound`, revision/state races → `Aborted` or `FailedPrecondition`, quota/limits → `ResourceExhausted`, dependency outage → `Unavailable`, deadline → `DeadlineExceeded`, and unexpected invariant failure → `Internal`. Public adapters translate typed domain errors to Problem Details and never pass through an internal status message verbatim.

## Authentication, authorization, and setup

### API credentials

- All protected HTTP and WebSocket handshakes use `Authorization: Bearer <access-token>` over TLS. Tokens are forbidden in URLs, form fields, WebSocket subprotocol strings, log attributes, and error bodies.
- Access tokens are short-lived and audience-bound to the deployment. Refresh credentials rotate on every use and are revocable; reuse detection invalidates the token family.
- `401` responses follow RFC 6750 `WWW-Authenticate` semantics. `403` denotes a valid identity lacking required scope.
- Tokens carry/resolve a stable subject, deployment audience, issued/expiry times, token ID, roles/scopes, and authentication session. Authorization is checked both at route/action level and on each target.
- Initial scopes should be resource/action based: `services:read`, `services:mutate`, `exec:open`, `logs:read`, `secrets:manage`, `configs:read-data`, `nodes:admin`, `settings:write`, and similar. Capabilities are the caller-filtered projection of these permissions and backend support.
- The native desktop stores refresh credentials in the OS credential store. Its ordinary profile store contains only non-secret metadata and a credential reference. Access tokens remain in native memory and are never returned to React.
- CLI interactive auth uses the same authorization-session flow. Non-interactive automation requires separately revocable service credentials or personal access tokens with explicit scopes and expiry; it must not reuse a desktop refresh token.

### Native authorization transaction

`POST /api/v2/auth/sessions` accepts provider, PKCE challenge, client nonce, and callback capability, then returns an opaque session ID, browser authorization URL, expiration, polling interval, and a separate high-entropy possession verifier. The native client opens the system browser and polls `GET /auth/sessions/{id}` with that verifier in a dedicated authorization header, never in the URL. Provider callback marks the transaction complete; the token response is returned once with `Cache-Control: no-store`. Polling too quickly returns `429` with `Retry-After`.

This replaces the public `/auth/listen` WebSocket and prevents a bearer token from crossing a browser bridge. OAuth state is single-use, integrity protected, bound to provider/session/PKCE, and expires. The browser callback displays a generic completion page without token, organization membership details, provider error dumps, or deep-link secrets.

### Setup

`GET /api/v2/setup/status` exposes only `required`, `inProgress`, or `complete`; it never validates a presented token. A setup credential is exchanged with `POST /setup/sessions` through a dedicated header/body that is automatically redacted. The resulting session is short-lived, single-purpose, rate-limited, and bound to the requesting client.

Bootstrap credentials contain at least 128 bits of entropy, are never placed in a URL, and are printed only to the explicit operator channel with a one-time warning. Completion is a transaction: validate provider/application configuration, persist encrypted secrets, initialize signing keys, verify login capability, then commit complete. Failure leaves setup resumable rather than half-complete.

## Settings boundaries

There are three distinct settings classes:

1. **Desktop-local preferences**: theme, terminal font/shell/scrollback, refresh behavior, window state, and profile metadata. These never use the agent settings API. Sensitive profile credentials live in the OS credential store.
2. **Controller runtime settings**: the typed `/api/v2/settings` resource. Each property has schema, validation, default/source (`default`, `file`, `environment`, `stored`), mutability, restart requirement, and sensitivity metadata.
3. **Deployment/agent configuration**: network identity, mTLS material, role, storage path, hard security limits, and environment-provided secrets. These are read-only capability/diagnostic facts or operator configuration, not arbitrary remote settings.

Settings use merge-patch with `If-Match`; omission means unchanged, explicit `null` resets only resettable properties, unknown keys are rejected, and the response returns the effective sanitized representation and new ETag. Secret values are write-only credential references or redacted as `{ "configured": true }`; placeholder strings can never be written back as values. Every accepted change creates an audit entry and, where application is asynchronous or needs restart, an operation.

## Streaming protocols

### Shared rules

- Stream endpoints require TLS and exact `Sec-WebSocket-Protocol` negotiation (`kontainer.logs.v2` or `kontainer.exec.v2`) under the [WebSocket protocol](https://www.rfc-editor.org/rfc/rfc6455.html). Missing/unsupported subprotocol fails before upgrade with `400` Problem Details and an advertised supported list; a non-WebSocket request may use `426` with the required upgrade metadata.
- Native clients authenticate in the handshake header. A one-time stream ticket exists only for clients that cannot forward that header. After an otherwise unauthenticated upgrade, the ticket is the only permitted first protobuf envelope; the server allows no target work and closes within five seconds if authentication fails. Tickets never appear in a query parameter, cookie, or subprotocol.
- A ticket expires within 30 seconds, is single-use, stored hashed, and bound to principal, target(s), action, origin/client session, protocol, and options. Replay fails before upgrade.
- One binary WebSocket message contains one length-checked protobuf envelope. JSON control frames are not mixed with terminal bytes.
- Initial client `Open` and server `Ready` envelopes negotiate protocol minor, compression (off by default), heartbeat, maximum message/frame sizes, and capability options. Unsupported mandatory options close with a protocol error.
- Common envelope metadata is `protocol_version`, `session_id`, `request_id`, `sequence`, `kind`, and one typed payload. Data messages carry target/channel identity and monotonic per-session sequence.
- Application events are `ready`, `data`, `warning`, `gap`, `error`, `heartbeat`, and `end`. Errors have stable code/retryability and a safe message. WebSocket close codes communicate transport/session termination, not a Docker error dump.
- Standard WebSocket ping/pong provides transport liveness (15-second initial interval, close after two missed intervals). An application heartbeat is used only when it advances state such as a log resume lease; exec does not duplicate ping/pong. Backpressure is bounded; slow consumers receive `gap` or are closed rather than growing memory without bound.
- Stream authorization is rechecked for long sessions and immediately after credential/session revocation where practical.

Axum's permissive WebSocket defaults must be overridden. Initial defaults are 64 KiB maximum application message, 64 KiB maximum frame, 1 MiB total per-stream outbound queue, 32 concurrently subscribed log sources per connection, and four concurrent exec sessions per principal. Discovery reports effective values.

### Logs

The client `OpenLogs` contains subscriptions with exact service/task targets, stdout/stderr selection, timestamp mode, optional `tailLines` (default 100, maximum 10,000), optional `since`, and an optional opaque resume token. The server returns normalized UTF-8 line/chunk events and preserves blank lines; invalid bytes have explicit lossy/binary flags rather than silent rewriting.

Delivery is at least once. Each event has a monotonic sequence and opaque resume cursor. A cursor is bound to principal, subscription set/options, server instance/storage epoch, and expiry. Reconnect sends the cursor and uses exponential backoff with jitter. Duplicate sequence values may be replayed and clients deduplicate. If the bounded replay window cannot cover the cursor, the server emits a `gap` with first available position; it never silently jumps to a new tail.

Multiplexed subscriptions have explicit add/remove controls and acknowledgements. Per-target Docker failures are target-scoped warnings/errors and do not necessarily terminate other subscriptions. Memory, line length, reconnect attempts, replay age, and fan-out are bounded by the logging design.

### Exec

`OpenExec` identifies an authorized task/container target, requested shell argv/profile, user, terminal dimensions, and environment keys from an allow-list. The server resolves routing; the client does not choose an arbitrary follower address. Target syntax, dimensions, user, and shell are validated before process creation.

Typed messages are `stdin`, `stdout`, `stderr`, `resize`, `signal`, `exit`, `error`, and `heartbeat`. Every input and output is binary-safe. `Exit` contains the real exit code/signal when known and an explicit `unknown` outcome otherwise; EOF is never invented as exit 0. Follower routing passes the user and all negotiated fields consistently.

Exec is not resumable in the v2 baseline: connection loss terminates or detaches according to an explicit policy, and reopening creates a new process. A future detachable-session capability requires durable PTY ownership, one-time reattach tickets, audit policy, timeout, and separate protocol negotiation.

### SSE operations and resource events

`GET /api/v2/events` serves `text/event-stream` to authenticated native clients. Filters are allow-listed operation/resource kinds and IDs. Events contain a stable `id`, `event` type, and one JSON data object with `apiVersion`, `requestId`, resource reference, revision, and a minimal change summary.

Clients reconnect with `Last-Event-ID`. Delivery is at least once, order is guaranteed only within the documented deployment/event partition, and consumers refetch the resource after notifications. Expired cursors emit a `reset` event and require a fresh list/read. Heartbeat comments keep intermediaries alive. Browser `EventSource` without an Authorization header is not a supported direct React path; Tauri/GPUI/CLI use the native HTTP client.

## Internal Tonic/protobuf boundary

### Service design

Use versioned packages such as `kontainer.agent.v2`, `kontainer.controller.v2`, and `kontainer.enrollment.v2`, not `agent`. Split the generic service into bounded responsibilities consistent with the cluster communication design:

- standard `grpc.health.v1.Health` with `Check` and `Watch`;
- agent-served `AgentInfo.Describe`: attested identity, role, build/protocol/capability negotiation;
- agent-served `NodeMetrics.CollectSnapshot`: parity/diagnostic snapshot path;
- agent-served `ImageInventory.ListImages` and `NodeMaintenance.RemoveImage`/`PruneImages`: bounded inventory and allow-listed node actions;
- agent-served `ContainerExec.Exec`: typed bidi PTY messages;
- controller-served `TelemetryIngest.Export`: metric batches, acknowledgements, and replay/spool coordination;
- enrollment-only `BootstrapEnrollment.Enroll` and `CertificateRenewal.Renew` on their restricted listeners;
- a typed container-log stream only if remote-node log collection cannot use an existing bounded inventory/stream service; it retains the same cursor/gap semantics and never becomes a generic Docker stream proxy.

There is no generic Docker socket proxy, arbitrary method/path tunnel, arbitrary command RPC, or opaque `rawSpec` mutation. Each RPC has an authorization policy, maximum size, deadline, idempotency classification, retry policy, and audit event.

### Security and identity

- All production internal connections use HTTP/2 gRPC with mandatory mTLS. Certificates identify deployment, workload/agent instance, and role; both peers validate trust, SAN, expiry, and revocation/rotation state.
- The static `x-kontainer-secret` is removed. During migration it may authenticate only an isolated legacy service on a separate listener; it is never accepted by v2 services.
- Authorization checks the authenticated certificate identity against registry membership and allowed RPC/target scope. Message `node_id` is asserted data, not identity.
- Discovery registers an agent instance ID, node ID, task ID, addresses, certificate identity, protocol range, capabilities, and lease. Multiple instances on one Swarm node do not overwrite one another.
- Public responses expose stable agent/node IDs and health/capabilities only. Internal addresses, certificate material, and discovery tokens are privileged diagnostics.

### Deadlines, retries, size, and flow control

- Every unary and stream establishment call has an explicit deadline propagated from the public request/operation budget. Server work observes cancellation. No library default is accepted silently.
- Suggested initial unary deadlines: health 2 seconds, identity/capabilities 3 seconds, local read 5 seconds, inventory page 10 seconds, and action acceptance 10 seconds. Long execution continues under a durable operation, not a longer public request.
- Automatic retry applies only to calls documented as safe/idempotent, only for selected transient statuses, with bounded exponential backoff, jitter, attempt count, and remaining deadline. Mutating RPCs carry the operation/idempotency identity and deduplicate on the agent.
- Configure Tonic encode/decode limits per method: 64 KiB health/identity/control, 1 MiB inventory pages, and 256 KiB stream message chunks unless a design explicitly proves another bound. Large datasets are paged/streamed, not sent as one 4 MB object.
- Configure per-connection and global concurrency, HTTP/2 stream/window/keepalive settings, and load shedding. Tonic exposes these server controls ([transport server API](https://docs.rs/tonic/latest/tonic/transport/server/struct.Server.html)); overload maps to `ResourceExhausted`/`Unavailable` according to whether the constraint is a caller quota or server health.
- Each streamed message has sequence, maximum chunk size, bounded queues, acknowledgements where replay matters, and explicit gaps. A stalled consumer cannot hold unbounded Docker readers or metric buffers.

### Protobuf evolution

- Package major versions are immutable. Add fields with new numbers; never reuse field numbers or enum numeric values; reserve removed names/numbers.
- Use `optional` or wrapper/message presence when absent differs from the scalar default. Every enum begins with `*_UNSPECIFIED = 0`; receivers preserve/report unknown enum numeric values. Prost generates idiomatic Rust and retains unknown enum values through its integer representation as documented by [Prost](https://docs.rs/prost/latest/prost/).
- Do not expose protobuf maps where entry order, per-key validation, or future metadata matters; use repeated entry messages.
- Oneof payloads define stream frames and typed action results. Bytes are `bytes`, not base64 strings internally.
- Controller and agents support the current and previous internal major during rolling upgrade unless an explicit release plan says otherwise. The identity handshake reports min/max major/minor and capability set before functional calls.
- CI runs protobuf lint and breaking-change checks against the last released descriptors and rolling N/N-1 integration tests.

## Public DTOs and code generation

### Rust types

Use explicit conversion layers:

```text
HTTP JSON DTO ↔ application command/query ↔ domain value/object
protobuf DTO ↔ application command/query ↔ domain value/object
persistence record ↔ domain value/object
desktop IPC view model ↔ kontainer-client DTO
```

The public DTO crate is dependency-light, uses explicit Serde names/nullability, denies unknown request fields, and has no Axum, Tonic, Docker, persistence, or Tauri dependency. Response DTOs do not use `skip_serializing_if` unless omission is the specified wire meaning. Numeric and time wrappers centralize JSON-safe encoding.

Published Rust client enums that may gain variants use `#[non_exhaustive]` as defined by the [Rust Reference](https://doc.rust-lang.org/reference/attributes/type_system.html#the-non_exhaustive-attribute), while the wire decoder retains an unknown raw representation so an older client can display rather than reject a newer value.

Domain types do not derive public serialization merely for convenience. Conversions are fallible where validation is possible and are covered with fixture tests. Sensitive fields use wrapper types whose `Debug`, `Display`, serialization, tracing, and error behavior redact by default.

### OpenAPI and clients

- Utoipa annotations/schema types generate OpenAPI 3.1.x during CI/build tooling, but the normalized `openapi.json` is checked in as the reviewable public contract.
- CI fails when generated output differs from the checked-in schema. A breaking-change checker compares it with the last release; exceptions require an approved API-major decision.
- TypeScript HTTP models/client code are generated from OpenAPI. Hand-written UI types may compose generated models but may not redefine their wire shapes.
- A Rust `kontainer-client` offers typed request builders, auth hooks, pagination streams, operations wait/cancel, ETag handling, Problem Details, SSE, and WSS protocols. It can use generated core models plus hand-written ergonomic transport without importing server code.
- Other supported SDKs are generated from the same checked-in artifact and verified against the server. Generated output includes an identifying schema hash/version.
- Internal Rust code is generated from checked-in `.proto` files by Prost/Tonic. Internal protobuf definitions are not used to generate the public JSON API.
- Tauri command/channel bindings may be generated with a qualified tool, but the current Specta TypeScript package is still labeled against [Specta 2.0 release candidates](https://docs.rs/specta-typescript/latest/specta_typescript/). Pinning it is a separate dependency/maintenance decision. Until accepted, small IPC models remain explicit and contract-tested.

## Tauri, GPUI, and CLI readiness

### Tauri

- One managed native API client per profile reuses connection pools, TLS policy, refresh coordination, rate state, and request defaults.
- Tauri commands invoke `kontainer-client`; commands return typed success or a sanitized IPC error containing `code`, `message`, `requestId`, `retryable`, field errors, and optional operation ID.
- Long streams use Tauri channels/events with bounded buffers and cancellation handles. React does not create `fetch`, WebSocket, or EventSource connections to an agent.
- Profile metadata is separate from credential references. No command returns a refresh or bearer token to JavaScript. JWT claims decoded client-side are hints only and never authorization evidence.
- URL construction is performed by a typed URL library with encoded path/query values.

### GPUI

A future GPUI application consumes the same `kontainer-client` directly. UI state/event adapters are GPUI-specific, but protocol, authentication refresh, operation polling, pagination, SSE, and stream protobuf codecs are shared. This makes a UI rewrite independent of server deployment and prevents the public server from importing desktop framework concepts.

### CLI and automation

The public contract is usable without the desktop:

- stable OpenAPI operation IDs and resource URLs;
- non-interactive scoped credentials with expiry/revocation;
- deterministic pagination and sorting;
- idempotency keys, `If-Match`, `--wait`/`--no-wait`, operation URLs, and meaningful exit statuses;
- machine JSON output that preserves Problem Details and partial warnings; human output is a presentation layer;
- `--request-id` displays the server ID but cannot replace it; trace context is opt-in;
- no interactive HTML is required after a credential is provisioned.

## Limits, timeouts, and load policy

Initial public defaults:

| Limit | Default |
|---|---|
| Request header section | 16 KiB total; individual values further bounded |
| Request target | 8 KiB |
| Normal JSON body | 256 KiB decompressed |
| Secret/config data body | 500 KiB decompressed, with a smaller backend-safe effective data limit reported by capabilities |
| Future stack/spec upload | 2 MiB only on its dedicated route |
| JSON response | 8 MiB; larger data must page/stream |
| Collection page | 50 default, 100 maximum |
| Synchronous liveness/read | 2–15 seconds by route |
| Mutation acceptance | 10 seconds; continued work becomes an operation |
| Auth/setup session | 10 minutes maximum; stream ticket 30 seconds |
| WebSocket message/frame | 64 KiB each |
| SSE event | 64 KiB; consumers refetch full resources |

Limits apply to compressed and decompressed representations, nesting depth, collection/string/map counts, and streaming aggregates; content encoding is not a bypass. Route-local layers must enforce the documented value rather than inherit Axum/Tower defaults accidentally.

Rate and concurrency policy is keyed by deployment, authenticated principal, credential, source network where appropriate, route cost, target, and live stream count. Auth/setup also has IP and transaction throttles. Rejections return Problem Details, `429`, and `Retry-After`. Effective coarse limits are discoverable; internal security thresholds are not.

Server deadlines include queue time and propagate to downstream gRPC. Timeouts do not cancel already accepted operations; their operation remains queryable. Clients use a slightly larger transport deadline than the server budget and never implement unbounded retries.

## Redaction, privacy, and audit

The following are always classified sensitive: Authorization/Cookie headers, refresh/access/setup credentials, stream tickets/resume tokens, OAuth codes/state/verifiers, mTLS keys/cert bootstrap material, secret/config data, registry credentials, environment values matching sensitivity policy, shell input/output by default, internal addresses, raw provider/Docker/database errors, and idempotency keys.

- Structured tracing has an allow-list, not a deny-list. It records method, route template, status, duration, request ID, trace ID, principal ID, safe target IDs, operation ID, byte counts, and stable error code.
- Request bodies, query strings, headers, log lines, terminal bytes, secret/config contents, and settings values are not logged. Sensitive wrapper types redact `Debug` and serialization.
- Audit records capture authenticated principal, action, target, submitted/terminal outcome, request/operation IDs, safe changed-field names, policy decision, and timestamp. They do not capture payload values.
- Problem Details and stream errors contain safe summaries plus `requestId`; detailed causal chains remain in protected logs.
- `/agents`, capabilities, metrics status, and readiness reveal no internal address, certificate identity, stack trace, storage path, or provider response unless a separate privileged diagnostic scope explicitly requests a sanitized diagnostic resource.
- Compression is disabled for credential and secret-bearing responses to reduce cross-origin compression leakage.

## Versioning, migration, and deprecation

### Version rules

- Public breaking changes create a new path major (`/api/v3`). Additive fields, endpoints, optional request inputs, and enum values advance a documented minor version while preserving old-client behavior.
- OpenAPI `info.version`, `Kontainer-API-Version`, capability min/max, client user agent, and telemetry make negotiated behavior observable. Clients fail clearly when the server major is unsupported.
- Stream subprotocol names carry a major. Minor negotiation occurs in `Open`/`Ready`; unknown mandatory features abort before data.
- Internal protobuf packages carry a major. Minor capability negotiation is explicit, and rolling upgrades support current/previous major as release policy.
- Database schema, desktop IPC, public HTTP, stream, and protobuf versions are independent; none is inferred from the binary version alone.

### Migration stages

1. Freeze v1 fixtures from the Go server, Rust Tauri decoder, TypeScript types, WebSocket frames, and protobuf descriptors—including the mismatches in this document.
2. Ship `/api/v2` discovery and read-only resources alongside v1. The new native client probes the catalog/capabilities and selects v2; it falls back only when configured to support v1.
3. Add v2 operations, preconditions, auth custody, settings, and versioned streams. Run visible-parity tests against both stacks.
4. Move follower traffic to Tonic mTLS services with mixed-version controller/agent tests. Remove public leakage of internal addresses.
5. Stop enabling the optional v1 adapter by default after the announced support window. Remove it only after route-use telemetry and release policy permit.

There is no transparent v1/v2 response mixing. A request stays in one protocol version from authentication/capability discovery through its operation and streams. Migration tooling converts local profiles to credential references and removes plaintext tokens after verified secure-store persistence; it does not silently delete the only credential copy.

### Deprecation

- Deprecated operations are marked `deprecated: true` in OpenAPI and return `Deprecation: @<unix-seconds>` per RFC 9745 plus `Link: <...>; rel="deprecation"` to migration guidance.
- Once a removal date is committed, add `Sunset: <HTTP-date>` per RFC 8594 and a link to the successor. `Deprecation` states that retirement has begun; `Sunset` states when the endpoint is expected to become unavailable.
- Announce deprecation in release notes and discovery/capabilities, measure use by route/client major without logging sensitive inputs, and test the headers.
- No endpoint is removed merely because callers appear absent in a short telemetry window. The exact minimum support window is an open product decision below.

## Contract verification strategy

Required automated gates:

- normalize/diff generated OpenAPI against checked-in `openapi.json`; lint operation IDs, tags, security, examples, status responses, limits, nullability, and `additionalProperties`;
- run an OpenAPI breaking-change check against the last released v2 schema;
- compile and run Rust and TypeScript generated-client conformance suites against an Axum test server;
- maintain golden request/response/error fixtures for every route, including unknown fields/enums, max/min values, JSON-safe integer strings, timestamps, nullability, and partial results;
- freeze legacy Go/Tauri/TypeScript fixtures and prove the optional v1 adapter matches the actual v1 server, including known contradictory client fixtures as expected failures or repaired-v1 cases;
- property/fuzz test URI encoding, cursor validation, Problem Details redaction, DTO/domain conversions, ETag races, idempotency replay/conflict, and decompression/nesting limits;
- test each mutation for duplicate delivery, controller restart after acceptance, operation durability, cancellation, partial node outcomes, and audit linkage;
- lint protobuf and check descriptors for breaking changes; run current/previous controller-agent combinations, deadline/cancellation, mTLS identity/rotation, retry/deduplication, size, and overload tests;
- use binary golden frames plus state-machine/fuzz tests for logs and exec: negotiation, fragmentation, invalid lengths, backpressure, heartbeat, duplicates, expired cursor, explicit gap, exit status, and ticket replay;
- test OAuth/setup state expiry/replay, PKCE, provider denial, organization policy, refresh rotation/reuse, secure-store migration, `WWW-Authenticate`, origin policy, and token redaction;
- run CLI smoke tests for discovery, pagination, JSON output, conditional mutation, idempotent retry, operation wait/cancel, partial warning, and every Problem status;
- assert React bundles and network tests cannot access agent credentials or open direct agent HTTP/WS connections.

Contract tests are release artifacts. A code refactor cannot update golden wire files implicitly; schema changes require an explicit review describing compatibility impact.

## Acceptance checklist

### v1 audit and parity

- [ ] Every route and gRPC RPC in this inventory has a frozen v1 fixture and an owner decision: migrate, legacy-only, or remove with rationale.
- [ ] Every listed Tauri/server/TypeScript mismatch has a regression test.
- [ ] Every v1 visible workflow passes on the v2 desktop/server combination.
- [ ] Migration notes enumerate all intentional wire breaks; `/api` is not an alias for v2.

### Public HTTP

- [ ] API catalog, API root, OpenAPI, and caller-filtered capabilities agree on versions/routes/features/limits.
- [ ] Every collection is bounded, deterministically ordered, cursor-tested, and explicit about partial results.
- [ ] Every mutation declares authorization, validation, idempotency, precondition, synchronous/operation behavior, audit event, and retry policy.
- [ ] Every mutable resource returns ETag; required mutations reject missing/stale `If-Match` with `428`/`412`.
- [ ] Every error is safe Problem Details with stable code/request ID; auth challenges and retry headers are correct.
- [ ] Media types, compression, cache policy, request/response sizes, deadlines, rate limits, and unknown-field behavior are enforced per route.

### Authentication and secrets

- [ ] No access/refresh/setup credential or stream ticket appears in URL, WebView JavaScript, logs, traces, errors, profile JSON, or compression-eligible output.
- [ ] Authorization-session PKCE/state/expiry/replay and refresh rotation/reuse are tested.
- [ ] Setup status is not token verification; completion is atomic/resumable.
- [ ] Public agent/capability/health output does not leak cluster addresses or credential material.

### Streaming and internal protocols

- [ ] WebSocket subprotocol negotiation, protobuf envelope, frame/queue limits, heartbeat, ticket rules, and safe close/error behavior are implemented.
- [ ] Log resume/deduplication/gap behavior is deterministic; exec disconnect/exit semantics are explicit.
- [ ] SSE supports `Last-Event-ID`, reset, heartbeats, filters, and authorization.
- [ ] Internal services use versioned protobuf, mTLS identity/authorization, standard health, explicit deadline/retry/size/concurrency policy, and current/previous rolling tests.
- [ ] No internal RPC is an arbitrary Docker/HTTP/command proxy.

### Clients and generation

- [ ] Normalized OpenAPI and protobuf schemas are checked in, generated deterministically, linted, and breaking-diffed.
- [ ] Rust/TypeScript clients compile and pass live conformance; TypeScript wire types are not duplicated manually.
- [ ] Tauri reuses managed native clients and streams via bounded channels; React cannot reach the agent directly.
- [ ] `kontainer-client` works from Tauri, a GPUI proof harness, and CLI smoke tests without importing server/domain code.
- [ ] Local preferences, controller settings, deployment config, and credential storage have separate typed boundaries.

## Issue-ready open decisions

Each item can become one design issue with a named owner and deadline. None should be resolved accidentally during implementation.

1. **OpenAPI publication level** — Baseline recommendation: emit 3.1.x with Utoipa 5.5 and evaluate 3.2.0 only after the complete lint/diff/TypeScript/Rust generator matrix passes. Decide exact 3.1 patch and generator versions.
2. **Legacy adapter support window** — Decide whether v2 ships `legacy-v1`, the supported client versions, default enablement, minimum deprecation duration, and final sunset criteria. Recommendation: opt-in after one stable transition release; no `/api` alias.
3. **Public authentication credential format** — Choose opaque access tokens vs signed JWT access tokens, issuer/key rotation, audience, maximum lifetime, refresh-family persistence, and revocation latency. Preserve the header-only transport and native custody regardless.
4. **Desktop authorization callback** — Select polling with possession verifier (recommended baseline), registered loopback redirect, or OS deep link. Threat-model hijacking, multi-profile concurrency, firewall/proxy behavior, and CLI reuse.
5. **Automation identities** — Define PAT vs service-account issuance, scopes, expiry ceiling, revocation, audit attribution, headless bootstrap, and whether OAuth device authorization is needed.
6. **Authorization model** — Approve roles/scopes/policies and resource tenancy. Capability discovery cannot stabilize until action-level permissions are named.
7. **Resource identifiers** — Decide canonical opaque IDs for stack and volume name-backed Docker resources and migration/display-name rules.
8. **Operation storage** — Select durable store, retention, per-node result schema, crash recovery, deduplication transaction, cancellation policy, and garbage collection. Recommendation: operation and idempotency record commit atomically before dispatch.
9. **Idempotency profile** — Ratify key syntax, canonical request digest, scope, 24-hour retention, replay headers, and behavior after operation retention expiry, acknowledging the IETF document is currently expired.
10. **ETag revision source** — Define canonical hashing/version-index rules for Docker-backed and controller-owned resources, weak list ETags, and exact mutations that permit no precondition.
11. **Pagination consistency** — Choose snapshot storage vs best-effort per domain, cursor signing key rotation, TTL, and whether any route pays the cost for exact totals.
12. **Partial-result policy** — Define which fan-out resources may return partial `200`, minimum meaningful result, warning codes, and aggregation status rules.
13. **Settings schema** — Enumerate v2 controller settings, source precedence, mutability/restart requirements, reset behavior, sensitive credential-reference types, and audit requirements. Explicitly retire the arbitrary string map.
14. **Metrics numeric/time schema** — Ratify unit suffixes, JSON-safe decimal-string values, timestamp fields, series point caps, freshness, partial semantics, and alignment with the metrics storage design.
15. **Config data encoding** — Choose explicit `{encoding:"utf8",text:...}` / `{encoding:"base64",data:...}` union, permission to read data, maximum backend-safe bytes, and migration from v1's ambiguous base64/text behavior.
16. **Log protocol persistence** — Decide replay window/storage, cursor TTL/cryptography, line vs chunk semantics, maximum line handling, multiplexing caps, and whether one connection may span services and tasks.
17. **Exec disconnect policy** — Choose terminate-on-disconnect (recommended baseline) vs bounded detach; define signal, exit-unknown, idle timeout, shell allow-list, user policy, and audit metadata.
18. **SSE event retention/order** — Define storage, partition/order guarantee, cursor TTL, reset behavior, filters, and whether operation events alone ship first.
19. **Stream authentication fallback** — Confirm native Authorization-header handshake and whether any supported client truly requires one-time tickets. If tickets ship, specify the bounded first-frame authentication message, pre-authentication timeout/rate limits, and origin binding.
20. **mTLS identity and rotation** — Select CA/bootstrap mechanism, SAN identity format, certificate lifetime/rotation/revocation, discovery lease, multi-agent-per-node identity, and emergency recovery.
21. **Internal compatibility window** — Ratify current/previous protobuf major support, controller-first vs agent-first upgrade order, capability negotiation, and rejection behavior outside the range.
22. **Limits and quotas** — Load-test and approve every provisional body/frame/page/deadline/rate/concurrency value in this document; document operator override floors/ceilings.
23. **Public SDK generation** — Choose the Rust and TypeScript OpenAPI generators, normalization/breaking tools, generated-code ownership, release packaging, and schema hash mechanism.
24. **Tauri IPC generation** — Decide whether to qualify/pin Specta/tauri-specta or retain explicit IPC models with golden contract tests. This decision does not change the public API schema.
25. **RateLimit headers** — Ship `429` + `Retry-After` now; decide whether/when to adopt a future final IETF RateLimit field standard instead of pinning the current draft.
26. **Content encoding expansion** — Keep public REST JSON-only initially; require benchmarks and client/proxy compatibility evidence before adding public protobuf or CBOR.
27. **Deprecation service level** — Define minimum notice (recommended: at least one stable release and 90 days, longer for automation APIs), telemetry threshold, release-note channels, and emergency security exception process.
28. **Legacy secrets cleanup** — Specify safe migration from `profiles.json`/`session.json` plaintext tokens to OS credentials, verification before deletion, backups, logout/revocation, and redaction of support bundles.

## Definition of done

The v2 API boundary is production-ready when the route/schema artifacts, generated clients, Rust conversions, authorization policy, operation/idempotency store, preconditions, streams, mTLS services, limits, redaction, and mixed-version tests all satisfy this document's checklist—and when the desktop demonstrates v1 visible parity without relying on v1 wire bugs or giving the WebView an agent credential.

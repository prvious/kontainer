# Swarm resources in Kontainer v2

Status: proposed architecture and parity specification  
Research date: 2026-08-19  
Scope: Docker Engine and Swarm resource control in the Rust rewrite. The stack deployment reconciliation algorithm is intentionally out of scope.

## Executive decision

Kontainer v2 should use Bollard behind a Kontainer-owned `EngineClient` boundary. Every agent should connect to its local Docker Engine, negotiate an API version, publish capabilities, and expose domain operations rather than proxying arbitrary Docker endpoints. The leader should use its local manager Engine for Swarm-scoped resources and use authenticated agent RPC for resources that exist on a particular node.

This preserves the useful part of the v1 topology while fixing its main correctness gaps:

- Swarm mutations become version-aware rather than implicit read-modify-write operations with no client-visible conflict model.
- Desired state, observed state, rollout progress, and task history become separate concepts.
- Images, containers, stats, exec sessions, and ordinary local volumes have explicit node scope.
- Multi-node reads and writes report partial results instead of silently omitting failed followers.
- The REST/Tauri contract is generated or contract-tested from one set of DTOs.
- Capabilities are discovered from the daemon and the deployment topology; the UI does not infer them from a resource type or a Docker version string alone.

Docker Engine 29.7 currently exposes API 1.55 and accepts clients down to API 1.40. Docker documents API negotiation as choosing the highest mutually supported version and warns that `DOCKER_API_VERSION` disables negotiation ([Engine API overview](https://docs.docker.com/reference/api/engine/)). Bollard 0.21.x is generated against Moby API 1.52 and exposes `negotiate_version`, so a schema version slightly behind the newest daemon is expected and should not be confused with the negotiated wire version ([Bollard crate](https://docs.rs/bollard/latest/bollard/), [`Docker::negotiate_version`](https://docs.rs/bollard/latest/bollard/struct.Docker.html#method.negotiate_version)).

The recommended v2 baseline is API 1.41 for the management plane. It includes service status counts and replicated/global job modes, while still covering Docker Engine 20.10-era installations. An agent connected to an older Engine may start and report local health, but it should publish the unsupported capabilities instead of pretending the complete v2 Swarm contract is available. Whether v2 formally supports a degraded API 1.30–1.40 tier remains an open product decision.

## What v1 actually implements

The inventory below follows the concrete routes, controllers, actions, orchestrator interface, Tauri commands, hooks, and screens—not aspirational feature lists. The public `/api/*` routes exist only on the leader and require JWT. Followers expose internal gRPC and health endpoints. React calls a Tauri command, the Tauri command calls the Go leader, and the leader fans node-local work out to followers when needed.

Classification used in the matrices:

- **Parity**: reproduce the useful v1 behavior, with contract and safety repairs included.
- **Missing parity affordance**: UI/product affordance already implied by v1 but absent, a no-op, or currently broken across the Tauri/HTTP boundary.
- **Enhancement**: new v2 behavior needed for a production-grade control plane.
- **Deferred**: intentionally outside this resource layer.

### Cluster, services, and tasks

| Resource / operation | Existing v1 API and behavior | Existing UI/Tauri expectation | v2 disposition |
|---|---|---|---|
| Agent inventory | `GET /api/agents` returns `nodeId`, hostname, address, leader/follower role, health, and `lastSeen`. This describes Kontainer agents, not Engine or Swarm capabilities. | No complete cluster-capability view. | **Parity** plus capability records and explicit stale/unreachable state. |
| Health/setup | `/health` returns a basic status. `/ping` distinguishes `ok` from `setup_required`. | Login/setup uses these coarse states. | **Parity**; keep liveness separate from Docker readiness and Swarm readiness. |
| Cluster/Swarm info | No public cluster-info operation. Leader discovery internally reads Swarm tasks/nodes and pings followers. | Product screens need enough information to explain manager availability, node health, and feature support. | **Missing parity affordance**: add sanitized Engine and Swarm summaries; never return join tokens or daemon security material. |
| Swarm initialize/update/leave/unlock | No v1 public operations. Existing setup assumes an already usable Engine/Swarm environment. | No established UI contract. | **Deferred** pending the membership/bootstrap security design; capability discovery must still explain `inactive`, `pending`, `locked`, worker, and manager states. |
| List services | `GET /api/services?stack=`; optional `com.docker.stack.namespace` label filter. v1 lists all desired-running tasks to count observed running replicas, strips image digests for display, and uses `-1` as global desired count. | Service table expects identity, image, stack and replica counts. Some screen expectations such as mode/ports are not in the list DTO. | **Parity**; use Engine service status counts when supported and retain a bounded fallback. Never use magic `-1`; model unknown/calculated values explicitly. |
| Inspect service | `GET /api/services/{id}` returns image, stack, replicated/global mode, optional replicas, timestamps, env, ports, mounts, constraints, labels, update status, previous image, and an unused `rawSpec`. Job modes are not modeled. | Detail/edit flows consume the fields, but TypeScript nullability and labels do not fully match Go. | **Parity** with job modes, version index, desired/observed split, endpoint state, health/rollout information, and typed nullability. Do not expose a raw Bollard spec. |
| Create service | No v1 API; the corresponding UI affordance is missing/no-op. | Feature documentation implies service creation. | **Missing parity affordance**: create from a validated Kontainer spec with registry-auth reference and an idempotency key. |
| Update replicas | Server: `POST /api/services/{id}/scale`, inspect then replace full spec using current version; global mode is rejected. | Tauri calls `PATCH /api/services/{id}`, so the current action cannot reach the route. | **Parity blocker**: define one route and response; support replicated services, define replicated-job semantics separately, and expose version conflicts. |
| Force restart | `POST /api/services/{id}/restart`; increments `TaskTemplate.ForceUpdate`. | Reachable mutation. | **Parity** as an asynchronous rollout operation. It is not naturally idempotent because repeating it increments again. |
| Roll back | `POST /api/services/{id}/rollback`; updates with `rollback=previous`. | Reachable mutation. | **Parity**; capability is conditional on `PreviousSpec`. Return accepted rollout, not “converged.” |
| Replace environment | `PUT /api/services/{id}/env`; replaces the entire environment array after inspect/update. | Tauri expects `{env:[...]}` but the server returns `{"status":"updated"}`. Docker may be updated even though client deserialization reports failure. | **Parity blocker**: consistent response. Preserve the entire service spec, validate duplicate keys, distinguish replace from patch, and redact configured sensitive values. |
| Update image | `PUT /api/services/{id}/image`; replaces image in the inspected service spec. No registry credential or digest-resolution policy. | Tauri expects `{image}` but the server returns status only; successful mutations can appear failed. | **Parity blocker** plus requested-reference/resolved-digest fields and a server-side registry credential reference. |
| Delete service | No standalone v1 route; stack removal deletes services with the stack label. | No complete standalone delete flow. | **Missing parity affordance** with `ignoreMissing`, dependency warnings, and an idempotency key. |
| List tasks | `GET /api/tasks` with optional service/node filters; service- and node-specific aliases also exist. It returns all matching historical tasks, looks up names through full service/node lists, and strips image digests. | “Containers” and task views expect current and historical task information. | **Parity**; push filters down, add `history=current|all`, bounded page size, job iteration, and stable sort. |
| Inspect task | `GET /api/tasks/{id}` inspects the task and separately resolves service and node. It includes container ID/PID, env, mounts and networks when present. | Detail, logs, and exec treat a task/container as actionable. | **Parity**; include version and replacement hints, and return capability reasons when the task has churned or the container is gone. |
| Aggregate task counts | `GET /api/task-stats` aggregates total/running/failed/preparing/complete/shutdown/desired-running. A separate `/api/tasks/stats` collects local live container stats for running local tasks. | The TypeScript aggregate omits `shutdown`; the registered Tauri command uses the aggregate endpoint. | **Parity blocker**: one shared enum/DTO. Keep lifecycle aggregates distinct from live CPU/memory stats. |
| Service/task logs | WebSockets at `/api/services/{id}/logs` and `/api/tasks/{id}/logs`. Tail is capped at 10,000; timestamps default true. Server always enables both stdout/stderr even though clients send flags. Server streaming uses a background context, not socket cancellation. | Log viewer expects follow, tail, timestamps and stream selection. | **Parity** with effective options reflected back, cancellation, limits, structured stream errors, and driver-based capability checks. Swarm service logs require a manager and supported logging drivers ([`docker service logs`](https://docs.docker.com/reference/cli/docker/service/logs/)). |
| Container exec | WebSocket `/api/exec/{containerId}?nodeID=...`; leader executes locally or forwards to a follower. `/bin/sh`, TTY, user, size, binary IO and text exit/error control are supported. | Interactive terminal expects node routing and resize. | **Parity**; resolve from a fresh running task, authorize separately, make node identity mandatory, define control frames, propagate cancellation, and audit without recording terminal payloads. |

The Engine models a service as desired state and the manager continually reconciles it. Tasks move only forward through their lifecycle; when a task stops, Swarm creates a new task rather than restarting the same object ([Swarm services](https://docs.docker.com/engine/swarm/how-swarm-mode-works/services/), [task states](https://docs.docker.com/engine/swarm/how-swarm-mode-works/swarm-task-states/)). A task ID is therefore an observation, not a durable “container” identity.

### Nodes and stacks

| Resource / operation | Existing v1 API and behavior | Existing UI/Tauri expectation | v2 disposition |
|---|---|---|---|
| List nodes | `GET /api/nodes` returns ID, hostname, status, availability, role, Engine version, OS/architecture, address, leader flag, and update time. | Node list and detail use these fields. | **Parity** with manager reachability, membership, version index, agent reachability, capabilities and a clear observed timestamp. |
| Inspect node | No dedicated public inspect route; list DTO is the effective detail. | Rich node detail is implied by the app. | **Missing parity affordance**: inspect by opaque ID and include safe manager/agent/Engine summaries. |
| Set availability | Server uses `PATCH /api/nodes/{id}/availability` and inspect/version/update. | Tauri sends `PUT`, so it fails before the mutation. | **Parity blocker**; accept only `active|pause|drain`, require expected version, and expose task-eviction rollout separately. Docker documents that drain reschedules service tasks while standalone containers continue running ([manage nodes](https://docs.docker.com/engine/swarm/manage-nodes/)). |
| Set role | Server uses `PATCH /api/nodes/{id}/role` and inspect/version/update. | Tauri sends `PUT`, so it fails before the mutation. | **Parity blocker**; preflight quorum and last-manager risks, then let Engine perform authoritative validation. Managers are responsible for Swarm management and quorum ([nodes](https://docs.docker.com/engine/swarm/how-swarm-mode-works/nodes/), [admin guide](https://docs.docker.com/engine/swarm/admin_guide/)). |
| Join/add/remove node | No v1 public operations. Follower registration is discovery, not Swarm membership management. | “Add node” is listed as missing. | **Missing parity affordance**, but join-token handling needs a separate threat model. Do not return tokens through ordinary resource DTOs. |
| List stacks | `GET /api/stacks`; synthesized by grouping services with `com.docker.stack.namespace`. Counts services, replicated desired replicas, running tasks, and newest service timestamp. Global/job desired counts are incomplete. | Stack table expects aggregate health. | **Parity** as a derived read model with completeness and aggregation warnings. |
| Deploy/update stack | No v1 endpoint; deploy UI is missing/no-op. | Product roadmap expects stack deployment. | **Missing parity affordance**, implemented above the Engine adapter. Deep reconciliation is **deferred** to a dedicated design. |
| Remove stack | `DELETE /api/stacks/{name}` sequentially removes labeled services, networks, secrets and configs, stops at first error, returns no partial progress, and preserves volumes. | Tauri expects `{removed:number}` while server returns status only. | **Parity blocker**: return a durable multi-resource operation with per-resource results; default to retaining volumes, and touch only resources owned by the stack. |

“Stack” is not a Docker Engine API object. `docker stack deploy` is a client command that reads a Compose document and creates or updates Swarm resources; it must target a manager and provides client-side options such as pruning, registry-auth propagation, and image resolution ([CLI reference](https://docs.docker.com/reference/cli/docker/stack/deploy/), [Swarm stack guide](https://docs.docker.com/engine/swarm/stack-deploy/)). Docker CLI itself establishes the `com.docker.stack.namespace` convention and name scoping ([Docker CLI conversion source](https://github.com/docker/cli/blob/master/cli/compose/convert/compose.go)). Kontainer must therefore own parsing, normalization, diffing, ordering, rollout observation, and recovery. Bollard cannot supply a “stack deploy” call because Engine has no such endpoint.

### Networks, volumes, secrets, and configs

| Resource / operation | Existing v1 API and behavior | Existing UI/Tauri expectation | v2 disposition |
|---|---|---|---|
| List networks | `GET /api/networks` calls the leader daemon only. It returns ID, name, driver, scope, internal/attachable/ingress flags and timestamps. `services` is always empty and IPAM/subnet is absent. | Network screen expects inventory; TypeScript already has an optional subnet. | **Parity** plus scope-aware aggregation, IPAM, peers/services when explicitly requested, and node/completeness metadata. |
| Inspect network | No public inspect operation. | Detail affordances are incomplete. | **Missing parity affordance** using Engine’s verbose inspect only on demand. |
| Create/remove network | No v1 operations. | Create is listed as missing; feature language implies fuller lifecycle control. | **Missing parity affordance** with swarm/local scope, IPAM validation, idempotent name/spec comparison, ingress/predefined protection, and in-use conflict mapping. |
| Connect/disconnect container | No v1 operation. | No established UI contract. | **Deferred** until a container model exists; it is node-local and should not be disguised as a Swarm-service attachment edit. |
| List volumes | `GET /api/volumes` calls only the leader daemon. It returns name, driver, mountpoint, scope, labels/options, time, ref count/size and stack label. Missing usage data becomes `-1`; no node ID is returned. | Volume inventory appears cluster-like even though ordinary volumes are local. | **Parity blocker**: fan out local volumes with `nodeId`, model unknown values as null, and distinguish Engine cluster volumes from local volumes. |
| Inspect volume | No public inspect operation. | Detail affordance is incomplete. | **Missing parity affordance**, routed by node for local volumes. |
| Create/remove/prune volume | No v1 operations. | Create is listed as missing; full management is aspirational. | **Missing parity affordance** with explicit node or cluster scope. Remove/prune require in-use protection, preview, confirmation, and per-node results. |
| List secrets | `GET /api/secrets`; name, labels and timestamps only. | Secret list does not expect secret bytes. | **Parity**. Secret material must never be returned: Engine does not expose it after creation. |
| Create secret | `POST /api/secrets`; Engine result ID is discarded and the server returns only name. | Tauri expects `{id,name}`, so creation can succeed and then deserialize as failure. | **Parity blocker**: return ID/name/version metadata, never echo payload, accept an idempotency key, and enforce the Engine size limit. |
| Remove secret | `DELETE /api/secrets/{id}`. | Tauri expects `{removed:bool}` but server returns `{status:"removed"}`. | **Parity blocker** with consistent delete semantics, `ignoreMissing`, and `resource_in_use` conflict. |
| Inspect/update/rotate secret | No inspect route beyond list metadata and no update operation. | No material-read expectation should be introduced. | Metadata inspect is an **enhancement**. Secret-data update is impossible by design; rotation means create a new secret and update service references ([Swarm secrets](https://docs.docker.com/engine/swarm/secrets/)). |
| List configs | `GET /api/configs`; metadata plus decoded byte length. | Config list expects size. | **Parity** with version index. |
| Inspect config | `GET /api/configs/{id}` base64-encodes Engine bytes. | UI places this string directly in an editor, so text configs display base64. | **Parity blocker**: respond with explicit `{content, encoding, byteLength, sha256}`; use UTF-8 text when valid and base64 otherwise. |
| Create config | `POST /api/configs`; Engine result ID is discarded. | Tauri expects `{id,name}` and may report a successful creation as failure. | **Parity blocker**: return metadata, enforce size, and accept idempotency key. |
| Remove config | `DELETE /api/configs/{id}`. | Tauri expects `{removed:bool}` but server returns status. | **Parity blocker** with consistent delete semantics. |
| Update/rotate config | No v1 operation. Engine allows label metadata updates with a version, but config data is immutable. | Editing content implies replacement, not an in-place byte update. | **Enhancement**: create a new versioned config and update consumers; make label-only update explicit. Docker’s recommended stack workflow is to redeploy after a config change ([Swarm configs](https://docs.docker.com/engine/swarm/configs/)). |

Docker volumes normally exist on one host. With the default local driver, each Swarm task gets the volume on the node where it runs; service removal does not remove that volume ([volumes](https://docs.docker.com/engine/storage/volumes/)). A v2 “cluster volume list” is therefore an aggregation with provenance, not one authoritative list. Network inventory is also scope-sensitive: local bridge networks belong to one daemon, while Swarm overlay networks are cluster resources.

### Images, registries, metrics, and streaming

| Resource / operation | Existing v1 API and behavior | Existing UI/Tauri expectation | v2 disposition |
|---|---|---|---|
| List images | `GET /api/images`; leader lists locally and fans out to follower agents. Rows contain image ID/tags/size/created plus node ID/hostname. Follower failures are logged and omitted. | Image view appears cluster-wide but cannot tell complete from partial. | **Parity blocker**: retain node provenance and return node results/warnings/completeness. Identical digest on two nodes remains two placements, optionally grouped in a summary. |
| Remove image | `DELETE /api/images/{id}?node=...&force=...`; routes locally or to a follower and returns deleted/untagged details. | Node-specific removal is supported. | **Parity** with mandatory node scope, conflict mapping, idempotency behavior, and exact image reference semantics. Image removal affects a host, not a registry ([`docker image rm`](https://docs.docker.com/reference/cli/docker/image/rm/)). |
| Prune images | `POST /api/images/prune?nodeId=...`; no filters. Omitted node means all nodes, with follower failures silently skipped. Current UI exposes cluster-wide prune. | User cannot preview or select dangling versus all unused. | **Parity blocker**: explicit `mode=dangling|unused`, filters, preview, confirmation/idempotency key, node results and reclaimed bytes. Docker’s default is dangling images only; `-a` expands to all unused images ([`docker image prune`](https://docs.docker.com/reference/cli/docker/image/prune/)). |
| Inspect/pull/tag/push image | No v1 operations. “Pull image” is listed as missing. | Service image edits assume images can become available but do not manage distribution. | Pull/inspect are **missing parity affordances**; tag/push are **deferred** until registry scope is defined. Pull is node-local and may be a fan-out operation. |
| Registry management | No OCI/Docker registry resource, login store, catalog search, or credential management exists. The Go `Registry` is the follower-agent registry, not an image registry. | GHCR/private-registry ideas appear only in future-facing documentation. | **Enhancement**: initially support credential references for pull and service updates. Full registry browsing, account management, push, vulnerability data and auto-pull policy are **deferred**. |
| Metrics | Cluster and node metrics exist separately from resource inventory; local agents collect host metrics and live container stats. | Dashboards consume node/container observations. | Preserve as an adjacent telemetry subsystem. Do not merge high-frequency samples into resource DTOs; join by node/task/container IDs and timestamps. |

## Required production semantics

### Connection and API-version negotiation

Each agent owns one logical Engine client pool for its local daemon. Startup should:

1. Resolve an explicitly configured endpoint, otherwise the platform-appropriate local default. Avoid exposing arbitrary daemon URLs through the public API.
2. Construct Bollard with TLS/socket settings and bounded connect/request timeouts.
3. Call `negotiate_version` before ordinary operations.
4. Fetch `/version` and `/info`, retain daemon API maximum/minimum, negotiated version, Engine version, platform, Swarm state, manager control availability and plugins.
5. Publish a capability snapshot and refresh it after reconnect, role changes, Engine upgrades, or a bounded interval.

Bollard’s `Docker` type supports Unix sockets, named pipes, HTTP/TLS transports, version negotiation, timeouts, and the required service/node/task/config/secret/network/volume/image/container operations ([Bollard `Docker`](https://docs.rs/bollard/latest/bollard/struct.Docker.html)). The implementation must not set `DOCKER_API_VERSION` as its normal negotiation mechanism because Docker documents that this forces a version and disables negotiation ([Engine API overview](https://docs.docker.com/reference/api/engine/)). Configuration may allow an explicit version only as a diagnostic override, surfaced prominently in health.

Forward compatibility must be conservative. The Engine API uses an open schema where clients should ignore unknown response fields, but an older generated client still cannot invoke a newly added endpoint or express an unknown request field ([Engine API v1.52](https://docs.docker.com/reference/api/engine/version/v1.52/)). The upstream machine-readable contract lives with Moby ([Moby Engine API schema](https://github.com/moby/moby/blob/master/api/swagger.yaml)). A Bollard upgrade or a narrow hand-written escape hatch is required before Kontainer advertises such a capability.

### Manager-only operations and quorum

Swarm service, task, node, secret, config, stack-derived, and Swarm overlay-network control must go through a manager daemon. Docker CLI references consistently mark service, node, secret and config list/control commands as manager-only ([services](https://docs.docker.com/reference/cli/docker/service/ls/), [nodes](https://docs.docker.com/reference/cli/docker/node/ls/), [secrets](https://docs.docker.com/reference/cli/docker/secret/ls/), [configs](https://docs.docker.com/reference/cli/docker/config/ls/)). V2 should require:

- `Swarm.LocalNodeState == active` for Swarm operations.
- `Swarm.ControlAvailable == true` for manager operations.
- a reachable manager and, for writes, a functioning manager quorum.

The leader role alone is insufficient proof. Kontainer may be misconfigured onto a worker or remain reachable while the Swarm has lost quorum. Reads and writes should fail with domain errors such as `swarm_inactive`, `not_manager`, or `quorum_unavailable`, including a safe remediation hint. Existing tasks can continue when managers lose quorum even though management operations cannot proceed, so UI health must not label the entire workload “down” ([Swarm administration guide](https://docs.docker.com/engine/swarm/admin_guide/)).

Node role changes require preflight warnings for demoting the current leader, the last manager, or enough managers to lose quorum. Engine remains authoritative and the UI must never imply that the preflight is a transactional guarantee.

### Optimistic concurrency and version indexes

Swarm objects carry an `ObjectVersion.Index`. Updates to services, nodes, secrets, and configs require the current version query parameter. Docker’s API explicitly requires it, and Bollard’s service update example first inspects the service and passes its version ([Engine API v1.52](https://docs.docker.com/reference/api/engine/version/v1.52/), [Bollard service module](https://docs.rs/bollard/latest/bollard/service/)).

V2 rules:

- Every mutable Swarm DTO exposes `version: u64`.
- A UI mutation sends `expectedVersion`. Missing versions may be accepted only on explicitly legacy-compatible routes.
- The server fetches the current full spec, compares the expected version, applies a typed patch, and submits the full preserved spec with the current index.
- A mismatch returns HTTP 409 `version_conflict` with the expected and current indexes. It does not silently overwrite another operator’s change.
- Do not blindly retry conflicts. The user or an operation-specific merge policy must rebase against the new object.
- Generated model integer widths must be checked at the adapter boundary; domain DTOs remain `u64`.

This is especially important for service updates: sending a sparse `ServiceSpec` can clear unrelated fields. Scale, image, environment, restart, rollback and stack reconciliation must share one preservation-safe patch path. A successful Engine update means the new desired state was accepted, not that its rollout converged.

### Desired state, observed state, and task churn

Resource DTOs must not flatten desired and observed state into ambiguous counters. A service should expose, at minimum:

```text
ServiceSummary
  identity: id, name, stackName, version, createdAt, updatedAt
  spec: mode, image.requested, replicas?, placement summary
  status: desiredTasks?, runningTasks, completedTasks?, updateState?, message?
  convergence: converged | progressing | degraded | paused | unknown
  completeness: complete | partial | stale
```

Replicated services have a numeric desired replica count. Global services target eligible nodes, so the desired count is computed from current placement eligibility and must be marked calculated or unknown; it must not be `-1`. Replicated jobs and global jobs need their own completion/iteration fields rather than being coerced into long-running service semantics.

Task DTOs should carry task ID, service ID, node ID, slot when meaningful, desired state, current lifecycle state, version, job iteration, timestamps, status message/error, container ID/status and image reference. UI lists key rows by task ID. A `serviceId + slot` can help group replacement attempts for replicated services, but it is not globally unique across job iterations and must not be used as an Engine identity.

Default workload views should use `history=current`; an explicit `history=all` view preserves v1’s complete task history. Logs and exec should re-resolve the selected task immediately before opening a stream. If replacement churn makes the container unavailable, return `task_replaced` or `container_not_running` with enough information to refresh rather than a generic 500.

Engine events are useful to invalidate caches and advance rollout observers, and can be filtered by Swarm resource type ([Engine API v1.52](https://docs.docker.com/reference/api/engine/version/v1.52/)). They are not a durable source of truth. Reconnects must trigger a full resync, and operation completion must be confirmed from current resource/task state.

### Node-local and cluster-scoped resources

| Resource | Authoritative scope | Routing rule |
|---|---|---|
| Services, tasks, nodes, secrets, configs | Swarm manager | Leader’s local Engine only when it is an active manager. |
| Swarm overlay networks | Swarm manager | Manager for cluster truth; attach node observations only on demand. |
| Bridge/host/null and other local networks | Node | Route to the named node agent. |
| Local volumes | Node | Require `nodeId`; cluster inventory fans out and retains provenance. |
| Engine cluster volumes | Cluster, if supported | Separate kind/capability from ordinary local volumes. |
| Images | Node | Require `nodeId` for mutation; aggregation never erases placement. |
| Containers, live stats, exec | Node | Resolve task to node, then call that agent. |
| Service/task log aggregation | Swarm manager | Manager API, additionally gated by task logging driver. |

Every fan-out response should include `queriedNodes`, `successfulNodes`, `failedNodes`, `snapshotAt`, and per-node warnings. A missing follower cannot be represented as an empty resource list. Mutations that target several nodes return a durable operation or a `207 Multi-Status`-style body with per-node outcomes; the API team should select one convention and use it consistently.

### Idempotency and retry safety

Read calls are safe to retry within their deadlines. Writes need operation-specific rules:

| Operation class | V2 rule |
|---|---|
| Set scale/image/environment/availability/role | Compare desired value with current spec and return `noChange` if equal. Use `expectedVersion`. |
| Create named resource | Require an idempotency key. If name exists, compare normalized immutable and mutable fields; return the same result only for an equivalent request, otherwise `name_conflict`. |
| Delete resource | Default missing resource to `not_found`; allow explicit `ignoreMissing=true` for convergent workflows. Record successful deletion under the idempotency key. |
| Restart (`ForceUpdate++`) | Never automatically retry without an idempotency record; every un-deduplicated attempt creates another rollout. |
| Rollback | Treat as an operation with one accepted transition; replay through the same idempotency record. |
| Prune | Require target nodes, selection mode/filters, preview token or confirmation, and an idempotency key. Never retry an uncertain result as a fresh request. |
| Stack remove/deploy | Durable multi-step operation; steps are individually convergent and resumable. Return partial progress. |
| Logs/exec | Stream reconnect is a new session. Do not claim exactly-once delivery; logs can resume by timestamp with overlap/deduplication, while exec cannot be replayed. |

An idempotency key maps to request hash, authenticated principal, operation/result and expiry. Reusing a key with a different hash is `idempotency_conflict`. The storage technology and retention period are open decisions.

### Filtering, pagination, and bounded work

Engine list endpoints accept useful filters but most Swarm list endpoints do not provide server pagination. Services support filters such as ID, label, mode and name; tasks add desired state, node and service; nodes include membership and role; secrets/configs include ID, label and name. Networks, volumes and images have their own driver/scope/reference/dangling filters ([Engine API v1.52](https://docs.docker.com/reference/api/engine/version/v1.52/)).

V2 should:

- Pass exact supported filters to Engine before enrichment.
- Apply Kontainer-only text search and sort after the narrowest Engine query.
- Use a deterministic tie-breaker, normally `(sortField, id)` or `(sortField, nodeId, id)`.
- Default `limit` to 50 and cap it at 100 for UI requests; allow a separately authorized export path.
- Return `page: { limit, nextCursor, total?, snapshotAt }`. `total` is optional because exact counts can require fetching the full result.
- Treat a cursor as best-effort against a changing collection unless it refers to a materialized, expiring snapshot. Never claim snapshot isolation over live task churn.
- Bound fan-out concurrency, per-node timeouts, task enrichment and service-log tail size.

For API 1.41 and newer, service listing should request Engine-computed service status counts instead of listing every desired-running task merely to compute replicas. The API version history records the service-status and job-mode additions in 1.41 ([Engine API version history](https://docs.docker.com/reference/api/engine/version-history/)). A fallback task aggregation can exist for a degraded compatibility tier, with its additional cost and uncertainty surfaced.

### Error taxonomy

Bollard distinguishes daemon HTTP errors from transport, timeout, stream, JSON, socket and TLS failures ([Bollard errors](https://docs.rs/bollard/latest/bollard/errors/enum.Error.html)). The Engine normally returns an HTTP status plus a JSON `message`; callers should not parse arbitrary message text as the primary contract ([Engine API v1.52](https://docs.docker.com/reference/api/engine/version/v1.52/)). Map both into a stable domain error:

```json
{
  "code": "version_conflict",
  "message": "The service changed after it was loaded.",
  "status": 409,
  "retriable": false,
  "resource": { "kind": "service", "id": "..." },
  "operationId": "...",
  "details": { "expectedVersion": 42, "currentVersion": 44 }
}
```

Required codes include:

- `invalid_input`, `validation_failed`, `unauthenticated`, `forbidden`
- `not_found`, `task_replaced`, `container_not_running`
- `name_conflict`, `version_conflict`, `resource_in_use`, `idempotency_conflict`
- `unsupported_mode`, `unsupported_capability`, `swarm_inactive`, `not_manager`, `quorum_unavailable`
- `node_unreachable`, `engine_unavailable`, `engine_timeout`, `engine_rejected`, `stream_failed`
- `registry_auth_failed`, `image_pull_failed`, `digest_resolution_failed`
- `partial_failure`, `cancelled`, `internal`

Use 400 for malformed input, 401/403 for authentication/authorization, 404 for absent identities, 409 for state/resource conflicts, 422 for semantically invalid specs, 503 for unavailable Engine/manager/quorum and 504 for upstream deadlines. Multi-node partial success must never be a plain success body with missing rows. Sensitive daemon paths, registry responses, environment values, secret/config bytes and socket/TLS detail belong in redacted structured logs, not client messages.

## Stable resource DTOs

Bollard/Moby models are adapter types, not the public contract. Kontainer DTOs should be versioned, camelCase, flat enough for desktop consumption, and lossless for decisions the UI can make. Rust enums need an unknown-value representation so a newer daemon does not make deserialization fatal.

Common fields:

```text
ResourceMeta { id, name?, version?, createdAt?, updatedAt?, labels }
NodeLocator { nodeId, hostname?, agentState, observedAt }
Completeness { state: complete|partial|stale, warnings[], failedNodes[] }
ImageRef { requested, canonical?, digest?, tag? }
OperationRef { operationId, status: accepted|running|succeeded|failed|partial|noChange }
```

Resource-specific design:

- **ClusterInfo**: Engine identity/version/API min/max/negotiated, Swarm local state/node ID/control availability/cluster ID, manager reachability, agent counts and capability revision. Redact join tokens, TLS material and daemon labels unless explicitly allow-listed.
- **ServiceSummary/Detail**: metadata; stack ownership; desired spec (mode, image, replicas/job settings, env summary, ports, mounts, constraints, resources, update/rollback policy); observed task counts, endpoint and update status; previous-spec availability; mutation capabilities with reasons.
- **TaskSummary/Detail**: immutable attempt identity, service/node relationships, slot/job iteration, desired/current state, status timestamps/message, image, container observation, networks/mounts, and per-action capabilities. Avoid a fabricated `user` field unless sourced from the container spec.
- **NodeSummary/Detail**: Swarm status, availability, role, manager state/reachability/leader, address, platform/Engine, version index, agent state and capabilities.
- **StackSummary**: derived namespace, owned resource counts, service convergence totals, most recent change, completeness and warnings. It is never serialized as if Engine supplied a stack object.
- **Network**: ID/name, scope (`local|swarm|global` plus unknown), driver, IPAM configs, attachable/internal/ingress/config-only flags, node provenance for local networks, optional verbose peers/containers/services.
- **Volume**: name, kind (`local|cluster`), driver/scope, node provenance, mountpoint (privileged detail), labels/options, usage as nullable bytes/ref count, stack ownership and in-use references.
- **Secret**: metadata only, size only if known at creation, never payload. A payload hash may be stored only if its threat model and keying are approved; Engine cannot supply it later.
- **Config**: metadata plus optional explicit content representation for authorized inspect. Preserve arbitrary bytes and never ambiguously label base64 as text.
- **ImagePlacement**: node locator plus ID, repo digests, repo tags, size, created time, platform when known and container-use count. A separate group view may aggregate placements by digest.
- **RegistryCredentialRef**: opaque credential ID/provider and display host/account metadata. Passwords/tokens never cross back to React and never appear in service DTOs.

Mutation responses should use one convention. Recommended: immediate creates return the created DTO; idempotent updates return `{operation, resource}` with `noChange` available; rollout, prune, fan-out, stack and node-drain work returns `202 Accepted` with an operation DTO. Deletes may return the operation/result rather than mixing v1’s `status`, `removed`, and counters.

## Capability discovery and enforcement

`GET /api/v2/capabilities` should return a revisioned cluster view plus per-node capability records. Inputs are current `/version`, `/info`, Swarm inspection where permitted, agent reachability, negotiated API, Engine plugins, and resource-specific preconditions.

Static/dynamic examples:

| Capability | Required evidence | Common disabled reason |
|---|---|---|
| `swarm.read` | active Swarm + manager connection | `swarm_inactive`, `not_manager`, `engine_unavailable` |
| `swarm.write` | `swarm.read` + working quorum | `quorum_unavailable`, authorization |
| `service.statusCounts` | negotiated API >= 1.41 | old API compatibility tier |
| `service.jobs` | negotiated API >= 1.41 | old API compatibility tier |
| `service.scale` | replicated service | global or job mode |
| `service.rollback` | previous spec exists | no rollback target |
| `service.logs` | manager + compatible logging driver | unsupported driver |
| `task.exec` | reachable node agent + running container | task churn, stopped container, unreachable node |
| `image.local` | reachable node Engine | follower/Engine unreachable |
| `volume.local` | reachable node Engine | node not selected/unreachable |
| `volume.cluster` | negotiated API/model support and plugin | unsupported API/driver |
| `registry.auth` | credential provider configured and authorized | missing/expired credential |

The UI can use capabilities to hide or disable actions and explain why, but every command must enforce the same rule server-side. Capability snapshots are hints with timestamps; a race can still make an allowed operation fail. Unknown enum values or plugin names should degrade only the affected feature.

## Image and registry behavior

Service-image updates need three distinct values: the user’s requested reference, the registry-resolved canonical digest, and the image actually observed on each task/node. Swarm managers normally resolve tags to digests before distributing a service; if resolution fails, workers may independently resolve a tag and run different content ([deploy services](https://docs.docker.com/engine/swarm/services/)). V2 should make the policy explicit:

- Default to resolving a tagged reference and record the digest.
- Permit an explicit best-effort mode only with a warning.
- Prefer digest-pinned references for repeatable updates.
- Send registry credentials only from a server-side credential reference, using the Engine registry-auth mechanism; never accept/store a browser-returned raw token in an ordinary resource object.
- Report pull/distribution failures per node and distinguish authentication, missing manifest, platform mismatch, rate limit, network failure and digest mismatch when the upstream response supports it.

Initial registry scope is credential-backed pull and service-update support. Registry catalogs, tag browsing, push/build, garbage collection, vulnerability scanning, GHCR-specific automation and registry administration are separate products and should not be smuggled into the Engine resource adapter.

## Stack-management boundary

This report defines only the resource boundary needed by a later stack deployment design:

```text
Compose input
  -> parser/validator
  -> normalized StackPlan
  -> StackReconciler (diff, ownership, ordering, policy)
  -> EngineClient resource operations
  -> rollout/event observer
  -> durable StackOperation result
```

The resource layer owns safe CRUD, version conflicts, capability checks, registry-auth handoff, node routing, error mapping and observation. The later stack report must own Compose compatibility, canonicalization, naming/labels, external resources, dependency ordering, prune policy, update versus replacement, rollback boundaries, recovery after leader failure and how desired documents are stored.

Minimum stack invariants for this layer:

- List/read is derived from explicitly owned labels and reports incomplete or suspicious ownership.
- Deploy/update can consume only a normalized, validated plan; this layer does not parse Compose.
- Delete touches only resources carrying the expected stack namespace and ownership marker. A matching namespace label alone is not sufficient to adopt arbitrary resources in v2.
- Predefined/ingress networks and resources declared external are never deleted.
- Volumes are retained by default, matching the durable-data expectation of Docker stack removal ([`docker stack rm`](https://docs.docker.com/reference/cli/docker/stack/rm/)). Destructive volume deletion is a separate explicit policy and operation.
- Multi-resource work is not an Engine transaction. Persist each step, return partial state, and support safe resume/repair.

## Rust client approaches

| Approach | Decision | Rationale |
|---|---|---|
| **Bollard 0.21.x behind `EngineClient`** | **Accepted** | Async Tokio-native client, broad Engine/Swarm operation surface, generated Moby models, Unix/named-pipe/TLS transports, streaming, and built-in version negotiation. Its documented `Docker` surface covers services, nodes, tasks, secrets, configs, networks, volumes, images, events, stats, logs and exec ([Bollard `Docker`](https://docs.rs/bollard/latest/bollard/struct.Docker.html)). The adapter prevents generated types and crate upgrade churn from becoming the product API. |
| `docker-api` 0.14.x | **Rejected as primary; reevaluate if Bollard blocks a required endpoint** | It exposes a broad typed Docker interface including Swarm resources, but the current crate is based on an older Hyper/http ecosystem and offers less alignment with the proposed adapter than Bollard ([docker-api crate](https://docs.rs/docker-api/latest/docker_api/)). Maintaining two general Docker clients would add more risk than it removes. |
| `shiplift` 0.7.x | **Rejected** | Its high-level documented surface is useful for common container/image/network operations but does not provide the complete task/node/secret/config Swarm control surface this project needs ([shiplift crate](https://docs.rs/shiplift/latest/shiplift/)). |
| Raw Hyper/Reqwest over Unix socket | **Rejected as the main client; narrow escape hatch deferred** | Kontainer would own versioned paths, query encoding, streaming multiplex formats, TLS/socket differences and schema drift. A small adapter-local implementation can be justified for a required endpoint newer than Bollard’s generated schema, with fixtures against supported daemons. |
| Docker CLI subprocesses | **Rejected for runtime resource control** | Adds a binary/runtime dependency and unstable human-oriented output/error/credential behavior. It also obscures cancellation and version conflicts. CLI behavior can be a test oracle; using Compose conversion code is a separate stack-design decision. |
| Generate and vendor a full Moby client | **Rejected for now** | It provides maximum schema control at the cost of owning generation, transport, compatibility patches and upgrades already handled by Bollard. Reconsider only if audited gaps persist. |

Before dependency adoption, pin the selected Bollard release, audit its transitive dependencies and licenses, and test against every supported Engine baseline. This report recommends a crate; it does not authorize a dependency-file change.

## V2 operation matrix

This matrix is the implementation backlog boundary. “Manager” means an active manager with working control plane; “node” means the selected node’s agent/Engine.

| Domain | Read operations | Mutation operations | Scope | Delivery |
|---|---|---|---|---|
| Cluster/capabilities | cluster summary, agent list, capability matrix | refresh/reprobe | manager + all agents | **Parity + enhancement** |
| Swarm lifecycle | state/lock summary | initialize, update CA/spec, unlock, leave | local Engine/manager by operation | No v1 behavior; **deferred** pending membership/bootstrap security design |
| Services | list/filter/page, inspect, rollout/status, previous-spec availability | create, patch scale/env/image/spec, restart, rollback, delete | manager | **Parity**, repair broken routes/responses; create/delete are **missing parity affordances** |
| Tasks | list/filter/history/page, inspect, lifecycle aggregate | no direct task mutation | manager | **Parity** with churn model |
| Task runtime | logs, stats, container observation | exec/resize/cancel | manager for aggregated logs; node for stats/exec | **Parity** with capability/cancellation fixes |
| Nodes | list/filter/page, inspect | availability, role | manager | **Parity**, repair HTTP method mismatches |
| Swarm membership | join-token metadata/state (never token by default) | join/promote/demote/remove/rotate token | manager + target agent | **Missing parity affordance**, security design required |
| Stacks | derived list/inspect/operation status | deploy/update/remove/repair | manager | List/remove **parity**; deploy **missing**; algorithm **deferred** |
| Networks | list/filter/page, inspect verbose | create/remove; connect/disconnect later | manager or node by scope | List **parity**; lifecycle **missing**; container attachment **deferred** |
| Volumes | list/filter/page, inspect, usage | create/remove/prune | node or cluster by kind | List **parity repair**; lifecycle **missing** |
| Secrets | list/inspect metadata | create/remove; rotate workflow | manager | **Parity** plus safe rotation enhancement |
| Configs | list, inspect content/metadata | create/remove, label update, rotate workflow | manager | **Parity** plus safe rotation enhancement |
| Images | list/filter/group, inspect, distribution status | pull, remove, prune; tag/push later | node(s) | List/remove/prune **parity repair**; pull **missing**; push **deferred** |
| Registries | credential/provider capability | create/update/delete credential reference; catalog/push later | Kontainer secret store + registry | Auth **enhancement**; registry product **deferred** |
| Events/operations | event watch, operation list/inspect | cancel where safe | manager + operation store | **Enhancement** |

## Acceptance checklist

### Client and capability foundation

- [ ] Each agent negotiates its Engine API and publishes daemon maximum, minimum, negotiated version, Engine version and last probe time.
- [ ] Normal operation does not depend on a forced `DOCKER_API_VERSION`.
- [ ] The management plane enforces the selected minimum API/capability policy and supports unknown response fields/enums safely.
- [ ] `/version`, `/info`, Swarm state/control availability, plugin inventory and agent reachability feed one revisioned capability model.
- [ ] Manager-only operations are rejected before mutation on a worker/inactive Swarm and distinguish lost quorum from workload failure.
- [ ] Bollard types and errors do not escape the adapter into public DTOs.

### V1 parity and contract repair

- [ ] All implemented v1 service list/detail/scale/restart/rollback/env/image behavior is available through one consistent v2 contract.
- [ ] Scale and node-role/availability routes agree across Rust server, Tauri and React.
- [ ] Service env/image, secret/config create/remove, and stack-remove response types round-trip without post-success deserialization errors.
- [ ] Service/task enums and nullability match in Rust and TypeScript, including shutdown and job modes.
- [ ] Config text is not shown as accidental base64; arbitrary bytes remain representable.
- [ ] Logs honor stream flags, limits and client cancellation; exec has an explicit node and current running container.
- [ ] Existing list/inspect/remove operations for nodes, stacks, networks, volumes, secrets, configs and images have tests for empty, normal, error and unknown-field cases.

### Concurrency and desired-state correctness

- [ ] Mutable Swarm DTOs expose `version`, and writes accept/check `expectedVersion`.
- [ ] Service patch tests prove unrelated spec fields survive scale, env, image and restart changes.
- [ ] Version races return 409 and never silently overwrite changes.
- [ ] Mutations distinguish Engine acceptance from rollout convergence and expose operation progress.
- [ ] Replicated, global, replicated-job and global-job services have correct desired/observed models.
- [ ] Task history/current views handle replacement churn, terminal states and disappearing containers.
- [ ] Event reconnect performs a full resync; events are not the only source of truth.

### Locality, aggregation, and destructive work

- [ ] Every local image, volume, container stat and exec target carries node provenance.
- [ ] Fan-out responses expose queried/successful/failed nodes and never silently drop failures.
- [ ] Local and cluster volumes, and local and Swarm networks, are distinct kinds/scopes.
- [ ] Per-node timeouts and bounded concurrency prevent one unreachable agent from blocking the cluster view.
- [ ] Create/restart/rollback/delete/prune/fan-out endpoints implement documented idempotency behavior.
- [ ] Prune has explicit selection semantics and preview/confirmation; default dangling behavior is covered by tests.
- [ ] Stack removal persists per-resource progress, preserves volumes by default and refuses unowned/predefined/external resources.

### Errors, security, and registry behavior

- [ ] All Bollard daemon/transport/timeout/stream failures map to stable public error codes and appropriate HTTP status.
- [ ] Authorization is enforced per operation; WebSocket authentication is not weaker than REST authentication.
- [ ] Secret payloads and registry credentials never appear in read DTOs, logs, errors or browser persistence.
- [ ] Config access has an explicit permission and encoding; sensitive environment values follow an approved redaction policy.
- [ ] Image updates distinguish requested tag, resolved digest and observed task digest.
- [ ] Registry-auth failures, missing manifests, platform mismatches and per-node pull failures are distinguishable where Engine provides evidence.

### API quality and verification

- [ ] Filters are pushed to Engine where possible; all UI lists have deterministic bounded pagination.
- [ ] Cursor documentation states whether it is live best-effort or a materialized snapshot.
- [ ] Rust/TypeScript DTOs are generated from one schema or protected by bidirectional contract tests.
- [ ] Integration coverage runs against at least the minimum supported Engine and a current Engine, including a multi-node Swarm and manager loss.
- [ ] Fault tests cover follower timeout, partial image/volume results, stale version, task replacement during logs/exec, lost event stream and mid-operation leader restart.
- [ ] Audit logs record principal, target, normalized mutation, result and operation ID without secrets.

## Open decisions for GitHub issues

1. **Set the minimum supported Engine/API matrix.** Confirm API 1.41 as the full management baseline; decide whether API 1.30–1.40 gets a documented degraded tier or a capability-only health mode.
2. **Choose v2 HTTP and Tauri schema generation.** Select OpenAPI/TypeScript generation or a shared Rust schema pipeline, version the routes, and define the v1 migration window.
3. **Standardize mutation and partial-result envelopes.** Decide between `202` durable operations for all fan-out work and a synchronous multi-status body for small operations.
4. **Define optimistic concurrency UX.** Decide which mutations require a client `expectedVersion`, which legacy calls may use server-time inspection, and how React presents/rebases conflicts.
5. **Select idempotency storage and retention.** Specify persistence, TTL, request hashing, principal scoping and recovery after leader failover.
6. **Define pagination consistency.** Choose best-effort live cursors or an expiring materialized snapshot for tasks/images/volumes and set limits/export behavior.
7. **Specify task-history policy.** Choose default/current filtering, terminal-history retention, job iteration display and replacement navigation.
8. **Define global-service desired counts.** Specify eligible-node calculation, placement-constraint evaluation, stale-node handling and when the result is “unknown.”
9. **Model network and volume scope.** Decide whether cluster inventory always fans out, how duplicate local names are grouped, and whether Engine cluster volumes are in the initial release.
10. **Design registry credential custody.** Select the secret backend/provider, authorization model, credential refresh, registry-host matching, redaction and audit rules.
11. **Set service image resolution policy.** Decide default digest pinning, best-effort tag behavior, multi-platform reporting and when credentials propagate to Swarm agents.
12. **Define safe node membership operations.** Threat-model join tokens, bootstrap, token rotation, agent installation and last-manager/quorum safeguards.
13. **Specify log and exec protocols.** Define WebSocket frame schema, auth refresh, cancellation, reconnect/overlap semantics, supported log drivers and terminal audit policy.
14. **Approve destructive-operation policy.** Define prune previews, volume retention/deletion, stack ownership/adoption markers, confirmation freshness and break-glass permissions.
15. **Write the separate stack reconciliation design.** Cover Compose compatibility, normalized plan storage, diff/order/prune, external resources, rollout gates, partial failure, rollback and resume after leader change.
16. **Decide configuration/environment secrecy.** Define which environment values are redacted and who may inspect config content; avoid claiming ordinary environment variables are secrets.
17. **Choose operation/event persistence.** Specify whether rollout observation survives leader restart, event retention, cancellation and client polling/subscription semantics.

These decisions should become separately reviewable issues. None requires exposing the Docker socket or turning the agent into a generic Engine proxy.

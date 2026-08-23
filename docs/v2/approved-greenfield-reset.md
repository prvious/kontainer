# Kontainer greenfield Rust reset

Status: approved implementation authority  
Decision date: 2026-08-23  
Scope: product versioning, repository layout, framework, persistence, protocols, identity, compatibility, and initial platform support

## Authority

Kontainer currently has no users or production installations. The Rust rewrite is therefore a greenfield product, not a compatibility-preserving v2 migration.

This decision supersedes every conflicting recommendation in the remaining `docs/v2` research, including SQLite profiles, `/api/v2`, SSE, Go/Rust coexistence, v1 adapters, user-data migration, and mandatory v1 parity. The older reports remain useful technical research, risk analysis, and feature archaeology; they are not authoritative where they conflict with this file.

## Delivery tracking

- GitHub Issues in `prvious/kontainer` are the execution source of truth.
- Use one Kontainer `1.0` product parent, domain epics as native sub-issues, and review-ready vertical implementation issues beneath the owning domain epic.
- Use native issue dependencies for real execution blockers rather than duplicating dependency checklists in prose.
- Assign release-gate milestones to leaf issues, not long-lived epics that span several gates.
- Keep deferred capabilities out of the active epic hierarchy until a later product decision promotes them.
- A GitHub Project is optional and should be added only when Issues, sub-issues, dependencies, labels, and milestones no longer provide sufficient scheduling visibility.

## Product and compatibility

- Develop the new product as `0.x`; the first stable product release is Kontainer `1.0`.
- The public HTTP contract starts at `/api/v1`.
- Private protobuf packages start at `kontainer.agent.v1`.
- Do not ship `/api/v2`, an unversioned `/api` alias, or a legacy compatibility adapter.
- Delete the Go server rather than retaining a coexistence or cutover path.
- Discard existing Go/SQLite development data and credentials. There is no importer or user migration.
- Treat the v1 inventory and feature ledger as product research, not a mandatory parity contract.

## Repository and processes

- Replace the current contents of `server/` with one Rust Cargo workspace.
- Ship separate `kontainer-controller` and `kontainer-node-agent` binaries and OCI images.
- The controller owns the public API, PostgreSQL, manager-scoped Docker operations, operations, authorization, membership, and controller-to-agent calls.
- The node agent runs on every supported Swarm node and owns node-local Docker operations, metrics collection, log/exec session work assigned to it, and a bounded local metrics spool.
- The node agent never connects directly to PostgreSQL.

## Rust application stack

- Pin Rust `1.98.0`, use Rust edition `2024`, and commit `Cargo.lock`.
- Use Loco as the convention-oriented controller framework.
- Use Loco/Axum middleware and request extractors for authenticated principal and request context.
- Use Loco's application context at the controller edge, but pass explicit dependencies into domain actions instead of resolving global state inside domain logic.
- Use SeaORM models, typed query builders, and builder-based migrations for PostgreSQL. Avoid handwritten SQL for ordinary persistence; allow reviewed PostgreSQL-specific SQL only where the ORM/query builder cannot express required locking, indexing, or correctness semantics.
- Use plain Tokio and Tonic for the node-agent process and private controller-agent RPC.
- Use Kontainer-owned supervised tasks for Docker reconciliation, membership, metrics ingestion, operations, logs, exec, cancellation, and shutdown.
- Do not use a general framework scheduler or queue as the source of truth for uncertain Docker mutations. Loco tasks, PostgreSQL-backed workers, and scheduling remain available for ordinary background work when justified.
- Exact crate versions and enabled features are pinned after a compatibility/build spike. Avoid prerelease dependencies unless a separate decision accepts them.

## API and transport

- Use JSON REST with checked OpenAPI for ordinary public requests.
- Use polling of durable operation resources for progress.
- Do not implement SSE initially. Add it only after measured product need.
- Use binary WSS for logs and exec.
- Use versioned Tonic/protobuf over mandatory mTLS for private controller-agent RPC.
- Keep domain types, public DTOs, internal protobuf messages, persistence models, and desktop IPC/view models distinct where they cross different trust or compatibility boundaries.

## Persistence and metrics

- PostgreSQL is the only central database technology.
- Keep control and workload metrics logically separated with distinct schemas or databases; select the exact layout during the persistence spike.
- Controllers are the only writers/readers of central PostgreSQL on behalf of node agents.
- Node agents retain unacknowledged metric batches in bounded, checksummed append-only disk segments. Overflow deletes the oldest retained segment and reports an explicit gap.
- A standard stable deployment may use one controller and one PostgreSQL instance and makes no automatic-failover promise.
- An HA deployment uses multiple controllers, readiness-aware ingress, and an actually highly available PostgreSQL deployment. HA qualification is separate from standard-release stability.
- Sampling cadence, retention, rollups, queue sizes, limits, SLOs, and maximum supported cluster sizes remain benchmark-derived rather than product promises copied from research.

## Identity, authorization, and desktop

- GitHub is the only initial human identity provider. Keep a provider seam for later OIDC without making OIDC a `1.0` blocker.
- Start with Owner, Admin, Operator, and Viewer roles, intersected with explicit capabilities for sensitive actions such as config-content reads, logs, exec, root exec, role changes, and cluster-wide destructive operations.
- Keep short-lived access credentials, rotating/revocable server-side sessions, OS credential-vault custody, and native Tauri networking.
- Keep Tauri + React as the shipping desktop. Reuse a UI-neutral Rust client from Tauri.
- Identify a logical desktop profile by `(cluster_id, principal_id)` and keep a separate immutable local UUID as its storage handle. URL and display label are mutable metadata.

## Initial product scope

- Remove the public `/api/agents` v1 route; expose safe membership summaries later only through an intentional diagnostic contract.
- Do not preserve the leader-local `/api/tasks/stats` route. Add live per-container data later through the cluster metrics contract if needed.
- Remove the arbitrary server-side string-map settings API. Future controller settings must be typed.
- Defer Compose deployment, GPUI, the historical unmerged service CPU/memory editor, Kubernetes, Windows agents, remote Docker endpoints, and advanced compliance profiles.
- Existing useful desktop workflows inform backlog priority but do not create backward-compatibility obligations.

## Platform and release policy

- Support Linux agents using the local Unix Docker socket.
- Treat both `linux/amd64` and `linux/arm64` as first-class `1.0` targets and require native smoke evidence for each; success on one architecture is not evidence for the other.
- Keep the Docker Engine API floor provisional until the real-Engine compatibility suite establishes it.
- Defer the first-party license decision during private development. Mark workspace packages `publish = false`; approve a license before public source/artifact distribution or a stable release.

## Immediate implementation gate

Production implementation may begin after one bounded foundation spike proves that the selected Loco, SeaORM, Tokio, Tonic, Bollard, rustls, OpenAPI, PostgreSQL, and WebSocket dependency graph:

1. compiles together on Rust 1.98.0 without duplicate incompatible runtime/TLS stacks;
2. can run the controller HTTP lifecycle alongside private Tonic ownership and graceful shutdown;
3. supports PostgreSQL migrations, transactions, typed models, and the locking primitives required by durable operations;
4. negotiates Docker Engine access behind a Kontainer-owned typed port;
5. builds the controller and node-agent artifacts independently for the intended Linux targets.

# Kontainer Rust rewrite research map

Status: greenfield reset approved; Rust foundation workspace landed and spike in progress

Prepared: 2026-08-19

Scope: production-grade Rust controller and node agent, desktop integration, and release qualification

> **Approved greenfield reset:** Kontainer has no users or production migration obligations. [The approved reset](./approved-greenfield-reset.md) supersedes conflicting research in this directory: the Go server is deleted, `server/` is a Rust workspace, PostgreSQL is the only central database, the public API starts at `/api/v1`, SSE and legacy compatibility are absent, and Loco + SeaORM are the controller foundation. [Foundation evidence and remaining gates](./foundation-spike-results.md) are tracked separately. This directory retains its historical `v2` name only as a research location.

## The recommendation

Build Kontainer in Rust as a greenfield product architecture rather than a line-for-line Go port.

The proposed system has two deliberately separate Rust workloads:

- `kontainer-controller` runs on Swarm managers, exposes the public API, owns manager-scoped Swarm operations, authorization, durable operations, and cluster-wide read models;
- `kontainer-node-agent` runs globally, including on managers, and owns node-local Docker operations, metrics collection, image/volume work, and exec sessions.

The controller uses public REST/OpenAPI under `/api/v1`, polling of durable operations, and binary WSS for logs and exec. SSE is absent until measured need justifies it. Controller-to-agent traffic uses private `kontainer.agent.v1` protobuf/Tonic over mandatory mTLS. Both processes access Docker only through typed Kontainer actions; neither is an arbitrary Docker socket proxy.

PostgreSQL is the only central database in every profile. A standard stable deployment may run one controller and one PostgreSQL instance without promising automatic failover. An HA profile adds multiple controllers, readiness-aware ingress, and an actually highly available PostgreSQL deployment; HA is a separately qualified claim.

Keep Tauri + React as the shipping parity client. Build a UI-neutral Rust client crate so a later GPUI application can reuse the protocol and authentication work. A simultaneous GPUI product rewrite would combine the riskiest backend, protocol, migration, and UI changes in one cutover, so it is deferred rather than rejected.

Compose stack deployment is a separate feature MVP. Existing stack list and removal behavior are parity; authoring/deploying Compose files is new scope and must not block the core rewrite unless that product decision is deliberately reversed.

## Major routes considered

This is the short version of the alternative analysis. The linked domain reports contain the evidence, edge cases, and conditions that could reopen a choice.

| Area | Proposed route | Rejected or deferred route | Why |
|---|---|---|---|
| Agent shape | Separate controller and node-agent binaries/images | One daemon with a role flag; combined privileged task | Makes authority, Docker-socket access, identity, resource limits, and rollout independently enforceable |
| Docker access | Bollard behind a narrow `EngineClient`, with a narrowly audited raw adapter only for proven gaps | Docker CLI subprocesses; a generic socket proxy; hand-writing the entire Engine client | Typed/testable domain surface without coupling product semantics to a CLI or exposing arbitrary Engine authority |
| Internal transport | Tonic/protobuf over mTLS with deadlines, limits, identity, and versioning | Plaintext gRPC/shared secret; controller connections to every daemon; gossip/NATS/QUIC by default | Fits bounded request/stream contracts and workload identity without adding an unnecessary coordination system |
| Coordination | Swarm for membership and desired placement; shared SQL leases/claims for HA singleton work | A new consensus layer; public-request leader; SQLite on network storage | Uses existing orchestration truth and a transactional store where durable coordination is actually required |
| Control storage | Embedded `control.sqlite3` for the non-HA profile; PostgreSQL adapter for HA | Pretending SQLite is HA; one database/file for control and high-volume telemetry | Keeps a simple install path while making the availability boundary honest and preventing telemetry contention |
| Metrics storage | Separate `metrics.sqlite3` initially, node spool plus idempotent push, scale-triggered external backend | Serial controller pull; per-container permanent streams; bundled Prometheus or custom TSDB as the default | Node locality, bounded Engine load, outage tolerance, and a viable embedded profile without owning a database product |
| Logs | Native manager service/task logs, bounded binary WSS, best-effort resume with explicit gaps | Routing all Swarm logs through node agents; persisting workload logs in Kontainer; exact replay promise | Docker managers already expose the Swarm abstraction; the Engine is not a durable log store |
| Exec | Resolve the current task and run every session through that node's agent | Controller-local exec on managers; transparent reopen; SSH/raw TCP/host shell | Exec is node-local and process-creating; one route makes identity, cleanup, exit state, and audit consistent |
| Public API | Versioned REST/OpenAPI for automation, WSS for interactive streams, SSE/polling for operations | Public gRPC, GraphQL, universal wire DTOs, an implicit `/api` alias | Keeps the external contract approachable while separating public, internal, domain, persistence, and desktop types |
| Desktop | Keep Tauri/React for parity; share a framework-neutral Rust client | Rewrite product UI in GPUI during the agent cutover | Preserves working user workflows and lets GPUI be judged later without wasting the Rust networking/auth work |
| Stack deploy | Rust-owned strict Compose subset and reconciler as a separate feature MVP | Runtime Docker CLI; full Compose compatibility immediately; mixing parser types into core | Makes supported behavior, ownership, diff, recovery, and security explicit without blocking parity |
| Runtime/load | Event-invalidated shared snapshots plus jittered reconcile, bounded queues/fan-out, one-shot stats | Per-request full Engine scans; unbounded streams; speculative scheduler tuning | User count should not multiply Docker work, and overload must be visible rather than destabilizing the Swarm |

## How to read this research

This directory separates facts, decisions, delivery, and tracking so no large issue can hide missing behavior.

Normative precedence is:

1. [the approved greenfield reset](./approved-greenfield-reset.md) and later approved ADRs;
2. [foundation spike results](./foundation-spike-results.md) as implementation evidence, not new product authority;
3. [the consolidated decision register](./decision-register.md), where it does not conflict with approved decisions;
4. the owning domain research report, as non-normative design research where not yet approved;
5. [the feature ledger](./feature-ledger.md) and [v1 archaeology inventory](./v1-parity.md), as product research rather than compatibility requirements.

The domain reports record the routes considered, why one was selected, and why alternatives were rejected or deferred. Where an older report conflicts with the consolidated register, the register supersedes it. The [completeness audit](./completeness-review.md#cross-document-dependency-and-contradiction-gaps) records the known errata.

Use two independent status axes:

- **research status** says whether the behavior and proposed route have been mapped;
- **implementation status** says whether working, tested Rust code has landed.

Every domain below is research-complete. Foundation implementation is beginning from the approved reset. An accepted proposal in older research is not completed software and does not override an approved decision.

## Feature conversion map

The IDs below are intended to become direct sub-epics of one GitHub parent epic. The item-level `V1-*` mapping lives in [the feature ledger](./feature-ledger.md); implementation issues must reference both their epic and every affected ledger ID.

| Epic | Conversion outcome | First delivery gate | Research | Implementation |
|---|---|---|---|---|
| **E01 Rust foundations** | Workspace, two binaries, acyclic crate boundaries, stable domain errors, toolchain/dependency/license policy, reproducible code generation, and a reusable native client | Foundation | Complete; superseded choices resolved by greenfield reset | Workspace, binaries, protobuf generation, and Engine port landed; client remains |
| **E02 Lifecycle and configuration** | Loco YAML, secret-file references, supervised tasks, bounded shutdown, migrations, probes, packaging, install, and upgrade lifecycle | Foundation → parity | Complete; old v1 config mapping is discarded | HTTP/RPC supervision and environment configs landed; probes/packaging remain |
| **E03 API and protocols** | Greenfield `/api/v1` REST/OpenAPI, operation polling, protobuf WSS streams, private Tonic, stable errors, limits, and generated clients | Foundation → parity | Complete where reset is authoritative | Status/OpenAPI and `kontainer.agent.v1` check-in skeletons landed |
| **E04 Security and identity** | Atomic bootstrap, GitHub parity auth, generic OIDC production profile, RBAC/capabilities, scoped service tokens, rotating sessions, native credential vault, encryption, mTLS, and append-only audit | Foundation → beta | Complete; provider, role, vault, and KMS decisions remain | Not started |
| **E05 Cluster communication** | Replace leader/follower registration with manager controllers plus global node agents, authoritative Swarm membership, generation-safe identity, bounded fan-out, Tonic/mTLS, and HA leases/claims | Foundation → beta | Complete; PKI and ingress decisions remain | Separate processes and mandatory-mTLS server seams landed; enrollment/membership remain |
| **E06 Swarm resources** | Services, tasks, nodes, networks, volumes, images, secrets, configs, derived stacks, optimistic concurrency, locality-aware routing, and explicit partial results | Parity | Complete; Engine support floor requires approval | Not started |
| **E07 Persistence and operations** | PostgreSQL control truth and telemetry separation; transactional operation, idempotency, ordered state, audit intent, backup, and schema migration | Durable operation → beta | PostgreSQL-only and no v1 migration approved | SeaORM/migration foundation landed; first durable-operation schema is next |
| **E08 Metrics** | Node-local one-shot collection, bounded spool, idempotent push, cluster snapshots, history/rollups, gaps, retention, and separate metrics storage | Parity → GA | Complete; definitions and HA backend require evidence | Not started |
| **E09 Logs** | Manager-native service/task log streaming, correct Docker multiplex decoding, binary WSS framing, quotas, cancellation, slow-consumer handling, and honest best-effort resume | Parity | Complete; final limits/resume policy remain | Not started |
| **E10 Exec** | Target-node agent owns container exec; binary stdin/stdout/stderr/resize, real or explicitly unknown exit, session quotas, cleanup, RBAC, and content-free audit | Parity | Complete; shell/user/session policy remains | Not started |
| **E11 Tauri cutover** | Reuse the native Rust client through typed Tauri commands, migrate profiles and credentials to the OS vault, isolate query/client state per profile, and preserve current screens/workflows | Parity | Complete at architecture level; profile state machine remains | Not started |
| **E12 Packaging and recovery** | Two minimal OCI images, Swarm manifests, controller-first rolling upgrades, schema compatibility, backup/restore, DR, offline bundles, and rollback rules | Parity → GA | Complete; release matrix values remain | Not started |
| **E13 Observability** | Structured diagnostic logs, private process metrics, optional traces, correlated operation history, truthful readiness, alerts, redacted support bundles, and separate security audit | Foundation → GA | Complete; exporters/retention values remain | Not started |
| **E14 Reliability and performance** | Fake Engine plus real Swarm suites, property/fuzz/contract tests, resource budgets, load/soak/chaos/security/upgrade/DR gates, and immutable release evidence | Every gate | Complete; numeric SLOs require benchmarks | Not started |
| **E15 Supply chain** | Locked/offline builds, dependency/license/advisory policy, native `amd64`/`arm64` evidence, SBOM, signatures, provenance, scanning, and build-once promotion | Foundation → GA | Rust/toolchain and both architectures approved; license deferred | Toolchain and lockfile landed; Linux/native release evidence remains |
| **E16 V1 parity and retirement** | Give every current behavior an explicit migrate/fix/retire/defer/reject outcome, import or archive old state, cut over clients, remove Go only after evidence, and publish rollback limits | Parity → GA | Inventory complete; several product dispositions remain open | Not started |
| **E17 Stack deployment MVP** | Strict supported Compose subset, normalize/validate/plan/diff/apply, strong ownership, journaled rollout and recovery, without a Docker CLI subprocess | Separate feature MVP | Complete; conditional and decision-blocked | Not started |

## What v2 converts

The complete source inventory contains 411 stable `V1-*` items and 12 unresolved `V1-Q-*` questions. At capability level, the rewrite converts:

- setup, authentication, sessions, profiles, authorization, and health;
- service, task, node, stack, network, volume, image, secret, and config reads and mutations that exist today;
- agent discovery, node-local routing, scheduled reconciliation, migrations, configuration, and role-specific startup;
- cluster and node metrics, retention, query ranges, and storage behavior;
- service/task logs and interactive container exec;
- all 51 Tauri commands, their Rust/TypeScript boundary types, current desktop workflows, and local profile/session behavior;
- packaging, installation, upgrades, backups, observability, support, testing, and release evidence;
- known bugs and cross-language contract mismatches as regression tests for corrected behavior, not as compatibility promises.

The initial ledger classifies those 411 items as 144 visible-capability migrations, 104 repairs, 80 folds into cleaner contracts, 41 removals, 14 deferrals, 11 data migrations, 9 archives, and 8 evidence-only items. It currently marks 262 rows `decision-blocked` and 149 `open`; both still mean **not implemented**. The high blocked count is useful—it exposes shared decisions that must be made once before many leaf issues can become ready.

Useful v1 outcomes are preserved, but the wire format is intentionally allowed to break under `/api/v2`. That is safer than maintaining accidental response nesting, wrong status handling, unbounded collections, query-string JWTs, plaintext internal RPC, process-local coordination, and successful mutations that the client reports as failures.

## What does not convert as parity

These are absent from the initial parity promise unless a linked decision explicitly promotes them:

- a GPUI product UI, Kubernetes or Compose orchestration backends, Windows/ARMv7 agents, remote/NAT agents, or cross-Swarm federation;
- service creation, node join, general network/volume lifecycle, image pull, registry/build/GitOps workflows, or generic Docker API access that v1 never completed;
- Compose file deployment—tracked separately as E17—although current stack list/removal stays in parity;
- durable workload-log storage/search, exact historical log replay, transparent/detachable exec, privileged arbitrary exec, or host shell access;
- several interchangeable metrics databases or message buses “just in case”; backend ports remain extensible, but only qualified profiles ship;
- UI placeholder cards and labels with no backend. Each is removed, capability-disabled, deferred, or promoted through an explicit issue; a mock control is not parity.

V1 defects are also not converted. The v2 acceptance contract must instead prove the corrected behavior.

## Decisions to make before production Rust code

Evidence spikes may be written first, but production code should wait for the following decision groups. Their full alternatives and dependencies are in [the decision register](./decision-register.md#decisions-required-before-the-first-production-rust-code).

1. Release vocabulary and product profiles; where the Rust workspace coexists with Go; Rust edition/MSRV/license/dependency policy.
2. The two-binary authority split and acyclic crate direction.
3. Public/internal protocol families, separate domain/wire/persistence types, auth redemption and WSS authentication, compatibility window, and code-generation authority.
4. Approval to replace plaintext desktop `session.json`/profile credentials with native OS-vault custody.
5. Capability names and minimum roles/scopes before handlers or migrations encode policy.
6. Embedded/HA storage profiles, one SQLite access route, migration ownership, and the atomic operation/idempotency/audit-intent transaction.
7. Internal workload identity, certificate SAN/delegation rules, and a test PKI before private RPC code.
8. The initial Linux/Docker Engine API support floor.

The first implementation backlog also needs explicit decisions for:

- the unique desktop profile identity and its edit/switch/remove/logout state machine;
- whether backend-only agent discovery, live task stats, and arbitrary controller settings become typed capabilities, diagnostics, or removed APIs;
- a key-by-key migration or retirement outcome for all 70 v1 configuration items;
- copy/transform/archive/discard outcomes for every v1 server and desktop data class;
- each current placeholder UI affordance;
- whether the historical, unmerged service CPU/memory editor is a real v2 feature or rejected history.

Do not hide these in implementation PRs. Create decision issues, resolve them, then update the stable `ADR-CAND-*` row without renumbering it.

## Delivery order

The recommended sequence is intentionally vertical: prove permanent boundaries with one real path before porting breadth.

| Milestone | Deliverable | Exit meaning |
|---|---|---|
| **0 — architecture ready** | Approve blocking ADRs; run HTTP/TLS, Docker/Bollard, storage, protobuf, OpenAPI/client, runtime, PKI, desktop-client, and supply-chain spikes | Production implementation may begin |
| **1 — secure walking skeleton** | Authenticated `/api/v2/capabilities` through controller, manager Engine adapter, same-node mTLS node agent, and reusable native client | Permanent process, identity, contract, Docker, lifecycle, and shutdown boundaries work |
| **2 — one durable operation** | Service list/detail and scale with ETag, idempotency, atomic operation/audit intent, crash recovery, real Swarm tests, and desktop error mapping | The mutation substrate is real and reusable |
| **3 — embedded parity** | Manager read models, node-local images/volumes, metrics, remaining mutations, logs, then exec; desktop and v1 data migration included | Corrected v1-visible workflows pass on a clearly non-HA profile |
| **4 — production beta** | Three manager/controllers, shared control store, automated PKI, production ingress, native arm64, restore, upgrade, chaos, and medium soak | Production topology and operations are proven as a prerelease |
| **5 — v2.0 GA** | Exact advertised profile matrix passes SLO, load, security, compatibility, canary, DR, support, and immutable-artifact gates | Production release; no rebuild during promotion |
| **Feature MVP — optional** | Compose stack deploy behind explicit capability discovery and its own parser, policy, recovery, and chaos gates | Independent product feature; does not silently redefine parity |

Within parity, implement shared manager read models first, node-local unary operations second, metrics third, mutations on the proven operation substrate fourth, manager-native logs fifth, and node-agent exec last. [The phasing review](./phasing-review.md) contains the dependency graph, parallel tracks, risk order, and exit evidence.

## GitHub tracking recommendation

GitHub Issues are now the execution source of truth. Do not use one giant checkbox issue as the source of truth.

The live hierarchy contains:

1. [one Kontainer `1.0` parent issue](https://github.com/prvious/kontainer/issues/40);
2. 15 greenfield domain epics as native sub-issues, with the obsolete v1 migration/retirement and deferred Compose epics removed;
3. five review-ready [Foundation issues](https://github.com/prvious/kontainer/milestone/1) for PostgreSQL operations, real Docker negotiation, real mTLS, binary WebSocket framing, and native Linux AMD64/ARM64 evidence;
4. native blocked-by relationships for real ordering;
5. Foundation, `0.1 — Secure skeleton`, `0.5 — Product beta`, `1.0 — Stable`, and HA qualification milestones.

Every new implementation issue should include:

- owning epic, milestone/profile, and unresolved decisions that genuinely block it;
- outcome, accepted route, scope, explicit non-goals, and failure/partial-result behavior;
- security/privacy and performance/resource-budget implications;
- unit, contract, integration, real-Swarm, migration, chaos, or release evidence appropriate to the change;
- observability, documentation, rollout, and rollback behavior where applicable;
- a completion rule that requires merged code and evidence, not merely an opened PR.

GitHub's hierarchical sub-issues and issue dependencies are sufficient for the initial three-level structure. A Project remains optional; the current token lacks Project scope and no Project was created. [The tracking document](./github-tracking.md) records the live hierarchy and retains the earlier blueprint as historical template material.

## Delivery workflow

1. Start from the next unblocked, review-ready leaf issue; initially this is [the PostgreSQL durable-operation walking skeleton](https://github.com/prvious/kontainer/issues/56).
2. Link implementation PRs to their leaf issue and use `Closes #...` only when the full acceptance criteria and evidence are complete.
3. Add leaf issues only when their contract and blockers are understood; do not expand every research bullet into speculative tickets.
4. Close epics only when their observable outcomes and gate evidence are complete, not when their initially imagined children happen to be closed.
5. Promote deferred capabilities through an explicit product decision before adding them to the active hierarchy.

## Research report index

| Report | What it owns |
|---|---|
| [V1 parity inventory](./v1-parity.md) | Exhaustive repository archaeology and stable behavior IDs |
| [Feature ledger](./feature-ledger.md) | One disposition, owner, replacement, acceptance path, and status per v1 item |
| [Decision register](./decision-register.md) | Consolidated accepted, provisional, open, deferred, and rejected routes |
| [Completeness review](./completeness-review.md) | Cross-report omissions, contradictions, errata, and closure requirements |
| [Delivery phasing](./phasing-review.md) | Walking skeleton, vertical slice, phase dependencies, parallel work, and gates |
| [GitHub tracking](./github-tracking.md) | Ready-to-use epic structure, templates, labels, milestones, and ordering |
| [Rust architecture](./rust-architecture.md) | Workspace/crates, boundaries, actions, runtime model, dependency spikes, and migration strategy |
| [Agent lifecycle](./agent-lifecycle.md) | Processes, configuration, scheduling, health, shutdown, packaging, installation, upgrade, and DR |
| [API and protocol](./api-protocol.md) | HTTP/WSS/SSE/gRPC contracts, clients, compatibility, types, limits, and generation |
| [Security](./security.md) | Bootstrap, identity, RBAC, sessions, secrets, mTLS, audit, threat controls, and supply chain |
| [Cluster communication](./cluster-communication.md) | Membership, identity, routing, coordination, Tonic services, HA, and partitions |
| [Swarm resources](./swarm-resources.md) | Docker API approach, resource semantics, locality, actions, partial results, and concurrency |
| [Metrics and storage](./metrics-storage.md) | Collection, spool/push, schemas, queries, retention, backend choices, and migration |
| [Logging](./logging.md) | Sources, Docker framing, WSS stream semantics, resume, quotas, and tests |
| [Exec](./exec.md) | Session state machine, node routing, raw upgrade handling, I/O, exit, auth, and cleanup |
| [Stack deployment](./stack-deployment.md) | Compose subset, compiler/reconciler design, ownership, plans, operations, and recovery |
| [Performance](./performance.md) | Engine-load controls, snapshots, fan-out, queues, budgets, benchmarks, and scale profiles |
| [Observability](./observability.md) | Diagnostics, process telemetry, tracing, health, SLOs, alerts, and support bundles |
| [Reliability and testing](./reliability-testing.md) | Test pyramid, environments, parity/beta/GA gates, chaos, load, upgrades, and release evidence |

## Current truth

The greenfield Rust foundation and GitHub execution hierarchy both exist. The first active implementation issue is [#56](https://github.com/prvious/kontainer/issues/56). Older v1 research remains useful archaeology, not a compatibility or migration backlog. No GitHub Project has been created.

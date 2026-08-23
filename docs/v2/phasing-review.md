# Kontainer v2 delivery phasing review

Status: recommended delivery plan

Review date: 2026-08-19

Scope: dependency-respecting delivery from zero Rust production code through embedded parity, beta, and production GA

## Executive recommendation

Treat **parity as a secure, explicitly non-HA release checkpoint**, not as production GA. Reserve the first production GA claim for the shared-store, multi-controller profile described by the [decision register](./decision-register.md), [reliability gates](./reliability-testing.md#release-gates), and [cluster design](./cluster-communication.md#v1-parity-and-v2-production-requirements). This resolves the reports' most consequential ambiguity: SQLite parity is a valid product profile, but it is not controller failover.

The recommended first-GA scope is deliberately narrow:

- Linux Docker Swarm on the approved Engine window, with native `amd64` and `arm64` evidence;
- the existing desktop-visible v1 capability set, corrected for security, contract, resource-bound, locality, and honesty defects;
- one controller plus global node agents for the embedded parity checkpoint, then active controllers on managers with a shared transactional store for beta and GA;
- GitHub identity for parity and the approved OIDC profile before making the corresponding production/enterprise identity claim;
- no Compose deployment in the first GA capability set unless ADR-CAND-039 is explicitly reversed and the independent stack feature gate passes.

The smallest end-to-end walking skeleton is not “all resource lists” and is not a temporary plaintext clone of v1. It is one authenticated capability read through the permanent boundaries:

```text
native test host / kontainer-client
  -> HTTPS /api/v2/capabilities on kontainer-controller
  -> controller capability action
       -> local manager Engine /version + /info through EngineClient
       -> mTLS AgentInfo.Describe on the same manager's node-agent task
            -> node-local Engine /version + /info through EngineClient
  -> bounded, typed public response with freshness and capability reasons
```

Run it first on a one-manager fixture where the controller and node agent coexist on the same node but have distinct workload identities. The integration harness may inject a test principal and test PKI at composition; release binaries must contain no insecure authenticator, plaintext RPC, shared-secret fallback, or “skip verification” switch. This skeleton proves role separation, dependency direction, configuration, lifecycle, public and private contracts, identity, Docker adaptation, bounded I/O, client reuse, and shutdown without prematurely taking on OAuth UI or a destructive mutation.

Immediately after the skeleton, port one representative mutation end to end: **service scale**. It is the smallest useful operation that proves the permanent mutation substrate: authorization, fresh read and ETag, idempotency, atomic operation/audit intent, Engine version conflict handling, uncertain outcome reconciliation, polling, and desktop error mapping. Do not fan out into the rest of parity until this path survives crash points.

## Milestone contract

This review uses the decision register's milestone vocabulary and recommends approving ADR-CAND-001 accordingly.

| Milestone | Product meaning | Release posture |
|---|---|---|
| **Foundation** | Decisions, evidence spikes, workspace, contracts, and secure walking skeleton | Not a user release |
| **Embedded parity** | All supported v1-visible workflows, corrected, on one controller plus global node agents and local SQLite stores | Preview/non-HA; availability and RPO limits are visible |
| **Beta** | Production-shaped three-manager/controller topology, shared control store, automated identity lifecycle, signed RC artifacts, migration/restore, and medium chaos/soak evidence | Prerelease; capability set is frozen enough for upgrade testing |
| **GA** | Advertised HA/storage/profile matrix passes compatibility, security, load, canary, restore, and DR gates on the exact promoted artifacts | Production release; no rebuild during promotion |
| **Feature MVP** | A separately gated new capability, notably Compose stack deployment | Does not redefine parity or GA automatically |

## Phase 0: decisions and spikes before production implementation

Time-boxed evidence code may precede these decisions, but it must stay outside production paths and must not make an unapproved dependency or workspace layout a fait accompli.

### Decisions that block the first production Rust commit

| Decision group | Required closure | What it prevents |
|---|---|---|
| Product and repository | ADR-CAND-001, ADR-CAND-002, ADR-CAND-079 | An ambiguous GA claim, a repository-rule violation, or an unowned toolchain/license/dependency graph |
| Process and crate authority | Ratify ADR-CAND-003 through ADR-CAND-007 | A universal mode-flag daemon, controller-local exec, node-owned Swarm logs, or a controller/stack crate cycle |
| Contract authority | ADR-CAND-012, ADR-CAND-013, ADR-CAND-016, ADR-CAND-021, ADR-CAND-033, ADR-CAND-066 | Wire types spreading before public/internal versioning, auth redemption, WebSocket authentication, generation, and compatibility are stable |
| Security policy | ADR-CAND-019 and the accepted identity model in ADR-CAND-045 | Handlers, RPCs, and migrations encoding ad hoc roles or trusting message claims as workload identity |
| Storage and mutation truth | ADR-CAND-029, ADR-CAND-030, ADR-CAND-034, ADR-CAND-078 | Competing operation/audit ledgers, an accidental one-file database, or persistence code that cannot grow into HA |
| Engine support | Ratify or explicitly mark provisional ADR-CAND-048 | Resource adapters silently promising unsupported Docker semantics |
| Desktop custody | Approve ADR-CAND-018 now; implement its vault migration before desktop auth code | Recreating plaintext `session.json`/`profiles.json` storage in new code |

The repository-rule change in ADR-CAND-018 is part of the first-code approval gate even though the actual vault adapter lands with desktop auth. ADR-CAND-031, ADR-CAND-032, ADR-CAND-041, ADR-CAND-046, ADR-CAND-047, ADR-CAND-049, and ADR-CAND-051 may remain open during early embedded work behind explicit ports and test keys; they must close before the production claim they control. Dependency-file changes still require the repository's separate approval.

### Spike order and commitment points

1. **Run together before the dependency graph is committed:** HTTP/TLS graph unification, SQLite/Postgres selection, protobuf/codegen, rustls/PKI, and supply-chain/license/build-budget spikes (Rust spikes 1, 3, 4, 6, and 10).
2. **Prototype runtime ownership in parallel:** spike 8 must prove tracked cancellation, bounded queues, terminal records, and shutdown. Its result shapes every later long-lived task.
3. **Before the first Engine-backed capability:** spike 2 must settle Bollard coverage, API negotiation, type-3 log frames, raw TTY leading bytes, cancellation, and the exact narrow raw escape hatch.
4. **Before public DTOs or generated clients spread:** spike 5 must freeze the normalized OpenAPI path and generator behavior.
5. **Before desktop protocol migration:** spike 9 must prove a UI-neutral client, native WebSocket headers, refresh coordination, cancellation, and no Tauri/server dependency leakage.
6. **Defer spike 7 with Compose deployment:** it is not a core, parity, or beta prerequisite when ADR-CAND-039 follows the recommendation.

Phase 0 exits only when each spike has reproducible fixtures, measured results, license/build evidence, and a decision update. “Library builds” or “hello world succeeds” is not an exit criterion.

## Production controls by introduction point

Controls that define an interface or invariant must exist before feature breadth. Controls that require production topology or measured evidence can be layered later without weakening the early architecture.

| Introduction point | Required controls |
|---|---|
| **First production Rust commit** | Separate binaries and composition roots; inward dependency checks; strict role-specific config; safe typed errors and redaction; one lifecycle/health state owner; root/child cancellation and tracked tasks; bounded item-and-byte queues, admission, deadlines, and ordered shutdown; test-PKI mTLS with verified workload identities; deny-by-default capability hooks; operation/idempotency/audit-intent transaction interfaces before any mutation; checked source contracts and golden generation; negotiated typed `EngineClient` with no raw proxy; deterministic clock/random/test seams; `forbid(unsafe_code)` where applicable; locked toolchain/dependencies, no-zero-test enforcement, and basic build metadata/SBOM policy. |
| **Before embedded parity** | GitHub bootstrap/login with state, PKCE, one Owner, recovery, short access/rotating refresh sessions; OS-vault migration and native credential custody; manual per-workload certificates on the correct mTLS protocol; file/Docker-secret KEK and encrypted provider/signing material; local audit chain/checkpoint; patched pinned SQLite, explicit migrations/import/backup/restore; pagination, ETags, durable operations, partial/stale semantics; shared clients/snapshots, one-shot stats, bounded fan-out and streams; safe JSON diagnostics, private process metrics, truthful probes, local support bundle; all v1 mismatch and parity fixtures. |
| **Before beta** | Shared transactional control store and leases/claims; three active controllers and stable ingress; automated workload enrollment/renewal/revocation; approved signing/KMS profile; revocation and admission propagation; migration crash recovery and restore; signed build-once RCs with SBOM/provenance; native arm64 smoke; dashboards/alerts/runbooks and bounded trace export; current/previous mixed-version rollout; medium load, soak, and core chaos. |
| **Before GA** | Final HA metrics path for the advertised metrics capability; external audit anchoring; hardened installer/socket/network profile; approved SLO/RPO/RTO and complete Engine/OS/ABI matrix; five-manager/loss-of-two evidence where advertised; full load/72-hour storage/eight-hour large stress; penetration and complete chaos gates; expand/contract plus downgrade; canary waves and error-budget evidence; timed fresh-environment DR; verified artifact signatures/attestations and no-rebuild promotion. |

Telemetry, security, and resource bounds are not a polish phase. Their minimal typed interfaces and safe defaults belong in the first production code; richer exporters, external anchors, shared limit propagation, dashboards, and numeric policy qualification belong at the milestone where their topology and measurements exist.

## Dependency-respecting delivery sequence

### Milestone 0 — architecture ready to code

Deliver the approved ADRs and evidence described above. Establish the first-party license, Rust location/coexistence plan, exact release profiles, and dependency approval boundary.

Exit criteria:

- no item in the decision register's “required before first production Rust code” list remains open;
- one selected dependency/toolchain path exists for HTTP/TLS, protobuf, SQLite, and build policy;
- controller, node-agent, client, protocol, stack, and storage dependency directions are acyclic on paper and enforceable in CI;
- embedded and HA profiles, and the meaning of the first GA, are named without contradictory claims;
- the Compose deployment decision is explicitly outside the core critical path.

### Milestone 1 — secure walking skeleton

Create the minimum workspace split needed to enforce boundaries, not the entire proposed crate tree. Implement separate controller/node-agent binaries, strict config validation, lifecycle state, probes, tracked shutdown, test PKI, versioned `Health`/`Describe`, the narrow Engine capability adapter, the public API root/capability DTOs, and the UI-neutral client call described above.

Exit criteria:

- controller and node agent on the same manager remain distinct identities and processes;
- wrong CA, cluster, workload kind, node/task binding, expiry, plaintext, and shared-secret attempts fail closed;
- the public capability response accounts for controller and agent observations, carries freshness, and is bounded;
- fake Engine and pinned one-manager DinD scenarios cover success, unsupported API, daemon stall, malformed response, disconnect, and cancellation;
- `SIGTERM` makes both processes unready, cancels and joins all work, and exits within the measured budget with no leaked tasks, permits, sockets, or secret-bearing output;
- OpenAPI/protobuf artifacts regenerate deterministically and core crates import no transport, SQL, Bollard, or generated wire types.

This milestone is infrastructure evidence, not a parity preview.

### Milestone 2 — one durable user operation

Build the controller resource snapshot for services and implement service list/detail plus scale through the public client. Scale must use a strong revision, an idempotency key, the shared operation transaction, required audit intent, a preservation-safe Engine patch, observation of accepted versus converged state, and typed uncertainty. Operation polling is required; SSE may follow when ADR-CAND-070 closes.

In parallel, complete the real GitHub bootstrap/session path and native-vault client seam so the same service workflow can run through the shipping desktop without changing the server action.

Exit criteria:

- a second through hundredth observer does not multiply manager Engine reads; list output is bounded, stable, and explicit about snapshot age;
- scale replay returns the same operation, key/hash conflict fails, stale ETag fails, and every crash point after admission is recoverable or honestly uncertain;
- unrelated service-spec fields survive the patch and Engine version races never overwrite them;
- authorization, operation, audit, diagnostics, and client errors share stable IDs without sharing payload types;
- both fake Engine and a real Swarm prove acceptance, rollout observation, cancellation limits, and restart recovery.

This is the “architecture is real” gate. Other mutations should copy it, not invent their own lifecycle.

### Milestone 3 — embedded v1-visible parity

Convert capability families in dependency order:

1. **Manager read models first:** services, current/history-bounded tasks and task summary, nodes, derived stacks, Swarm networks, secrets/config metadata, authorized config content with explicit UTF-8/base64 encoding, and capabilities. Build these from shared event-invalidated snapshots plus reconcile, not per-user Docker scans.
2. **Node-local unary work next:** image inventory and node-scoped remove/prune, then local volume provenance. This proves bounded fan-out, target accounting, agent-side deduplication, and same-manager routing before interactive streams.
3. **Metrics next:** node-agent collection/spool and idempotent push at the corrected 60-second parity cadence; controller commit/query into separate metrics storage; current snapshot and bounded 1/6/24-hour history. Do not build a serial pull scheduler that will be discarded.
4. **Manager mutations after the operation substrate is proven:** restart, rollback, image/environment update, node availability/role, secret/config create/remove, typed settings, and stack removal. Put high-risk and multi-resource work after ordinary service operations; stack removal must journal every resource and retain volumes.
5. **Streams last:** manager-native service/task logs first, then exec through the target node agent. Exec is the final parity family because it composes desktop native streaming, WSS, target resolution, authorization grants, mTLS bidi RPC, raw Docker upgrade framing, session quotas, content-free audit, and orphan/unknown-exit semantics.

Authentication/setup, desktop migration, contract fixtures, observability, and release evidence progress alongside these resource waves. They are not postponed until the resources are “done.”

Exit criteria are exactly the [reliability parity gate](./reliability-testing.md#gate-1-v1-visible-parity), with these scope clarifications:

- every desktop-visible v1 workflow is present, while backend-only, placeholder, and aspirational surfaces have an explicit decision;
- all V1-GAP mismatches have corrected contract tests; no successful destructive action can be presented as a decode failure;
- no JWT appears in a URL or WebView, all internal traffic is mTLS, and manual certificate provisioning is clearly non-production;
- all collections, fan-outs, Docker records, protobuf/JSON messages, queues, sessions, and response sizes are bounded and expose partial/stale/gap outcomes;
- v1 control/profile/metric data has a tested migrate, archive, or discard result and a verified pre-migration restore path;
- the one-controller dual-SQLite profile is visibly non-HA, and only the supported `amd64` preview artifact is claimed at this gate unless additional native evidence has passed.

### Milestone 4 — production-shaped beta

Replace profile adapters, not domain actions: add PostgreSQL control repositories and database-backed leases/claims, production ingress, automated PKI lifecycle, and the selected shared/routed metrics path. Run three active controllers; do not introduce a public-request leader. Qualify the adjacent-release controller-first/agent-second rollout and reverse rollback pairing.

Exit criteria are the [reliability beta gate](./reliability-testing.md#gate-2-beta), including:

- three managers/controllers and global node agents survive a manager loss with correct same-node identity and singleton claims;
- schema migration, backup/restore, wrong-key, disk-full, controller kill, agent partition, certificate rotation/revocation, and shared-DB interruption have bounded, truthful outcomes;
- current/previous Engine lines and native `amd64`/`arm64` runtime smoke pass;
- the medium load and soak meet provisional budgets with dashboards, alerts, audit, readiness, and support evidence;
- the exact prerelease artifacts are scanned, SBOMed, signed, attested, install-smoked, and independently verified;
- an operator other than the author has executed upgrade, rollback, restore, incident, and support runbooks.

Do not add Compose deployment during beta stabilization. If product elects to ship it, run its Phase 1 feature MVP as a parallel capability behind explicit discovery and do not let it change the beta definition until its parser, policy, operation, recovery, and chaos gates pass.

### Milestone 5 — production GA

Freeze the advertised topology, storage, Engine, platform, architecture, and capability matrix. Run qualification on the exact release candidates and promote the same digests.

Exit criteria are the [reliability GA gate](./reliability-testing.md#gate-3-production-ga):

- HA controllers use the selected shared transactional store; the advertised manager-failure envelope passes;
- every advertised metrics, identity, audit, ingress, and storage profile has its own conformance, backup/restore, and operational owner;
- N/N-1 protocol and schema windows, rolling upgrade, binary downgrade, failed-canary rollback, and unsupported-version refusal pass;
- all declared load envelopes, SLOs, chaos/security/penetration gates, and headroom reports pass without silent partial or uncertain success;
- timed fresh-environment DR meets the published RPO/RTO with fencing and application reconciliation, not merely a successful database command;
- alerts fire and clear, canary waves remain inside the error budget, and release promotion performs no rebuild;
- support, security reporting, artifact revocation, end-user migration, rollback, and incident ownership are published and exercised.

## Parallel tracks and integration points

Work can proceed in parallel only while each track targets the already-approved ports and contracts.

| Track | Can progress independently on | Must integrate at |
|---|---|---|
| Architecture/toolchain | workspace, dependency policy, codegen, lint, feature profiles, build budgets | Milestone 1 dependency graph and every release artifact |
| Runtime/diagnostics | task supervision, lifecycle registry, safe telemetry, fake clocks, bounds, support bundle core | every listener/owner loop; operation recovery; shutdown and release gates |
| Security/identity | policy registry, token/key types, test PKI, bootstrap/session/provider adapters, vault migration | before protected public routes, node RPC, desktop cutover, and every high-risk action |
| Engine/resources | typed Engine ports, fake daemon, snapshots, resource conversion and preservation | Engine spike; operation store for mutations; agent routing for local work |
| Storage/operations/metrics | repository contracts, SQLite/Postgres conformance, migrations, operation state, spool/ingest | first mutation, metrics wave, beta shared-store cutover, restore/DR |
| Client/desktop | `kontainer-client`, explicit IPC adapters, generated TypeScript, profile/vault migration, UI parity fixtures | stable OpenAPI/streams, real auth, each converted capability family |
| Verification/release | fake Engine, DinD/VM fixtures, contract baselines, native runners, artifact and chaos pipeline | every milestone exit; exact RC qualification at beta/GA |
| Stack feature | source/parser spike, normalized plan, policy and reconciler behind shared operation ports | only after core operation/security/storage contracts; independent feature gate |

The principal integration points are deliberately few:

1. contract/toolchain freeze before workspace code spreads;
2. test identity plus mTLS/Engine skeleton before node-local features;
3. one atomic operation transaction before any mutation family;
4. normalized OpenAPI/client contract before desktop commands migrate;
5. shared store plus automated PKI before multi-controller beta;
6. exact immutable artifacts before load, upgrade, canary, and DR evidence counts.

## Migration and compatibility adapters

Adapters should isolate migration risk and expire. They must not leak v1 shapes into v2 domain types.

| Adapter | Required treatment | Removal gate |
|---|---|---|
| Go v1 coexistence | Keep Go v1 and Rust v2 as distinct deployable surfaces during migration. Use catalog/capability selection; one request/session never mixes versions. | All supported clients select v2 and the announced support window ends. |
| Optional `legacy-v1` HTTP/WS | Ship only if ADR-CAND-069 finds a real old-client need. Compile/deploy it separately, translate frozen actual Go shapes to v2 actions, and never alias `/api` to `/api/v2`. | Route-use policy, frozen fixtures, and sunset criteria pass. |
| Desktop IPC | Preserve narrow existing Tauri command names temporarily where that reduces React churn, but implement them over `kontainer-client`; credentials and direct WebView WebSockets are not preserved. | React uses generated/public models and the consolidated native client/stream managers. |
| Profile credentials | Copy refresh/session authority into the selected OS vault, verify read-back and server usability, then remove plaintext. Metadata keeps only the cluster/canonical URL and vault reference. A URL change requires reauthentication. | Canaries prove no credential remains in `session.json`, `profiles.json`, WebView state, logs, bundles, or crash output. |
| Control data | Explicit import maps users/providers/config into principals, external identities, typed provider/security records, encrypted secrets, and roles. Existing access JWTs are not upgraded into refresh sessions; require reauthentication where trust cannot be preserved. | Counts, invariants, key custody, audit continuity, rollback, and restore fixtures pass. |
| Metrics data | Detect both legacy `node_metrics` and unified `metrics`; copy into the new metrics store, retain a read-only archive, or discard only after explicit product choice and notice. | ADR-CAND-075/legacy policy is implemented with row/value verification. |
| Legacy stacks | Keep namespace-derived list and explicitly authorized removal. Treat them as unmanaged; never silently adopt from `com.docker.stack.namespace`. | Each stack is removed, remains legacy, or is explicitly adopted by the later stack feature. |
| Health aliases | Map `/health`, `/ping`, and draft health paths to the one lifecycle registry; do not preserve independent always-healthy logic. | Client/deployment support window ends. |

The recommended cutover is read-first and reversible: run frozen differential reads against Go and Rust, move a profile to v2 discovery/auth, enable v2 reads, then enable v2 mutations and streams only after their operation and cancellation evidence passes. Never dual-write Docker mutations through both agents. Rollback selects the previous complete deployment and compatible schema; it does not restore a database as a routine substitute for reconciliation.

## Critical path

The critical path is:

```text
profile/workspace/toolchain decisions
  -> dependency and protocol/storage/runtime spikes
  -> secure two-binary walking skeleton
  -> operation/idempotency/audit transaction + service-scale slice
  -> auth/vault/client cutover + resource/agent/metrics/stream parity
  -> migration and embedded parity gate
  -> shared store + automated PKI + three-controller beta
  -> mixed-version/load/chaos/restore evidence
  -> exact-artifact canary + timed DR + GA promotion
```

Compose deployment, GPUI, extra orchestrators, external log search, detachable exec, native service packaging, remote/NAT agents, additional metrics backends, and advanced security profiles are not on this path.

## Risk burn-down order

| Order | Risk to retire | Why it is early |
|---:|---|---|
| 1 | Repository, license, toolchain, dependency, and protocol-generation uncertainty | Every crate and artifact otherwise accumulates migration cost before implementation starts. |
| 2 | Role/authority and trust-boundary mistakes | A plaintext or ambiguous controller/node topology would infect every RPC, test, and deployment. |
| 3 | Runtime ownership and bounded-resource behavior | Detached tasks and unbounded streams become harder to remove after features depend on them. |
| 4 | Engine/API and byte-stream correctness | Bollard gaps, version floors, type-3 log frames, and raw TTY detection can invalidate the central adapter. |
| 5 | Atomic operation, idempotency, audit intent, and crash recovery | Every mutation, stack step, lease, and partial result depends on one authoritative state model. |
| 6 | Native auth and credential custody | The desktop cannot safely cut over, and stream transport cannot stabilize, until secrets stay out of React and URLs. |
| 7 | Data/profile migration and rollback | Parity is not usable if upgrade loses access, settings, or history without a declared outcome. |
| 8 | Parity breadth, ordered from reads to unary mutations to streams | Feature bugs are cheaper to fix after the substrate has survived one representative vertical slice. |
| 9 | Shared-store, PKI automation, and controller failover | These turn the proven embedded interfaces into the production topology without rewriting domain actions. |
| 10 | Scale, compatibility, canary, supply-chain, and DR evidence | Numeric and operational policy should be finalized from the complete, exact artifacts rather than prototypes. |

The riskiest v1 capabilities should therefore be converted last: exec; multi-resource stack removal; node-role changes; secret/config destructive operations; and long-running/high-volume logs. The first conversions should be capability discovery and read-only service/task/node views, followed by service scale as the mutation exemplar and image inventory as the node-RPC/fan-out exemplar.

## Explicit exclusions

The following are absent from embedded parity and the recommended first GA unless a separately approved capability gate says otherwise:

- Compose stack deploy/update, multi-file merge, Git/OCI sources, GitOps, image build/push/signing workflows, and deployment strategies such as canary or blue/green;
- service creation/deletion as a product workflow, network/volume lifecycle, Swarm initialize/join/token management, registry catalog browsing, and image pull UI merely because old UI copy implied them;
- Kubernetes, non-Swarm Compose runtime, cross-Swarm federation, NAT/edge relays, QUIC, NATS, and a second consensus system;
- GPUI product rewrite, native controller/node services, Windows, ARMv7, musl/static claims, or additional desktop platforms;
- node-local container-log fallback, exact Engine-backed historical resume/`until`, durable log storage/search, and bundled external log backends;
- detachable/replayable exec, arbitrary argv/environment/workdir, privileged exec, node shells, and automatic reopen;
- several external metrics stores, PromQL product surface, long raw per-container retention, arbitrary labels, or a custom time-series engine;
- public gRPC/GraphQL/JSON-RPC/Connect, public protobuf/CBOR, raw Docker specs, arbitrary Docker proxying, and generic string settings;
- SPIRE/TPM/DPoP/WebAuthn/SCIM/policy-as-code and other advanced profiles beyond the approved baseline seams;
- automatic support upload, public debug/heap/task dumps, default core dumps, self-update, mutable production tags, or database restore as routine rollback.

Unsupported capabilities must be absent and reported through discovery, not represented by empty endpoints, disabled buttons that imply imminent support, or compatibility paths that weaken v2 controls.

## Recommended first GitHub epic

**Epic: Rust v2 secure controller/node-agent walking skeleton**

Outcome: from an approved Rust workspace, an independent native test host can call the v2 capability API and receive a bounded view assembled from a controller's manager Engine and the same manager's distinctly identified node agent over mTLS; both processes start, become ready, reject invalid identity/configuration, and shut down cleanly.

Epic scope:

- close and link the Phase 0 ADR/spike prerequisites relevant to the workspace, HTTP/TLS, protobuf, runtime, Engine, storage selection, and supply chain;
- establish the minimum enforceable crate/binary split, locked toolchain, lint/dependency policy, deterministic contract generation, and no-zero-test CI;
- implement strict controller/node-agent config, lifecycle registry, canonical probes, tracked cancellation, bounded admission, safe diagnostics, and ordered shutdown;
- implement test-PKI mTLS, standard health plus versioned `AgentInfo.Describe`, identity/capability checks, and the controller's generation-aware single-agent channel;
- implement only the Engine `/version`/`/info` capability adapter behind Kontainer types, with fake-daemon and pinned DinD evidence;
- implement `/api/v2`, protected `/api/v2/capabilities`, the minimum `kontainer-client` call, explicit DTO/protobuf/domain mappings, and integration-only test principal injection;
- prove controller and node-agent coexistence on one manager, unsupported Engine behavior, cancellation, daemon/RPC failure, secret redaction, and leak-free shutdown.

Epic non-goals: GitHub/OIDC setup, production token issuance, React screens, arbitrary resource lists, SQLite domain schemas beyond startup compatibility, mutations/operations, metrics collection, logs, exec, Compose parsing, HA, release publishing, or a general Docker client.

The epic closes only on the executable end-to-end evidence. It must not close because scaffolding compiles, generated files exist, or unit tests pass without the two-process capability flow.

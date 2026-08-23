# Kontainer GitHub execution tracking

Status: greenfield hierarchy created in GitHub

Prepared: 2026-08-19

Implemented: 2026-08-23

Scope: Kontainer `1.0`, its Rust controller and node agent, desktop integration, and release qualification

## Live GitHub state

- [Product parent #40](https://github.com/prvious/kontainer/issues/40) owns 15 native domain sub-epics [#41 through #55](https://github.com/prvious/kontainer/issues?q=is%3Aissue%20state%3Aopen%20label%3Aepic).
- The first Foundation implementation/evidence set is [#56 through #60](https://github.com/prvious/kontainer/milestone/1), owned by its relevant domain epics.
- Five release-gate milestones exist: Foundation, `0.1 — Secure skeleton`, `0.5 — Product beta`, `1.0 — Stable`, and HA qualification.
- Labels distinguish epics, decisions, spikes, foundation work, evidence requirements, and major product areas.
- Native parent/sub-issue and blocked-by relationships are in use.
- No GitHub Project was created. Issues provide the initial source of truth, and the available token does not currently have Project scope.

The [approved greenfield reset](./approved-greenfield-reset.md) and the live GitHub issues supersede the proposed bodies below wherever they mention `/api/v2`, SQLite profiles, SSE, Go coexistence, v1 migration/parity, a `v2.0` release, or deferred capabilities as active epics. The remainder of this file is retained as historical template and research material; do not paste it into new issues without reconciling it to the reset.

## Purpose

This document turns the v2 research reports into a reviewable GitHub issue structure without pretending that the implementation has already been decomposed. It defines the parent epic, domain sub-epics, decision boundaries, milestones, release gates, labels, optional Project fields, issue template, sizing rules, and delivery order.

The authoritative engineering decisions remain in [the decision register](./decision-register.md). The authoritative verification requirements remain in [the reliability strategy](./reliability-testing.md). The exhaustive v1 inventory remains in [the parity report](./v1-parity.md). This file is a tracking view over those sources, not a competing architecture document.

## Repository convention audit

The private `prvious/kontainer` repository was inspected read-only on 2026-08-19.

- The default branch is `main`.
- The only repository issues are two closed issues, #5 and #10. Issue #5 uses `documentation` and `enhancement`; #10 has no labels. Both use free-form prose, and #5 uses a requirements section plus an acceptance checklist.
- The repository has only GitHub's nine default labels: `bug`, `documentation`, `duplicate`, `enhancement`, `good first issue`, `help wanted`, `invalid`, `question`, and `wontfix`.
- There are no milestones and no checked-in issue or pull-request templates.
- Most merged pull requests use Conventional Commit-style titles, especially `feat:`, `fix:`, `refactor:`, and `docs:`. Most PRs were not linked to an issue; #6 and #11 demonstrate the desired closing-link behavior.
- Project configuration could not be inspected because the available GitHub token lacks `read:project`. The optional Project design below must therefore be reviewed against any private organization Project before it is adopted.

Preserve the concise existing naming style. Add structure in issue bodies and relationships rather than putting a taxonomy into every title.

## Tracking principles

1. The decision register is the source of truth for architecture status. Issues reference `ADR-CAND-*`; they do not silently reinterpret those rows.
2. Open or provisional decisions are decision work, not implementation work. An implementation issue cannot close while a listed blocking decision remains open.
3. Accepted design is copied into implementation issues as a constraint. Reopening it requires a decision issue and a decision-register update.
4. Parent/sub-issue relationships express decomposition. `Blocked by`/`Blocking` relationships express execution order. A checklist mention is not a substitute for either.
5. Milestones represent release gates, not teams or domains. Domain is represented by the parent epic or an optional Project field.
6. Create leaf issues only when the relevant sub-epic is ready for implementation. Do not pre-create hundreds of route-, DTO-, or test-case issues from the v1 inventory.
7. A feature is complete only when its required evidence is complete. Tests, migration, observability, documentation, and rollback are part of the same implementation issue unless they are independently deliverable outcomes.

GitHub supports native hierarchical sub-issues, including progress in Projects, with up to 100 direct sub-issues per parent and eight nesting levels ([GitHub: adding sub-issues](https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/adding-sub-issues)). Native issue dependencies expose blocked work in repository and Project views ([GitHub: creating issue dependencies](https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/creating-issue-dependencies)). Kontainer needs only three levels:

```text
Kontainer v2 parent epic
├── decision and ratification issues (not implementation)
└── domain sub-epics
    └── review-ready implementation issues
        └── optional task sub-issues only when one issue has independently owned deliverables
```

Do not make PRs sub-issues. Link them through the Development relationship and closing keywords.

## Ready-to-use parent epic body

Create one issue titled `Kontainer v2: Rust controller and node-agent rewrite`. Use the body below, then attach decision issues and domain epics as native sub-issues.

```markdown
## Outcome

Replace the Go/Goravel agent with a production-qualified Rust controller and Rust node agent while preserving the accepted v1 user workflows, repairing known correctness and security defects, and keeping the Tauri + React desktop as the shipping client.

The release ships two explicit binaries and two minimal OCI images from one coordinated Rust workspace:

- `kontainer-controller` serves `/api/v2`, owns manager-scoped Swarm operations and durable coordination, and runs only on managers.
- `kontainer-node-agent` runs globally on every supported Swarm node, including managers, and owns node-local Docker and host operations behind mandatory mTLS.

## Product gates

- **Foundation:** blocking architecture/product decisions and required dependency spikes are accepted with evidence.
- **Parity preview:** accepted v1-visible workflows pass on the explicitly non-HA, one-controller embedded profile with mandatory safety and honesty repairs.
- **Beta:** production-shaped three-manager/shared-control-store operation, security, native amd64/arm64 artifacts, migration/restore, chaos, and medium soak evidence pass.
- **v2.0 GA:** every advertised topology, storage backend, Engine version, architecture, upgrade path, security control, SLO, canary, and disaster-recovery gate passes on the exact immutable artifacts.

No milestone name changes these definitions. See the [decision register](https://github.com/prvious/kontainer/blob/main/docs/v2/decision-register.md) and [reliability gates](https://github.com/prvious/kontainer/blob/main/docs/v2/reliability-testing.md#release-gates).

## In scope

- Rust workspace and strict controller/node-agent boundaries
- public REST/OpenAPI, WSS/SSE, private protobuf/Tonic, and reusable native client
- secure bootstrap, human and service identity, authorization, credential custody, mTLS, and audit
- accepted Swarm resource reads/mutations, logs, exec, metrics, and Tauri desktop integration
- durable operations, idempotency, embedded and HA storage profiles, migration, backup, and recovery
- lifecycle, packaging, deployment, upgrades, observability, support tooling, performance, reliability, and signed release artifacts
- explicit v1 migration, parity exceptions, cutover, and retirement

## Out of scope unless a linked decision changes the release

- Compose stack deployment in the first GA capability set
- GPUI product rewrite, Kubernetes/Compose backends, Windows/ARMv7 agents, native controller service, remote/NAT agents, cross-Swarm federation, and self-update
- durable workload-log storage/search, detachable/resumable exec, arbitrary Docker proxying, and generic node shell
- additional metrics backends, GitOps/build pipelines, and advanced security profiles listed as deferred in the decision register

Unsupported capabilities are absent and documented; they are not empty endpoints or disabled UI promises.

## Tracking contract

- Native sub-issues hold decision work and domain sub-epics. Domain sub-epics own review-ready implementation issues.
- Native `Blocked by` relationships enforce decision and cross-domain dependencies.
- Every implementation issue lists its `ADR-CAND-*`, `V1-*`, report anchor, target gate, accepted design, and required evidence.
- Open/provisional decisions are resolved in decision issues and reflected back into `docs/v2/decision-register.md` without renumbering.
- The parent epic closes only after the v2.0 GA milestone passes and the v1 retirement outcome is complete. Conditional stack deployment is tracked separately unless ADR-CAND-039 includes it in v2.0 GA.

## Authoritative reports

- [Decision register](https://github.com/prvious/kontainer/blob/main/docs/v2/decision-register.md)
- [V1 parity inventory](https://github.com/prvious/kontainer/blob/main/docs/v2/v1-parity.md)
- [Reliability and release gates](https://github.com/prvious/kontainer/blob/main/docs/v2/reliability-testing.md)
- [Rust architecture](https://github.com/prvious/kontainer/blob/main/docs/v2/rust-architecture.md)
- [API and protocol](https://github.com/prvious/kontainer/blob/main/docs/v2/api-protocol.md)
- [Security](https://github.com/prvious/kontainer/blob/main/docs/v2/security.md)
- [Cluster communication](https://github.com/prvious/kontainer/blob/main/docs/v2/cluster-communication.md)
- [Swarm resources](https://github.com/prvious/kontainer/blob/main/docs/v2/swarm-resources.md)
- [Metrics storage](https://github.com/prvious/kontainer/blob/main/docs/v2/metrics-storage.md)
- [Logging](https://github.com/prvious/kontainer/blob/main/docs/v2/logging.md)
- [Exec](https://github.com/prvious/kontainer/blob/main/docs/v2/exec.md)
- [Lifecycle and release](https://github.com/prvious/kontainer/blob/main/docs/v2/agent-lifecycle.md)
- [Observability](https://github.com/prvious/kontainer/blob/main/docs/v2/observability.md)
- [Performance](https://github.com/prvious/kontainer/blob/main/docs/v2/performance.md)
- [Stack deployment](https://github.com/prvious/kontainer/blob/main/docs/v2/stack-deployment.md)

## Completion rule

The exact immutable release subjects—not a local rebuild—must pass the GA gate. Every supported Engine, architecture, topology, and backend must have reproducible evidence; N/N-1 rollout and restore must be proven; queues, fan-outs, streams, writes, migrations, and failures must be bounded and observable; and an operator other than the implementer must be able to diagnose, roll back, and recover the system from the published runbooks and support bundle.
```

## Domain sub-epics

Create these as direct sub-issues of the parent epic. `Depends on` names other sub-epics; decision dependencies use stable decision-register IDs so issue numbers can be substituted later. A sub-epic may begin design-neutral test or interface work before every downstream policy is fixed, but it cannot close while its exit criteria depend on an open decision.

### E01 — Rust foundations and architectural boundaries

**Suggested title:** `Kontainer v2: establish the Rust workspace and core boundaries`

**Scope:** Approve the workspace location and Go coexistence; set edition/MSRV/toolchain/license/dependency policy; establish crate direction, distinct domain/wire/persistence types, action-per-file operations, stable errors, ports, feature profiles, unsafe policy, build budgets, and dependency-boundary checks. Run the ten evidence spikes from the Rust report without treating spike code as production code.

**Depends on:** ADR-CAND-001, ADR-CAND-002, ADR-CAND-003 through ADR-CAND-007, ADR-CAND-035, ADR-CAND-047, ADR-CAND-066, ADR-CAND-078, and ADR-CAND-079.

**Exit criteria:** blocking ADRs are ratified; every required spike has reproducible evidence; the dependency graph is acyclic and role-separated; locked builds work offline for supported targets; lint/license/advisory/unsafe/build-budget policies run in CI; core crates import no transport, storage, Docker, Tauri, or generated protocol types.

**Reports:** [Rust architecture](./rust-architecture.md), [decision register](./decision-register.md), [reliability testing](./reliability-testing.md).

### E02 — Runtime lifecycle, configuration, and health

**Suggested title:** `Kontainer v2: implement bounded runtime lifecycle and strict configuration`

**Scope:** Root/child cancellation, task supervision, owner loops, blocking admission, startup/degraded/drain/shutdown state machines, strict role-specific TOML, secret-file references, reload classes, build/version reporting, `/livez`, `/startupz`, `/readyz`, authenticated deep status, and compatibility health aliases.

**Depends on:** E01; ADR-CAND-010, ADR-CAND-011, ADR-CAND-014, ADR-CAND-043, ADR-CAND-044, and the storage/config portions of ADR-CAND-078.

**Exit criteria:** zero detached production tasks; all queues and blocking work are admitted and bounded; invalid static configuration fails before exposure; dependency failures change only the documented readiness/capabilities; SIGTERM drains in order within budget and records honest terminal states; lifecycle and probe failure matrices pass.

**Reports:** [Agent lifecycle](./agent-lifecycle.md), [Rust architecture](./rust-architecture.md), [observability](./observability.md), [performance](./performance.md).

### E03 — Public API, protocols, operations contract, and reusable client

**Suggested title:** `Kontainer v2: establish versioned public protocols and native client`

**Scope:** `/api/v2` REST, Problem Details, pagination, stable ordering, partial/stale state, ETags, idempotency, operation resources, SSE events, binary WSS envelopes, checked OpenAPI 3.1, protobuf generation, capability discovery, explicit conversions, and framework-neutral `kontainer-client` behavior for REST/SSE/WSS.

**Depends on:** E01 and E02; ADR-CAND-012 through ADR-CAND-016, ADR-CAND-021, ADR-CAND-022, ADR-CAND-033, ADR-CAND-034, ADR-CAND-043, and ADR-CAND-066 through ADR-CAND-070 plus ADR-CAND-080 where applicable.

**Exit criteria:** normalized OpenAPI/protobuf/stream goldens and deterministic generators are checked in; generated TypeScript and independent Rust conformance pass; collections and mutations have bounded, retry-safe contracts; N/N-1 compatibility rules are automated; native clients keep credentials out of UI code and preserve cancellation/backpressure.

**Reports:** [API protocol](./api-protocol.md), [Rust architecture](./rust-architecture.md), [reliability testing](./reliability-testing.md).

### E04 — Human identity, authorization, secrets, and desktop credential custody

**Suggested title:** `Kontainer v2: implement secure bootstrap, identity, and authorization`

**Scope:** Locked bootstrap and recovery; GitHub/OIDC provider boundary; possession-bound native login and redemption; asymmetric access JWTs; rotating opaque refresh/service tokens; deny-by-default capability/scoped RBAC; WebSocket authentication; public TLS/proxy trust; typed secret records, envelope encryption, KMS/KEK ports, and OS-vault migration with no bearer in React or JSON stores.

**Depends on:** E01 and E03; persistence from E07 for durable sessions/audit; desktop integration from E11 for final vault acceptance; ADR-CAND-017 through ADR-CAND-021, ADR-CAND-041, ADR-CAND-044, ADR-CAND-047, ADR-CAND-068, ADR-CAND-071, ADR-CAND-072, and the public parts of ADR-CAND-077.

**Exit criteria:** every route/message/action declares capability, scope, risk, and audit class; bootstrap, refresh reuse, revocation, IDOR, provider, rate-limit, TLS, WebSocket, KMS, vault-migration, and canary-secret suites pass; high-risk actions fail closed when required authorization/audit intent cannot persist; no production v1 credential mechanism remains.

**Reports:** [Security](./security.md), [API protocol](./api-protocol.md), [reliability testing](./reliability-testing.md), [v1 parity](./v1-parity.md).

### E05 — Internal identity, cluster membership, Tonic, and coordination

**Suggested title:** `Kontainer v2: implement secure controller–agent communication`

**Scope:** Versioned Tonic API over mTLS; controller/node workload identity; enrollment, renewal, revocation, root rotation; Swarm-authoritative discovery and membership; generation-aware channel ownership; capabilities; bounded fan-out/fan-in; target-bound delegation grants; schedule leases/claims; and multi-controller failure semantics.

**Depends on:** E01, E02, E03, and the identity policy in E04; ADR-CAND-004, ADR-CAND-005, ADR-CAND-033, ADR-CAND-045, ADR-CAND-046, ADR-CAND-049, ADR-CAND-072, and ADR-CAND-077.

**Exit criteria:** controller and node-agent identities cannot collide on managers; no plaintext/shared-secret runtime mode ships; stale addresses and removed identities cannot retain authority; every fan-out target is accounted for; deadlines/retries/flow control are method-specific and bounded; adjacent-version and partition/failover tests pass.

**Reports:** [Cluster communication](./cluster-communication.md), [security](./security.md), [agent lifecycle](./agent-lifecycle.md), [performance](./performance.md).

### E06 — Docker Engine adapter and Swarm resource parity

**Suggested title:** `Kontainer v2: implement the typed Engine adapter and Swarm resources`

**Scope:** Bollard qualification behind `EngineClient`; API negotiation/capabilities; manager-versus-node authority; services, tasks, nodes, stacks, secrets, configs, networks, volumes, images, stats, safe mutations, optimistic concurrency, pagination, locality, fan-out completeness, registry handling, and stable error mapping. No raw Docker proxy.

**Depends on:** E01, E03, and E05; E07 before exposing durable mutations; ADR-CAND-035 through ADR-CAND-037, ADR-CAND-048, ADR-CAND-067, ADR-CAND-068, and ADR-CAND-077.

**Exit criteria:** every accepted `V1-DKR-*` workflow and every `V1-GAP-*` contract repair has a traced scenario; fake Engine byte-level tests and the supported real Engine/Swarm matrix pass; unrelated service fields survive patches; unsafe retries are absent; local resources name their node; destructive work has preview/confirmation/operation evidence.

**Reports:** [Swarm resources](./swarm-resources.md), [v1 parity](./v1-parity.md), [performance](./performance.md), [reliability testing](./reliability-testing.md).

### E07 — Control persistence, durable operations, idempotency, and audit truth

**Suggested title:** `Kontainer v2: implement authoritative control storage and operation recovery`

**Scope:** Embedded `control.sqlite3`, shared transactional HA control store, migrations, leases/fencing, atomic operation/idempotency/audit-intent admission, ordered events, per-target outcomes, uncertainty reconciliation, audit chain/checkpoints, retention, backup/restore, and v1 control-data migration.

**Depends on:** E01 and E03; policy inputs from E04; ADR-CAND-029, ADR-CAND-030, ADR-CAND-034, ADR-CAND-040, ADR-CAND-041, and ADR-CAND-078. HA exit also depends on the topology/SLO portions of ADR-CAND-032 and ADR-CAND-051.

**Exit criteria:** one transaction establishes operation, request hash, idempotency, initial event, and audit intent before side effects; all targets reach terminal or uncertain state; crash points are resumable/fenced; migrations use explicit expand/backfill/verify/contract; online backup and isolated restore pass for every advertised profile; diagnostics cannot replace or corrupt product truth.

**Reports:** [Decision register](./decision-register.md), [API protocol](./api-protocol.md), [security](./security.md), [agent lifecycle](./agent-lifecycle.md), [observability](./observability.md).

### E08 — Metrics collection, spool, storage, and queries

**Suggested title:** `Kontainer v2: implement bounded workload metrics`

**Scope:** Correct node/container math; local one-shot collection; independent versioned push batches; per-agent durable spool; `metrics.sqlite3`; one bounded writer; snapshots, series, gaps, freshness, rollups, retention, disk-pressure behavior, authorization, and backend conformance seams. Process telemetry remains separate.

**Depends on:** E03, E05, E06, and E07; ADR-CAND-025 through ADR-CAND-031 and ADR-CAND-075.

**Exit criteria:** Phase 0 corrected one-minute/24-hour parity passes first; the 30-second embedded default ships only after the benchmark and 72-hour soak; duplicate/late/out-of-order/replayed batches preserve explicit sequence/gap semantics; storage pressure cannot consume control reserves; HA metrics claims use the selected shared backend and pass equivalent API conformance.

**Reports:** [Metrics storage](./metrics-storage.md), [performance](./performance.md), [cluster communication](./cluster-communication.md), [v1 parity](./v1-parity.md).

### E09 — Service and task log streaming

**Suggested title:** `Kontainer v2: implement bounded authenticated log streaming`

**Scope:** Manager-native service/task logs; safe Docker mux/raw/system-error decoding; normalized records; bounded owner loops and writer queues; authenticated binary WSS; cancellation, driver capability, reconnect overlap/dedup hints, explicit gaps, and fair multi-source fan-in only when its policy is approved.

**Depends on:** E02, E03, and E06; authorization from E04; ADR-CAND-022, ADR-CAND-024, ADR-CAND-035, ADR-CAND-043, and ADR-CAND-073.

**Exit criteria:** TTY/mux/type-3/oversize behavior cannot panic, allocate proportionally, or misclassify; reusable JWTs never enter URLs; every disconnect/shutdown joins Docker work; slow consumers are bounded and visible; supported Engine/log-driver and three-node high-volume soak matrices pass. Exact durable search/resume remains absent unless separately approved.

**Reports:** [Logging](./logging.md), [API protocol](./api-protocol.md), [performance](./performance.md), [reliability testing](./reliability-testing.md).

### E10 — Container exec sessions

**Suggested title:** `Kontainer v2: implement task-bound container exec`

**Scope:** Controller authorization and fresh task placement; all execution through the target node agent including manager nodes; target-bound delegation; byte-exact terminal protocol; Docker create/attach/start/resize/inspect; deterministic shell/user policy; single-owner cleanup; quotas; authoritative metadata and exit outcomes.

**Depends on:** E02 through E06; E07 for durable audit/operation intent where required; ADR-CAND-021 through ADR-CAND-023, ADR-CAND-035, ADR-CAND-043, ADR-CAND-044, and ADR-CAND-074.

**Exit criteria:** local and remote paths use the same typed request and tests; stale placement cannot execute the wrong container; remote user is preserved; exit zero is never fabricated; slow/malicious peers cannot create unbounded input, output, sessions, or tasks; every terminal path emits one redacted audit outcome and releases resources. Reopen starts a visibly new process.

**Reports:** [Exec](./exec.md), [cluster communication](./cluster-communication.md), [security](./security.md), [v1 parity](./v1-parity.md).

### E11 — Tauri desktop cutover and secure profile migration

**Suggested title:** `Kontainer v2: migrate the Tauri desktop to the native v2 client`

**Scope:** Wrap all agent access in `kontainer-client` and Tauri commands/TanStack Query; register commands and synchronize Rust/TypeScript IPC types; isolate query keys by profile; move WSS/networking out of the WebView; migrate profile/session metadata and credentials safely; port accepted visible workflows and partial/stale/error/operation UX.

**Depends on:** E03 and E04; consumes E06, E08, E09, and E10 incrementally; ADR-CAND-008, ADR-CAND-009, ADR-CAND-018, ADR-CAND-037, and ADR-CAND-069.

**Exit criteria:** all `V1-IPC-*`, `V1-DTO-*`, `V1-SCR-*`, `V1-PROFILE-*`, and applicable `V1-UI-*` outcomes are accepted, repaired, deferred, or rejected explicitly; real Rust IPC/client contract and packaged desktop smoke tests complement Vitest; no bearer reaches React, URLs, JSON stores, logs, or screenshots; profile switching cannot leak cached cluster data.

**Reports:** [V1 parity](./v1-parity.md), [API protocol](./api-protocol.md), [security](./security.md), [reliability testing](./reliability-testing.md).

### E12 — Packaging, installation, upgrade, backup, and disaster recovery

**Suggested title:** `Kontainer v2: deliver hardened lifecycle and operator workflows`

**Scope:** Two minimal binaries/images, strict config and filesystem contract, hardened Swarm services, controller/node placement, control overlay, installation/preflight, node lifecycle, schema migration, backup/restore, controller-first rolling upgrade, rollback, certificate rotation, offline bundle, and DR runbooks.

**Depends on:** E02, E05, E07, E14, and E15; ADR-CAND-003, ADR-CAND-004, ADR-CAND-030, ADR-CAND-032, ADR-CAND-033, ADR-CAND-046, ADR-CAND-047, ADR-CAND-051, ADR-CAND-069, ADR-CAND-077, ADR-CAND-078, and ADR-CAND-079.

**Exit criteria:** service specs are digest-pinned, least-privilege, private, bounded, and readiness-aware; install/add/remove/restart/upgrade/rollback/rotation/offline flows pass on supported architectures; N/N-1 and schema windows are proven; backup is verified before risky change; timed fresh-environment DR meets approved RPO/RTO; restore is never routine rollback.

**Reports:** [Agent lifecycle](./agent-lifecycle.md), [cluster communication](./cluster-communication.md), [security](./security.md), [reliability testing](./reliability-testing.md).

### E13 — Observability, SLOs, support bundles, and incident operations

**Suggested title:** `Kontainer v2: make operations diagnosable without exposing workload data`

**Scope:** Typed `tracing`; safe diagnostic events; request/trace/operation/audit correlation; private process metrics; bounded exporters; lifecycle health registry; panic/crash policy; dashboards, alerts, runbooks, local support bundle, symbols, and game-day evidence. Diagnostic, trace, metric, operation, and audit planes remain separate.

**Depends on:** E01, E02, E03, E05, and E07; every functional epic supplies domain telemetry; ADR-CAND-034, ADR-CAND-040 through ADR-CAND-042, ADR-CAND-049, and ADR-CAND-051.

**Exit criteria:** the versioned field/metric/bundle catalog and secret-canary tests pass; exporters can fail without blocking control or audit reserves; alerts fire and clear under injected failures; a blind operator can diagnose and recover representative incidents; bundles remain bounded, allowlisted, non-uploading, and contain zero prohibited content.

**Reports:** [Observability](./observability.md), [security](./security.md), [performance](./performance.md), [reliability testing](./reliability-testing.md).

### E14 — Test infrastructure, performance, chaos, and release qualification

**Suggested title:** `Kontainer v2: build the verification and qualification system`

**Scope:** Unit/property/model tests, goldens, fake Engine, fuzzing/sanitizers, pinned DinD, real VM Swarms, native architecture runners, database and provider fakes, coverage/flake policy, load/soak, fault injection, Engine matrix, upgrade/restore, canary, SLO, incident, and DR evidence retention.

**Depends on:** E01 for harness/tooling policy and E03/E06 for initial contracts/fakes; then runs across every domain. Decision blockers include ADR-CAND-048 through ADR-CAND-051 and ADR-CAND-080.

**Exit criteria:** PR/main/nightly/weekly/RC/canary lanes match the reliability report; required jobs fail on zero/ignored/flaky tests; every supported topology/Engine/architecture/storage cell has exact artifact evidence; bounded-resource and manager-impact budgets pass; no retry masks a required failure; parity, beta, and GA gate reports are reproducible.

**Reports:** [Reliability testing](./reliability-testing.md), [performance](./performance.md), [agent lifecycle](./agent-lifecycle.md).

### E15 — Release engineering and software supply chain

**Suggested title:** `Kontainer v2: produce verifiable build-once release artifacts`

**Scope:** Protected semantic tags; pinned actions/toolchains/bases; isolated least-permission build; native amd64/arm64 subjects; checksums, manifests, notices, SBOMs, provenance, attestations, signatures, vulnerability/license policy, GHCR publication, offline verification, immutable releases, canary promotion, and revocation.

**Depends on:** E01, E12, and E14; ADR-CAND-047, ADR-CAND-050, ADR-CAND-051, and ADR-CAND-079.

**Exit criteria:** each release subject is built once, tested by digest, independently verified online/offline, canaried, and promoted without rebuild; mutable tags are never the deployment or rollback record; published SLSA claims match audited evidence; compromised-artifact response is exercised.

**Reports:** [Agent lifecycle](./agent-lifecycle.md), [reliability testing](./reliability-testing.md), [security](./security.md), [Rust architecture](./rust-architecture.md).

### E16 — V1 parity ledger, migration, cutover, and retirement

**Suggested title:** `Kontainer v2: close the v1 parity and retirement ledger`

**Scope:** Own the normalized mapping of every `V1-*` family; classify accepted/fixed/deferred/rejected behavior; maintain differential fixtures and the intentional-difference ledger; migrate/archive/discard every v1 data class; govern any opt-in legacy adapter; remove false UI/docs promises; cut over and retire Go only after evidence.

**Depends on:** ADR-CAND-037 and ADR-CAND-069. It consumes the parity outcomes of E03 through E13 and the evidence from E14; it must not redefine domain design.

**Exit criteria:** every v1 inventory/checklist row has a disposition and evidence link; all `V1-GAP-*` defects have corrected regression tests; accepted workflows pass through the packaged desktop and Rust agents; every legacy data class has a tested migrate/archive/discard outcome; the support window and Go-oracle removal criteria are published and satisfied.

**Reports:** [V1 parity](./v1-parity.md), [API protocol](./api-protocol.md), [reliability testing](./reliability-testing.md), [decision register](./decision-register.md).

### E17 — Conditional Compose stack deployment feature

**Suggested title:** `Kontainer v2: deliver the versioned stack-deployment feature MVP`

**Scope:** This is a separate, conditional feature epic. Implement the accepted Rust reconciler, strict versioned Compose subset, secure source bundle, deterministic plan/diff, strong ownership, journaled operations, safe reconciliation, rollout observation, cancellation, recovery, retained volumes/external resources, and desktop deploy workflow.

**Depends on:** ADR-CAND-038, ADR-CAND-039, and ADR-CAND-076; E03, E04, E06, E07, E13, and E14. Do not place its children in a core v2 milestone until ADR-CAND-039 says the feature belongs there.

**Exit criteria:** the chosen parser/dialect and dangerous-service policy are approved; feature-MVP compatibility/security/chaos gates pass; retries produce one operation; controller failure resumes safely; partial effects remain inspectable/repairable; removal deletes only owned disposable resources and never volumes/external resources; no runtime Docker CLI dependency exists.

**Reports:** [Stack deployment](./stack-deployment.md), [Swarm resources](./swarm-resources.md), [decision register](./decision-register.md).

## Decision work is not implementation work

Use issue type/label `decision` and a title such as `Decide v2 GA storage and topology (ADR-CAND-001, -031, -032)`. A decision issue has options, evidence, an owner, a deadline, migration/compatibility impact, selected outcome, rejected alternatives, and exact downstream issues it blocks. It closes when the selected outcome is recorded in the decision register or an approved ADR—not when code lands.

Do not put implementation checklists in decision issues. Do not bury an undecided policy inside an implementation PR.

### Wave 0: required before production Rust implementation

Open these first, grouping only decisions that share one owner and one approval:

| Decision issue | IDs | Unblocks |
|---|---|---|
| Milestone vocabulary, first GA claim, and advertised profiles | ADR-CAND-001 | all milestones and GA scope |
| Rust workspace location and Go coexistence | ADR-CAND-002 | E01 |
| Rust toolchain, license, dependency, ABI, and supply-chain policy | ADR-CAND-079 | E01, E15 |
| Ratify binary roles, authority, crate direction, and core architecture baseline | ADR-CAND-003 through ADR-CAND-007 | E01, E05, E06 |
| Public/internal protocol boundaries, auth redemption, stream ticket model, and compatibility unit | ADR-CAND-012, ADR-CAND-013, ADR-CAND-016, ADR-CAND-021, ADR-CAND-033, ADR-CAND-066 | E03, E05 |
| Native credential vault and repository-rule amendment | ADR-CAND-018 | E04, E11 |
| Capability names and minimum role/scope matrix | ADR-CAND-019 | E03, E04, all handlers |
| Embedded/HA control storage, migration ownership, and atomic operation contract | ADR-CAND-029, ADR-CAND-030, ADR-CAND-034, ADR-CAND-078 | E07 and all mutations |
| Internal X.509 identity/SAN/delegation model and test PKI | ADR-CAND-045; production enrollment remains ADR-CAND-046 | E05 |
| Initial Docker Engine/API support floor | ADR-CAND-048 | E06, E14 |

The `accept` rows above are ratification work, not a request to reopen every alternative. The issue should either approve the converged baseline or identify a precise conflict and update the register.

### Later blocking decisions

Create these only as the owning sub-epic approaches refinement:

- Product/release: ADR-CAND-031, ADR-CAND-032, ADR-CAND-039, ADR-CAND-049, ADR-CAND-051, and ADR-CAND-069.
- Security/identity: ADR-CAND-041, ADR-CAND-044, ADR-CAND-046, ADR-CAND-047, ADR-CAND-068, ADR-CAND-071, ADR-CAND-072, and ADR-CAND-077.
- Contract/resource policy: ADR-CAND-043, ADR-CAND-067, ADR-CAND-070, and ADR-CAND-080.
- Domain production policy: ADR-CAND-026 through ADR-CAND-028, ADR-CAND-035, ADR-CAND-040, ADR-CAND-073 through ADR-CAND-076.

Rows marked `defer` or `reject` are constraints, not backlog invitations. Open an enhancement decision only when evidence or product scope justifies revisiting one.

## Milestones and release gates

Create four milestones after ADR-CAND-001 is approved. GitHub milestones track progress over related issues and PRs ([GitHub: about milestones](https://docs.github.com/en/issues/using-labels-and-milestones-to-track-work/about-milestones)). Assign milestones to implementation/decision leaf issues and their PRs, not to the master epic. Domain epics span gates and should normally have no milestone.

### `v2-foundation`

Exit gate:

- Wave 0 decisions are approved.
- Required dependency/runtime/protocol/storage/security/supply-chain spikes have evidence.
- Workspace, dependency direction, test skeleton, generated-contract authority, and role boundaries pass.
- No production code depends on an unresolved foundation choice.

This is an engineering gate, not a user release.

### `v2-parity-preview`

Exit gate is Gate 1 from [reliability testing](./reliability-testing.md#gate-1-v1-visible-parity): accepted v1 desktop workflows and repaired mismatches pass through the Rust implementation on a bounded one-controller/non-HA profile; public/internal/stream contracts, fake Engine, pinned 1M/2W integration, security foundations, bounded resources, migrations, and amd64 package/image smoke pass.

The milestone description must say **preview, embedded, non-HA, not production**.

### `v2-beta`

Exit gate is Gate 2 from [reliability testing](./reliability-testing.md#gate-2-beta): three real managers/controllers, shared control store, current/previous selected Engines, native amd64/arm64, migration/restore, N/N-1 rollout, core chaos, medium load/24-hour soak, SLO/alerts/support bundle, signed prerelease artifacts, and independently exercised runbooks pass.

### `v2.0-ga`

Exit gate is Gate 3 from [reliability testing](./reliability-testing.md#gate-3-production-ga): the advertised topology/storage profile, complete Engine/architecture/OS matrix, full profile load/soak, chaos/security/penetration testing, N/N-1 and schema rollback windows, RPO/RTO and fresh-environment DR, SLO alerts, approved canary, build-once promotion, verified supply-chain claims, and published/exercised support guidance pass.

Create a separate `stack-deployment-mvp` milestone only if ADR-CAND-039 keeps it outside `v2.0-ga`. Do not create milestones for deferred platforms or backends until they are approved product work.

## Labels

Keep labels deliberately small. Preserve the existing defaults and add only:

| Label | Use |
|---|---|
| `v2` | Any issue whose delivery or decision belongs to this rewrite |
| `epic` | Parent and domain sub-epics; never leaf implementation work |
| `decision` | ADR/product/policy choice with no production implementation in scope |
| `release-blocker` | A currently failing required milestone gate; remove it when the blocker clears |

Continue to use existing `bug`, `documentation`, and `enhancement` when they describe the issue. Use `question` only for an information request, not for an owned decision. Do not add labels for domain, status, priority, size, milestone, blocked state, architecture status, or every test layer; those belong to hierarchy, native dependencies, milestones, or optional Project fields.

## Optional GitHub Project

Sub-issues, dependencies, milestones, and labels are sufficient for a small team. Add a Project only when cross-domain scheduling or release reporting becomes painful. GitHub Projects can show parent and sub-issue progress and support typed custom fields ([GitHub: understanding Project fields](https://docs.github.com/en/issues/planning-and-tracking-with-projects/understanding-fields)).

Use these fields if a Project is adopted:

| Field | Type | Values |
|---|---|---|
| Status | single select | Backlog, Ready, In progress, In review, Blocked, Done |
| Gate | single select | Foundation, Parity preview, Beta, v2.0 GA, Later |
| Domain | single select | the E01–E17 domain names |
| Priority | single select | P0 release/security blocker, P1 critical path, P2 normal, P3 opportunistic |
| Size | single select | XS, S, M, L, XL/split required |
| Decision state | single select | N/A, Open, Provisional, Accepted, Rejected, Deferred |

Avoid duplicating milestone, assignee, labels, parent, dependency, or target date in custom fields. Suggested views:

- **Decisions:** `label:decision`, grouped by Decision state, sorted by Priority.
- **Critical path:** Gate is not Later, grouped by Status, showing blocked relationships.
- **Domains:** grouped by Parent issue or Domain, with sub-issue progress.
- **Release:** filtered to one milestone/Gate, grouped by Status.
- **Ready queue:** Status = Ready and not blocked, sorted by Priority then Size.

## Standard implementation issue template

Use this for implementation, bug, refactor, dependency, protocol, schema, deployment, and operational issues. A spike or decision uses its own evidence/outcome template. `N/A` requires a reason.

```markdown
## Outcome

<!-- One observable result, stated from the user/operator/system perspective. -->

## Tracking

- Parent epic: #
- Domain sub-epic: #
- Gate/milestone: Foundation | Parity preview | Beta | v2.0 GA
- Kind: implementation | bug | refactor | dependency | protocol | schema | deployment | operations
- Traceability IDs: ADR-CAND-___; V1-___; OBS-___ (where applicable)
- Authoritative report anchors:
- Blocked by decisions/issues:
- Blocking:

## Accepted design

<!-- State the already-approved constraints this issue implements. Link the exact decision-register rows and report sections. Do not reopen them here. -->

## Scope

-

## Out of scope

-

## Behavior and failure contract

- Normal outcome:
- Boundary/empty outcome:
- Timeout and cancellation:
- Retry and idempotency:
- Partial/uncertain outcome:
- Rollback/reconciliation:
- Compatibility and unsupported behavior:

## Acceptance criteria

- [ ]

## Security and privacy

- Authentication/identity:
- Capability and resource scope:
- Input/body/frame/rate limits:
- Secret/data classification and redaction:
- Audit event/intent:
- Threat-model change or `N/A` reason:

## Performance and reliability

- Deadline and cancellation budget:
- Item/byte/concurrency/queue/disk bounds:
- Manager, Docker, database, and network impact:
- SLI/SLO/readiness/alert impact:
- Supported profile and overload behavior:

## Tests and evidence

- [ ] Unit tests cover normal, boundary, empty, and typed-error paths.
- [ ] Applicable property/state-machine/concurrency/fuzz regression has a reproducible seed/corpus.
- [ ] Applicable public/protobuf/stream/IPC/database golden or compatibility baseline is reviewed.
- [ ] Applicable fake Engine/dependency test asserts requests, responses, deadlines, cancellation, reuse, limits, and redaction.
- [ ] Applicable DinD/real Engine/3-node/5-node/native-architecture scenario is added or referenced.
- [ ] Applicable migration, backup/restore, N/N-1, chaos, security, load, and soak evidence is attached.
- [ ] Required jobs run non-zero tests with no unexplained ignored, quarantined, flaky, or coverage result.

## Documentation, migration, and rollout

- User/operator/API/config documentation:
- Data/config/client migration or explicit `N/A`:
- Release note and known-limit impact:
- Rollout waves and abort condition:
- Exact rollback artifact/procedure and reconciliation:

## Definition of Done

- [ ] Observable behavior, invariants, non-goals, parity item, and release gate are documented.
- [ ] Supported Engine/API, topology/profile, OS/architecture, storage backend, and client versions are identified.
- [ ] Security, authorization, audit, secret classification, resource bounds, manager/DB impact, and SLO/readiness effects are reviewed.
- [ ] Structured telemetry and operator diagnostics/runbooks are implemented and fault-tested where applicable.
- [ ] Rust, TypeScript, protobuf, OpenAPI, configuration, migrations, and documentation remain synchronized.
- [ ] Required CI lanes pass on the exact commit/artifact; failure artifacts are reviewed.
- [ ] Evidence records commit/artifact digest, versions, fixture manifest, seed/fault timeline, raw result, and reviewer where applicable.
- [ ] User/operator release notes, migration/known-limit/support guidance, and linked follow-up issues are complete.
```

For high-risk or release-gating work, copy any additional applicable checks from [the full reliability Definition of Done](./reliability-testing.md#definition-of-done--verification). Do not weaken that source by treating this shorter template as exhaustive.

## Issue size and decomposition rules

An implementation issue should describe one reviewable outcome and normally fit one focused PR in one to three working days. Five working days is the upper bound for a planned leaf. Mark anything larger `XL/split required` and split it before moving to Ready.

Split an issue when any of these is true:

- it changes more than one independently releasable public, internal, persistence, or desktop contract;
- it mixes a decision/spike with production implementation;
- it spans more than one release gate;
- it includes more than one independently reversible migration or external dependency decision;
- it has independently ownable deliverables with different blockers;
- reviewers cannot validate the normal, failure, security, performance, and migration effects as one coherent change.

Do not split only by technical layer. Prefer a thin vertical outcome such as “list services through the typed Engine adapter and `/api/v2` with contract tests” over separate model, adapter, handler, and test issues. Shared infrastructure with several downstream consumers is a legitimate focused issue when its contract and exit evidence are cohesive.

Epics are never closed by a code PR. A sub-epic closes when all required outcomes and gate evidence in its exit criteria are complete, not merely when its initially imagined children are closed.

## PR linkage and closure rules

- Keep existing PR title style: `feat:`, `fix:`, `refactor:`, `docs:`, or another agreed Conventional Commit type, followed by a concise outcome.
- Every implementation PR links its leaf issue in the Development sidebar or body.
- Use `Closes #NNN` only when that PR completes the leaf's full Definition of Done. GitHub closes a linked issue when a PR targeting the default branch merges ([GitHub: linking a pull request to an issue](https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/linking-a-pull-request-to-an-issue)).
- For a multi-PR leaf, use `Refs #NNN` on intermediate PRs and `Closes #NNN` only on the final PR after integrated evidence is attached.
- A PR should close one leaf issue by default. It may reference its domain epic, decision IDs, and other dependencies, but must not close the parent or a domain epic.
- Generated contract changes include their source and generated output in the same PR unless the approved generator workflow requires a separate bot PR with an explicit dependency.
- Follow-up defects found during review become linked issues only when they are truly outside the accepted scope. Required acceptance work is not deferred merely to merge sooner.

## Suggested ordering

### Wave 0 — decisions and evidence

Approve ADR-CAND-001/002/079 and the Wave 0 decision set. Run dependency spikes 1, 3, 4, 6, and 10 first; run the Bollard, runtime, and native-client spikes as their decisions approach. This wave changes the decision register and evidence, not production behavior.

### Wave 1 — foundations and a secure walking skeleton

Start E01, E02, E03, and E14 together after their blockers close. Prove one vertical read-only path: both role-specific binaries start, validate configuration, expose truthful health, use test PKI over Tonic, negotiate one fake Engine call, map one domain DTO to `/api/v2`, and consume it through the native client. Establish operation/audit interfaces without exposing mutations yet.

### Wave 2 — identity, cluster, storage, and Engine authority

Build E04, E05, E06, and E07 around the accepted boundaries. Implement the node agent and `Describe`/health first, then controller discovery/channel ownership, then manager/node resource locality. Land atomic operation/idempotency/audit intent and migration/backup contracts before public mutations.

### Wave 3 — corrected parity domains

Port E08, E09, and E10 using the shared client, identity, Engine, operation, and lifecycle foundations. In E06 port resource families as vertical slices. E11 follows each stable contract incrementally. E16 records each `V1-*` disposition and evidence; it does not wait until the end to reconstruct parity.

### Wave 4 — parity preview

Complete embedded storage, desktop workflows, migration policy, lifecycle packaging, core observability, fake Engine and pinned 1M/2W scenarios. Close only the `v2-parity-preview` milestone; do not make production or HA claims.

### Wave 5 — beta production shape

Add the shared control store, multiple controllers, leases/claims, automated identity lifecycle, current/previous Engine lines, native arm64, signed RC artifacts, migration/restore, core chaos, SLO operations pack, and medium load/24-hour soak. Exercise operator runbooks independently.

### Wave 6 — v2.0 GA qualification and cutover

Freeze the advertised matrix, pass full load/soak/security/penetration/N/N-1/DR/canary gates on exact build-once artifacts, publish support and upgrade policy, execute the v1 cutover/retirement decision, and close the parent only after the immutable GA release is verified.

### Conditional wave — stack deployment

Refine E17 only after ADR-CAND-039 and ADR-CAND-076. If it is outside first GA, schedule it independently after the core operation, Engine, security, observability, and reliability seams are stable. Its feature-MVP gate must not silently become a parity requirement.

## Initial creation set

After review, create only:

1. the parent epic;
2. E01 through E16, plus E17 marked conditional;
3. the Wave 0 decision issues;
4. the four approved milestones and four added labels.

Then refine only E01, E02, E03, and E14 into the first implementation/spike issues. Decompose later domains when their blocking decisions and upstream contracts are close enough for review-ready acceptance criteria. This keeps GitHub useful as a delivery system rather than turning the reports into hundreds of speculative tickets.

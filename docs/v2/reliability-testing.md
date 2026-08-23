# Kontainer v2 reliability and production verification strategy

Status: proposed production gate

Research and repository audit date: 2026-08-19

Scope: the Rust controller and node-agent rewrite, its public and internal protocols, Docker Engine and Swarm behavior, persistence, scheduling, desktop boundary, binaries and images, release pipeline, operational evidence, incident response, and disaster recovery.

## Executive decision

Kontainer v2 must be released by accumulated evidence, not by a single end-to-end suite. The accepted verification stack is:

1. pure unit, property, state-machine, and concurrency-model tests for domain invariants;
2. checked-in public/protobuf/stream/database golden contracts;
3. a programmable fake Docker Engine HTTP server for exhaustive request, response, stream, timeout, and corruption behavior;
4. pinned Docker-in-Docker (DinD) Swarms for fast functional integration;
5. ephemeral Linux virtual machines with real packaged Docker Engines for compatibility, quorum, cgroup, filesystem, network, upgrade, backup, and chaos evidence;
6. native architecture runners for every released binary and image architecture;
7. profile load, soak, security, migration, rolling-upgrade, canary, SLO, support-bundle, and disaster-recovery gates.

These layers are complementary. A trait mock cannot prove an Engine request is encoded correctly. A fake HTTP daemon cannot prove Swarm scheduling. DinD cannot prove host reboot, systemd, real cgroups, kernel networking, disk failure, or multi-host quorum. A five-node happy-path test cannot enumerate malformed frames or every crash boundary.

Release gates are hierarchical:

- **v1 parity** proves every supported current user workflow on a bounded non-HA topology, while repairing known unsafe or dishonest behavior.
- **Beta** adds production-shaped security, three-manager operation, migration/restore, signed release candidates, native `amd64`/`arm64` smoke tests, core chaos, and a medium soak.
- **Production GA** adds the advertised HA/storage profile, five-node and Engine compatibility evidence, N/N-1 upgrade and downgrade, complete security and supply-chain gates, profile load/soak, SLO alert validation, a canary, and a timed disaster-recovery exercise.

No test retry converts a failure into success. A retry may diagnose a flake, but a flaky result still fails the required check and creates an owned, expiring quarantine issue. No unsupported target, backend, Engine release, or scale is implied by a successful cross-compile or best-effort test.

## Current repository evidence

This is an audit of the checked-out `main` revision, not a description of the intended v2 implementation.

### Tests and reproducibility

| Area | Present evidence | Important gap |
|---|---|---|
| Go server | 10 `_test.go` files and 43 test entry points, excluding `TestMain`; one test is explicitly skipped. | Almost every action, HTTP controller, middleware, Docker adapter, cluster-discovery path, gRPC controller/interceptor, scheduler, migration, and mutation is untested. |
| Go WebSocket | Helpers, Docker mux stdout/stderr/timestamp handling, exit-code encoding, tail parsing, and identifier syntax have focused tests. | No size/backpressure/heartbeat/auth/origin/reconnect/cancellation tests; the current empty-line assertion preserves behavior v2 explicitly changes. |
| Go metrics | Small SQLite fixtures cover latest-per-node, container attachment by timestamp, range filtering, empty state, staleness, and field mapping. | No collection math, batch replay, fan-out failure, pruning, migration, database failure, high cardinality, or backup/restore tests. |
| React | 39 Vitest files with 300 `it(...)` cases cover visible screens, hooks, mocked Tauri invocations, auth/profile flows, logs, exec, mutations, loading/empty/error states, and navigation. | All native calls are globally mocked. There is no real Rust IPC, HTTP, WebSocket, generated-schema, browser E2E, accessibility, packaging, or agent compatibility suite. |
| Tauri Rust | `cargo test --locked` builds successfully but executes zero unit, integration, or doc tests. | Native HTTP actions, DTO decoding, credential-store migration, stream transport, error mapping, and commands have no Rust test evidence. |
| Coverage/fuzzing | No configured coverage, property, mutation, fuzz, sanitizer, Miri, or concurrency-model lane. | Coverage and parser/state-machine risk are unknown. |

Audit execution exposed reproducibility failures that a new pipeline must prevent:

- `go test ./...` failed all ten metric tests because the relative SQLite path resolved to a package working directory with no `storage` directory. Tests must use per-test temporary paths and never depend on the invocation directory.
- The frontend test command did not start Vitest because the local package-manager policy rejected unapproved dependency build scripts. CI must install from the committed lockfile under an explicit build-script policy, then run tests without mutating manifests or approval state.
- `cargo test --locked` passed while running zero tests. “Green” with no discovered tests must fail a test lane that expects tests; cargo-nextest supports an explicit no-tests failure mode.

### Old tests as behavior specifications

The Go implementation will be removed, but selected assertions are valuable as input to v2 tests. Preserve the behavior, not the Go/Goravel structure.

| Existing tests | Reuse decision | v2 treatment |
|---|---|---|
| Metrics query tests in `server/tests/metrics/get_cluster_metrics_test.go` | **Reuse as behavior fixtures** | Port seeded timestamps and expected latest/history/freshness outcomes to backend conformance tests. Update them for v2 pagination, gaps, partial state, decimal-string counters, split control/metrics stores, and rollups. Do not preserve equality-joining as the public contract. |
| `AuthBridge` send/receive, timeout, unknown-session, cleanup | **Reuse the lifecycle intent** | Express as authorization-session state-machine tests: one terminal delivery, cancellation, expiry, unknown ID, disconnect, replay, and cleanup. The in-memory channel implementation is not reusable and v2 additionally requires PKCE, possession proof, persistence, and rate limits. |
| `SessionStore` generate/consume/unknown token | **Reuse the single-use invariant** | Test hashed, expiring, transactionally consumed state/nonces and concurrent double redemption. The concrete store and token format are not a compatibility contract. |
| GitHub provider name/config/registry and access-denied error | **Reuse only provider-selection and stable denial intent** | Test provider discovery and normalized denial codes through the public contract. Do not freeze Go OAuth config types, registry construction, or raw strings. |
| Setup-token generation/config tests | **Do not reuse exact assertions** | `ktsetup_`, length 20, environment fallback, and panic-on-random-failure conflict with the v2 32-byte, file/local-command, hashed, expiring bootstrap design. Retain only uniqueness, adequate entropy, one-time use, expiry, and fail-closed RNG behavior. |
| WebSocket tail clamp (`100`, `all`, max `10,000`) | **Reuse as a v1-visible boundary** | Keep default/max parity where the v2 logging contract agrees, then property-test canonical parsing and limit discovery. |
| Docker mux stdout/stderr, multi-frame, multi-line, and RFC3339 timestamp tests | **Reuse as adapter goldens** | Port the byte fixtures to the Rust Docker log decoder, add partial headers/payloads, zero/huge/invalid lengths, invalid UTF-8, cancellation, and bounded allocation. |
| Empty log lines are skipped | **Reject** | V2 logging requires preserving blank lines. Add a regression proving they survive. |
| Exec status includes exit code zero and nonzero | **Reuse as a public outcome** | Put exact exit/error/cancel outcomes in versioned binary-frame and public event goldens. |
| Closed-error string matching | **Reject** | Substring matching implementation-specific network errors is not a stable behavior. Test typed cancellation/EOF/transport classifications. |
| Container-ID/name regex | **Reuse only as adversarial examples** | Resolve authoritative task/container IDs before use; validate protocol fields with a documented grammar. Do not make the old regex an authorization boundary. |
| Orchestrator compile-time interface test | **Do not count as behavior** | Rust trait conformance is checked by compilation. Add adapter behavior tests instead. |
| Skipped `UpsertUser` test | **Not reusable evidence** | It never ran. Replace with real transactional identity/upsert, issuer+subject uniqueness, membership change, and concurrent login tests. |
| Example test whose only assertion is `true` | **Delete when Go retires; not evidence** | It specifies no behavior and must not contribute to coverage or gate counts. |

The React suite is reusable at a different boundary:

- **Port and retain as parity specifications:** authenticated navigation; profile restore/switch/logout/token-expiry outcomes; lists and detail fields; loading/empty/error/partial states; scale/restart/rollback/image/env/node/secret/config/stack actions; refresh behavior; log pause/resume/cleanup; exec session/tab lifecycle; and mutation confirmation.
- **Update rather than freeze:** old `invoke` command names and payload shapes, query-string WebSocket URLs, v1 error strings, broken server/Tauri DTO assumptions, polling cadence, and actions that currently expect a result the Go server does not return. Expected values must come from the v2 schema and structured errors.
- **Keep as frontend-only regression, not agent parity:** exact copy, CSS classes, menu placement, font/theme choices, and purely visual loading skeletons.
- **Never treat as integration evidence:** a mocked `invoke` call proves that React called a function; it does not prove the Rust command is registered, serialized correctly, protects a token, sends the right HTTP request, handles TLS, or interoperates with the server. Add generated-client/IPC contract tests and packaged-desktop smoke tests for those claims.

### Development Swarm fixture

`dev/docker-compose.yml` starts three privileged mutable `docker:dind` images: one manager and two workers. TLS certificate generation is disabled; daemon TCP port 2375 is published on host ports 2375, 2376, and 2378. Named daemon volumes persist state. `dev/setup.sh` polls `docker info`, initializes one manager, joins two workers, and seeds stacks. The seed script uses a fixed five-second sleep instead of convergence assertions. Agent processes then run on the host through three `DOCKER_HOST=tcp://localhost:*` endpoints.

This fixture is useful for development, but it is not currently a test:

- images are not pinned by release and digest;
- names and host ports are fixed, so parallel runs collide;
- it is privileged and exposes unauthenticated Docker APIs; Docker warns that an unsecured remote daemon can grant host-root access, while `--privileged` gives a container nearly host-equivalent device and security access ([Docker remote access](https://docs.docker.com/engine/daemon/remote-access/), [privileged containers](https://docs.docker.com/engine/containers/run/#privileged));
- it has no health/convergence deadline, assertion oracle, artifact capture, or automatic cleanup on failed setup;
- it covers 1-manager/2-worker availability, not manager quorum, multi-controller state, or schedule leasing;
- nested daemons share one host kernel and do not model separate systemd, disks, clocks, cgroups, routing, reboot, or architecture.

Retain it as a manual convenience until a namespaced, pinned, assertion-driven test fixture replaces it. Never expose these ports outside loopback or run untrusted code against a persistent DinD host.

### CI, image, and release state

The only product build workflow is `.github/workflows/build-agent.yml`; the other workflows are assistant automation, not product verification. The build workflow is presently nonfunctional for this tree: its triggers, context, and Dockerfile use `agent/**` and `./agent`, while the Go agent lives in `server/` and no Rust agent exists yet.

It also:

- runs no Go, Rust, frontend, schema, integration, security, migration, or smoke tests;
- grants `packages: write` at the job level even for pull-request builds;
- references third-party actions by movable major tags rather than full commit SHA; GitHub states that a full-length commit SHA is the only immutable action reference ([GitHub Actions secure-use reference](https://docs.github.com/en/actions/reference/security/secure-use));
- determines a new patch version by querying registry tags on every main push rather than publishing an immutable source tag;
- executes a build and then a second build for push, instead of promoting the exact tested digest;
- publishes mutable `latest` and SHA tags without an explicit vulnerability gate, SBOM, signature verification, release manifest, or rollback/canary record.

There are no local Git tags and no checked-in release workflow for agent binaries or the Tauri application. The Tauri manifest is version `0.1.0`; the Go module has no release metadata. The Go Dockerfile runs `go mod tidy` during build, uses mutable `golang:alpine` and `alpine:latest`, copies `.env` into the runtime image, and declares no non-root user, health check, or supply-chain outputs. These are blockers to using any current artifact as a v2 release candidate.

### Error handling, scheduling, and persistence

The current code establishes useful requirements by showing failure modes:

- Go exposes only `ErrNotFound` and `ErrGlobalMode` as common Docker errors. Some controllers distinguish not-found, while many collapse daemon, transport, timeout, validation, role, conflict, and partial failures to a logged error plus generic HTTP 500.
- Tauri agent actions repeatedly construct clients and return `Result<_, String>`, often embedding an upstream response body. Native callers cannot reliably distinguish authentication, retryability, validation, partial results, timeout, or a request ID.
- production discovery runs every ten seconds, sequentially health-checks followers, and clears the complete registry when a manager task/node list fails.
- leader schedules use background contexts, no explicit run deadline or startup jitter, and `SkipIfStillRunning`. Discovery runs at five/ten seconds, collection every minute, and pruning every five minutes. The configured metrics interval is ignored; the retention provider reads a different key from the one configuration registers.
- local metrics collection and follower fan-out are sequential. SQLite permits up to 100 open connections but has no explicit WAL, busy timeout, checkpoint, or backup policy. A broad delete prunes metrics. There are six migrations, no migration test harness, and no documented tested rollback/restore path.
- Tauri storage migrates `session.json` to `profiles.json`, but the migration and crash/partial-write/credential-redaction paths have no Rust tests.

The v2 error taxonomy, schedule lease/claim design, dual SQLite embedded profile, external HA profile, and performance limits are specified in [API protocol](api-protocol.md), [cluster communication](cluster-communication.md), [metrics storage](metrics-storage.md), and [performance](performance.md). This document defines how to prove them.

## Test architecture

### Hermetic rules

Every automated test must:

- create a unique temporary directory, database, ports, network, Swarm name, registry namespace, certificates, and idempotency scope;
- use an injected wall clock plus monotonic clock, seeded random source where deterministic behavior is required, and explicit deadlines; do not use sleeps as readiness or correctness assertions;
- pin toolchain, dependency lock, container/VM image digest, Docker package version, architecture, kernel, filesystem, and fixture schema in its result metadata;
- clean up on success and preserve a bounded diagnostic bundle on failure;
- avoid the developer's Docker socket, credentials, profiles, databases, networks, and images;
- fail if zero tests ran, an unexpected test was ignored, a golden was rewritten, a fixture leaked, or background tasks/FDs/ports remained;
- report the test seed, fake-daemon script, fault timeline, relevant request/operation IDs, and exact artifact digest.

Integration cleanup must be idempotent and scoped by a validated unique run ID. A janitor may delete only resources carrying that exact run label after a TTL; it must never use a broad name glob.

### Unit and property tests

Keep most product logic outside Axum, Tonic, SQLx, Bollard, and Tauri types. Required pure tests include:

- DTO/domain/Engine/protobuf conversions, including missing/unknown enum values and integer boundaries;
- service-spec patching that preserves every unrelated field and version index;
- stable sorting, filters, pagination cursors, ETags, request canonicalization, and idempotency hashes;
- Docker stats math for cgroup v1/v2, reset/wrap/zero/negative counters, no limit, reboot, container replacement, and floating-point rejection;
- error classification and public Problem Details/Tonic status mapping with mandatory redaction;
- RBAC capability/scope decisions generated from a reviewed decision table;
- JWT/JWK/certificate/SPIFFE identity validation, expiry/leeway, key rotation, and replay state;
- schedule occurrence calculation, stable jitter, retry budgets, backoff, leases, fencing terms, and operation recovery;
- metric batch ordering/dedup/gap/rollup/retention math;
- log/exec state machines, sequence/cursor overlap, heartbeat, cancellation, exit, and bounded-buffer decisions;
- stack canonicalization, dependency ordering, diff, compensation, and adoption rules;
- support-bundle allow-listing and secret redaction.

Use property tests for invariants over broad inputs: encode/decode round trips, order stability, cursor tamper rejection, patch preservation, idempotency equivalence, lease mutual exclusion in the model, rollup conservation, bounded backoff/jitter, parser allocation bounds, and “redacted output contains none of the injected canaries.” Persist every minimized failure as an ordinary regression fixture.

For in-process concurrency primitives, use deterministic Tokio time and a model tool such as Loom where its abstraction fits. Loom explores valid concurrent executions rather than relying on timing luck ([Loom documentation](https://docs.rs/loom/latest/loom/)). Database lease and distributed-operation claims still require real-database multi-process tests; Loom does not prove transaction isolation.

### Contract, golden, and differential tests

The following reviewed artifacts are versioned and checked in:

| Contract | Golden evidence | Compatibility check |
|---|---|---|
| Public HTTP | normalized OpenAPI, representative requests/responses, headers, Problem Details | breaking-change check against last GA and generated Rust/TypeScript client conformance |
| Internal gRPC | protobuf source and descriptor set | `buf lint` and `buf breaking` against N-1; current/previous runtime matrix |
| Logs/exec WebSocket | binary frames for handshake, data, gap, heartbeat, resize, exit, error, end | decode old minor, reject unsupported major, preserve unknown fields as specified |
| Tauri IPC | command/channel schema plus TypeScript fixtures | bidirectional Rust/TypeScript serialization and registration smoke |
| Persistence | schema fingerprint and generated fixtures for every released schema version | forward migration and supported N/N-1 binary/schema window |
| Support bundle | manifest/schema and redacted example | backward-readable bundle parser and secret-canary scan |
| Release | artifact manifest, checksums, SBOM and provenance subject digests | offline/online signature, attestation, architecture, and install verification |

Golden updates are never automatic in normal CI. A pull request changing one must carry a semantic compatibility explanation, before/after fixture diff, migration/deprecation plan, and reviewer from the owning boundary.

V1 and v2 are intentionally not wire-compatible. Differential parity tests therefore compare a normalized user/domain outcome, not raw JSON:

1. freeze representative Engine, database, and auth-provider inputs;
2. run read paths through the Go implementation and Rust implementation and normalize known v1 defects;
3. run each mutation on an independently reset clone of the fixture;
4. compare resulting Docker state, visible fields, audit/operation outcome, and error class;
5. record intentional changes in a reviewed parity exception ledger.

The old implementation is an oracle only where its behavior is explicitly accepted above. Security weaknesses, silent partial success, raw error leakage, wire mismatches, unsafe retries, and fake setup verification must not win a differential comparison.

### Programmable fake Docker Engine

Build an adapter-local HTTP test daemon from the Moby OpenAPI fixtures plus hand-authored adversarial cases. It records method, negotiated API path, headers, query/filter encoding, body, cancellation, connection reuse, and stream lifecycle. Scripts can:

- advertise any min/max API and Engine/platform/Swarm state;
- return normal, empty, unknown-field, missing-field, duplicate, and out-of-order data;
- return every relevant HTTP status and JSON `message` without making message text the primary classifier;
- delay headers/body, stall forever until cancellation, reset mid-body, close a hijacked connection, split every byte, or send malformed/chunked/oversized data;
- race an inspect version with an update conflict;
- replace a task between resolution and logs/exec;
- emit duplicate/gapped events and disconnect outside the retained window;
- provide raw Docker multiplexed log frames and stats sequences;
- assert that retries happen only for allowed reads or deduplicated operations.

Three levels remain distinct:

- trait fakes test domain actions and return typed outcomes;
- fake Engine HTTP tests prove the Bollard/HTTP adapter request and response behavior;
- real Engine tests prove daemon semantics.

Captured real-daemon fixtures must be minimized, generated from non-sensitive resources, normalized for nondeterministic IDs/times, and reviewed before commit. Never record authorization headers, registry credentials, secret/config bytes, environment values, host paths, or daemon labels by default.

### Fuzzing, sanitizers, and undefined behavior

Use `cargo-fuzz` with a pinned nightly for in-process LLVM libFuzzer targets. Cargo-fuzz is the Rust wrapper for libFuzzer and supports corpus coverage/minimization; it currently requires Unix-like x86-64 or AArch64 plus nightly sanitizer support ([cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz)). LLVM describes libFuzzer as coverage-guided, corpus-based, and able to emit minimized crash/timeout artifacts; seed it with varied valid and invalid structures and replay the saved corpus as normal CI tests ([LLVM libFuzzer](https://llvm.org/docs/LibFuzzer.html)).

Required fuzz targets:

- Engine JSON and error decoding across negotiated versions;
- Docker mux logs, event JSON lines, stats bodies, and hijacked exec frames;
- public JSON, content encoding, nesting/size limits, identifiers, URLs, cursors, ETags, and idempotency canonicalization;
- protobuf decode and conversion for every exposed message, including unknown enum numbers and compressed limits;
- log and exec WebSocket handshake/frame state machines, fragmentation, sequence, heartbeat, and backpressure decisions;
- Compose/stack YAML parsing, interpolation, normalization, and dependency graph;
- JWT/JWK, OAuth state, certificate/SAN/URI identity, and delegation-grant parsing;
- metric batch/chunk/rollup/gap processing;
- support-bundle redaction and manifest parsing.

Cadence:

- every PR replays all committed corpora and every historical crash with zero mutation time;
- nightly runs a bounded smoke budget for each target and uploads crash/timeout artifacts;
- weekly/continuous infrastructure runs long campaigns with ASan/UBSan where supported, merges/minimizes corpora, and opens an issue automatically;
- every RC completes an agreed aggregate CPU-hour budget with no untriaged crash, OOM, timeout, sanitizer finding, or unbounded corpus regression.

Run Miri weekly over pure/unsafe crates and focused tests; Miri detects many categories of Rust undefined behavior but is not a complete correctness proof ([Miri](https://github.com/rust-lang/miri)). Run AddressSanitizer/LeakSanitizer and ThreadSanitizer lanes on supported native Linux targets; Rust sanitizer flags remain unstable and must use a pinned nightly, separate from the stable release compiler ([Rust sanitizer documentation](https://doc.rust-lang.org/beta/unstable-book/compiler-flags/sanitizer.html)). Any `unsafe` block requires a documented invariant, direct test, Miri/sanitizer applicability decision, and owner review.

### Test quality and coverage policy

Use cargo-nextest for Rust unit/integration execution, process isolation, per-test timeouts, JUnit, partitions, and slow-test reporting; keep `cargo test --doc` separate because nextest does not replace doctests. Nextest supports partitioning, stress runs, no-tests failure, and retries ([nextest running tests](https://nexte.st/docs/running/), [partitioning](https://nexte.st/docs/ci-features/partitioning/)). Configure required profiles with zero success-masking retries and `flaky-result = fail`.

Collect Rust line/function/region coverage with LLVM source instrumentation. Rust documents that processing coverage can require the LLVM version matching rustc ([rustc instrument coverage](https://doc.rust-lang.org/rustc/instrument-coverage.html)). Coverage is a risk signal, not a substitute for assertions.

Initial merge thresholds, to be ratified by issue before implementation:

- at least 90% line and 85% function coverage in authn/authz, protocol conversion, scheduling/idempotency, migration, redaction, and Docker mutation core crates;
- at least 80% line coverage for the Rust workspace and 90% changed-line coverage;
- no reachable untested `unsafe`, migration, public error code, RBAC capability, or protocol frame variant;
- frontend workspace at least 80% line coverage while retaining explicit visible-workflow cases;
- exclusions limited to generated code and mechanically declarative bindings; generated code is covered through contract conformance, not counted toward the denominator;
- coverage may not fall in a risk-owning crate without an approved issue and replacement evidence.

Track mutation-test score periodically on pure critical crates to detect assertions that execute code without checking it. Do not make whole-workspace mutation testing a PR latency blocker until runtime and false-positive rate are measured.

## Test infrastructure decisions

| Route | Decision | Proper use | Why it is not sufficient alone |
|---|---|---|---|
| Rust trait mocks | **Accepted** | Fast domain/action tests and injected typed failures | Do not exercise Engine HTTP encoding, streams, or daemon semantics. |
| Programmable fake Engine HTTP | **Accepted as primary component harness** | Exhaustive protocol/error/timeout/corruption/cancellation cases and deterministic property/fuzz tests | A fake can reproduce only modeled behavior and cannot prove Swarm/kernel behavior. |
| Testcontainers for Rust | **Accepted as lifecycle helper** | Provision temporary fake daemons, databases, proxies, registries, OIDC endpoints, and small compose fixtures with random ports and cleanup. Testcontainers requires a Docker-compatible API and notes remote/alternative setups are not all tested ([requirements](https://rust.testcontainers.org/system_requirements/docker/)); its compose support is async ([compose support](https://rust.testcontainers.org/features/docker_compose/)). | It is orchestration, not an assertion model, and using it does not turn nested containers into real hosts. |
| Pinned DinD | **Accepted with constraints** | PR 1M/2W functional parity, API negotiation, services/resources, simple logs/exec, and packaging smoke | Privileged, shared kernel, nested storage/networking, weak failure realism, and not safe on persistent shared runners. Never use it as GA chaos/performance evidence. |
| Docker socket mounted into tests | **Rejected for destructive/integration suites** | At most an explicitly read-only developer smoke outside required CI | A test bug can mutate the runner host and unrelated workloads; it destroys hermeticity. |
| Nested ephemeral Linux VMs | **Accepted as authoritative Swarm integration** | Real packaged Engines, separate kernels/disks/clocks/systemd, 3/5-node quorum, network faults, daemon reboot, upgrade, backup and native cgroups | Slower and costlier; schedule outside the fastest PR lane and maintain reproducible images. |
| Ephemeral remote bare-metal/cloud runners | **Accepted for scale and native Arm** | Native `arm64`, high I/O, large profile, long soak, real multi-host networking | Requires strong teardown, spend limits, image provenance, no long-lived credentials, and zero reuse after untrusted code. |
| Long-lived privileged self-hosted runner | **Rejected** | None for pull requests | Cross-run state, credential, Docker, and kernel persistence make isolation and cleanup unverifiable. |
| GitHub-hosted runners | **Accepted for unprivileged/unit/native desktop lanes** | Linux/Windows/macOS builds and tests; GitHub currently exposes x64/Arm choices with differing availability ([runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)) | Hosted Linux runners do not provide the multi-host/privileged VM topology for authoritative Swarm chaos. |
| Docker Desktop local cluster | **Accepted for developer convenience only** | Manual client/UI smoke on macOS/Windows | VM networking/filesystem and version update behavior are not controlled release evidence. |
| Docker CLI as runtime dependency | **Rejected** | The pinned CLI may be a test oracle for stack conversion and independent state inspection | Human-oriented output is not the agent contract; runtime remains Engine API through the adapter. |
| Rootless Docker/Podman compatibility | **Deferred** | Non-blocking exploratory lane after Swarm/socket/cgroup capabilities are defined | The production target is Docker Engine Swarm; Testcontainers' Docker alternatives are not all covered upstream. Do not advertise until the full conformance suite passes. |

## Environment and topology matrix

Every environment creates a fixture manifest describing node roles, Engine/API versions, OS/kernel/architecture, storage driver, cgroup mode, filesystem, MTU, overlay encryption, controller/agent versions, DB version, and deployed workload hash.

| Topology | Frequency | Required assertions |
|---|---|---|
| Fake one-daemon | every PR | all Engine adapter calls/errors/streams, timeouts, retries, cancellation, redaction, request reuse |
| DinD 1 manager + 2 workers | selected PRs and main | v1 workflows, resource locality, service/task logs, task-bound exec, image/volume node scope, explicit partial results |
| Real VM 1 manager + 2 workers | nightly | same parity on real cgroups/filesystems/overlay, agent reschedule, daemon restart, packaging/install |
| Real VM 3 managers | nightly/RC | controller per manager, manager quorum, ingress readiness, schedule lease/claims, controller+node-agent same-node identity, one-manager loss |
| Real VM 3 managers + 2 workers | weekly/RC | mixed-role routing, drain/promote/demote, workload rescheduling, loss of one manager and workers, rolling agents |
| Real VM 5 managers | RC/GA for large HA profile | five controllers, loss of one and two managers, schedule takeover, DB pressure, rolling controllers, quorum restoration |
| Profile scale cluster | RC | small/medium/large counts from [performance](performance.md), native multi-host network, load/churn/stream mixes |

Docker documents that three managers tolerate one manager loss and five managers tolerate two; it recommends odd manager counts and no more than seven ([Swarm node model](https://docs.docker.com/engine/swarm/how-swarm-mode-works/nodes/)). The tests must remove managers only in an order that preserves the intended control scenario. Loss-of-quorum is deliberate chaos, not routine teardown.

Fixture readiness is semantic: every daemon answers `_ping`; Swarm node IDs and roles match; manager quorum and leader are observed; overlay peers exist; every controller/agent certificate and capability handshake is valid; database migrations are complete; seeded services reach their desired state; and an independent Engine client confirms the expected resources. A bounded deadline failure captures all daemon journals, `docker info`, node/task/service state, network state, agent logs, and fixture manifest.

## Real Docker Engine compatibility matrix

Docker Engine and API compatibility is a product contract, not “whatever Bollard compiles against.” Docker's API is version-negotiated and backward compatible on a best-effort basis; forced `DOCKER_API_VERSION` disables negotiation ([Engine API overview and matrix](https://docs.docker.com/reference/api/engine/)). At the audit date, Docker 29.7.2 is current and the published matrix maps 29.7 to API 1.55/minimum 1.40; 28.5 maps to 1.51/minimum 1.24; 27.5 maps to 1.47/minimum 1.24.

The seed matrix below is a proposal, not a support declaration:

| Lane | Engine | Architecture/topology | Gate |
|---|---|---|---|
| oldest candidate | latest 27.5 patch, API 1.47 | native Linux amd64 single + 1M/2W | blocking only if ratified as support floor |
| previous | latest 28.5 patch, API 1.51 | native Linux amd64 single + 3 node | blocking beta/GA |
| current | exact latest stable 29.7 patch (29.7.2 at audit), API 1.55 | native amd64 and arm64; 3 and 5 node | blocking beta/GA |
| next/test channel | exact tested prerelease | single/3 node | scheduled informational until declared supported |
| mixed rolling environment | oldest/current combinations allowed by the operator upgrade policy | 3M/2W | blocking GA for that policy |

Before beta, an issue must choose the support floor and window. Docker documents stable and test channels but says open-source Engine is community-supported rather than giving a multi-major commercial support promise ([Engine install and channels](https://docs.docker.com/engine/install/)). Kontainer must not infer an upstream security-support window that does not exist. A conservative default is current plus previous qualified lines, with an older compatibility tier only when product demand and security policy justify it.

For every blocking Engine version:

- pin the exact package repository snapshot or official image digest and retain the install manifest;
- negotiate normally, then assert daemon max/min, selected API, capability projection, and disabled newer features;
- run every resource list/inspect/mutation, optimistic conflict, events, stats `one-shot`, logs, exec, registry auth, local inventory, and error mapping applicable to that version;
- feed unknown future response fields and verify tolerant decoding;
- test a diagnostic forced API override separately and make its degraded state visible;
- run current and minimum on native Arm where Arm is released; QEMU build success is not runtime qualification;
- rerun the matrix on every Bollard, TLS, Tokio, protobuf, Docker base image, or Engine support-policy change.

Patch upgrades within a qualified line get an expedited smoke plus regression run; a new Engine minor/major starts non-blocking, then becomes blocking only after the complete matrix and known-issue review pass.

## Multi-node functional scenarios

The seeded workload must include replicated and global services, replicated/global jobs, rolling-update history, placement constraints, health failures, secrets/configs, overlay and local networks, named and anonymous local volumes, multiple log drivers, private-registry images, large image inventories, terminated/replaced tasks, and containers that support/non-support configured shells.

Required three-/five-node scenarios:

1. build an empty cluster, enroll all agents, create the control overlay, and prove no node-agent port is public;
2. start controllers and node agents on the same managers without identity collision;
3. list and inspect every resource with stable order, pagination, node provenance, correct manager/local authority, and explicit completeness;
4. scale/update/restart/rollback services and preserve unrelated Engine spec fields under version conflicts;
5. drain/pause/activate/promote/demote with preconditions and quorum protection;
6. create/list/inspect/remove secrets/configs without ever returning secret bytes; exercise rotation-by-replacement;
7. preview and execute image prune, remove local images/volumes on exact nodes, and account for every target;
8. deploy/update/remove stacks, including partial convergence, external resources, conflict, resume, and rollback boundaries from [stack deployment](stack-deployment.md);
9. stream service/task logs across replacement, cursor reconnect, unsupported driver, huge/invalid frames, slow clients, and controller loss;
10. open local/remote exec, verify fresh placement and user/shell, resize, cancellation, exit code, target replacement, abandoned client, and controller loss;
11. collect/push metrics under duplicate/out-of-order/late/missing batches, agent spool replay, restart, and retention;
12. verify event disconnect triggers a full resync rather than invented continuity;
13. upgrade/downgrade controller and agent versions in the approved order;
14. leave the cluster clean, with no residual service, volume, network, certificate, lease, operation, or runner secret.

An independent oracle talks directly to each Engine and the database. Product API success alone cannot prove that Docker reached the intended state, that unrelated fields survived, or that operation/idempotency records are correct.

## Fault injection and chaos

Use deterministic fake faults for exhaustive coverage and real faults only for behaviors requiring operating systems/networks. A real chaos run has a timeline, steady-state assertion, blast-radius guard, abort condition, recovery deadline, and cleanup verification.

| Fault | Injection level | Mandatory truth |
|---|---|---|
| Engine status/JSON/stream corruption, delay, reset, half-close | fake Engine | stable typed error, bounded allocation/time, cancellation, no unsafe write retry, redacted diagnostics |
| Task changes between resolution and mutation/log/exec | fake + real | precondition conflict or safe re-resolution; never act on the wrong container |
| Controller killed before claim/after claim/during Engine call/before result commit | process + real DB/Swarm | one durable claim, fenced owner, observable succeeded/failed/uncertain state, no fabricated result |
| Schedule holder partitioned from DB | proxy/network | stops claiming immediately on renewal risk; takeover meets SLO; no duplicate edge-triggered action |
| Agent/controller partition, latency, loss, reset, MTU black hole | `tc netem`/firewall/proxy on VMs | bounded retry/backoff, explicit stale/partial/gap, healthy targets remain within latency budget |
| Engine or VM restart | system service/VM | readiness drops, cached state is honest, reconnect/resync occurs, resources/FDs/tasks recover |
| One/two manager loss and quorum loss | real 3/5 managers | documented tolerance, mutation refusal without quorum, existing workloads represented honestly, bounded restore |
| DB stop/failover/lock/deadlock/slow fsync | real DB/proxy | admission/stale behavior, lease safety, no partial transaction, bounded queue, recovery/no duplication |
| Disk 70/85/95% and ENOSPC during write/checkpoint/backup | quota/loop device | exact degradation policy, protected control quota, no corruption or restart loop, actionable alert |
| Clock +/-5 minutes and backward wall jump | VM clock/test clock | monotonic intervals safe, certificate/time rejection clear, lease uses DB time, UTC buckets/gaps correct |
| Leaf/intermediate/root rotation and revocation mid-flight | test PKI | allowed overlap only, new unauthorized connections fail, existing-session policy enforced, no plaintext fallback |
| Registry unavailable/slow/wrong credentials/digest drift | fake + real registry | credentials redacted, digest semantics explicit, operation resumable/terminal as designed |
| CPU/memory/FD/task/connection pressure | cgroup/ulimit/load | admission/load shedding before OOM, bounded queues, recovery to baseline |

Run the core subset nightly. Run the complete 3M/2W and 5M suite for RC. A fault test passes only if the user/API outcome, Docker state, database state, audit trail, telemetry, alert, and cleanup all agree.

## Security verification

The detailed controls are in [security](security.md); these tests make them release gates.

### Identity, authentication, and authorization

- OAuth/OIDC discovery, exact redirect, PKCE, state/nonce expiry and replay, provider denial, membership removal, account linking, and concurrent callback tests;
- bootstrap token entropy, file permissions, expiry, one-time use, rate limit, resumable transaction, first-owner binding, and locked pre-setup surface;
- JWT algorithm/type/issuer/audience/required-claim/key-use allow lists; duplicate keys; huge tokens; `none`/confusion; clock/leeway; key rotation; session/authorization epoch;
- refresh rotation, concurrent legitimate refresh, lost response retry, old-token family reuse detection, revocation, and logout;
- table-driven allow/deny tests for every capability, system role, custom-role constraint, resource scope, recent-auth rule, and user-versus-service identity;
- positive and negative IDOR tests using guessed IDs, names, node routes, stack labels, and filtered collections;
- WebSocket origin/subprotocol/header/ticket binding, ticket single use/expiry, resume scope, and authorization recheck;
- mTLS absent/wrong/expired/not-yet-valid/revoked certificate, wrong cluster/workload/node/task SAN, source-task mismatch, enrollment replay, and root rotation.

### Input, isolation, and abuse

- request/frame/decompressed/nesting/string/map/page/series/point limits at boundary, one below, exact, one above, and compressed bomb;
- SSRF/path/URL normalization, host confusion, redirect, proxy-header trust, CORS, CSRF, open redirect, header injection, request smuggling at the supported proxy, and WebSocket cross-site cases;
- stack YAML anchors/aliases/interpolation/path-like fields, command/shell/user validation, archive/tar handling, registry reference ambiguity, and Unicode normalization;
- rate/concurrency/session/source/byte quotas by principal and global scope, including distributed controllers;
- Docker socket and service manifest checks: non-root where feasible, exact socket group, read-only rootfs, capability drop, no host PID/network, private overlay, resource limits, secret files, and no published agent port;
- container escape and host-root impact reviewed by an independent penetration test before GA.

### Secret and privacy tests

Inject unique canaries into access/refresh/setup tokens, OAuth codes, provider keys, registry credentials, secret/config data, env, certificate keys, database values, URLs, log/exec payload, and idempotency keys. Scan responses, errors, panic/crash output, structured logs, traces, metrics labels, audit records, support bundles, CI artifacts, SBOM/provenance, and screenshots. Any canary outside its explicit authorized sink is a release blocker.

Run dependency/advisory/license/source policy on Cargo, pnpm, container OS packages, GitHub Actions, and build tools. Use GitHub dependency review where available; it can block newly introduced vulnerable dependencies ([GitHub dependency review](https://docs.github.com/en/pull-requests/how-tos/review-pull-requests/reviewing-dependency-changes-in-a-pull-request)). Run CodeQL's Rust security-extended queries where repository licensing permits; GitHub now documents Rust built-in queries ([CodeQL Rust queries](https://docs.github.com/en/code-security/code-scanning/managing-your-code-scanning-configuration/rust-built-in-queries)). No Critical/High known exploitable runtime issue may ship without a time-bounded, owner-approved exception, mitigations, disclosure decision, and fix SLA.

## Persistence, migration, backup, and restore

Every advertised storage profile implements one conformance suite. Embedded v2 uses separate `control.sqlite3` and `metrics.sqlite3` as specified in [metrics storage](metrics-storage.md); HA uses the selected shared transactional store. Metrics may have a different backup/RPO policy, but control, identity, operation, idempotency, audit, lease, key metadata, and migration truth may not.

### Migration gates

For every released schema version and meaningful legacy state:

1. generate a deterministic fixture using the old migration code, not a hand-edited “final” schema;
2. include empty, ordinary, maximum-size, Unicode, expired/revoked, partial-operation, and high-cardinality states;
3. record row counts, logical checksums, schema fingerprint, encryption/key version, and representative authorized query results;
4. copy the fixture, run the exact packaged v2 migrator under a deadline and disk budget, then verify integrity, foreign keys, logical checksums, defaults, indexes, and application reads;
5. kill at every migration phase and restart; the outcome must be old intact or new complete, never silently partial;
6. inject lock, slow disk, ENOSPC, corrupt page, wrong key, and unsupported future schema;
7. test the N/N-1 mixed-binary schema window and refusal when a binary cannot safely read/write the schema;
8. restore the pre-migration backup and prove the documented rollback procedure.

Migrations are forward-only in ordinary operation. Use expand/migrate/contract: add compatible schema, backfill resumably, switch readers/writers, and remove old representation no sooner than the next release after downgrade support expires. A `down` migration is not a backup strategy.

Legacy Go migrations require an explicit policy for both `node_metrics` and unified `metrics`; one existing migration drops old history without copying it. The parity gate must select copy, read-only archive, or explicit discard with UI/operator notice and a verified count report.

### Backup/restore gates

- SQLite backup uses the online backup API or `VACUUM INTO`, never a copy of only the main file while WAL is active; SQLite documents its online backup interface for consistent snapshots ([SQLite backup API](https://www.sqlite.org/backup.html)).
- Back up control DB, schema/version manifest, encrypted secret envelopes, trust bundles, audit/operation state, and separately protected KEK/CA recovery material. Never put an unencrypted reusable key beside its ciphertext.
- Take backups during representative write, checkpoint, retention, and long-reader load; restore into a fresh isolated host and run integrity/foreign-key checks plus application conformance.
- Verify wrong/missing key behavior, key-version rotation, backup encryption, retention, immutability, access audit, partial upload, checksum mismatch, and stale backup detection.
- PostgreSQL/other external profiles must test base backup plus WAL/PITR or the selected native mechanism, fresh-cluster restore, failover/fencing, and point selection. Vendor tooling alone is not evidence until exercised with Kontainer schema and keys.
- Metrics backup is off by default only if loss is an explicit product promise and status/history degrade honestly. If metrics RPO is advertised, test it at full profile rate.

Docker Swarm's Raft state is separate from Kontainer state. Kontainer must not claim its DB backup restores a Swarm. Docker recommends stopping one manager before copying the entire `/var/lib/docker/swarm` directory, retaining the unlock key, and preserving manager quorum; hot copies are less predictable ([Docker Swarm backup and recovery](https://docs.docker.com/engine/swarm/admin_guide/#back-up-the-swarm)). The operator runbook and DR exercise must state whether Kontainer merely links to that process or orchestrates a separately authorized capability.

## Performance, load, and soak

[Performance](performance.md) owns workload envelopes and budgets. The reliability harness owns reproducibility, pass/fail comparison, and artifact retention.

### Required workloads

- microbenchmarks: DTO/protobuf/JSON/log decode, snapshot build/index/filter/sort/page, event application, stack normalization/diff, stats math, batch DB writes/queries, redaction, and rate/permit paths;
- component load: programmable Engine and agents with slow/stalled/error mixes, 100 UI clients, log/exec quotas, database contention, storage thresholds, and Tonic size/concurrency limits;
- end-to-end: small/medium/large envelopes, baseline without Kontainer, warm-up, steady load, churn, 10% unavailable/slow agents, and recovery;
- soak: at least 24-hour medium and eight-hour large stress from [performance](performance.md); metrics/storage RC qualification extends the medium steady/churn run to 72 hours where that document requires it.

Record p50/p95/p99/max, throughput, attempted/succeeded/failed/partial counts, CPU/RSS/allocation, FDs/tasks/connections/permits, queue bytes/items, disk/WAL/checkpoint, network, manager Engine request rate/latency, and recovery slope. Compare to a pinned baseline on equivalent hardware. Protected microbenchmarks fail on unexplained >10% p95 or allocation regression; end-to-end gates use the explicit profile/SLO budget, not a generic percentage.

The harness must prove:

- UI users do not linearly amplify manager Engine work; the second through hundredth UI changes manager request rate by less than the performance gate;
- no list/query performs per-row Engine calls or per-node database N+1 work;
- every queue and fan-out is bounded by items and bytes;
- overload yields `429`, `RESOURCE_EXHAUSTED`, stale/partial/gap, or admission failure before OOM or manager starvation;
- after churn, FDs/tasks/connections/permits return to the defined baseline and RSS has no disallowed positive slope;
- manager management latency remains within the accepted delta from the no-Kontainer baseline.

Benchmark results contain raw machine-readable samples, fixture manifest, commit/artifact digest, queries/flamegraphs where relevant, and comparison output. Do not accept screenshots or averages alone.

## Rolling upgrade and downgrade

Support current and previous internal protocol major/minor exactly as ratified in [API protocol](api-protocol.md) and [cluster communication](cluster-communication.md). The handshake advertises protocol ranges and capabilities before functional calls; unknown majors fail clearly without crash or partial registration.

### Required matrix

| Controller | Node agent | Database/schema | Required outcome |
|---|---|---|---|
| N | N | N | complete conformance |
| N | N-1 | N compatible | all old capabilities work; new capability absent/degraded honestly |
| N-1 | N | N compatible, if agent-first is allowed | only if release policy explicitly supports it; otherwise clean rejection before rollout |
| N and N-1 controllers | N-compatible schema | expanded | both ready and safe during controller rollout |
| N-1 binary after N canary | pre-contract schema | rollback | starts and serves supported behavior without data corruption |
| N-2 or future unsupported | newer/older schema | any | refuses with actionable version error; no migration attempt |

Recommended order pending the open decision:

1. verify signed artifact and preflight Engine/API, disk, DB, backup, key, protocol, and capacity;
2. take and verify a restore-capable control backup;
3. apply only backward-compatible expand migration with one migrator/lock;
4. upgrade one standby controller, keep it from singleton claims, and run read/mutation/stream probes;
5. upgrade remaining controllers one at a time while serving mixed N/N-1 traffic;
6. upgrade one node agent on a low-risk node, then 10%, 50%, and 100%, respecting Swarm update order/delay/failure action;
7. enable new capabilities only after fleet compatibility and persisted backfill are proven;
8. keep contract migration and old-field removal for a later release after the downgrade window.

At every wave assert availability, schedule uniqueness, operation durability, agent registry cardinality/generation, metrics gaps, active stream policy, Docker manager load, error budget, and rollback readiness. Stop automatically on an SLO/security/integrity threshold.

Downgrade is a tested binary rollback within the declared N/N-1 window, not arbitrary schema reversal. Re-deploy the exact previous digest; verify it against the still-compatible schema; replay smoke tests; and leave new unknown data intact. After a contract migration, rollback requires restoring the pre-upgrade backup into an isolated new deployment and following the data reconciliation runbook. Never automatically attach a restored older database while newer controllers can still write; fence old/new deployments and rotate credentials before cutover.

## Cross-platform binaries and container images

Release only targets with native runtime evidence. Rust's Tier 1 targets are built and tested by Rust upstream, while Tier 2 generally guarantees builds rather than full runtime testing ([Rust platform support](https://doc.rust-lang.org/rustc/platform-support.html)); Kontainer must still test its own Docker socket, TLS, SQLite, filesystem, signals, and packaging.

| Artifact/target | v2 decision | Verification |
|---|---|---|
| controller/node-agent `x86_64-unknown-linux-gnu` | **Required GA** | native compile/test/install, minimum glibc/kernel, real Engine/Swarm, signals, DB, TLS, image smoke |
| controller/node-agent `aarch64-unknown-linux-gnu` | **Required GA** | native Arm test runner and real Arm Engine; cross-build/QEMU is additional, not replacement evidence |
| OCI `linux/amd64` + `linux/arm64` manifest | **Required GA** | per-arch native health/integration, manifest/digest/labels/user/layers, SBOM/vulnerability/signature/provenance verification |
| musl static targets | **Deferred until ABI/image decision** | both are Rust Tier 2; require native DNS/TLS/SQLite/CA/timezone/plugin tests and size/security comparison before advertising |
| Windows node agent/Windows containers | **Deferred product scope** | compile-only is informational. GA would require native Windows Engine/Swarm, named pipe, HCS stats/log/exec, filesystem, service install, upgrade, and security suite. |
| macOS agent | **Rejected as Swarm runtime** | macOS may run the desktop/client, not a production Docker Engine Swarm node. |
| future CLI binaries | **No implicit release** | if shipped, native smoke on every listed Linux/macOS/Windows target and its own install/update/signing contract |

The desktop remains a separate product matrix: packaged Tauri smoke and v2 client conformance on supported macOS, Windows, and Linux variants; secure-store behavior; deep-link/browser auth; upgrade; signature/notarization; and no WebView access to bearer credentials. That matrix does not make the server agent cross-platform.

Images use pinned digest base/build images, a locked toolchain/dependency graph, reproducible build inputs, a minimal runtime, explicit non-root/socket-group design, OCI labels, health check, read-only-rootfs-compatible paths, and no `.env`, compiler, package manager, shell, or test credential unless explicitly required. Deployment smoke inspects effective capabilities, mounts, networks, secrets, limits, and published ports because image metadata alone cannot prove runtime hardening.

## Supply-chain and release artifact verification

The release workflow builds each subject once, tests that subject, and promotes the exact digest. It never rebuilds after approval.

### Required outputs

- per-target compressed binary/package, SHA-256 checksums, install manifest, license notices, and artifact manifest;
- multi-architecture OCI manifest plus immutable per-architecture digests;
- SPDX or CycloneDX SBOM per binary/package and an image SBOM attestation;
- vulnerability/license/policy result tied to the digest;
- SLSA provenance and Sigstore/GitHub attestation for every user-consumed binary and image;
- keyless OCI signature bound to the repository, exact release workflow identity, ref/tag, and digest;
- OpenAPI, protobuf descriptor set, default configuration schema, migration manifest, release notes, upgrade/downgrade runbook, and known issues.

GitHub artifact attestations sign provenance for binaries and images using OIDC; the required permissions are `id-token: write`, `attestations: write`, `contents: read`, plus `packages: write` only for image publication ([GitHub artifact attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations)). GitHub says artifact attestations alone provide SLSA v1 Build L2, while an appropriately isolated reusable build workflow can support Build L3 ([attestation concepts](https://docs.github.com/en/actions/concepts/security/artifact-attestations), [reusable workflow/L3](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/increase-security-rating)). SLSA v1.2 defines L2 as signed hosted-build provenance and L3 as a hardened isolated build platform ([SLSA v1.2 build track](https://slsa.dev/spec/v1.2/build-track-basics)). Kontainer must claim the achieved level only after verifying all requirements, not merely because an action ran.

For registry interoperability, sign image digests keylessly with Cosign and verify certificate identity/issuer plus transparency inclusion. Sigstore recommends bundles containing signature, certificate, and transparency proof for blobs and supports identity-based ephemeral keys ([Sigstore signing](https://docs.sigstore.dev/cosign/signing/signing_with_blobs/)); verification is the step that realizes the integrity benefit ([Sigstore verification](https://docs.sigstore.dev/cosign/verifying/verify/)).

BuildKit/Docker can attach `mode=max` provenance and SBOM to pushed images; attestations are not retained by a local `load`, and build args may appear in max provenance, so secrets must use secret mounts rather than args ([Docker SBOM/provenance attestations](https://docs.docker.com/build/ci/github-actions/attestations/)).

Release security rules:

- pin every action/reusable workflow by full commit SHA and record the human-readable version in a comment;
- default workflow/job permissions to read-only and grant publish/attest permissions only to isolated release jobs;
- untrusted pull requests receive no secrets, OIDC publication permission, package write, privileged persistent runner, or `pull_request_target` execution of PR code;
- release only from a protected signed semantic tag after required checks and environment approval;
- create a draft, attach all verified assets, then publish an immutable release. GitHub immutable releases lock tags/assets and create a release attestation ([immutable releases](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases));
- independently download and verify checksums, architecture, signatures, attestations, SBOM subjects, release integrity, and clean-host install before publication;
- deployment examples use an image digest. Mutable convenience tags are pointers only and never the rollback record;
- revoke/delete trust in a compromised artifact according to a published incident process; signatures prove origin, not safety.

Target SLSA Build L3 for GA release subjects. If the selected release environment cannot meet isolated-build requirements, disclose the lower achieved level and block GA until the open supply-chain decision is resolved.

## CI lanes and evidence retention

| Lane | Trigger/target | Contents | Target time / retention |
|---|---|---|---|
| Source hygiene | every PR | formatting check, Clippy `-D warnings`, Rustdoc/doctest, forbidden APIs/unsafe policy, generated diff, frontend lint/types/format/test, shell/YAML/Dockerfile lint | <=10 min; logs 30 days |
| Rust fast | every PR | nextest unit + properties with fixed/random seeds, corpus replay, fake Engine, DB temp fixtures, all supported features, no-tests/ignored-test check | <=10 min; JUnit/coverage 30 days |
| Contract | every PR | OpenAPI generation/breaking, generated Rust/TS clients, protobuf lint/breaking, WebSocket/IPC/golden, v1 parity exception ledger | <=10 min; contract diffs retained with PR |
| Security PR | every PR | dependency/license/advisory/action review, secret scan, CodeQL/SAST, policy manifests, decompression/limit/redaction tests | <=15 min; SARIF per policy |
| DinD integration | relevant PR/main | pinned namespaced 1M/2W fixture, parity workflows, migration/backup smoke, package/image smoke | <=30 min; failure bundle 30 days |
| Native build | every PR/main | Linux amd64/arm64 compile/test where capacity allows; desktop OS matrix; MSRV/minimum OS compile | <=30 min; artifacts 7-30 days, never release |
| Nightly | schedule/main | current/previous Engine, real 1M/2W and 3M, sanitizers/Miri, fuzz smoke, core chaos, performance trend | <=4 h; raw results 90 days |
| Weekly extended | schedule | 3M/2W, five-manager faults, old Engine candidate, native Arm, long fuzz, security attack suite, backup/PITR, medium load | bounded by suite; results 1 year |
| RC qualification | protected `v2.*-rc.*` | full Engine/topology/security/migration/upgrade/cross-platform/supply-chain/load/chaos/DR suite on exact artifacts | no arbitrary time cap; immutable evidence for release lifetime |
| Canary | promoted RC digest | production-shaped deployment, synthetic workflows, SLO/error-budget/alert/upgrade/rollback observation | minimum duration selected below; summary retained for release lifetime |

Run independent jobs in parallel, but serialize destructive use of a particular cluster through an explicit lease. Shard deterministic tests with nextest by stable hash; do not shard stateful scenario steps that share one cluster. Every job uses a pinned tool version and lockfile, and caches are performance hints only: a cache miss must still build correctly, and release subjects do not trust mutable compiled-output caches.

Required-branch checks use stable lane names and merge queue. Cancellation stops new work, propagates into tests, and still runs bounded cleanup/artifact capture. Flakes are measured as a product SLI: zero known flakes is the target; quarantine requires issue, owner, reproduction, reason, expiry <=14 days, and a check that still reports failure. No RC runs quarantined tests.

## Coverage matrix

Codes: **P** required by v1 parity, **B** additionally required by beta, **G** additionally required by production GA, **—** not the primary layer. Every later gate includes earlier letters.

| Domain | Unit/property/model | Contract/golden | Fake dependency | DinD/real integration | Chaos/security | Load/soak | Upgrade/restore |
|---|---|---|---|---|---|---|---|
| Public API, errors, pagination, idempotency | P | P | P | P | B | G | G |
| OAuth/OIDC, sessions, RBAC, bootstrap | P | P | B | B | B/G | G rate | G key/state |
| mTLS, discovery, registry, gRPC | P | P | P | P/B | B/G | G | G N/N-1 |
| Services/tasks/nodes/stacks mutations | P | P | P | P | B/G | G | G |
| Secrets/configs/registry custody | P | P | P | P | B/G | G abuse | G key/backup |
| Images/networks/volumes/local routing | P | P | P | P | B/G | G fan-out | G |
| Logs | P | P | P + fuzz | P | B/G | B/G soak | G mixed stream |
| Exec | P | P | P + fuzz | P | B/G | B/G soak | G terminate/mixed |
| Metrics math/transport/query/retention | P | P | P | P/B | B/G | B/G profile | B/G restore |
| Schedule lease/operation claims | P model | B | B | B 3M | B/G | G contention | G failover |
| DB migrations/control state/audit | P fixtures | P schema | P DB faults | B | B/G | G | B/G |
| Desktop/Tauri/secure store | P UI | P IPC/client | P HTTP/WS | B packaged | B/G | B refresh | G client/server |
| Observability/SLO/alerts/support bundle | P fields | B schema | B faults | B | B/G | G | G DR |
| Packaging/binaries/images/supply chain | — | B manifest | — | B native smoke | B/G scan | G startup | G rollback |

An issue that touches a row must select the applicable cells. A unit test alone is insufficient for a protocol, persistence, deployment, or operational behavior change.

## Release gates

### Gate 1: v1 visible parity

This gate permits an explicitly non-HA one-controller/SQLite preview. It does not permit production claims.

- [ ] Every v1 desktop-visible workflow in [API protocol](api-protocol.md) has a normalized parity scenario and intentional-difference entry.
- [ ] Every listed Go/Rust/TypeScript mismatch has a regression test against the v2 schema.
- [ ] Accepted old Go and React specifications above have been ported; rejected behavior is covered by the corrected expectation.
- [ ] Unit/property, public/gRPC/stream goldens, fake Engine, and pinned 1M/2W DinD suites pass.
- [ ] Rust agent/core/Tauri native code has real tests; required test jobs fail on zero tests or ignored tests.
- [ ] Lists/mutations/logs/exec/metrics have bounded size, deadline, cancellation, retry/idempotency, partial, error, and redaction tests.
- [ ] Mandatory safety repairs from [performance](performance.md) pass: shared clients, one-shot stats, node-local collection, pagination, bounded queues/fan-out, honest stale/partial state, and correct interval/retention wiring.
- [ ] mTLS, target identity, structured errors, authorization, audit context, and no query-string bearer token are present even in parity.
- [ ] Legacy database/profile migrations have policy and deterministic fixtures; pre-migration backup/restore works.
- [ ] Linux amd64 development image/package smoke passes, but it is not yet a GA artifact.

### Gate 2: beta

Beta includes all parity requirements plus:

- [ ] Three real managers/controllers and a global node agent pass normal operation, one-manager loss, schedule lease/claim, discovery generation, and controller/node-agent same-node tests.
- [ ] Current and previous selected Docker Engine lines pass; Linux `amd64` and native `arm64` binaries/images pass their runtime smoke.
- [ ] Public/internal security suite, rate/limit tests, canary-secret redaction scan, CodeQL/dependency/license/advisory/image scans, and an initial independent threat-model review have no unaccepted Critical/High finding.
- [ ] Every released-schema migration, crash-restart, control backup/restore, wrong-key, and disk-full core scenario passes with measured migration/restore duration.
- [ ] Current/previous protobuf compatibility and the approved first rolling-upgrade order pass on a three-manager cluster.
- [ ] Core chaos passes: controller/schedule-owner kill, manager/Engine restart, agent partition, DB loss/lock, certificate rotation/revocation, and storage thresholds.
- [ ] Medium one-hour load plus 24-hour medium soak meets the provisional budgets with bounded recovery; no unexplained resource leak.
- [ ] SLO metrics, dashboards, alerts, audit events, readiness, and a redacted support bundle exist and are tested by fault injection.
- [ ] Exact RC binaries/images are checksummed, scanned, SBOMed, signed, provenance-attested, independently verified, install-smoked, and published as a prerelease from a protected source tag.
- [ ] Upgrade, rollback, backup, restore, incident, support, and known-limit runbooks have been followed once by someone other than the author.

### Gate 3: production GA

GA includes all beta requirements plus:

- [ ] The advertised topology/storage profile is settled. If HA is advertised, multiple controllers use the selected shared transactional DB; five-manager and loss-of-two-manager tests pass.
- [ ] The final Engine support floor/current window, mixed-version operator path, native architectures, ABI/minimum OS, and unsupported targets are published and pass the exact matrix.
- [ ] All profile workload budgets and SLOs in [performance](performance.md) pass, including the 72-hour medium storage soak where applicable, eight-hour large stress, 100-user independence, churn, streams, and 10% slow/unreachable agents.
- [ ] Complete chaos/security/penetration tests pass with no unresolved Sev-1/Sev-2 or exploitable Critical/High issue and no silent partial/uncertain outcome.
- [ ] Controller/agent N/N-1, database expand/contract window, rolling upgrade, binary downgrade, failed-canary rollback, and unsupported-version refusal pass on exact release artifacts.
- [ ] RPO/RTO, backup destination/encryption/key custody, restore, split-brain fencing, and a fresh-environment timed disaster-recovery exercise meet the published objective.
- [ ] SLO alert rules fire and resolve under injected conditions; an error-budget report and capacity headroom report cover the canary.
- [ ] The exact release digest completes the approved canary duration and waves with no gate breach; promotion performs no rebuild.
- [ ] SLSA claim is verified against the chosen build platform, Sigstore/GitHub attestations verify online and offline, immutable release assets verify, and the deployment example resolves to the signed digest.
- [ ] Support bundle, escalation/incident roles, security reporting, artifact revocation, rollback, and end-user upgrade guidance are published and exercised.

## Release candidates and canaries

1. Cut `v2.x.y-rc.n` only from a protected commit after PR/main lanes pass.
2. The isolated release workflow builds all binaries/images once, emits the manifest/SBOM/provenance/signatures, and records subject digests.
3. Qualification downloads those subjects from the staging registry/release, verifies them, and runs the RC matrix. Local workspace builds are not substitutes.
4. Deploy to an internal 3M/2W canary. Run synthetic read/mutation/log/exec/metrics workflows and one rollback.
5. For an external/production canary, upgrade one standby controller, then one low-risk node agent, then 10%/50%/100% waves according to the approved order. New capabilities remain dark until mixed-version safety is proven.
6. Observe at least 24 hours after the final internal wave and a proposed seven days of production-shaped canary before first GA. The exact durations and user exposure are an open release-policy decision.
7. Automatically stop or roll back for security/integrity failure, migration/backup failure, duplicate schedule/operation claim, readiness/failover breach, error-budget burn, persistent partial/gap regression, or resource budget breach.
8. Promote the same manifest digest to GA tags and publish the prepared immutable release. Verify again from a clean consumer environment.

A rollback records trigger, old/new digests, schema version, operation reconciliation, lost/duplicated work check, user impact, and follow-up. A failed RC number is never reused.

## Observability and SLO validation

Use the service objectives in [performance](performance.md): cached reads 99.9%, admitted mutation API 99.5%, cached list p95 <=250 ms medium/<=500 ms large and p99 <=2 s, resource visibility p95 <=2 s/correctness <=60 s, current task freshness p99 <=15 s, metric freshness p99 <=45 s, eligible sample coverage >=99% per five-minute window, controller failover p95 <=10 s, schedule takeover p95 <=30 s/p99 <=60 s with zero duplicate claims, complete fan-out accounting, and zero OOM/unbounded queue.

For each SLI define in code/reviewed configuration:

- numerator, denominator, valid/invalid traffic, clock and aggregation window;
- which dependency and maintenance failures are included or broken out;
- low-cardinality labels and cardinality budget;
- objective, short/long burn alerts, severity, owner, runbook, and synthetic probe;
- exact test that produces success, failure, partial, stale, timeout, overload, and recovery samples.

Validation runs fault and load scenarios, then queries the same telemetry backend operators use. Assert spans propagate request/operation context through HTTP, gRPC, DB, and Engine; metrics account for every target/outcome without raw IDs; logs/audit contain safe codes and durations; alerts fire within their deadline and clear after recovery; dashboards show budgets and stale telemetry. A test-only internal counter does not validate production observability.

Readiness and liveness are independently tested. Liveness never restarts for an ordinary dependency outage. Controller readiness fails for unavailable shared DB or local manager/certificate/auth prerequisites; agent readiness fails for identity/Engine/capability prerequisites. Deep diagnostics are authenticated and redacted.

## Incident response and support bundles

### Incident exercises

Before GA and at least twice yearly thereafter, run a game day covering:

- bad image/signature or vulnerable dependency recall;
- controller crash loop and failed migration;
- database loss/PITR and stale controller fencing;
- manager quorum loss and separate Swarm restoration responsibility;
- CA/KEK compromise or loss, certificate/session revocation, and re-enrollment;
- runaway logs/exec/metrics causing resource exhaustion;
- authorization bypass or secret exposure;
- failed rolling upgrade and artifact rollback.

Each exercise records detection source/time, severity, incident commander/roles, customer/maintainer communication, containment, evidence preservation, recovery, RTO/RPO achieved, artifact/version scope, and follow-up owners. Test the security-reporting channel without publishing a real vulnerability.

### Support bundle contract

Provide an offline command that creates a versioned, bounded archive only after local authorization and a preview/consent step. Default content is allow-listed:

- build/artifact digest, protocol/schema versions, enabled capability names, OS/kernel/architecture, and effective non-secret configuration sources;
- redacted structured logs for a bounded window, health/readiness reasons, SLO/queue/resource summaries, and request/operation/audit IDs/outcomes;
- DB engine/schema/integrity/size/checkpoint/retention status, never database rows by default;
- allow-listed Docker `/version`/`/info`, Swarm role/quorum summary, Kontainer service task state, and control-network diagnostics;
- manifest, SBOM/provenance verification result, migration/backup status, clock/skew, and bundle schema/checksums.

Always exclude bearer/refresh/setup/ticket/idempotency secrets, OAuth codes/verifiers, cookies, registry credentials, secret/config data, container env, private keys, raw certificates beyond safe fingerprints/metadata, database files, arbitrary Docker labels, full internal addresses unless explicitly opted in, log/exec payload, core dumps, and URL query strings.

Bundle tests inject secret canaries, huge/high-cardinality fields, malformed logs, symlinks, path traversal, disk-full, interrupted creation, and concurrent rotation. Verify stable redaction, file modes, no archive traversal, size cap, checksums, deterministic schema, expiration/deletion guidance, and optional operator-public-key encryption. Upload is never automatic; the user chooses transport.

## Disaster recovery exercises

Run a component restore on every RC and a full fresh-environment drill at least quarterly for every production profile. The full drill starts from documented prerequisites, release artifacts, backup objects, and separately protected key material—never from an existing controller disk.

Scenarios:

1. lose all Kontainer controllers while Swarm and DB remain; redeploy exact signed digest and recover without replaying unsafe mutations;
2. lose the control DB; restore newest valid backup/PITR, fence old writers, reconcile Docker versus operation state, rotate credentials as required, and measure data loss/time;
3. lose metrics DB only; preserve control operation, expose the promised gap, and rebuild/restore according to metrics RPO;
4. lose online intermediate/KEK access; exercise documented recovery or explicit irrecoverable/rebootstrap path without a hidden master key;
5. lose Swarm manager quorum; follow Docker's separate recovery runbook, then reconcile Kontainer identities, node/task bindings, operations, and caches;
6. restore a pre-upgrade backup after failed contract migration into an isolated deployment, validate it, and perform a fenced cutover.

Acceptance compares database logical checksums and schema, principal/session/certificate policy, Docker desired/observed state, operation/idempotency outcomes, audit continuity, metric gap, SLO, and user workflows. Record actual RPO/RTO and every manual/undocumented step. A successful command exit without application reconciliation is not a successful recovery.

## Definition of Done template for every GitHub issue

Copy this section into every implementation, bug, refactor, dependency, protocol, schema, deployment, and operational issue. `N/A` requires a reason and reviewer; it is not a silent deletion.

```markdown
## Definition of Done / verification

### Behavior and scope
- [ ] Observable behavior, invariants, non-goals, affected v1 parity item, and gate (parity/beta/GA) are written.
- [ ] Supported Engine/API versions, topology/profile, OS/architecture, storage backend, and client versions affected are identified.
- [ ] Failure, timeout, cancellation, retry/idempotency, partial/uncertain, and rollback behavior is specified.

### Tests
- [ ] Smallest unit tests cover normal, boundary, empty, and typed-error paths.
- [ ] Applicable property/state-machine/concurrency/fuzz regression tests are added with reproducible seeds/corpora.
- [ ] Applicable public/protobuf/stream/IPC/database golden and compatibility baselines are intentionally updated and reviewed.
- [ ] Applicable fake Engine/dependency tests assert exact request, response, deadline, cancellation, connection reuse, and redaction.
- [ ] Applicable DinD/real Engine/3-node/5-node/native-architecture scenario is added or referenced.
- [ ] Applicable migration, backup/restore, N/N-1 upgrade/downgrade, chaos, security, load, and soak evidence is attached.
- [ ] No unexpected ignored/quarantined/flaky/zero-test result; coverage meets critical/workspace/changed-line policy.

### Security and privacy
- [ ] Authentication, authorization capability/scope, audit, secret classification, input/size/rate limits, and threat-model impact are reviewed.
- [ ] Logs/errors/traces/metrics/support bundles contain stable safe codes and no injected secret canary or high-cardinality ID label.
- [ ] Dependency/license/advisory/SBOM/image/action changes pass policy; any exception has owner, expiry, mitigation, and fix SLA.

### Reliability and operations
- [ ] Deadline, resource/queue/concurrency/disk budget, manager/DB impact, SLI/SLO/alert, readiness, and capacity-profile impact are measured or justified.
- [ ] Structured telemetry and operator diagnostic/runbook changes are implemented and fault-tested.
- [ ] Backward/forward compatibility, rollout waves, abort condition, exact rollback artifact/procedure, and data reconciliation are documented.

### Delivery
- [ ] Rust/TypeScript/protobuf/OpenAPI/config/documentation stay in sync; generated output is clean.
- [ ] Required CI lanes pass on the exact commit/artifact; failure artifacts are reviewed.
- [ ] User/operator release notes, migration/known-limit/support guidance, and issue links are complete.
- [ ] Acceptance evidence contains commit and artifact digest, versions, fixture manifest, seed/fault timeline, raw results, and reviewer.
```

## Issue-ready open decisions

Each decision issue needs an owner, deadline, options, threat/failure analysis, measured evidence, migration impact, and explicit exit criterion.

1. **Ratify Docker Engine support floor and window.** Decide current/previous/older lines, exact oldest API/capabilities, patch qualification cadence, distro-packaged Engines, mixed-major upgrade support, and retirement notice. Exit: published matrix passes native amd64/arm64 and 3-node tests.
2. **Resolve GA storage/topology contract.** Reconcile embedded dual-SQLite MVP with multi-controller/shared-DB production requirements; name which profile may be called GA/HA. Exit: one backend is selected per advertised profile with lease, migration, load, backup/PITR, failover, and DR evidence.
3. **Choose release targets and ABI.** Decide GNU minimum glibc/kernel, musl yes/no, `amd64`/`arm64`, Windows Swarm scope, CLI artifacts, and desktop OS versions. Exit: native test/install matrix and unsupported-target statement approved.
4. **Fund ephemeral privileged test infrastructure.** Choose VM/cloud provider, image builder, Arm capacity, network fault mechanism, secrets/identity, cost/TTL, artifact retention, and emergency cleanup. Exit: reproducible 3M/2W and 5M runs from a clean account with no persistent runner.
5. **Approve SLO, RPO, and RTO numbers.** Ratify performance objectives, maintenance/dependency accounting, embedded and HA control RPO/RTO, metrics loss policy, backup cadence/destination, and alert burn windows. Exit: fault/load and one timed DR drill meet them.
6. **Ratify controller/agent/database compatibility and rollout order.** Choose protocol N/N-1 range, controller-first versus agent-first, capability dark launch, schema expand/contract delay, downgrade window, and unsupported-version behavior. Exit: complete mixed-version matrix passes.
7. **Ratify coverage and test-quality policy.** Approve critical/workspace/changed-line thresholds, generated exclusions, mutation cadence, flake quarantine SLA, no-tests/ignored enforcement, and evidence retention. Exit: baseline report and branch protection configured without existing unexplained gaps.
8. **Choose supply-chain implementation and claimed SLSA level.** Select GitHub attestations versus SLSA generator details, isolated reusable workflow, Cosign identity policy, SBOM formats/tools, vulnerability/license policy, offline verification, immutable release, and artifact revocation. Exit: clean consumer verifies an RC and the claimed SLSA v1.2 requirements are audited.
9. **Select chaos tooling and safety policy.** Choose proxy/netem/firewall/disk/clock/process injectors, blast-radius guards, abort authority, cleanup, and cloud isolation. Exit: deterministic core chaos runs without affecting another job.
10. **Choose fuzzing service, budgets, and corpus governance.** Decide nightly/continuous provider, per-target PR/nightly/RC budgets, sanitizer targets, corpus privacy/licensing/size, triage SLA, and crash disclosure. Exit: all required targets run, corpora replay, and an injected crash opens a useful issue.
11. **Define RC/canary policy.** Decide minimum internal/external duration, user exposure, waves, synthetic destructive target, error-budget thresholds, approval roles, failed RC retention, and emergency security release exception. Exit: one beta canary and rollback complete using exact digests.
12. **Approve support-bundle schema and support workflow.** Define allow list, time/size limit, encryption/upload, internal address handling, audit access, retention/deletion, and backward parser support. Exit: canary-secret/path/size tests and a support dry run pass.
13. **Choose v1 data and behavior retirement.** Decide legacy metric/profile migration, parity exception ledger owner, v1 adapter/window, frozen Go oracle lifetime, test fixture privacy, and removal criteria. Exit: every v1 workflow/data class has a tested migrate/archive/discard outcome.
14. **Set security release policy.** Define severity model, exploitable dependency exception, fix/disclosure SLA, penetration-test scope/provider, signing compromise/revocation, and emergency credential/CA recovery. Exit: tabletop and one technical exercise pass.
15. **Define test data and evidence governance.** Decide whether real Engine/provider fixtures may be captured, sanitization/canary review, log/core/pcap retention, access control, deletion, and customer-data prohibition. Exit: CI artifact policy and automated secret scan are enforced.

## Production readiness statement

Kontainer v2 is production-ready only when the exact immutable release subjects pass the GA gate; every supported Engine, architecture, topology, and backend has reproducible evidence; N/N-1 rollout and restore are proven; security and secret-redaction tests are clean; all queues, fan-outs, streams, writes, migrations, and failures are bounded and observable; the canary remains within its error budget; and an operator other than the implementer can diagnose, roll back, and recover the system from the published runbooks and support bundle.

# Kontainer v2 Rust architecture

**Status:** Proposed architecture for review

**Research date:** 2026-08-19

**Scope:** The Rust controller, node agent, shared protocols, storage and engine adapters, and reusable Rust client. This document proposes boundaries and decisions; it does not authorize dependency changes or implementation.

## Executive decision

Kontainer v2 should be a Cargo workspace with two deliberately different server binaries:

- `kontainer-controller` runs on manager nodes. It owns the public API, controller-side reconciliation, durable control state, manager Docker access, agent membership, and controller-to-agent calls.
- `kontainer-node-agent` runs globally. It exposes only the private mTLS agent protocol and owns node-local Docker access, log and exec sessions, node metrics, and local bounded spooling.

The architecture should be **hexagonal at volatile or expensive boundaries**, **domain/action-oriented inside each capability**, and **actor-like only for stateful, long-lived streams and resource owners**. It should not adopt an actor framework, a framework-wide service locator, a generic repository for every record, or one large `Orchestrator` facade. Binaries are explicit composition roots. Core crates depend inward on domain types and ports; Axum, Tonic, Bollard, SQL libraries, generated messages, and persistence rows remain adapters.

The workspace should also contain a small `kontainer-client` crate. Tauri, GPUI, and a future CLI can reuse it without depending on controller internals, Docker types, storage types, Tauri commands, or internal agent protobuf. Public REST DTOs and public stream frames are the shared contract. The native desktop shell owns credentials and network connections; React still does not receive the agent bearer token, consistent with the [API and protocol report](./api-protocol.md) and [security report](./security.md).

This design preserves the useful part of v1's action-per-file organization while correcting its main coupling problems: the broad `Orchestrator` interface, wire annotations on application types, concrete route wiring, framework-global service resolution, ad hoc error mapping, and unowned background goroutines.

Several choices remain gated. In particular, Bollard framing, SQLite access, the rustls crypto provider, protobuf code generation, Compose parsing, and the public client generation path need bounded spikes before their dependencies are committed. The candidate versions below are a research snapshot, not an instruction to modify a manifest.

## Inputs and constraints

This proposal synthesizes the completed v2 specialist reports rather than replacing them:

- [Agent lifecycle](./agent-lifecycle.md) defines explicit controller and node-agent roles, strict configuration, task ownership, graceful shutdown, upgrade behavior, and health semantics.
- [API and protocol](./api-protocol.md) establishes `/api/v2`, RFC 9457 errors, OpenAPI 3.1, internal versioned protobuf, WSS protobuf streams, SSE events, and a native-client trust boundary.
- [Cluster communication](./cluster-communication.md) defines controller-to-agent mTLS gRPC, membership, capability negotiation, deadlines, and direct node routing.
- [Exec](./exec.md) specifies the stateful exec lifecycle, single-owner pumps, bounded input/output, resize coalescing, cancellation, and replay-resistant session authorization.
- [Logging](./logging.md) specifies raw-byte correctness, bounded fan-out, flow control, cursor behavior, daemon error-frame handling, and explicit task ownership.
- [Metrics and storage](./metrics-storage.md) separates control state, metrics state, and node spool behavior, and distinguishes embedded SQLite from production shared storage.
- [Performance](./performance.md) requires shared Docker clients, immutable snapshots, event-plus-reconcile behavior, bounded queues and concurrency, and measured resource budgets.
- [Reliability and testing](./reliability-testing.md) defines model, property, contract, integration, VM, chaos, load, and upgrade coverage.
- [Security](./security.md) defines native credential custody, mTLS, secret redaction, safe-Rust policy, supply-chain controls, and audit requirements.
- [Stack deployment](./stack-deployment.md) requires a native reconciler, a typed plan, server-side apply semantics, rollback records, and no Docker CLI dependency.
- [Swarm resources](./swarm-resources.md) defines the resource/action surface, capability handling, snapshots, optimistic concurrency, and operation semantics.

The repository rules also require grouping by domain, action-per-file for discrete operations, explicit Tauri state injection, synchronized Rust/TypeScript boundary types, and tests for every code change. A future Rust tree is therefore shown as an architectural target only. Creating a new top-level directory, changing dependencies, or replacing `server/` still requires a separately approved implementation decision.

## What v1 teaches us

The Go agent has useful seams worth carrying forward:

- Actions such as `ScaleServiceAction.Execute` give operations descriptive names and a natural unit-test boundary.
- The Swarm adapter's narrower `DockerAPI` interface is a better test seam than constructing a real daemon client in application code.
- Controllers, actions, services, and Tauri commands are often split by operation, which makes navigation predictable.
- Context is already passed through many application operations, making deadlines and cancellation conceptually visible.

The v2 architecture should not reproduce these v1 patterns:

- `services/docker.Orchestrator` combines many unrelated resource operations and exports types carrying JSON concerns. It is difficult to fake faithfully and causes unrelated changes to share a compilation boundary.
- Bootstrap and route code instantiate concrete controllers/actions and lean on framework facades. This hides dependency direction and makes lifecycle ownership ambiguous.
- Error behavior is mostly a few sentinel errors plus controller-specific string mapping. Clients cannot reliably distinguish retryable dependency failure, conflict, capability absence, uncertainty, and cancellation.
- Exec and other real-time paths create goroutines with local conventions, sometimes using background contexts. There is no process-wide task ownership contract.
- The protobuf service is broad and not packaged as an explicitly versioned internal protocol boundary.
- The Tauri root registers a large command surface. Without a client boundary, transport logic can drift across commands.

The lesson is not “replace actions with layers.” It is to keep named operations while making their dependencies, transaction boundaries, cancellation, and adapter translations explicit.

## Architecture styles considered

### Hexagonal / ports and adapters — accepted selectively

Ports are justified where an implementation is volatile, privileged, slow, or essential to deterministic testing:

- Docker Engine access
- control and metrics repositories
- clock, randomness, and identity generation
- controller-to-agent RPC
- identity provider, key management, and certificate validation
- audit and event publication
- process/task supervision where failure policy is observable

The core owns these port contracts. Adapters implement them using Bollard, SQLx or rusqlite, Tonic, rustls, and other infrastructure. Transport handlers call application actions; they do not become the application.

The rejected form is “a trait for every helper.” Pure parsing, small value transformations, and stable in-process collaborators should usually be concrete modules or functions. Excessive fine-grained traits obscure call flow, make errors generic, encourage mocking implementation details, and increase dynamic-dispatch and maintenance overhead.

### Domain and action per file — accepted as the application shape

Each user-meaningful operation should have one primary action, for example:

```text
services/scale_service.rs       ScaleService
stacks/apply_stack.rs           ApplyStack
operations/cancel_operation.rs  CancelOperation
exec/open_exec.rs               OpenExec
```

An action is not a one-line wrapper. It owns the operation's authorization-independent business policy, validation against domain invariants, idempotency/operation creation, transaction boundary, calls to coarse ports, and translation of adapter-neutral outcomes into domain errors. Related actions, domain types, policies, and ports live together under their capability, not in global `controllers/`, `services/`, and `repositories/` buckets.

Tiny read-only lookups may be direct application functions when a separate action type adds no semantic boundary. The action-per-file rule should improve discoverability, not force boilerplate.

### Actor-based streams and sessions — accepted as a local pattern

Actor-like single ownership is the right fit for components with mutable protocol state and a lifecycle:

- one exec session owner
- one log subscription/fan-out owner
- one node-channel or membership owner
- one metrics batch writer/spool owner
- one snapshot reconciler or operation runner

Each owner has a bounded mailbox or channel, a child cancellation token, explicit admission limits, and a tracked join handle. It serializes mutation of its own state and reports a typed terminal outcome. This agrees with the execution, logging, lifecycle, reliability, and performance reports.

The workspace should not adopt a general actor runtime. Most request/response actions do not need mailboxes, supervision trees, message enums, or actor addresses. A framework would introduce a second lifecycle model alongside Tokio, make protocol backpressure less visible, and couple otherwise straightforward domains to an abstraction chosen for a few long-lived components. Tokio tasks plus small project-owned owner loops are sufficient unless a spike demonstrates otherwise.

### Service layer — accepted narrowly, rejected as a generic middle tier

Small application services are useful for shared lifecycle-bearing collaborators such as an operation coordinator, snapshot cache, agent directory, policy evaluator, or task supervisor. They should expose domain-specific methods, depend on owned ports, and state their concurrency contract.

A global “service manager,” framework facade, or `Arc<AppServices>` bag is rejected. So are god services whose methods merely mirror every endpoint. Axum/Tonic state should contain a small number of explicit capability handles; each handler receives only the action/facade needed for that transport. Composition happens in the binary.

### Generated clients — accepted at contract edges, not as the internal architecture

Generation has different answers for different contracts:

- Internal agent RPC: generate Prost/Tonic message and service code from checked-in, versioned `.proto` files.
- External TypeScript and third-party SDKs: generate from the normalized, checked-in OpenAPI artifact, with golden and compatibility checks.
- In-repository Rust clients: provide a hand-written ergonomic `kontainer-client` over stable public DTO and stream crates. In CI, an independently generated Rust client should exercise conformance so the OpenAPI contract is not biased toward the hand-written implementation.
- Tauri IPC: keep explicit serialized command types initially. Do not make Specta or another generator an architectural dependency until its enum, error, and versioning behavior has been qualified.

Using generated public Rust code as the only in-repo client was rejected because generator output tends to expose transport mechanics, weakly model authentication/stream lifecycles, churn across generator upgrades, and provide a poor native UI API. Hand-writing the HTTP client does not mean hand-writing the contract: the OpenAPI document remains authoritative and checked.

Protobuf must not become a universal domain model. Generated fields, unknown-enum behavior, optionality, and wire compatibility are protocol concerns, not controller invariants or database layout.

## Proposed workspace

The exact filesystem location is an ADR because adding a top-level directory requires approval. `agent-rs/` is used below only to make the proposal concrete; `server-rs/` or an eventual in-place replacement of `server/` are alternatives.

```text
agent-rs/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── deny.toml
├── bins/
│   ├── kontainer-controller/
│   ├── kontainer-node-agent/
│   └── kontainer-cli/                 # optional, later
├── crates/
│   ├── kontainer-domain/
│   ├── kontainer-runtime/
│   ├── kontainer-controller-core/
│   ├── kontainer-node-core/
│   ├── kontainer-api-types/
│   ├── kontainer-stream-protocol/
│   ├── kontainer-agent-protocol/
│   ├── kontainer-client/
│   ├── kontainer-security/
│   ├── kontainer-docker/
│   ├── kontainer-storage/
│   ├── kontainer-transport-http/
│   ├── kontainer-transport-grpc/
│   ├── kontainer-stack/
│   └── kontainer-test-support/         # publish = false
├── proto/
│   ├── agent/v2/
│   └── public-stream/v2/
├── schemas/openapi/openapi.json
└── migrations/
    ├── control/sqlite/
    ├── control/postgres/
    └── metrics/sqlite/
```

This is a desired end state, not necessarily the first pull request. Early implementation can keep closely related crates together until a boundary earns independent compilation or reuse. Conversely, collapsing all modules into one crate would make it too easy for domain code to import framework, database, or generated SDK types. The minimum useful initial split is domain, controller core, node core, protocols/types, client, infrastructure adapters, and binaries.

### Crate responsibilities

| Crate | Owns | May depend on | Must not expose or depend on |
|---|---|---|---|
| `kontainer-domain` | IDs, validated value objects, resource snapshots, desired-state rules, operation state machine, stable error codes | Small foundational crates only; preferably `std` | Axum, Tonic, Bollard, SQLx/rusqlite, Tauri, generated wire types, persistence rows |
| `kontainer-runtime` | Clock/random ports, cancellation/task supervision contracts, build metadata, shared bounded-resource primitives, telemetry facade | Tokio, `tokio-util`, `tracing`, domain | Docker, DB, transports, application actions |
| `kontainer-controller-core` | Controller use cases/actions and their coarse ports, reconciliation policy, agent routing policy, operation orchestration | Domain, runtime | Axum/Tonic generated server types, Bollard, SQL rows |
| `kontainer-node-core` | Node-local actions and state machines for logs, exec, metrics, node capabilities | Domain, runtime | Tonic server types, Bollard models, persistence rows |
| `kontainer-api-types` | Stable public REST request/response DTOs and RFC 9457 extension fields | Serde and deliberately small public type dependencies | Domain behavior, DB/Docker/Tauri types |
| `kontainer-stream-protocol` | Versioned public WSS protobuf frames and public stream state transitions | Prost, small common wire support | Internal agent RPC, server core, Docker |
| `kontainer-agent-protocol` | Versioned private controller-agent protobuf messages, generated client/server traits, descriptors | Prost/Tonic | Public client API, domain behavior, storage |
| `kontainer-client` | Base URL policy, credential provider, REST/WSS/SSE transports, retry hints, public client errors | API types, stream protocol, reqwest/rustls and qualified stream crates | Controller/node core, internal agent protocol, Docker, SQL, Axum, Tauri |
| `kontainer-security` | Authentication adapters, authorization/policy integration, envelope/KMS integration, PKI helpers, secret newtypes | Domain/core ports, rustls, secrecy as qualified | HTTP handler policy, raw secret serialization, UI APIs |
| `kontainer-docker` | `EngineClient` adapter, generated Moby conversion, connection negotiation, narrow raw HTTP upgrade/framing escape hatch | Bollard and its aligned HTTP/TLS stack | Public/domain exposure of Bollard types |
| `kontainer-storage` | SQLite/Postgres repositories, migrations, lease/claim implementation, spool/metrics persistence | Chosen SQL adapter(s), domain/core ports | SQL row types outside the crate |
| `kontainer-transport-http` | Axum handlers/middleware, DTO conversion, Problem Details, OpenAPI generation, SSE/WSS upgrade | Controller core, API/stream types, Axum/Tower/Utoipa | Database or Docker calls directly |
| `kontainer-transport-grpc` | Tonic server/client adapters, deadlines, metadata, mTLS identity extraction, domain/protobuf conversion | Controller/node core ports, agent protocol, rustls/Tonic | Public REST models, persistence |
| `kontainer-stack` | Compose ingestion, canonical model, validation, dependency graph, plan and reconciliation policy | Controller core/domain; parser behind a private adapter | Parser/YAML syntax types in public actions |
| `kontainer-test-support` | Deterministic clocks, fake ports, fixtures, fake Engine server, test PKI, builders | Any internal crate needed by tests | Production binary dependencies |

`kontainer-runtime` must remain small. It is not a “common” dumping ground. A type belongs there only if it has a process-lifecycle or bounded-resource meaning shared by controller and node agent. Domain-specific helpers remain with their domain.

### Dependency direction

```text
controller binary ─┬─> HTTP transport ────────> controller core ─┬─> domain
                   ├─> gRPC transport/client ─> controller core ─┘
                   ├─> Docker adapter ─────────> controller ports
                   ├─> storage adapter ────────> controller ports
                   └─> security adapters ──────> controller ports
controller core ────────────────────────────────────────────────> runtime

node-agent binary ─┬─> gRPC transport/server ─> node core ───────> domain
                   ├─> Docker adapter ─────────> node ports
                   ├─> spool/metrics storage ──> node ports
                   └─> security adapters ──────> node ports
node core ──────────────────────────────────────────────────────> runtime

Tauri / GPUI / CLI ──> kontainer-client ──> api-types + stream-protocol
```

Arrows point toward dependencies. Core crates define ports; outer crates implement them. The binary constructs concrete adapters and passes trait objects or concrete handles inward. No core crate imports an outer adapter to “find” an implementation.

The controller is not a proxy to arbitrary Docker socket calls. Its Engine port consists of operations the Kontainer domain needs, with Kontainer-owned inputs and outputs. That keeps policy enforceable and makes the [Swarm resource contract](./swarm-resources.md) the capability boundary.

## Types and conversions

The architecture requires five representations where data crosses five different trust or evolution boundaries:

1. **Domain types** express validated invariants and use case semantics. They avoid general-purpose serialization derives.
2. **Public wire DTOs** express the documented `/api/v2` JSON contract and RFC 9457 errors.
3. **Public stream frames** express versioned WSS protobuf messages for logs and exec.
4. **Internal wire messages** express controller-agent protobuf and capability negotiation.
5. **Persistence records** express schema-compatible nullable columns, database encodings, and migration history.

A sixth representation, **desktop IPC DTOs**, may exist where Tauri commands need a deliberately smaller or native-oriented surface. Tauri DTOs can reuse a public DTO when semantics and versioning truly match, but similarity alone is not a reason to couple them.

Conversions are explicit `TryFrom`/`From` implementations or named mapper functions at the owning edge. Fallible conversion reports the offending field and a stable error code without leaking the raw secret or adapter error. The following shortcuts are prohibited:

- deriving public JSON serialization on core entities for convenience
- returning Bollard structs from an action
- returning SQLx/rusqlite rows from a repository
- accepting generated protobuf messages in domain policy
- importing server-core crates into `kontainer-client`
- using a single “resource model” for API, database, Docker Engine, and UI

This does add mapping code. That code is valuable: it locates compatibility decisions and lets internal models evolve without accidental wire or schema breaks.

## Binary composition and role separation

Each binary has one `main` that performs only composition and process lifecycle:

1. read bootstrap inputs and locate the configuration file;
2. parse and validate the role-specific config;
3. initialize the selected rustls provider before TLS clients/servers;
4. initialize redaction-aware tracing and panic reporting;
5. open storage and verify migrations/version compatibility;
6. build Docker, identity, RPC, and repository adapters;
7. build actions/services with explicit dependencies;
8. start listeners and owned subsystem tasks under a root cancellation token;
9. publish readiness only after required dependencies are usable;
10. drain and shut down in the order defined below.

There should be no runtime `AGENT_ROLE` branch that makes one universal binary behave as either role. Separate manifests or binary targets make the security surface, features, image contents, and operational intent auditable. Shared source remains in crates.

The controller binary includes public HTTP, manager-local Engine operations, shared control storage, controller-to-agent RPC, cluster membership, stack reconciliation, scheduled collection, and singleton work claims. Multiple healthy controllers serve concurrently. Only work that must be singleton is guarded by a durable lease/claim. The architecture intentionally does not recreate a global leader/follower execution split; the terminology in v1 describes deployment roles, while the [lifecycle](./agent-lifecycle.md), [metrics/storage](./metrics-storage.md), and [cluster](./cluster-communication.md) reports require active controller replicas with explicit singleton coordination.

The node binary includes private gRPC, node-local Engine access, bounded sessions, local metrics collection/spool, and health endpoints only. It does not compile in the public Axum API, OAuth/OIDC browser flow, Postgres driver, Utoipa, or Compose stack parser. Its private listener never becomes a second public API.

## Configuration architecture

Configuration is strict, versioned TOML with separate root schemas:

```text
ControllerConfig
  schema_version
  instance
  public_http
  agent_client
  docker
  control_store
  metrics_store
  identity
  tls
  limits
  telemetry

NodeAgentConfig
  schema_version
  node
  agent_server
  docker
  spool
  identity
  tls
  limits
  telemetry
```

Do not model this as `Config { role, every_possible_field: Option<_> }`. Role-specific schemas prevent silent irrelevant fields and impossible combinations. Common validated value types such as `ByteLimit`, `RequestDeadline`, `BindAddress`, and `CertificatePath` may be shared.

Serde is appropriate for raw config decoding, with `deny_unknown_fields` on configuration structs and explicit defaults only where the specification defines them. Parsing should produce a raw form first, aggregate validation diagnostics with TOML paths, resolve secret-file references, and then produce an immutable validated config snapshot. Environment variables are accepted only for an allowlisted bootstrap set such as the config path or an explicitly designed container secret pointer; application code reads validated config, not arbitrary environment variables.

Secrets use file references or a configured secret provider. Values do not implement `Debug`, `Display`, or general serialization. The [`secrecy` crate](https://github.com/iqlusioninc/crates/tree/main/secrecy) is useful for explicit exposure and wiping on drop, but it is not memory locking, swap prevention, crash-dump protection, or an authorization boundary. Project-specific secret newtypes still carry purpose and validation.

Hot reload, if enabled, follows parse → validate → prepare adapters → atomically swap snapshot. Failure retains the prior snapshot and emits a redacted diagnostic. Identity, listener, database, and TLS changes that cannot be swapped safely require an explicit restart. These rules must be defined field by field rather than inferred from Serde behavior.

## Error taxonomy

Library crates use typed errors, normally derived with `thiserror`. `anyhow` is acceptable only at a binary or CLI top-level where the error is being logged and the process is exiting; it must not cross an API, RPC, repository, or library boundary.

The application error shape should carry:

```text
DomainError
  code             stable machine-readable code
  class            semantic class
  retry            never | safe | after_delay | after_reconcile
  operation_id     optional durable operation reference
  target           optional safe resource identity
  public_context   allowlisted structured fields
  source           private adapter/source chain
```

Required classes are:

- invalid argument / failed precondition
- unauthenticated / permission denied
- not found
- conflict / optimistic concurrency failure
- unsupported capability
- dependency unavailable
- deadline exceeded
- resource exhausted / backpressure
- cancelled
- partial success
- uncertain outcome requiring reconciliation
- invariant/internal failure

Adapters translate their native errors once. Bollard errors do not leak past `kontainer-docker`; SQL errors do not leak past `kontainer-storage`; Tonic status does not leak into controller actions. Unknown adapter errors become a dependency or invariant failure with a private source chain and a stable public code.

The HTTP adapter maps the taxonomy to status codes and RFC 9457 Problem Details as specified by the [API report](./api-protocol.md). The gRPC adapter maps it to status plus structured details. `kontainer-client` maps both REST and stream terminal errors into one public client error family while preserving a request/operation ID and safe retry advice. Raw SQL, daemon, TLS, filesystem, token, and certificate material is never serialized.

“Timed out” is not always “failed.” If an Engine mutation may have reached the daemon before a connection or deadline failure, the result is `uncertain`, a durable operation remains reconcilable, and the API does not invite a blind retry. This distinction is central to the reliability and Swarm operation models.

## Async runtime, cancellation, and task ownership

Use one Tokio multi-thread runtime per process. Tokio is the ecosystem common denominator for Axum, Tonic, Bollard, and the likely public client stack, and its current project documents the runtime and asynchronous building blocks under the [MIT license](https://github.com/tokio-rs/tokio/blob/master/README.md). Do not nest runtimes or create per-request runtimes.

Runtime worker count, blocking-thread ceiling, queue sizes, byte budgets, and concurrency permits are explicit configuration with safe clamps. Defaults must be derived from the [performance budgets](./performance.md), not copied from library defaults without measurement.

Cancellation forms a tree:

```text
process root token
├── listener/subsystem child
│   ├── request/operation child
│   └── session child
└── shutdown cleanup deadline token
```

[`tokio_util::sync::CancellationToken`](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html) provides the needed child-token semantics. A project-owned `TaskSupervisor` owns `JoinSet` or `TaskTracker` handles and records task name, class, start time, cancellation token, and failure policy. Production code should not call bare `tokio::spawn` outside that supervisor or a small actor implementation whose join handle is immediately registered.

Tasks are classified:

- **essential:** failure makes the process unready or starts a controlled drain, such as the agent server, controller public listener, operation recovery loop, or required database watcher;
- **restartable subsystem:** failure is retried with bounded backoff and a circuit-breaker/readiness signal, such as Docker event observation;
- **session-scoped:** failure terminates that log/exec/client session with a typed result;
- **best-effort cleanup:** runs only under a fresh short cleanup deadline and never delays shutdown indefinitely.

Dropping a response future is not assumed to cancel a daemon mutation. The action and adapter must define whether cancellation stops admission, interrupts a request, marks an outcome uncertain, or queues reconciliation.

### Shutdown order

The shared lifecycle contract is:

1. mark unready and stop new admissions;
2. stop accepting new HTTP, RPC, exec, log, and operation work;
3. cancel root children and signal actors to close;
4. allow in-flight read operations and bounded sessions their configured drain interval;
5. stop mutation runners at a safe checkpoint; persist partial or uncertain outcomes;
6. flush audit, operation, metrics, and spool batches within fixed deadlines;
7. close WebSocket/SSE/gRPC streams with an explicit reason where possible;
8. await tracked tasks, report stragglers by task name, then abort only after the hard deadline;
9. close storage and runtime resources.

Cleanup after a session cancellation uses a fresh, short child deadline. Reusing an already-cancelled token would skip daemon exec cleanup or terminal operation persistence.

### Stateful owner loops

Exec, logs, membership/channel management, metrics batching, and reconciliation runners use single-owner state loops. Each loop:

- owns its mutable state and daemon/network handles;
- accepts a small command enum over a bounded channel;
- enforces both item and byte limits;
- coalesces replaceable commands such as terminal resize;
- never blocks waiting to send to itself;
- exposes health/backlog metrics;
- returns a typed terminal reason;
- is cancelled and joined by its parent.

For exec, one owner coordinates client input, daemon output, resize, close, audit, timeout, and cleanup. For logs, the owner preserves raw bytes, handles multiplex headers and daemon type-3 error frames, and applies the per-subscriber backpressure policy in the [logging report](./logging.md). This pattern is deliberately local; HTTP request handlers and pure reconciliation functions remain ordinary async/functions.

## Blocking and CPU-heavy work

Async code must not disguise blocking work. The rule is to acquire a bounded permit **before** submitting work to `spawn_blocking`, because a queued blocking closure cannot be reliably aborted after it begins. Every blocking job has an input-size limit, a purpose-specific semaphore, a deadline for the awaiting caller, and a metric for queue and execution time.

Likely blocking or CPU-heavy boundaries are:

- rusqlite calls, if rusqlite is chosen;
- SQLite backup/checkpoint operations not exposed asynchronously by the selected adapter;
- YAML/Compose parsing and canonicalization;
- compression and decompression;
- certificate/key parsing or expensive cryptographic operations;
- large diff, hashing, or archive operations.

If rusqlite is selected, access occurs through a dedicated bounded database worker/owner or tightly controlled `spawn_blocking`; an `Arc<Mutex<Connection>>` used directly from executor tasks is not acceptable. SQLx provides an async interface but SQLite work still ultimately crosses into C and must be measured for cancellation and pool behavior.

No task may hold a synchronous lock, application semaphore unrelated to admission, or database transaction across a Docker, gRPC, HTTP, KMS, or user-stream await. Read required state, commit a claim if needed, release the transaction, perform remote work, then persist/reconcile the outcome.

## Dynamic dispatch versus generics

Use dynamic dispatch at coarse runtime-selected I/O and test seams:

```text
Arc<dyn EngineClient>
Arc<dyn ControlRepository>
Arc<dyn AgentDirectory>
Arc<dyn AgentRpc>
Arc<dyn IdentityVerifier>
Arc<dyn AuditSink>
Arc<dyn Clock>
```

These are relatively infrequent calls dominated by I/O. Trait objects make the binary composition root explicit, support embedded versus HA storage selection, keep handler types manageable, and reduce monomorphization across a large workspace. Ports should be object-safe and return project-owned values. Boxed streams/futures are acceptable where required by object safety, after measuring allocations in frame-hot paths.

Use generics for pure algorithms, parsers, iterators, small serialization helpers, and performance-sensitive inner loops where static dispatch is useful and the type does not propagate through every transport handler. A generic reconciliation function parameterized by a small read-only view can be valuable; an Axum router parameterized over twelve repositories is not.

Avoid dynamic dispatch per log frame, metric sample, or resource row. The owner loop should use concrete internal types once adapters have been selected. Avoid generic repository abstractions that erase database semantics; SQLite leases and Postgres claims need common behavioral contracts but may require different implementation strategies and tests.

## Feature and release profiles

Cargo features are additive and may be unified across a dependency graph; they are not a safe substitute for runtime role isolation. Cargo documents both [feature behavior](https://doc.rust-lang.org/cargo/reference/features.html) and [resolver rules](https://doc.rust-lang.org/cargo/reference/resolver.html). Use resolver 3 with the chosen edition/toolchain, build each production binary separately with a fixed feature set, and record the resulting feature graph and artifact digest.

Features represent optional capabilities:

| Crate | Candidate features | Rule |
|---|---|---|
| `kontainer-docker` | `unix-socket`, `windows-pipe`, `remote-rustls`, `raw-upgrade` | Enable only transports the artifact supports. Do not accept Bollard's entire default feature surface blindly. |
| `kontainer-storage` | `sqlite`, `postgres` | Embedded controller/spool builds use SQLite; HA controller builds include Postgres. Test each shipped combination. |
| `kontainer-transport-http` | `legacy-v1`, `openapi-tooling` | v1 compatibility is temporary and off after the migration window. Schema generation need not be in the minimal runtime artifact. |
| telemetry adapter | `otlp` | Structured local tracing remains available without an exporter. |
| `kontainer-stack` | parser-specific private feature | Controller only; parser choice must not affect public types. |

Prefer `default-features = false` for infrastructure crates, then opt into reviewed features. “No defaults” is not dogma for tiny foundational crates; it is important for crates whose defaults select TLS, native libraries, extra protocols, or large proc-macro trees.

Do not build one binary with `controller` and `node` role flags. The node artifact must not accidentally acquire server-only dependencies through feature unification. CI builds, tests, scans, and measures the exact controller-embedded, controller-HA, node, and client profiles rather than an arbitrary feature powerset or only `--all-features`.

## Protocol and code generation policy

Source contracts are checked in:

- `proto/agent/v2/*.proto` for private controller-agent RPC;
- `proto/public-stream/v2/*.proto` for public WSS messages;
- `schemas/openapi/openapi.json` as the normalized public REST artifact;
- generated descriptors and compatibility baselines needed by tests.

Prost generates idiomatic Rust protobuf types and preserves numeric enum values; its build tooling requires `protoc`, as documented by the [Prost project](https://github.com/tokio-rs/prost). Tonic's current project is maintained under [`grpc/grpc-rust`](https://github.com/grpc/grpc-rust), and its ordinary generation path also depends on the protobuf toolchain. Builds must not depend on whatever `protoc` happens to be installed on a developer host.

The recommended direction, subject to the codegen spike, is:

- pin Buf and `protoc` in a reproducible tool container or tool manifest;
- expose a dedicated `xtask codegen` or equivalent explicit command;
- check generated Rust and descriptor outputs into the repository so ordinary/offline builds do not require `protoc`;
- have CI regenerate into a clean tree and fail on diffs;
- run descriptor-breaking checks for the supported compatibility window;
- identify generated files in lint/license tooling and review generator upgrades as dependency upgrades.

If checked-in Rust output proves too noisy, the alternative is hermetic vendored `protoc` plus `OUT_DIR` generation. Ambient system `protoc` is rejected either way.

Utoipa should derive/generate OpenAPI from transport-owned schema declarations, normalize the result, and compare it to the checked artifact. The [Utoipa project](https://github.com/juhaku/utoipa) supports OpenAPI 3.1 and framework integration while remaining usable outside handlers. OpenAPI 3.2 is deferred as the API report recommends. Schema generation is a validation path; runtime handlers and clients do not parse OpenAPI to operate.

Generated TypeScript and external SDKs consume the checked artifact. A generated Rust client runs as a black-box conformance client but does not force `kontainer-client` to expose generator-specific request builders.

## Crate recommendations and risk register

Versions below are candidates observed on 2026-08-19. The committed version is the one recorded in an ADR and `Cargo.lock` after its spike. Version freshness alone is not acceptance; MSRV, transitive duplication, maintainership, license files in the packaged crate, and the exact enabled features must be reviewed.

| Need | Candidate snapshot | Decision and boundary | Maintenance, license, and native risk | Alternative / rejection rationale |
|---|---:|---|---|---|
| Async runtime | Tokio 1.53.x; `tokio-util` 0.7.x | **Accept.** One runtime; minimum features; cancellation/task utilities in runtime crate. | Mature, broadly maintained, MIT. Runtime defaults still need load testing. | `async-std`/smol would fight the Axum/Tonic/Bollard ecosystem and require compatibility layers. |
| Public HTTP | Axum 0.8.x | **Accept for controller edge.** Handlers are thin adapters. | [Axum](https://docs.rs/axum/latest/axum/) is built on Tokio, Hyper, and Tower and is MIT. Watch shared Hyper/http-body versions. | Raw Hyper creates avoidable routing/extraction work. Actix/Rocket introduce another ecosystem/lifecycle without a requirement that justifies it. |
| Docker Engine | Bollard 0.21.x, initially exact-qualified | **Conditional accept behind `EngineClient`.** Generated models never escape; keep a narrow raw upgrade/framing adapter. | [Bollard](https://github.com/fussybeaver/bollard) is Apache-2.0, async over Tokio/Hyper, offers local pipe and rustls options, currently generates through Docker API 1.52 and negotiates versions. Smaller maintainer surface, generator/API lag, default features, TLS provider alignment, and stream framing are material risks. | Direct hand-written Docker API for all endpoints has high schema burden. Docker CLI is prohibited. A narrow raw HTTP path is accepted only for verified Bollard gaps. |
| Internal RPC | Tonic 0.14.x + Prost 0.14.x | **Accept.** Private mTLS gRPC only, messages isolated in protocol crate. | Tonic is MIT; governance recently moved to the official gRPC Rust repository and its main branch may contain breaking development. Prost is Apache-2.0 and currently documents an MSRV around Rust 1.85; current Tonic documentation raises the workspace floor further, approximately Rust 1.88. External `protoc` is a supply/build input. | Hand-written HTTP RPC loses mature streaming/status/interoperability. JSON RPC weakens schema evolution. Cap'n Proto/FlatBuffers do not match the agreed gRPC ecosystem. |
| Relational storage | SQLx 0.9.x for Postgres; rusqlite 0.40.x or SQLx for SQLite **pending spike** | **Accept SQLx for Postgres. Do not yet choose SQLite.** Prefer SQLx for both only if it meets backup/version/single-writer and performance requirements; otherwise use rusqlite for SQLite behind the same behavioral ports. | [SQLx](https://github.com/transact-rs/sqlx) is MIT/Apache-2.0, async, and supports pure-Rust Postgres transport; its SQLite support uses `libsqlite3-sys` C/unsafe and has bundled/unbundled choices. The repository/organization transition and new 0.9 line merit qualification. [rusqlite](https://github.com/rusqlite/rusqlite) is MIT, gives lower-level SQLite and backup control, but is blocking and also uses native SQLite. Shipping both increases audit and migration complexity. | Diesel adds another ORM/query model and blocking concerns. An embedded KV store does not satisfy SQL-backed lease/reporting requirements. Never put SQLite on NFS for HA. |
| Serialization/config | Serde 1.0.x plus qualified TOML parser | **Accept at edges only.** Public/internal adapters, config, and storage representations; no blanket domain derives. | [Serde](https://github.com/serde-rs/serde) is MIT/Apache-2.0 and deliberately separates data model from formats. It is pure Rust; proc macros affect compile time and format crates require their own review. | Manual JSON/TOML parsing creates compatibility and security risk with no benefit. |
| OpenAPI | Utoipa 5.5.x | **Accept in HTTP/tooling boundary after generator spike.** Generate OpenAPI 3.1, normalize, diff. | MIT/Apache-2.0 and pure Rust; proc macros and optional integrations expand the compile graph. Exact nullable/enum/flatten behavior needs golden tests. | A fully hand-written document drifts. Runtime-first generators that own handler types would couple the application to tooling. |
| Diagnostics | `tracing` 0.1.x + `tracing-subscriber` 0.3.x | **Accept.** Libraries emit structured spans/events; binaries install subscribers and redaction. | [Tracing](https://github.com/tokio-rs/tracing) is MIT and designed for structured async diagnostics. Exporters can add large dependency/TLS graphs. | Ad hoc logging loses span context and structured fields. Keep OTLP optional rather than adopting an observability SDK in every crate. |
| TLS | rustls stable 0.23.x | **Accept, provider pending.** One provider chosen process-wide; use across Tonic, HTTP client, and remote Docker where supported. Do not adopt the 0.24 prerelease line. | [rustls](https://github.com/rustls/rustls) supports TLS 1.2/1.3 and is Apache-2.0/ISC/MIT. `ring` is generally the simpler baseline; `aws-lc-rs` has different C/assembly/build, feature, post-quantum, and FIPS implications. Provider duplication is both binary and policy risk. | `native-tls`/OpenSSL introduces platform variability and native patching unless a compliance profile specifically requires it. |
| Middleware | Tower 0.5.x, selected `tower-http` 0.7.x modules | **Accept at transport/client edges.** Admission, concurrency, rate/load shedding, request tracing, and short request deadlines. | [Tower](https://github.com/tower-rs/tower) is MIT and modular. Broad feature sets or stacked retries/timeouts can make behavior opaque. | Custom middleware duplicates ecosystem work. Do not wrap a long-lived WS/exec stream in a blanket request timeout; use phase/session deadlines. |
| Secret memory wrapper | secrecy 0.10.x | **Accept as a helper, not a security claim.** Project purpose-specific newtypes wrap it. | MIT/Apache-2.0, safe-Rust oriented, explicit exposure and zeroization. Release cadence is slower/stable; verify transitive `zeroize`. It cannot prevent swap, core dumps, copies made by external libraries, or serialization after exposure. | A custom wrapper is possible but repeats subtle redaction/zeroization work and raises audit cost. OS secret stores/KMS solve different persistence problems. |
| Error derivation | thiserror 2.x | **Accept for libraries.** | MIT/Apache-2.0, proc-macro compile cost is small relative to clarity. | Manual `Error` implementations add noise. `anyhow` is top-level only. |
| Testing | proptest 1.x, Loom 0.7.x, nextest 0.9.x, cargo-fuzz 0.13.x, tempfile 3.x, insta 1.x, testcontainers 0.28.x, Criterion 0.8.x | **Accept selectively as dev/tool dependencies after policy review.** | [nextest](https://nexte.st/) improves test execution; [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) wraps libFuzzer; [Loom](https://docs.rs/loom/latest/loom/) explores small concurrency models. These candidates advertise MIT, Apache-2.0, or dual-permissive licensing, but packaged license files still require lock-graph review. Testcontainers adds an external Docker runtime dependency; libFuzzer/nightly is a tooling-only native/toolchain concern. Snapshots require human semantic review. | A mocking framework is not the default. Project-owned fakes and protocol servers better validate semantics. Criterion may be complemented by scenario load tools for end-to-end budgets. |

Supporting public-client candidates such as reqwest, tokio-tungstenite, and an SSE decoder must be selected in the HTTP/TLS unification spike. They are intentionally not pre-approved merely because they are common; proxy behavior, cancellation, native roots, rustls provider, HTTP versions, WASM irrelevance, and duplicate Hyper trees matter.

### Native dependency policy

Pure Rust is preferred but not a blanket requirement. Any native dependency must identify:

- why it is required and which feature enables it;
- supported targets, cross-compilation path, C/assembly compiler needs, and static/dynamic linkage;
- patch and vulnerability ownership;
- license and attribution obligations for the compiled artifact;
- reproducible build behavior on linux/amd64 and linux/arm64, plus supported desktop targets for the client;
- an integration test that exercises the linked capability.

Known native inputs are SQLite (`libsqlite3-sys`) and potentially the rustls crypto provider. The protobuf compiler is a build/code-generation tool even if generated sources are checked in. OpenSSL should not enter transitively through default TLS features. Compose parser transitive dependencies require separate review; the [stack report](./stack-deployment.md) notes that the provisional `compose_spec` choice carries an archived `serde_yaml` and MPL-related risk.

The SQLite build must contain an upstream fix for the WAL-reset defect identified by the [metrics/storage report](./metrics-storage.md), currently expressed there as SQLite 3.51.3 or an explicitly verified backport. “Use the operating system SQLite” is not sufficient unless every supported image proves the version and compile options. Bundling improves control but transfers patch and native-build ownership to Kontainer.

## Docker adapter details

`kontainer-docker` exposes capability-oriented ports, not the complete Moby API:

```text
EngineInventory
ServiceEngine
TaskEngine
NodeEngine
NetworkEngine
VolumeEngine
SecretConfigEngine
ImageEngine
ExecEngine
LogEngine
EventEngine
```

These names illustrate capability separation; they need not all be public traits if a smaller combination remains coherent. Inputs and outputs are Kontainer types that capture only needed fields plus forward-compatible raw extension data where explicitly justified.

A cloneable Bollard connection is shared per daemon as the [performance report](./performance.md) recommends. Connection negotiation and observed daemon API/capabilities are cached with bounded refresh. An action asks for an operation, not a new client.

Bollard remains conditional because the reports found protocol-sensitive gaps:

- generated support currently trails the newest daemon API, so every required endpoint and field needs conformance against the oldest and newest supported engines;
- exec `start_exec` raw-versus-multiplex detection can misclassify a raw TTY stream whose first byte is `0`, `1`, or `2`;
- logs must preserve partial headers/frames and daemon stream type `3` errors without UTF-8 corruption;
- upgraded connection cancellation, half-close, resize ordering, and resource release need tests.

The accepted escape hatch is a narrow, project-owned HTTP/1 upgrade/framing module for the affected endpoint(s), built on the already-selected Hyper/TLS stack. It is not permission to implement a second general Docker client. Its invariants, daemon fixtures, fuzz targets, and removal/upstream tracking are mandatory.

## Storage profiles and repository contracts

Storage is a deployment profile decision, not one universal default:

- **Embedded/v1-parity profile:** one controller, local control SQLite, local metrics SQLite, and node-local SQLite spool. It is deliberately not controller HA.
- **Production HA profile:** multiple active controllers, a shared transactional control store (Postgres is the leading candidate), durable leases/claims, plus the metrics architecture selected by the metrics report. SQLite remains appropriate for node-local spool and possibly explicitly local metrics partitions, never an NFS-shared control database.

Repositories express behavior, not generic CRUD:

```text
OperationStore::create_idempotent(...)
OperationStore::claim_next(...)
OperationStore::record_uncertain(...)
ControllerLeaseStore::try_acquire_or_renew(...)
SnapshotStore::replace_if_newer(...)
MetricBatchStore::append_idempotent(...)
SpoolStore::ack_through(...)
```

Behavioral contract tests run against every supported backend. They cover transaction isolation, fencing tokens, clock/expiry boundaries, idempotency, migration from the previous supported release, crash recovery, SQLite busy handling, Postgres failover, and duplicate metric batches.

The SQLite selection remains open. SQLx for both SQLite and Postgres reduces the number of query/migration APIs and keeps async call sites consistent. Rusqlite offers lower-level backup/checkpoint/version control but creates an explicit blocking worker boundary and a second persistence stack. The spike must choose one SQLite library; production should not carry both as casual alternatives.

## Public client architecture

`kontainer-client` is a native Rust library with no server-internal dependencies. Its stable surface is organized by capability:

```text
KontainerClient
  services()
  stacks()
  nodes()
  operations()
  metrics()
  events()
  logs()
  exec()
```

Construction accepts transport policy and a credential provider interface. A request obtains an authorization value just in time; the client never returns the token to React or logs it. Tauri supplies a provider backed by native session/secure storage. GPUI can supply the same native provider contract. A CLI can supply an OS store, file-descriptor/agent provider, or explicit short-lived process credential without introducing a Tauri dependency.

The crate owns:

- base URL normalization and API-major routing;
- request/operation ID propagation;
- strict TLS and custom-CA policy inputs;
- native `Authorization` headers for HTTP and WebSocket handshakes;
- safe idempotent retries only where the API declares them;
- SSE reconnection and cursor behavior;
- log/exec stream state machines and typed close reasons;
- conversion between DTOs and ergonomic client values where beneficial.

It does not own browser OAuth UI, Tauri commands/events, GPUI state, CLI rendering, controller discovery through Docker, database access, or internal node routing.

The reports contain an authentication tension that must be explicit: the early [logging report](./logging.md) discusses short-lived WebSocket tickets, while the later cross-cutting [API](./api-protocol.md) and [security](./security.md) reports prefer the native client sending an `Authorization` header during the WebSocket handshake. The architecture adopts native headers as the primary path. A ticket endpoint is a documented compatibility fallback only for a client stack proven unable to set the header; it must be audience-bound, single-use, short-lived, and never placed in a query string or log.

This client split lets the desktop evolve from Tauri/React to GPUI, or add a CLI, without moving server implementation crates into the application process. It also prevents a UI framework upgrade from becoming a server protocol change.

## Compatibility policy and unresolved report disagreements

Where specialist reports differ, the disagreement remains visible until an ADR resolves it:

1. **SQLite default versus HA store.** These are compatible only as named deployment profiles. SQLite is the strict embedded/v1-parity profile; it is not the answer for multiple controllers. Postgres/shared transactional storage is required before claiming controller HA.
2. **SQLx versus rusqlite.** SQLx is selected for Postgres. The SQLite implementation remains spike-gated; one library should be chosen based on bundled version control, online backup/checkpoint support, write-tail latency, cancellation, and cross-build behavior.
3. **WebSocket bearer versus ticket.** Native handshake authorization is primary. Tickets are fallback-only as described above.
4. **Compatibility unit.** The cluster report uses “current and previous minor” language, while API/open-decision material sometimes describes current/previous major or release compatibility. Package paths make `v2` explicit, but this document does **not** silently promise N-1 major compatibility. An ADR must define whether the guarantee is N/N-1 release, minor, or API major and how capability negotiation behaves.
5. **Bollard versus raw Engine code.** Bollard is the default adapter. A narrow raw path is allowed only where conformance proves a correctness gap, especially upgraded log/exec streams.
6. **Generated clients.** External/TypeScript clients are generated from OpenAPI. The in-repo Rust client is ergonomic and hand-written over shared public contract crates, with an independently generated client in CI. Specta/Tauri IPC generation remains deferred.
7. **Controller TLS termination.** Direct rustls termination and termination at a trusted ingress/proxy both appear operationally plausible. Identity headers are trusted only from an authenticated, configured proxy path; the deployment/security ADR must decide supported modes.
8. **rustls provider.** `ring` is the likely portability baseline; `aws-lc-rs` is a distinct compliance/performance/PQ profile. Select one provider per artifact after the spike.
9. **Protocol generated code.** Checking generated Rust improves offline reproducibility but creates review churn. Hermetic `OUT_DIR` generation reduces checked diffs but makes a toolchain part of every build. The codegen spike decides, with ambient `protoc` rejected in both cases.
10. **Controller placement in the repository.** A new `agent-rs/`, a temporary `server-rs/`, or eventual replacement of `server/` changes migration mechanics and top-level layout. This needs approval independently of the crate design.

## Unsafe Rust policy

Workspace code is safe Rust by default:

- `#![forbid(unsafe_code)]` in domain, core, protocol, public client, authentication/policy, and ordinary transport crates;
- `#![deny(unsafe_code)]` at the workspace lint level, tightened to `forbid` per crate where no exception is possible;
- no unsafe code in generated project templates unless the generator and output are explicitly reviewed;
- native dependencies are not described as “safe” merely because their public wrapper is safe.

Rust's official reference makes clear that [`unsafe` enables specific unchecked operations rather than disabling all compiler checks](https://doc.rust-lang.org/stable/std/keyword.unsafe.html). If an adapter cannot meet a measured requirement without project-owned unsafe code, it must be isolated in a tiny private crate/module with:

- a written invariant at every unsafe block;
- safe public API and no exposed raw pointers/lifetimes;
- a demonstrated need and rejected safe alternative;
- focused property/fuzz tests;
- Miri where applicable and sanitizer testing for FFI/memory behavior;
- a named security reviewer and ADR;
- an inventory entry so `rg 'unsafe'` and audit tooling have an expected result.

No performance intuition alone is enough to grant an exception.

## Lint, audit, license, and provenance policy

The workspace should pin an MSRV and Rust toolchain after dependency spikes; current Tonic requirements likely make Rust 1.88 the initial floor. Use edition 2024, resolver 3, a committed `Cargo.lock`, central `[workspace.dependencies]`, no wildcard versions, and explicit feature lists. Lockfile updates are reviewed like code.

CI policy:

- `cargo fmt --check` and Clippy with workspace-owned warnings (`unsafe_code`, ignored results where meaningful, unexpected cfgs, missing error handling, and project-specific deny rules);
- unit/doc/integration tests on the MSRV and pinned CI toolchain;
- exact production feature-profile builds, not only `--all-features`;
- [`cargo-deny`](https://embarkstudios.github.io/cargo-deny/) for license, advisory, source, duplicate, and banned-crate policy;
- [`cargo-audit` / RustSec](https://rustsec.org/) for advisory monitoring, with time-bounded documented exceptions;
- [`cargo-vet`](https://mozilla.github.io/cargo-vet/) for review/audit provenance of high-risk crates and selected transitive dependencies;
- SBOM and license-notice generation from the locked, target-specific graph;
- source restrictions: crates.io registry and explicitly approved Git dependencies pinned to immutable revisions; no unreviewed path/download scripts;
- `cargo tree -d` review for duplicate Tokio/Hyper/http/rustls/SQLite stacks;
- artifact scanning and reproducible-build evidence for release images.

License policy must inspect the license files shipped in each crate/source archive, not trust only badges or SPDX metadata. MIT, Apache-2.0, BSD, ISC, and Unicode-style permissive licenses can be allowlisted after counsel/project approval. MPL and other weak copyleft dependencies require file-boundary and distribution review rather than a blanket silent acceptance. GPL/AGPL, noncommercial, source-available, or unknown licenses are denied unless explicitly approved.

There is no clear first-party Rust workspace license at the repository root today; the existing `server/LICENSE` carries Goravel-specific MIT attribution. Before publishing crates, images, generated SDKs, or an SBOM that asserts a project license, the project must select and add the correct first-party license/notice. This is a release blocker, not something dependency tooling can infer. SQLite itself is [public domain](https://sqlite.org/copyright.html), but its bundled build and notices still need to be represented accurately.

## Compile-time and artifact-size control

The proposed crate count exists to enforce dependency direction, not to maximize crates. Compilation policy should include:

- heavy proc-macro/codegen crates (`sqlx-macros`, Utoipa, Prost/Tonic build tooling) only where needed;
- stack/Compose parsing controller-only;
- Postgres controller-HA-only;
- public API/Utoipa absent from node agent;
- test-support and generators `publish = false` and absent from production dependency graphs;
- no duplicate TLS/HTTP stacks introduced by default features;
- dynamic dispatch at coarse I/O boundaries to constrain monomorphization;
- checked/generated artifacts or hermetic codegen so ordinary incremental builds do not repeatedly run toolchains unnecessarily;
- separate release builds for controller and node rather than a universal all-feature artifact.

Set budgets during the spikes for clean build time, incremental build after a core change, link time, stripped binary size, and container size on supported architectures. Record `cargo tree -d`; use `cargo llvm-lines`, `cargo bloat`, or equivalent diagnostic tools only as development tooling when a budget regresses. A crate split that creates cycles, duplicate schemas, or expensive rebuild fan-out should be revisited.

## Test seams and fakes

Ports exist for deterministic behavior, not for mock-count assertions. Core tests use project-owned in-memory fakes for:

- clock and randomness/ID generation;
- Engine capabilities and operation outcomes, including unknown/partial results;
- controller and metrics repositories;
- agent membership and RPC routing;
- identity, authorization facts, KMS/envelope operations, and audit sink;
- event/snapshot source and operation progress sink.

The important seams and test levels are:

| Level | Purpose |
|---|---|
| Pure unit/property | Domain invariants, reconciliation plans, idempotency, pagination/cursor rules, config validation, state machines |
| Model/Loom | Small owner-loop, lease, cancellation, and channel-close interleavings; Loom is not used to simulate the whole service |
| Golden/contract | OpenAPI, protobuf descriptors, RFC 9457 mapping, serialized frames, database behavior, migrations |
| Fake Engine protocol | Exact HTTP routes, chunk splits, multiplex headers, raw TTY leading bytes, type-3 errors, upgrades, half-close, backpressure, cancellation |
| In-process transport | Axum router tests; Tonic over local/in-memory transport with test PKI; independent generated-client conformance |
| Storage integration | Same behavioral suite against bundled/unbundled candidate SQLite and supported Postgres; crash/backup/restore/failover |
| Docker integration | DinD for breadth, then real VM Swarm for daemon, overlay, node, secret/config, log, and exec truth |
| Chaos/load/upgrade | Network partitions, controller death, daemon restart, slow clients, queue saturation, N/N-1 compatibility, rolling upgrade |

The fake Engine is a project-owned Hyper/Unix-socket test server, not a broad mocking framework. It captures request bytes and can emit adversarial frame boundaries that a high-level fake cannot represent. Real-daemon tests remain essential because generated schema and connection-upgrade behavior cannot be proven by mocks.

Time-sensitive tests use paused Tokio time and a fake domain clock where wall-clock semantics matter. Cancellation tests assert that all owned tasks join and permits are returned. Storage tests inject crashes at claim/mutate/persist boundaries. Snapshot tests are reviewed semantically; accepting a new snapshot is a contract change.

## Required dependency spikes before commitment

Each spike produces a short decision note, reproducible fixture/code outside production paths, locked dependency graph, license evidence, measured results, and an ADR update. A spike is not complete with “hello world.”

### Spike 1 — HTTP/TLS graph unification

**Question:** Can Axum 0.8, Tower 0.5, Tonic 0.14, Bollard 0.21, the chosen reqwest/WebSocket/SSE stack, and rustls share compatible Tokio/Hyper/http/rustls/provider versions?

**Exercise:** Build a minimal controller and client graph with public HTTPS, mTLS gRPC, a local Docker socket, WebSocket authorization headers, SSE cancellation, proxy/custom CA configuration, and both linux/amd64 and linux/arm64 builds. Inspect duplicates and enabled features.

**Exit criterion:** one provider per artifact, no accidental OpenSSL/native-tls, understood duplicate HTTP crates, successful cancellation/handshake tests, acceptable clean/incremental build and image size. Blocks ADRs for Axum/Tonic/client/rustls/Bollard versions.

### Spike 2 — Bollard Engine and stream correctness

**Question:** Does Bollard safely cover every required Swarm resource and upgraded log/exec path across supported daemon API versions?

**Exercise:** Conformance matrix for service/task/node/network/volume/secret/config/image/system/event/job operations; runtime API negotiation; log headers split at every byte; type-3 frames; raw TTY starting with `0`, `1`, or `2`; exec resize/input/half-close/cancel; slow-reader byte budgets; daemon restart.

**Exit criterion:** documented covered endpoints/fields, exact required raw escape hatch, upstream issue/patch plan, no generated Bollard type in core/public API, and buffer ownership proven under load. Blocks Engine adapter commitment.

### Spike 3 — SQLite and Postgres adapter choice

**Question:** Can SQLx 0.9 implement SQLite control/metrics/spool requirements as safely and predictably as rusqlite while also serving Postgres, or is a dedicated rusqlite worker justified?

**Exercise:** Verify linked SQLite version/compile flags and required WAL fix; migrations; online backup/restore; checkpoints; busy timeout; one-writer batched metrics; spool acknowledgements; lease/claim semantics; cancellation; crash recovery; file permissions; Postgres failover; amd64/arm64 cross-build. Compare SQLx-only with SQLx+rusqlite.

**Exit criterion:** one selected SQLite library and linkage policy, measured p95/p99 write/query tails, proven backups/restores and lease contracts, documented blocking boundary, and no NFS SQLite mode. Blocks storage ADR and HA claims.

### Spike 4 — Protobuf and compatibility toolchain

**Question:** What is the reproducible generation and compatibility unit for internal and public-stream protobuf?

**Exercise:** Pin Buf/`protoc`; compare checked-in Rust versus hermetic `OUT_DIR`; generate descriptors; run breaking checks; exercise unknown fields/enums and N/N-1 candidates; build offline; verify generated-code license/provenance.

**Exit criterion:** one codegen path, one compatibility statement (release/minor/major), deterministic diff, offline build story, and CI commands. Blocks protocol ADR.

### Spike 5 — OpenAPI and client generation

**Question:** Can Utoipa express the exact 3.1 contract, and can generated TypeScript plus independent Rust clients interoperate without shaping domain code?

**Exercise:** Problem Details, tagged/untagged enums, null versus omitted, 64-bit IDs/timestamps, pagination, operation polling, authorization, and representative error responses. Normalize/diff the artifact and test two generators.

**Exit criterion:** stable artifact, explicit unsupported schema patterns, selected TS generator, generated Rust conformance path, compile-time budget, and handwritten `kontainer-client` mapping rules. Blocks public schema/tooling ADR.

### Spike 6 — rustls provider and PKI operations

**Question:** Is `ring` sufficient for all platforms, or does a separate `aws-lc-rs`/FIPS profile justify its build and audit cost?

**Exercise:** Tonic, reqwest, Bollard remote TLS, client/server mTLS, test CA, certificate rotation/reload, revocation approach, SPIFFE-like identities if retained, custom roots, arm64 cross-build, and provider duplication. Evaluate compliance requirements explicitly.

**Exit criterion:** one provider per release profile, supported cipher/protocol policy, cert reload behavior, build prerequisites, and compliance statement. Blocks TLS ADR.

### Spike 7 — Compose parser and stack boundary

**Question:** Can a maintained parser meet the Compose subset, source-span diagnostics, duplicate-key policy, canonicalization, and security requirements?

**Exercise:** Compare the provisional `compose_spec` route and alternatives; audit archived `serde_yaml` and MPL implications; fuzz aliases/anchors/merge keys/duplicates/large inputs; retain source paths/spans; measure controller compile cost.

**Exit criterion:** selected parser or justified project-owned subset, license approval, supported/unsupported field matrix, input limits, deterministic canonical model, and removal path if an upstream project stalls. Blocks stack parser commitment.

### Spike 8 — Runtime ownership and shutdown prototype

**Question:** Do the proposed `CancellationToken`/TaskSupervisor and local owner loops close every resource under failure?

**Exercise:** One representative exec session, log subscriber, metrics SQLite batch, Docker event watcher, and operation runner. Inject client disconnect, daemon hang, database stall, panic, SIGTERM, and hard shutdown deadline; count tasks, permits, connections, and terminal records.

**Exit criterion:** zero detached tasks, deterministic terminal states, bounded shutdown, fresh cleanup deadlines, classified essential failures, and measured queue/byte limits. Blocks runtime/lifecycle ADR.

### Spike 9 — Reusable native client

**Question:** Can one client crate serve Tauri, GPUI, and CLI without UI/runtime leakage?

**Exercise:** Credential-provider implementations for test/Tauri-style/CLI-style hosts; REST, SSE, WSS logs/exec; proxy and custom CA; token refresh race; native WebSocket authorization header; reconnect/cursor; cancellation and backpressure. Confirm no Tauri/Axum/server dependency in `cargo tree`.

**Exit criterion:** stable capability API, secret custody proof, typed stream closure/retry behavior, dependency graph, and one test host for each integration style. Blocks client ADR.

### Spike 10 — Supply chain, license, and build budgets

**Question:** Can the exact target graphs meet policy and release budgets?

**Exercise:** Run deny/audit/vet prototypes; inventory build scripts and native code; verify packaged license files; resolve the first-party license; produce SBOMs; measure clean/incremental builds, artifacts, containers, and duplicate crates for each release profile.

**Exit criterion:** approved allow/deny policy, no unexplained advisory or license, reviewed native/build scripts, project license decision, artifact SBOM, and agreed budgets. Blocks dependency freeze and release.

## Proposed architecture decision records

These ADRs should be short, owned, and linked to spike evidence:

1. **ADR-001 — Rust workspace location, edition, MSRV, resolver, and migration coexistence with Go v1**
2. **ADR-002 — Separate controller and node-agent binaries; active-controller and singleton-work model**
3. **ADR-003 — Dependency direction, crate boundaries, and domain/wire/persistence type separation**
4. **ADR-004 — Action-per-file plus selective ports/adapters; no service locator or mega-orchestrator**
5. **ADR-005 — Tokio task ownership, cancellation tree, actor-like owner loops, and shutdown order**
6. **ADR-006 — Public REST/WSS/SSE versus private Tonic protocol boundaries**
7. **ADR-007 — Protocol compatibility unit and protobuf code generation/provenance**
8. **ADR-008 — Axum/Tower public transport and middleware/deadline policy**
9. **ADR-009 — Bollard adoption, Docker API support window, and raw framing escape hatch**
10. **ADR-010 — Embedded SQLite and HA Postgres profiles; SQLx/rusqlite choice and SQLite linkage**
11. **ADR-011 — rustls provider, certificate lifecycle, ingress termination, and trust roots**
12. **ADR-012 — Strict role-specific configuration, environment allowlist, secret references, and reload**
13. **ADR-013 — Stable error taxonomy, uncertainty, RFC 9457, gRPC mapping, and retry contract**
14. **ADR-014 — Reusable `kontainer-client`, credential provider, and generated-client policy**
15. **ADR-015 — Compose parser/reconciler boundary and license/security decision**
16. **ADR-016 — Safe-Rust exception process and native dependency policy**
17. **ADR-017 — Cargo features, release profiles, compile/artifact budgets, and codegen placement**
18. **ADR-018 — Testing layers, fake Engine protocol, real Swarm matrix, and compatibility gates**
19. **ADR-019 — Supply-chain, advisory, license, SBOM, and first-party licensing policy**
20. **ADR-020 — Telemetry schema, redaction/cardinality rules, and optional OTLP exporter**

## Issue-ready implementation checklist

The checklist is ordered so unresolved infrastructure does not leak into core APIs.

### Foundation and decisions

- [ ] Approve the Rust workspace location without modifying unrelated v1 code.
- [ ] Resolve first-party repository/crate licensing and notice ownership.
- [ ] Run spikes 1, 3, 4, 6, and 10; record accepted versions, MSRV, toolchain, licenses, and exact features.
- [ ] Land ADR-001 through ADR-007 before protocol or persistence types spread.
- [ ] Define exact supported Linux architectures, desktop client targets, Docker Engine versions/API range, Postgres versions, and embedded SQLite profile.
- [ ] Create workspace lint policy, resolver/toolchain, locked dependencies, deny/audit/vet policy, and target-specific SBOM job.
- [ ] Add build budgets and exact controller/node/client feature-profile CI.

### Core boundaries

- [ ] Create pure domain IDs, validated values, snapshots, operation state machine, capability model, and stable error taxonomy without Serde/SQL/Bollard derives.
- [ ] Define coarse ports for Engine, control store, metrics/spool, agent RPC, clock/randomness, identity/KMS, audit, and events.
- [ ] Port one representative v1 action end to end to prove action-per-file organization and explicit dependencies.
- [ ] Add compile-boundary checks or dependency-policy tests preventing core imports of transport, storage, Bollard, and generated protocol crates.
- [ ] Add explicit mappers and golden tests for domain ↔ public DTO, domain ↔ protobuf, and domain ↔ persistence record.

### Runtime and configuration

- [ ] Implement root/child cancellation and tracked `TaskSupervisor`; forbid untracked production spawns by review/lint convention.
- [ ] Implement task classes, failure reporting, readiness effects, and bounded shutdown with named straggler diagnostics.
- [ ] Implement strict `ControllerConfig` and `NodeAgentConfig`, aggregated path-aware validation, env allowlist, immutable snapshots, and secret-file handling.
- [ ] Add bounded blocking pools/semaphores and metrics before adding SQLite backup, Compose parsing, or compression work.
- [ ] Run spike 8 and turn failure cases into permanent lifecycle tests.

### Protocols and transports

- [ ] Check in versioned public-stream and private-agent protobuf source and compatibility baselines.
- [ ] Add deterministic codegen command, generator pinning, regeneration diff, offline-build verification, and breaking checks.
- [ ] Define `/api/v2` DTOs and Problem Details; generate and check normalized OpenAPI 3.1.
- [ ] Implement thin Axum handlers and Tonic adapters that translate once into application actions/errors.
- [ ] Apply Tower admission/rate/concurrency/short-deadline layers; implement phase/session deadlines for upgraded streams.
- [ ] Decide direct TLS versus trusted ingress and test rejection of spoofed identity headers outside the trusted path.

### Docker and resource behavior

- [ ] Run Bollard spike 2 across the supported Engine matrix.
- [ ] Implement Kontainer-owned Engine types and prevent Bollard types from crossing the adapter crate.
- [ ] Share one negotiated cloneable Engine connection per daemon; instrument connection/concurrency/queue usage.
- [ ] Implement and fuzz only the proven raw log/exec upgrade gap; link upstream tracking and removal criteria.
- [ ] Add fake Engine byte-level tests, DinD tests, and real VM Swarm gates for every resource/action family.
- [ ] Preserve uncertainty and reconciliation semantics when mutation transport outcomes are ambiguous.

### Storage and operations

- [ ] Select SQLx-only or SQLx+rusqlite based on spike 3; do not ship both SQLite implementations.
- [ ] Pin/verify patched SQLite and compile options on every embedded target.
- [ ] Implement explicit control, metrics, and spool migrations plus previous-release upgrade/rollback fixtures.
- [ ] Run the same repository behavior contracts against SQLite and Postgres where both are supported.
- [ ] Implement durable idempotency, operation claims/fencing, uncertain outcomes, and crash recovery before exposing mutations.
- [ ] Prove online backup/restore, WAL/checkpoint/busy behavior, Postgres failover, and no shared-NFS SQLite path.

### Streams, sessions, and performance

- [ ] Implement exec and log single-owner loops with bounded item/byte queues, coalesced resize, typed terminal states, and joined cleanup.
- [ ] Implement metrics batch/spool and membership/channel owners with the same lifecycle contract.
- [ ] Define global/per-user/per-target admission, queue, byte, and session limits; expose low-cardinality utilization metrics.
- [ ] Verify no polling amplification: shared event observation, immutable snapshots, event-plus-reconcile, and bounded refresh.
- [ ] Run slow-client, cancellation, daemon-restart, controller-death, and shutdown load tests against performance budgets.

### Client reuse

- [ ] Run client spike 9 and approve the public capability API and transport crates.
- [ ] Implement `kontainer-client` with only public DTO/stream dependencies and a credential-provider port.
- [ ] Implement native Authorization-header WebSocket handshakes; add ticket fallback only if a supported stack proves it necessary.
- [ ] Add Tauri, GPUI test-host, and CLI test-host adapters without adding their frameworks to the client crate.
- [ ] Generate TypeScript/external SDKs from checked OpenAPI and run an independently generated Rust client in conformance CI.
- [ ] Confirm logs/exec preserve bytes and backpressure without crossing React/Tauri event layers unnecessarily.

### Security, quality, and release

- [ ] Apply `forbid(unsafe_code)` to core/auth/protocol/client crates and inventory exceptions/native unsafe.
- [ ] Add secret newtypes, redaction tests, structured tracing field allowlists, and high-cardinality controls.
- [ ] Select one rustls provider per artifact; test certificate rotation, custom roots, mTLS identity, and expiry/failure readiness.
- [ ] Run property/model/Loom/fuzz suites for parsers, frames, leases, owner loops, and cancellation.
- [ ] Run nextest unit/integration suites, real VM Swarm, chaos/load, migration, backup/restore, and N/N-1 gates.
- [ ] Review exact target lock graphs with deny/audit/vet, license files, build scripts, native code, SBOM, and artifact signatures.
- [ ] Re-measure clean/incremental compilation, binary/container size, idle/load resources, and duplicate dependency budgets before release.

## Acceptance criteria for the architecture

The architecture is ready to move from proposal to implementation when:

- all blocking disagreements have an ADR owner and decision date;
- dependency spikes have evidence rather than popularity-based selections;
- controller, node agent, and client dependency graphs demonstrate the intended isolation;
- public, internal, domain, and persistence models have explicit conversion ownership;
- every long-lived task has a parent, cancellation token, join path, and terminal-state policy;
- embedded and HA storage profiles are named honestly and tested independently;
- Engine protocol correctness is proven for logs/exec and the supported Swarm surface;
- native dependencies, unsafe code, licenses, generators, and build scripts are inventoried;
- Tauri, GPUI, and CLI can use the same client contract without a server or UI-framework dependency;
- the compatibility guarantee is precise enough to turn into automated breaking and rolling-upgrade tests.

Until those gates pass, crate/version choices in this document remain candidates. The durable decision is the boundary: Kontainer-owned domain and client contracts point inward, infrastructure stays replaceable at explicit ports, and stateful asynchronous work always has one owner and one lifecycle.

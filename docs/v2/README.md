# Kontainer Rust rewrite research

This directory holds the research that preceded the Rust rewrite. It keeps its historical `v2`
name only as a location. Nothing here is a task list.

## Precedence

1. [The approved greenfield reset](./approved-greenfield-reset.md) — the one approved architecture decision.
2. [Architecture decision records](../adr/README.md) — decisions made after the reset, numbered and never renumbered.
3. [The decision register](./decision-register.md) — the pre-reset audit; rows marked `[reset: …]` are superseded.
4. The domain reports below — design research where nothing above has decided.
5. [The feature ledger](./feature-ledger.md) and [v1 archaeology](./v1-parity.md) — product research, not compatibility requirements.

The current build plan lives in [`docs/build/`](../build/README.md). The issue-generating documents
(`github-tracking.md`, `phasing-review.md`, `completeness-review.md`, `foundation-spike-results.md`)
were removed on 2026-09-28 when GitHub issues stopped being the execution source of truth; git history keeps them.

## Domain reports

| Report | What it owns |
|---|---|
| [Rust architecture](./rust-architecture.md) | Workspace/crates, boundaries, actions, runtime model |
| [Agent lifecycle](./agent-lifecycle.md) | Processes, configuration, scheduling, health, shutdown, packaging |
| [API and protocol](./api-protocol.md) | HTTP/WSS/gRPC contracts, clients, types, limits |
| [Security](./security.md) | Bootstrap, identity, sessions, secrets, mTLS, audit |
| [Cluster communication](./cluster-communication.md) | Membership, identity, routing, Tonic services, HA |
| [Swarm resources](./swarm-resources.md) | Docker API approach, resource semantics, partial results |
| [Metrics and storage](./metrics-storage.md) | Collection, spool/push, schemas, queries, retention |
| [Logging](./logging.md) | Sources, Docker framing, WSS stream semantics, resume |
| [Exec](./exec.md) | Session state machine, node routing, I/O, exit, cleanup |
| [Stack deployment](./stack-deployment.md) | Compose subset, reconciler design (deferred feature) |
| [Performance](./performance.md) | Engine-load controls, snapshots, fan-out, budgets |
| [Observability](./observability.md) | Diagnostics, telemetry, health, SLOs, support bundles |
| [Reliability and testing](./reliability-testing.md) | Test pyramid, environments, gates, release evidence |
| [mTLS evidence for issue 58](./issue-58-mtls-evidence.md) | Foundation evidence |

Several reports predate the reset and still say `/api/v2`, SQLite, or SSE. Read them with the
precedence list above; the reset wins where they conflict.

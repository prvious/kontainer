# Rust foundation spike results

Status: bounded foundation gate proven; production implementation may begin

Last updated: 2026-09-07 UTC

This report tracks the implementation gate in
`approved-greenfield-reset.md`. A compile-only result is not treated as proof
of a real PostgreSQL, Docker Engine, mTLS, or architecture claim.

## Proven in the checked-in workspace

- Rust `1.98.0` with edition `2024` is pinned by `server/rust-toolchain.toml`.
- Loco `1.1.0`, SeaORM `2.0.2`, Tokio, Axum, Tonic `0.14.6`, rustls, Bollard
  `0.21.1`, Prost, and Utoipa compile in one Cargo workspace.
- The resolved graph has one version each of Axum, Hyper, HTTP, HTTP Body,
  rustls, Tokio, SQLx, SeaORM, Tonic, and Prost.
- `kontainer-controller` and `kontainer-node-agent` build independently on the
  current ARM64 macOS development host.
- The controller owns Loco HTTP and private Tonic server tasks under one
  cancellation boundary and drains both on shutdown.
- Both private Tonic servers require a certificate, private key, and client CA;
  production controller configuration cannot select a plaintext RPC mode.
- The internal wire package is `kontainer.agent.v1` and generates both client
  and server bindings from one checked-in protobuf source.
- Docker access is behind the Kontainer-owned `ContainerEngine` trait rather
  than exposed as Bollard types.
- The public HTTP skeleton starts at `/api/v1` and generates an OpenAPI path for
  `/api/v1/status`.
- Formatting, warnings-as-errors Clippy, workspace tests, and both independent
  host builds pass.
- Pull requests that touch the Rust server run formatting, workspace checks,
  warnings-as-errors Clippy, non-zero workspace tests, and independent
  controller/node-agent builds on the pinned toolchain.
- SeaORM builder migrations create and roll back the durable operation,
  idempotency, ordered event, and audit-intent schema against PostgreSQL. The
  admission transaction, rollback boundary, and `FOR UPDATE SKIP LOCKED`
  operation claim were exercised against PostgreSQL 17.11 on the current ARM64
  macOS host; CI repeats the integration cases against PostgreSQL 17.6 on
  Linux AMD64.

## Important dependency caveat

Loco `1.1.0` currently enables SQLx/SeaORM SQLite support inside its `with-db`
feature. Kontainer does not configure, test, document, or support SQLite; the
driver is unused compiled code. Removing that transitive driver would require
patching Loco or bypassing its database integration, which would cost the
convention-oriented model/migration workflow chosen for the controller.

## Completed foundation leaves

All five implementation PRs are merged. Their issues are closed.

| Issue | Merged PR | Proven outcome |
| --- | --- | --- |
| [#56](https://github.com/prvious/kontainer/issues/56) | [#66](https://github.com/prvious/kontainer/pull/66) | PostgreSQL migrations, atomic admission/idempotency/audit, row-locked claims, polling, and uncertain transitions |
| [#57](https://github.com/prvious/kontainer/issues/57) | [#64](https://github.com/prvious/kontainer/pull/64) | Typed Unix-socket Docker adapter, bounded negotiation/inspection, capabilities, and real-daemon error behavior |
| [#58](https://github.com/prvious/kontainer/issues/58) | [#65](https://github.com/prvious/kontainer/pull/65) | Real bidirectional mTLS, certificate identity rejection, cancellation, and joined HTTP/RPC ownership |
| [#59](https://github.com/prvious/kontainer/issues/59) | [#67](https://github.com/prvious/kontainer/pull/67) | Binary WebSocket framing, bounded admission/queues, backpressure, close acknowledgements, and task cleanup |
| [#60](https://github.com/prvious/kontainer/issues/60) | [#63](https://github.com/prvious/kontainer/pull/63) | Independently built controller/agent binaries executed on native Linux AMD64 and ARM64 |

The common bootstrap [#61](https://github.com/prvious/kontainer/issues/61) and
retired-automation cleanup [#68](https://github.com/prvious/kontainer/issues/68)
are also complete. This closes the bounded Foundation gate, not the wider
architecture, security, reliability, or product epics.

## Integrated evidence

The final integrated source is
[`3b58145a9c7b79f3783692c4b025911e95e60b10`](https://github.com/prvious/kontainer/commit/3b58145a9c7b79f3783692c4b025911e95e60b10).
Its `server/Cargo.lock` SHA-256 is
`5ff36546bbc1a50f60c59516de1aed0907a7e24fb3e4306cb46c728d8d8c88fe`.

- [Integrated workspace verification](https://github.com/prvious/kontainer/actions/runs/34069698529)
  passes formatting, all-target checking, warnings-as-errors Clippy, tests,
  and independent binary builds. Tests include seven actual PostgreSQL cases,
  nine real mTLS cases, 15 Engine unit tests, and 12 WebSocket loopback cases.
- [Real-Docker qualification](https://github.com/prvious/kontainer/actions/runs/34069526730)
  passes on Docker 27.5.1 and 29.7.2, negotiating API 1.47 and 1.53
  respectively. Both lanes exercise the stopped-daemon outcome. Docker
  27.5.1 remains a provisional evidence floor, not a complete 1.0 support claim.
- [Stream qualification](https://github.com/prvious/kontainer/actions/runs/34069526745)
  records binary fixtures, explicit limits and close reasons, slow-consumer
  behavior, equal started/completed task counts, and zero failed joins.
- [Native Linux qualification](https://github.com/prvious/kontainer/actions/runs/34069698530)
  runs `x86_64-unknown-linux-gnu` on `x86_64` and
  `aarch64-unknown-linux-gnu` on `aarch64`. Both lanes execute both release
  binaries, test protobuf runtime compatibility, and verify CLI/configuration
  behavior. Each uploaded binary is checked independently for ELF architecture.

The dedicated Docker and stream runs use the immediately preceding integrated
commit `0e4a2ad4fcb67b16ce5844832bc3fc0cf9a63da1`. Their source, workflows, and
lockfile are unchanged in the final commit; the final workspace run also
reruns their default test suites. Native artifacts and workspace evidence use
the final commit itself.

Native artifact SHA-256 values from that run:

| Target | Binary | SHA-256 |
| --- | --- | --- |
| Linux AMD64 | controller | `1e55d822ffb826272bbdf2c757e7db983bc303b0fd794a60e0d90ba9dc5e14f5` |
| Linux AMD64 | node agent | `a24ca516adb2d1712c6648e7c97a2abde67a8f221a367992ca803bfd3e219244` |
| Linux ARM64 | controller | `55287ee2fe3a4500079fd4bf5fb8c52f35c97e177a4ad18fac52efe7c78d8c1b` |
| Linux ARM64 | node agent | `d9bd69934101af060395c2b923893e300ad84f5702a1aa5c6c9931fb2949798e` |

Native artifacts have 14-day retention. These are qualification artifacts,
not signed stable releases or a promise of durable distribution.

## Next implementation gate

The next milestone is `0.1 — Secure skeleton`: one secure end-to-end
controller/node-agent capability through the native Rust client. Its first
unblocked work belongs to
[E02 / #42](https://github.com/prvious/kontainer/issues/42): validate static
configuration and TLS before exposure, then add truthful lifecycle/probes and
a tested shutdown deadline. The foundation currently drains HTTP handlers to
completion; it does not yet guarantee a bounded production drain.

Authentication/authorization/session work under
[E04 / #44](https://github.com/prvious/kontainer/issues/44) and the public
contract/native-client path under
[E03 / #43](https://github.com/prvious/kontainer/issues/43) follow as reviewable
vertical leaves. Durable operation progress remains polling; log/exec product
semantics, automated PKI lifecycle, full Swarm resources, metrics, HA, and
stable-release qualification are not implied by this foundation result.

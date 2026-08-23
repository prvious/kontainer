# Rust foundation spike results

Status: in progress  
Last updated: 2026-08-23

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

## Important dependency caveat

Loco `1.1.0` currently enables SQLx/SeaORM SQLite support inside its `with-db`
feature. Kontainer does not configure, test, document, or support SQLite; the
driver is unused compiled code. Removing that transitive driver would require
patching Loco or bypassing its database integration, which would cost the
convention-oriented model/migration workflow chosen for the controller.

## Still required before the foundation gate closes

1. Run builder-based migrations against a real PostgreSQL container and prove
   transactions plus the row/advisory locking primitive chosen for durable
   operations.
2. Exercise the Bollard adapter against real Docker Engines and record an API
   compatibility floor.
3. Run a real mutually authenticated controller-agent handshake, including
   certificate rejection and coordinated shutdown tests.
4. Add the binary WebSocket transport dependency to the real log/exec walking
   skeleton and run a framed echo/close test.
5. Produce Linux `amd64` and `arm64` artifacts, then collect native smoke
   evidence on both architectures. A macOS ARM64 build does not satisfy either
   Linux target.

The next implementation slice is therefore the PostgreSQL durable-operation
walking skeleton: migration, SeaORM entity, repository/action, transaction and
locking tests, then a minimal `/api/v1/operations/{id}` polling resource.

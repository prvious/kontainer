# Architecture decision records

One file per decision, numbered `NNNN-slug.md`, never renumbered, never rewritten once accepted.
A superseded ADR gets `Status: superseded by NNNN` and stays in place.

The implementing agent writes an ADR whenever it:

- deviates from a spec in `docs/build/` or from `docs/v2/approved-greenfield-reset.md`;
- adds or removes a dependency (`Cargo.toml`, `package.json`);
- changes a contract: HTTP route or DTO, protobuf message, Tauri IPC type, database table;
- picks between two reasonable routes a reviewer would want explained.

Keep each ADR under a page. Link the spec and the commit that implements it.

## Template

```markdown
# NNNN. Title in the imperative

- Status: proposed | accepted | superseded by NNNN
- Date: YYYY-MM-DD
- Spec: docs/build/NN-name.md (or "none")

## Context

Two to five sentences. What forced the decision, what constraints applied.

## Decision

What was decided, stated as a rule someone can check code against.

## Consequences

What becomes easier, what becomes harder, what must be revisited and when.
```

## Index

| ADR | Title | Status |
|-----|-------|--------|
| [0001](./0001-password-authentication-first.md) | Password authentication first, GitHub login later as an integration | accepted |
| [0002](./0002-single-user-scope.md) | Single-user scope: no roles, capabilities, or recent-auth gates | accepted |
| [0003](./0003-node-metrics-as-check-in-samples.md) | Node metrics are check-in samples in PostgreSQL, no spool or rollups | accepted |
| [0004](./0004-dev-tcp-docker-override.md) | Dev-only TCP Docker host override for agents and controller | accepted |
| [0005](./0005-external-service-emulators.md) | External services are emulated, not mocked, when their features arrive | accepted |
| [0006](./0006-desktop-streams-through-tauri-rust.md) | Log and exec streams terminate in Tauri Rust and reach React over Channels | accepted |
| [0007](./0007-frontend-libraries.md) | Frontend libraries: Recharts, TanStack Table, no schema library, fixed fonts | accepted |
| [0008](./0008-build-plan-replaces-issues.md) | The build plan in docs/build replaces GitHub issues for this phase | accepted |
| [0009](./0009-in-process-operation-worker.md) | Durable operations run on an in-process controller worker | accepted |
| [0010](./0010-deferred-features-and-placeholders.md) | Deferred features and how their placeholders render | accepted |

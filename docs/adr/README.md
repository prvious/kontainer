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
| [0011](./0011-remove-codemirror.md) | Remove CodeMirror with the v1 editors | accepted |
| [0012](./0012-dev-environment-conventions.md) | Development environment conventions | accepted |
| [0013](./0013-shell-and-component-kit.md) | Shell and component kit decisions | accepted |
| [0014](./0014-password-auth-and-connections.md) | Password authentication, throttling, and saved connections | accepted |
| [0015](./0015-membership-and-node-metrics.md) | Membership, node samples, and fan-out | accepted |
| [0016](./0016-operation-execution.md) | Operation execution, events schema, and the desktop operation model | accepted |
| [0017](./0017-cluster-containers.md) | Cluster container list, container actions, and the container DTO | accepted |
| [0018](./0018-cluster-images.md) | Cluster images, removal, and pulls | accepted |
| [0019](./0019-node-volumes.md) | Node-scoped volumes, create, and remove | accepted |
| [0020](./0020-networks-and-manager-engine.md) | Networks, the manager engine, and container networks | accepted |
| [0021](./0021-swarm-services.md) | Swarm services, convergence, and the quorum state | accepted |
| [0022](./0022-stacks-from-labels.md) | Stacks from service labels, and stack removal | accepted |
| [0023](./0023-nodes-joined-with-swarm.md) | Nodes joined with Swarm, and the cluster summary | accepted |
| [0024](./0024-overview-and-connectivity.md) | The Overview, controller reachability, and paused changes | accepted |
| [0025](./0025-log-streams.md) | Service log streams: protocol, transport, reconnects, and the viewer | accepted |
| [0026](./0026-exec-sessions.md) | Exec sessions: protocol, agent RPC, relay, and the terminal | accepted |
| [0027](./0027-staged-settings.md) | Staged Settings, the controller status, and switching agents | accepted |
| [0028](./0028-integrations-placeholders.md) | Integrations hub and GitHub page as placeholders, and a kit breadcrumb | accepted |
| [0029](./0029-first-run-setup-code-and-bounded-sign-in.md) | Require a setup code to register the owner, and bound unauthenticated sign-in work | accepted |
| [0030](./0030-exec-sessions-follow-the-session-family.md) | End exec shells with the session family, not the access credential, and audit under the crate target | accepted |
| [0031](./0031-manager-engine-checked-per-call.md) | Check the manager engine's Swarm role per call, not once at boot | accepted |
| [0032](./0032-validate-where-less-trusted-data-enters.md) | Validate where less-trusted data enters a more-trusted component | accepted |
| [0033](./0033-exec-abandoned-outcome.md) | Report a shell left running after its exec session closes as `abandoned` | accepted |

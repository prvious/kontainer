# 0038. Remove the desktop operation-events chain until a screen shows operation history

- Status: accepted
- Date: 2026-10-04
- Spec: docs/build/06-operations.md
- Amends: ADR 0016

## Context

ADR 0016 gave the desktop a full path to an operation's history: the native client's
`list_operation_events`, a `Controller` trait method with its forward and fake, a
`SessionManager::operation_events` view, the Tauri `list_operation_events` command, and the
`OperationEventView` TypeScript type. The PR 135 review found that no React code calls it. Operation
Status polls `get_operation` and reads only the snapshot, and no screen shows the history.

That left about 300 lines across four layers, with tests, that ran only in tests. The snapshot's
`lastEventSequence` field crossed the Tauri boundary only to serve as this chain's cursor.

## Decision

- **Removed:**
  - the native client's `list_operation_events`, `OperationEvent` and `MAX_EVENT_PAGE`;
  - the Tauri `list_operation_events` command, the `Controller` trait method, its forward and fake,
    `SessionManager::operation_events` and `OperationEventView`;
  - the TypeScript `OperationEventView`, and the events rows in the shared IPC fixture.
- **`lastEventSequence` leaves the desktop IPC view.** The Tauri `OperationView` and the TypeScript
  `OperationView` no longer carry it.
- **Kept:**
  - The controller's `GET /api/v1/operations/{id}/events` route and its DTOs, which spec 06
    requires.
  - The native client's wire DTO still mirrors `OperationResponse` field for field, including
    `lastEventSequence`, because it rejects unknown fields and the contract test compares its keys
    with the OpenAPI schema. The desktop decodes the field and drops it before IPC.
- **Re-adding it.** When a screen shows operation history, rebuild the chain from that screen's
  needs, cursor included.

## Consequences

- Operation tracking on the desktop is unchanged: `useOperation` still polls the snapshot.
- Nothing on the desktop reads an operation's event history. A history view needs the client
  call, the IPC command and the type added back first.

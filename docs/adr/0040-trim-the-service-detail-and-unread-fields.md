# 0040. Trim the service detail, unread fields, and the unsent log close

- Status: accepted (amended by 0043)
- Date: 2026-10-04
- Spec: docs/build/06-operations.md, docs/build/08-images.md, docs/build/10-networks.md,
  docs/build/11-services.md, docs/build/12-stacks.md, docs/build/15-logs.md
- Amends: ADR 0016, ADR 0021, ADR 0025

## Context

The PR 135 review found data that the build carries through every layer but nothing reads:

- **Service detail.** `GET /api/v1/services/{id}` returned the service and its 50 newest tasks. Its
  only caller, the replica tooltip, reads the newest task error and nothing else, so about 150
  lines of task parsing across the controller, native client, Tauri and TypeScript did nothing.
- **Response fields.** Several fields the specs list are parsed and checked in three or four
  layers, and no screen shows them.
- **Log stream protocol.** No client sends `LogsClose`, since the desktop ends a stream by closing
  the socket. The server's `LogLine.seq` is overwritten by the client, and the log, exec and native
  exec senders each number envelopes that nothing reads.
- **Operations.** `operations.started_at` is written in the same claim as `claimed_at`, with the
  same value.

The owner decided to delete these (decisions D4.6, D4.7 and D5; review items PT-65 and PT-67). The
migrations haven't shipped, so they are edited in place.

## Decision

- **The service detail is `{ latestError }`.**
  - `ServiceDetailResponse` keeps only `latestError`. `TaskResponse` and its schema are deleted.
  - The native client's `ServiceDetail`, the Tauri `ServiceDetailView` and the TypeScript
    `ServiceDetailView` follow. `TaskItem` and `TaskView` are deleted.
  - The controller's `get_service` action still reads the service and every task, because scale
    convergence polls through it. It no longer sorts them, since nothing needs newest first.
- **Unread response fields leave the public DTOs and every desktop layer** (native client, Tauri
  views, TypeScript types and shared fixtures):
  - `updateMessage` on services. The engine's `ServiceSummary` keeps it, because a failed restart
    or rollback quotes it in its failure message. `updateState` stays, since the STATUS column
    reads it.
  - `snapshotAt` on the stack list.
  - `dangling` on images. The desktop treats an image with no tags as dangling.
  - `internal` on networks. `ingress` was never in the DTO.

  The controller's `ImageRow` and `NetworkRow` drop `dangling`, `ingress` and `internal` too, since
  no server logic reads them: built-ins are matched by name. The agent RPC and the engine still
  carry them as far as the controller's RPC client.
- **`LogsClose` is removed.** Frame kind 4 and the name `FRAME_KIND_LOGS_CLOSE` are reserved, and
  the `LogsClose` message is deleted.
  - A client ends a log stream by closing the socket. The controller's log session stops the follow
    and writes the `logs.close` audit event when the transport drops it.
  - The desktop's `logs_close` Tauri command stays. It never sent the frame: it cancels the
    follower, which closes the socket.
- **No sequence numbers on log and exec frames.**
  - `LogLine.seq` (field 1) is removed and reserved. The native follower already numbers the lines
    it delivers, and the desktop's `seq` is that count.
  - `LogMessage::into_frame` and `ExecMessage::into_frame` take no sequence. Every log and exec
    envelope carries `sequence` 0, and the per-connection counters are gone.
  - The envelope keeps its `sequence` field as part of the shared framing.
- **`operations.started_at` is removed.** Migration `m20260928_000006` no longer adds the column,
  and the entity, the admit and the claim no longer write it. `OperationResponse.startedAt` maps
  from `claimed_at`, so the public field is unchanged.
- **Kept on purpose.** `ExecStarted.exec_id` goes into the exec audit record, and
  `check_in_interval_seconds` lets the controller change agent cadence without redeploying agents.

## Consequences

- Each removed field comes back with the screen that needs it.
- The native client rejects unknown response fields, so this desktop can't read a controller built
  before this change. Both ship together, before the first release.
- A frame of kind 4 is now an unknown kind, and the transport closes the connection. No client ever
  sent one.
- A development database that ran the old migration keeps a nullable `started_at` column that
  nothing reads. Recreating the database removes it.
- The images' `dangling` and the networks' `ingress` and `internal` still cross the agent RPC
  (`ImageSummary` field 7, `NetworkSummary` fields 10 and 11) and stop in the controller's RPC
  client. Removing them there, and reserving those tags, is a follow-up.

# 0025. Service log streams: protocol, transport, reconnects, and the viewer

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/15-logs.md

## Context

Spec 15 streams one service's logs from the manager, through the controller's binary
WebSocket, into Tauri Rust, and on to React over a Channel (ADR 0006). The shared transport
crate was built for request and answer: it awaits each inbound frame's handler before it reads
the next frame, and it had no connection-scoped state. Docker's service-log API needs `details`
to name each line's task and node, and Bollard's options don't have it.

## Decision

- **Protocol (additive).** `kontainer.stream.v1` gains frame kinds and one payload message
  for each:
  - Client to server: `LogsOpen { service_id, since_unix_ms?, tail? }` and `LogsClose`.
  - Server to client: `LogLine { seq, ts_unix_ms, stream, task_id, node_id, line, truncated }`,
    `LogGap { dropped }`, and `LogEnd { reason: service_removed | error, code, message }`.
  - The Rust API is `LogMessage::into_frame` / `from_frame`, and payload errors are their own
    type. The data-frame fixture is unchanged.
- **Transport.**
  - `StreamAdmission::upgrade_with` runs an application for that one connection. The logs
    route passes the session it authenticated.
  - `OutboundSender::closed()` lets a producer stop when the writer stops.
  - `OutboundSender::close(code)` closes after the frames already queued. That is how a
    `LogEnd` arrives before its close, and how 4401 is sent.
  - `NativeStreamClient::connect_with_bearer` sends `Authorization: Bearer` on the upgrade, and
    the header is marked sensitive.
- **Engine.**
  - `service_logs(service_id, { since, tail, follow })` reads the multiplexed stream.
  - A request modifier on a cloned client adds `details=1` to service-log requests only.
  - Each line is parsed as `<RFC 3339 ts> k=v,… <message>`, keeping milliseconds.
- **Controller.**
  - `GET /api/v1/ws/logs` sits behind the access middleware, so a missing or bad credential
    is a 401 before the upgrade. The middleware now also stores the validated session (family
    and access expiry) for the route.
  - Before upgrading, the route checks that the manager answers (3 s). If it doesn't, the
    answer is 503 `manager_unavailable`. The first live pause showed why: an upgrade that
    succeeds and then ends at once reset the client's backoff on every attempt.
  - The ninth connection of one session is 429 `limit_reached`.
  - One connection follows one service. A backlog of 2,000 lines sits in front of the
    transport's bounded queue. When the client falls behind, the oldest lines are dropped and
    one `LogGap` with the count goes out ahead of the lines that survived.
  - Every 5 s the stream checks that the manager still lists the service. Docker keeps
    following a removed service, as the dev cluster showed, so this check is what ends the
    stream with `service_removed`. A manager that doesn't answer ends it with
    `manager_unavailable`.
  - Expiry mid-stream closes with 4401. Lines are never stored. Opens and closes are logged
    to the `kontainer::audit` tracing target without content.
  - **Deviation:** lines are cut at 60 KiB, not 256 KiB. ADR 0006 keeps the transport's 64 KiB
    frame limit, and a line has to fit one frame.
- **Native client.**
  - `connect_logs` makes one authenticated upgrade and sends the open. It maps 401 to
    `Unauthorized`, so the caller refreshes as for any other call, and 429 to the new
    `LimitReached`.
  - `LogFollower` is pulled by the caller's task, so the crate still spawns nothing:
    - It reconnects after a drop or a transient end, backing off 1 s, doubling, up to 30 s.
      `retry_handle()` skips the wait.
    - It reopens with `since = last line − 1 s` and a 5,000-line tail, and drops repeats by
      time, task, and text.
    - It numbers lines across reconnects.
    - After a 4401 it reconnects at once, and the connector refreshes the credential.
  - A custom PEM trust bundle refuses WebSockets rather than falling back to the public roots.
  - The crate doc now says the stream protocol's protobuf is the one exception to "no protobuf
    dependency".
- **Tauri.**
  - `logs_open(serviceId, since?, tail, channel) → streamId`, `logs_close`, and
    `logs_retry_now`.
  - A `LogStreams` registry in Tauri state; logout and window close cancel every stream.
  - A stream task runs the follower and sends lines to the Channel in 25 ms batches (at most
    500 lines each). Other events flush the pending lines first.
  - Connects go through `SessionManager::authenticated`. An unreachable controller or manager
    is retried; any other error ends the stream with its code.
- **React.**
  - The Logs tabs live in a provider in the authenticated layout, so streams survive leaving
    `/logs`. The status bar counts open tabs.
  - Each tab's buffer is an external store read with `useSyncExternalStore`, capped by the
    scrollback setting (10,000 lines by default), and rendered with `@tanstack/react-virtual`.
  - Rows run oldest to newest and follow at the bottom, as the spec says. The mock lists
    newest first, which would not fit a follow that sits at the bottom.
  - The Services and Containers row menus open `/logs?service=`, which takes an id or a name.
- **Dependencies.**
  - `@tanstack/react-virtual` (pre-approved).
  - The controller and the native client depend on the workspace's stream protocol and
    transport crates. The native client adds `axum` and `async-trait` as dev-dependencies for
    a loopback test.
  - The Tauri crate's direct `tokio-tungstenite` dependency is still unused, as before.

## Consequences

- The dev cluster streamed two services side by side, with task and node ids, and showed live
  nginx lines.
  - Across a controller restart the followers backed off 1 s, then 2 s, and resumed without
    repeating a line.
  - With the manager paused they backed off 1 s, 2 s, then 4 s, and recovered on unpause.
  - Removing the followed service ended its stream `service_removed` within 5 s. The stack was
    then redeployed.
- A Logs tab holds up to 10,000 lines in memory per tab until it is closed.

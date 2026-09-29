# 0026. Exec sessions: protocol, agent RPC, relay, and the terminal

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/16-exec.md

## Context

Spec 16 opens an interactive shell in a container. The path runs from React to Tauri Rust over
a Channel, then over the controller's binary WebSocket (ADR 0006), then over a new
bidirectional gRPC stream to the node agent, and finally to `docker exec` with a TTY. Unlike
logs, bytes flow both ways at once, and each session holds a process on a node. So each hop
needs bounds, and every close has to report the exit code or say that it is unknown.

## Decision

- **Protocol (additive).** `kontainer.stream.v1` gains frame kinds 8–15, one payload message
  each:
  - Client to server: `ExecOpen { node_id, container_id, shell?, cols, rows }`,
    `ExecInput { bytes }`, `ExecResize { cols, rows }`, and `ExecClose`.
  - Server to client: `ExecStarted { exec_id, shell }`, `ExecOutput { bytes }`,
    `ExecExit { code?, reason }`, and `ExecError { code, message }`.
  - **Contract addition:** `ExecExit` carries a `reason`: `exited`, `idle_timeout`,
    `max_duration`, or `connection_lost`. The spec asks for "`ExecExit` with a reason" after
    the idle and duration limits, and the footer names it. `code` is absent whenever it isn't
    known and is never filled in as 0.
  - Sizes are 1–1000 in both directions. The Rust API is `ExecMessage::into_frame` /
    `from_frame`.
- **Agent RPC.**
  - `kontainer.agent.v1` gains `Exec(stream ExecClientFrame) returns (stream ExecServerFrame)`.
    The frames mirror the WebSocket messages as oneofs.
  - The first frame must be `open`. The call is for the controller identity only
    (`require_controller`), over the unchanged mTLS channel.
  - The controller's deadline for the call is the 8 hour maximum plus one minute.
- **Engine.**
  - `ContainerEngine` gains `exec_create(container, command, env)`, `exec_start(exec_id)`,
    `exec_resize`, and `exec_inspect`. `exec_start` returns an output stream of bytes and an
    input sink.
  - Bollard's attached exec is kept. The real-engine test reads `ok\r\n` back from a TTY shell
    byte for byte, so the raw HTTP/1 upgrade path from `docs/v2/exec.md` isn't needed.
  - Bollard types stay inside `local.rs`.
- **Node agent.**
  - The agent checks that the container is running, or answers `target_not_running`.
  - Without a shell from the client it runs `/bin/bash -l`. If that exits with 126 or 127
    within 500 ms, it runs `/bin/sh` instead. Output from the probe window is buffered, not
    lost. If neither shell starts, the answer is `shell_not_found`.
  - A shell the client names is used verbatim, with no fallback.
  - `TERM=xterm-256color` is set.
  - After the output ends, the exit code comes from `exec_inspect`, polled for up to 2 s.
  - `ExecClose` shuts stdin and waits the same 2 s.
- **Controller.**
  - `GET /api/v1/ws/exec` sits behind the access middleware, like logs. One connection carries
    one shell.
  - `ExecOpen` is validated. The node is resolved through membership, and the agent stream is
    opened. The relay then runs both ways:
    - Output goes out in 32 KiB frames, under the 64 KiB frame limit.
    - A client that stalls the bounded queue (64 frames, 256 KiB) for 10 s is dropped with
      close 1013.
    - 30 minutes without input or output ends the session `idle_timeout`, and 8 hours ends it
      `max_duration`.
    - When access expires, the controller sends `ExecError session_expired` and closes with
      4401.
  - A node that can't be reached, or whose engine fails, is `node_unavailable`.
  - Opens and closes go to `kontainer::audit` with the principal, session family, node, and
    container. Nothing typed or printed is logged.
  - **Deviation:** the fifth session of one access-session family is refused before the
    upgrade with 429 `limit_reached`, the way logs refuse their ninth. The spec lists
    `limit_reached` as an in-band `ExecError`. Refusing first avoids spending an upgrade and a
    relay task only to close them, and the native client maps 429 to the same code.
  - The per-family counters are now one `SessionSlots` type shared by logs (8) and exec (4).
- **Transport.** `NativeStreamClient::split()` gives a sender and a receiver that work at the
  same time, because a shell reads and writes concurrently. Close codes map to `closed`,
  `session_expired` (4401), `slow_consumer` (1013), `server_shutdown` (1001), or
  `connection_lost`.
- **Native client.**
  - `connect_exec` validates the request, then upgrades with the bearer. It maps 401 to
    `Unauthorized`, so the caller refreshes once, and 429 to `LimitReached`.
  - It returns an input sink and an event source. Input is cut into 32 KiB frames.
- **Tauri.**
  - The commands are `exec_open` (answers a session id and feeds a Channel), `exec_write`,
    `exec_resize`, and `exec_close`.
  - Output crosses as base64 of the raw bytes, as the spec asks.
  - An `ExecSessions` registry owns each session's task. Logout and window close end them all.
  - **Dependency:** `base64 = "0.22.1"` is now a direct dependency of `app/src-tauri`. The
    ponytail ladder: the spec requires base64 output. The same version is already in the
    Tauri lockfile (transitively), in the server workspace, and in the native client. The
    standard library has no encoder, and its use is one call.
- **React.**
  - `ExecSessionsProvider` sits in the authenticated layout, so shells survive leaving `/exec`.
    Each tab's `ExecBuffer` is an external store for state. It keeps up to 2 MiB of output so a
    terminal that mounts again replays what it missed.
  - The terminal is `ghostty-web`, fitted to its container. The first fit happens before the
    shell starts, so the provider remembers the size and sends it once the session is live.
    The page tests found this.
  - An ended session blocks input, hides the cursor, dims the output, and offers RECONNECT and
    CLOSE TAB.
  - A shell that couldn't start shows its code. `target_not_running` offers START CONTAINER,
    which runs the spec 07 start and reopens once it succeeds, and VIEW LOGS. `shell_not_found`
    offers a custom-shell field (G11).
  - Only connecting and active shells count toward the four, the status line, and the status
    bar.
  - Container row Exec actions open `/exec?node=&container=`. They are `mutates`, so they
    pause while the controller is unreachable.
  - The sidebar has no unbuilt items left, so its disabled-item path is gone.
  - The status line shows no PID or TTY, because the exec API doesn't expose them (G25).

## Consequences

- Nothing about a session is persisted. A controller restart ends every shell with close
  1001, and the tab says why.
- The agent RPC is additive. An older agent answers `Unimplemented`, which the controller
  reports as `node_unavailable`.
- An exit code of 137 after an outside `docker stop` is reported as an exit, not as a lost
  connection, which is what Docker says happened.

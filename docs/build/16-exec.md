# 16. Exec

## Goal

Interactive shells in containers: WebSocket from Tauri Rust to the controller, a bidirectional
gRPC stream from the controller to the node agent, `docker exec` with a TTY on the node, and a
tabbed terminal in the desktop with the session-ended and can't-start states.

Design: Exec `f8D6u` (light `SOPcG`); states `wXClq` (08.S1 session ended), `p1V0Po` (08.S2
can't start a shell, session limit); captions `dTm9p`, `RomOS`. Gap G11 (target picker, custom
shell). Decisions: ADR 0006. Research: `docs/v2/exec.md`.

## Server

### Protocol (`kontainer-stream-protocol`, additive)

- client → `ExecOpen { node_id, container_id, shell?, cols, rows }`, `ExecInput { bytes }`,
  `ExecResize { cols, rows }`, `ExecClose`;
- server → `ExecStarted { exec_id, shell }`, `ExecOutput { bytes }`, `ExecExit { code? }`
  (absent when unknown), `ExecError { code, message }` with codes `target_not_running`,
  `shell_not_found`, `node_unavailable`, `limit_reached`, `session_expired`.

### Controller

- Route `GET /api/v1/ws/exec`, same upgrade and bearer rules as spec 15. On `ExecOpen` resolve
  the node (spec 05 membership), open a bidirectional `Exec` stream to that agent (new RPC in
  `kontainer.agent.v1` mirroring the messages) and relay frames both ways within the transport's
  bounds: 64 KiB input frames, 256 KiB outbound queue, and a hard drop of the session when the
  client stalls for 10 s. Limits: four exec sessions per session credential; idle 30 minutes and
  8 hours maximum, then `ExecExit` with a reason. Audit open and close without content.

### Node agent

- Engine trait: `exec_create(container, command, tty, env)`, `exec_start(exec_id)` returning an
  input sink and an output stream of bytes, `exec_resize`, `exec_inspect` (exit code, running).
  Use Bollard's attached exec first; only if a real-engine test shows TTY bytes being altered,
  implement the raw HTTP/1 upgrade path from `docs/v2/exec.md` and record it in an ADR.
- Open sequence: inspect the container, `target_not_running` unless it is running; start
  `/bin/bash -l` when the client sent no shell; if the process exits within 500 ms with code 126
  or 127, retry with `/bin/sh`; if that also fails, `shell_not_found`. A client-provided shell is
  used verbatim with no fallback.
- Exit code from `exec_inspect` after the stream ends; never fabricate 0. Closing the client
  stream closes stdin, waits up to 2 s, and reports the exit.

## Desktop (Tauri)

- Native client `open_exec(...)`; commands `exec_open(nodeId, containerId, shell?, cols, rows,
  channel) -> sessionId`, `exec_write(sessionId, data)`, `exec_resize(sessionId, cols, rows)`,
  `exec_close(sessionId)`. Channel events: `started { shell }`, `output { data }` (base64 of the
  raw bytes; measure before optimising), `exit { code }`, `error { code, message }`,
  `closed { reason }`. Logout and window close terminate every session.

## Desktop (React)

- Route `/exec?node=&container=`; when both are present a tab opens on load.
- Header: `EXEC`, `// interactive terminal sessions`.
- Tabs (`osf25`): container name, dot (success active, destructive ended), close, `+`. The `+`
  opens the target picker (G11): a Command dialog of running containers grouped by node. With
  four sessions open, `+` is disabled with a Tooltip Rich saying so and how to free a slot.
- Terminal: `ghostty-web` (installed) bound to the channel; the pre-spec-01 wiring is in git
  history (`git show <sha>:app/src/components/terminal-session-view.tsx`) as a reference for the
  library API only. Font size, scrollback, and default shell come from `AppSettings`. Resize on
  container size changes sends `exec_resize`.
- Connection status line under the terminal: `Connected to <container>` and `N active sessions`.
  PID and TTY are not available from the exec API and are not shown (design-gaps G25).
- Session ended (08.S1): the tab turns red, output stays readable but dimmed, the cursor
  disappears, input is blocked, a divider marks where the connection ended, and an inline block
  offers RECONNECT (a new shell in the same container) or CLOSE TAB. The footer names the close
  reason (`exit 0`, `connection lost`, `idle timeout`).
- Can't start (08.S2): the new tab shows an ErrorState with the code. `target_not_running` offers
  START CONTAINER (runs the spec 07 start operation, then retries) and VIEW LOGS;
  `shell_not_found` offers a custom shell text field with RETRY.
- Status bar `TERMINALS` counts open sessions.

## Tests

- Agent with a fake engine: shell fallback, not running, exit code propagation, resize.
- Controller relay: bounds, limit of four, idle timeout, session expiry closes with the code.
- Native client and Tauri: event mapping, cancellation on logout.
- React: tabs and limit, ended state blocks input, error states and their actions, resize calls.
- Real cluster: open a shell in a seed container, run `echo ok`, see the output; `exit` reports
  code 0; stop the container from another tab and see the ended state.

## Done when

- A shell in a dev container works end to end with resize and a clean exit, and the two error
  states render for a stopped container and a shell-less image (`busybox` with `sh` removed is not
  needed: assert with a custom shell path that does not exist); all checks pass; `PROGRESS.md`
  updated; ADRs for the protocol and the agent RPC.

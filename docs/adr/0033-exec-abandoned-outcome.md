# 0033. Report a shell left running after its exec session closes as `abandoned`

- Status: accepted
- Date: 2026-10-04
- Spec: docs/build/16-exec.md
- Amends: ADR 0026

## Context

ADR 0026 has the agent shut the shell's stdin on `ExecClose`, wait up to 2 s, and report the exit
code from `exec_inspect`. The controller relays every agent exit as `exited`.

The real-engine test `dev_cluster_exec_shell_outlives_its_closed_input` shows what Docker does with
a TTY: closing stdin ends the attach's output, but the terminal never sees end of file. The shell
is still running 2 s later, and it stays running. So the app said `exited · exit code unknown` for a
process that kept running in the container.

## Decision

- **Agent RPC (additive).** `ExecExitEvent` gains `bool still_running = 2`. The agent sets it when
  `exec_inspect` still reports the process running after the 2 s close grace. `code` is then absent.
- **The agent doesn't stop the process.** Killing it (for example an env tag plus `HUP`) waits for
  an owner policy.
- **Stream protocol.** `ExecExit.reason` gains `abandoned`: the session closed while the shell was
  still running, and it was left running. `code` is absent. `reason` was always an open string, so
  this is additive. The desktop already renders it as "the shell may still be running in the
  container".
- **Controller.**
  - It relays `still_running` as `ExecExit { code: None, reason: "abandoned" }`.
  - It writes `exec.abandoned` to the crate's audit target with the principal, session family,
    node, container and exec id, so an operator can find the process.

## Consequences

- Mixed versions keep today's answer. An older agent never sets the field, so a newer controller
  still says `exited`. An older controller ignores the field and says `exited`.
- Only closes the agent answers are reported. When the controller ends a session itself (idle
  timeout, the 8 hour limit, access expiry, a stalled client) or the WebSocket drops, it doesn't
  wait for the agent. Those shells are left running too, with no `exec.abandoned` record; the
  `exec.close` audit event still marks the end.
- Shells pile up in long-lived containers until a kill policy exists.

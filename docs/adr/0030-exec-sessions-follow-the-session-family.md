# 0030. End exec shells with the session family, not the access credential, and audit under the crate target

- Status: accepted
- Date: 2026-10-04
- Spec: docs/build/16-exec.md
- Amends: ADR 0026

## Context

ADR 0026 closed an exec session with `session_expired` / 4401 when the access credential presented at
upgrade expired. Access credentials live five minutes and exec never reconnects, so every shell died
within five minutes and the 30-minute idle and 8-hour limits could never apply. Separately, the audit
records went to the tracing target `kontainer::audit`, which Loco's default filter drops, because it
only enables its own whitelist plus `<crate name>=<level>`. Without `RUST_LOG`, no audit record was
written at all.

## Decision

- When the access credential presented at upgrade expires, the relay re-reads the session family
  (`session_family_active`). It checks that the family isn't revoked, is inside its absolute
  lifetime, and belongs to an active principal.
  - **Family still live:** the timer is re-armed for one more access lifetime and the shell keeps
    running.
  - **Otherwise:** the relay sends `ExecError session_expired` and closes with 4401, as before.
  - **Read fails:** the relay ends the shell (fail closed).
- Logout, revocation and a password reset therefore end shells within one access lifetime. The idle
  (30 min) and maximum (8 h) limits now apply as specified.
- Audit records use one target, `AUDIT_TARGET = "kontainer_controller::audit"`, which Loco's default
  filter enables. The records are `exec.open`, `exec.close`, `logs.open` and `logs.close`, plus a new
  `exec.started` record. `exec.started` carries the Docker `exec_id` and the shell, which gives the
  protocol's `ExecStarted.exec_id` a reader.

## Consequences

- A revoked session can keep a shell for up to one access lifetime (five minutes) after revocation.
- Log streams still close at access expiry and reconnect; the same family check could replace that
  later if the forced reconnect every five minutes becomes a problem.
- Operators who set `override_filter` must keep `kontainer_controller=info` (or the audit target) in
  it.

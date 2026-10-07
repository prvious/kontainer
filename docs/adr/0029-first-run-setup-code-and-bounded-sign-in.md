# 0029. Require a setup code to register the owner, and bound unauthenticated sign-in work

- Status: accepted
- Date: 2026-10-04
- Spec: docs/build/04-auth-and-connect.md
- Amends: ADR 0001 (first-come registration), ADR 0014 (login throttling)

## Context

The PR 135 review found that registration was first-come and ungated on a controller that binds
`0.0.0.0` by default, so any client that reached a fresh controller first became Owner, and
`GET /auth/bootstrap` told scanners when that window was open. It also found that the login
throttle checked the lock and counted the failure in separate steps with Argon2id in between, so
parallel requests got far more than ten guesses, and that unauthenticated Argon2id work (about
19 MiB each) had no bound at all. A sign-in that verified the old password could also mint a session
after a password reset that was meant to sign everyone out.

## Decision

- **Setup code.** At startup the controller generates a 128-bit code (32 lowercase hex characters
  from the ring CSPRNG) and keeps it in memory. While no owner exists, `serve` prints it once to
  standard output, not to the structured log. `POST /api/v1/auth/register` requires `setupCode`;
  a blank one is `fields.setupCode: "empty"` and a wrong one `"invalid"`, compared in constant
  time, ignoring surrounding whitespace and case. The owner-exists check (409) and the code check
  both run before any hashing. A restart prints a new code. The Create owner screen gains one field
  (design gap G27).
- **Bounded Argon2id.** Every hash and verify needs a permit from one process-wide semaphore sized to
  the CPU count. The permit moves into the blocking task, so it is released when the work ends,
  not when a cancelled request stops waiting. A request that can't get a permit within 2 seconds
  is answered 429 `{ "error": "busy" }` and is not counted against the email.
- **Atomic throttle.** Sign-in takes the permit, then reserves the attempt under the throttle
  mutex (`begin_attempt`): it refuses while locked, otherwise counts the attempt as a failure
  before the password is verified. Success clears the entry. At most ten attempts per window
  reach Argon2id, however many arrive at once.
- **Reset-safe sessions.** The session is created in a transaction that reads the password
  credential `FOR SHARE` and gives up if its `updated_at` is no longer the one that was verified.
  A reset that commits first fails the sign-in; one that commits after revokes the new session.

## Consequences

- An operator needs the controller's output to create the owner. `docker logs` or the terminal
  shows it; nothing else changes after the owner exists.
- The throttle map is still bounded only by its expiry pruning. Rotating emails can't exceed the
  Argon2id permits, so its growth is limited to the permitted rate over fifteen minutes.
- A burst beyond the CPU count waits up to two seconds and then gets `busy`; the desktop shows it as
  the controller being temporarily unavailable.

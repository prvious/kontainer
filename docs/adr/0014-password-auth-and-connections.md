# 0014. Password authentication, throttling, and saved connections

- Status: accepted (amended by 0029)
- Date: 2026-09-28
- Spec: docs/build/04-auth-and-connect.md

## Context

Spec 04 adds owner registration and email/password sign-in (ADR 0001), a sign-in throttle, an
operator reset task, and the desktop Connect card with saved connections. It changes the HTTP,
native-client, and Tauri IPC contracts and the desktop metadata document.

## Decision

- **Schema.** `principals` gains nullable `email` (unique on `lower(email)`, built with the SeaQuery
  expression index) and `display_name`; `password_credentials(principal_id pk → principals,
  password_hash, created_at, updated_at)` holds one Argon2id PHC hash (Loco's `hash_password`).
  Hashing and verification run on `spawn_blocking`. An unknown email verifies against a fixed
  equalizer hash so it costs the same work as a wrong password.
- **Throttle deviation.** The spec suggested Loco's in-memory cache. Loco's default cache is the null
  driver unless configured, and its get-then-insert counter can undercount concurrent failures. The
  throttle is instead a mutex-guarded map in controller memory (`auth/throttle.rs`): ten failures in
  fifteen minutes lock the email for fifteen minutes; success clears it. Because the reset task runs
  in a separate process, sign-in passes the credential's `updated_at` and a lock older than the
  latest reset is discarded; that is how `auth:reset-password` unlocks an email.
- **Inactive owner.** A correct password for a non-active principal is reported as
  `invalid_credentials` and counted, so the response never confirms the password.
- **Routes.** `GET /auth/bootstrap`, `POST /auth/register` (201 / 409 `owner_exists` / 400
  `validation_failed` with closed per-field codes `invalid | empty | too_short | too_long`),
  `POST /auth/password/login` (200 / 401 `invalid_credentials` + `attemptsRemaining` / 429 `locked` +
  `retryAfterSeconds` and `Retry-After`), `GET /auth/session`. The session body carries `email` and
  `displayName` instead of `githubLogin`. The two `show` operations are now `status` and
  `operation`.
- **Reset task.** `task auth:reset-password email:<email>`: Loco's task CLI takes `key:value`, not
  the `key=value` the spec wrote. The new password is read from stdin; every session family is
  revoked with reason `password_reset`.
- **Native client.** `NativeClientError` stays closed and `Copy`, gaining `InvalidCredentials`,
  `LoginLocked`, `OwnerExists`, and `ValidationFailed { fields }`. New calls return
  `Traced<NativeClientError>`, which adds the controller's sanitized `x-request-id` (≤ 64 plain
  characters) so the desktop can show it. `probe_ready` classifies failures as dns / tcp / tls /
  http / timeout by walking the transport cause chain (rustls errors are downcast; resolver failures
  are recognized by text).
- **Tauri IPC.** `session_bootstrap`, `session_login(origin, email, password, label?)`,
  `session_register`, `session_connections`, `session_forget_connection`, `connection_probe`,
  `controller_health`; `session_logout(summary?)`. Removed: `session_cancel_login`,
  `session_login_hint`, the browser opener, and the GitHub-only error codes (`browser_open_failed`,
  `login_cancelled`, `login_expired`, `login_denied`, `provider_unavailable`). Sign-in failures
  serialize as `LoginFailure { code, attemptsRemaining?, retryAfterSeconds?, fields?, requestId? }`.
  The password is a command argument only, wrapped in `Zeroizing` and dropped after the request.
- **Metadata document v2.** `SessionView` and the stored summary carry `email` and `displayName`.
  `session.json` keeps its login `hint` and gains `connections` (at most ten, most recent first,
  with an optional `{ nodes, services }` summary cached at sign-out). A v1 document reads as corrupt
  and startup replaces it, as it already did for any schema drift.
- **Session expired.** A rejected refresh sets `expired` in the auth context instead of clearing the
  session: the page stays mounted and `inert`, controller queries pause (`useQueriesPaused`), and
  signing in again to the same profile keeps the page's cached data.

## Consequences

- The login screen, switcher, and settings share one saved-connection list.
- A second controller replica would need a shared throttle store; revisit with ADR 0009's
  single-controller assumption.

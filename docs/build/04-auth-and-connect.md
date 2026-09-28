# 04. Auth and connect

## Goal

Email and password authentication on the controller, owner registration on first run, saved
connections and the cluster switcher on the desktop, and the session-expired and unreachable states.
GitHub login stays compiled and disabled. After this spec the owner can create their account, sign
in, restart the app and stay signed in, sign out, and recover from an expired session.

Design: Connect · Sign in `wwb6d` (light `HTrey`), Connect · Create owner `wLzP9` (light
`T0KgTd`), Cluster Switcher `syxFU` and the dropdown in Foundations OVERLAY `23tdn`, states
`PheRG` (02.S2 agent unreachable), `W8OmXO` (02.S7 invalid credentials), `ME9dW` (02.S5 session
expired), `MBhvp` (02.S6 switcher offline & failed). Read every caption (`U59AaU`, `J7Hnlc`,
`z202PW`, `TmVBR`). The GitHub OAuth `hO63m` and App Setup `JRxs4` screens are deferred.
Decisions: ADR 0001, ADR 0002.

## Server

### Schema (one builder migration)

- `principals`: add `email` (text, nullable, unique index on `lower(email)`) and `display_name`
  (text, nullable).
- `password_credentials`: `principal_id` uuid primary key referencing `principals`, `password_hash`
  text, `created_at`, `updated_at`.

### Actions (`server/src/auth/actions/`, one file each)

- `register_owner`: in one transaction, if `bootstrap_state` row 1 has no owner: insert an active
  human principal with email and display name, insert the Argon2id hash
  (`loco_rs::hash::hash_password`), claim the bootstrap row, then
  `create_session::create_in_transaction`. If an owner exists: `OwnerExists`. Reuse the claim code
  path from `redeem_github_login` by extracting the shared claim into its own function rather than
  copying it.
- `password_login`: look up the principal by lowercased email; verify with
  `loco_rs::hash::verify_password`; on unknown email verify against a fixed dummy hash so timing
  matches; on success `create_session::execute`; on failure record the attempt in the throttle.
  Errors: `InvalidCredentials` (both unknown email and wrong password), `Locked { retry_after }`,
  `PrincipalInactive`.
- Throttle: per lowercased email, Loco's in-memory cache (`cache_inmem` is enabled) holds a counter
  with a 15-minute TTL; ten failures lock the email for 15 minutes. Successful login clears it.
  Return the remaining attempts so the desktop can show "7 attempts left".
- `reset_password`: rehash and revoke every session family for the principal. Exposed as a Loco task
  `auth:reset-password` taking `email=` and reading the new password from stdin; it prints nothing
  but a confirmation. Document it in `README.md`.

### Routes (lifecycle group, no session middleware)

- `GET /api/v1/auth/bootstrap` → `{ ownerExists: bool }`. No auth. Cheap and unauthenticated on
  purpose; it reveals nothing beyond "setup done".
- `POST /api/v1/auth/register` `{ displayName, email, password }` → 201 with the same body the
  GitHub redeem returns (cluster id, principal id, credentials, expiries); 409 `owner_exists`;
  400 `validation_failed` with field errors (`email` invalid, `password` under 12 or over 128,
  `displayName` empty or over 80).
- `POST /api/v1/auth/password/login` `{ email, password }` → 200 same body; 401
  `invalid_credentials` with `attemptsRemaining`; 429 `locked` with `retryAfterSeconds`; 400
  validation.
- `GET /api/v1/auth/session` (protected) → `{ principalId, email, displayName }` for the titlebar
  and settings.
- Existing refresh and logout routes are unchanged. GitHub routes remain and still return 503 while
  disabled.
- Regenerate the OpenAPI file (`UPDATE_OPENAPI=1 cargo test`) and fix the duplicated `show`
  operation id while there (`status.show` and `operations.show`).

### Native client

Add `bootstrap_status`, `register_owner`, `password_login`, `current_session`, keep the rest.
Auth calls never retry. Update the contract fixtures under `crates/native-client/src/tests/fixtures`.

## Desktop (Tauri)

- `session.json` document gains `connections: [{ id, origin, label, lastUsedAt, lastSummary? }]`
  (max 10, most recent first) where `lastSummary` is `{ nodes, services }` cached at sign-out or
  switch. Non-secret only; the vault layout is unchanged.
- Commands (replace the GitHub-flow ones): `session_bootstrap(origin)`, `session_login(origin,
  email, password, label?)`, `session_register(origin, displayName, email, password, label?)`,
  `session_connections()`, `session_forget_connection(id)`, `connection_probe(origin)` (2 s timeout,
  returns `ready | unreachable` plus a short diagnostic class: `dns`, `tcp`, `tls`, `http`, `timeout`
  derived from the native client's error set). Keep `session_restore`, `session_logout`,
  `session_retry_commit`. Remove `session_cancel_login` and `session_login_hint`.
- The password never touches `session.json`, logs, or the store; it lives only in the request.
- Update the Rust session tests and the shared fixture `tests/fixtures/session-ipc.json`.

## Desktop (React)

- `/login` renders the Connect card. Fields: AGENT URL (prefilled from the most recent connection
  or `http://127.0.0.1:5150`), EMAIL, PASSWORD (eye toggle), primary "Sign in". Under the button:
  "Forgot the password? Reset it from the controller CLI." Below: SAVED CONNECTIONS with a count
  badge and rows (label, origin, status dot, chevron). Clicking a row fills the URL and focuses the
  email field.
- On URL blur (or when prefilled on mount) call `session_bootstrap`; `ownerExists=false` switches
  the card to Create owner: FIRST-TIME SETUP badge, NAME, EMAIL, PASSWORD with the rule text,
  CONFIRM PASSWORD, "Create owner account", and the note "This account becomes the Owner.
  Registration closes once it exists." Saved connections are hidden on this variant.
- Client validation on blur and submit via the FieldError pattern: email shape, password length
  12–128, confirm matches, name non-empty. Server field errors land on their fields.
- State 02.S2: while probing or signing in the URL field is locked and the button reads
  `CONNECTING…` in the pending state. On unreachable, the typed URL stays, the field turns
  destructive with the reason, a diagnostics line reads `DNS → TCP → TLS` with the failed step
  highlighted, and the button becomes TRY AGAIN.
- State 02.S7: invalid credentials marks the password field, clears it, keeps email and URL, shows
  "Email or password is incorrect. N attempts left before a 15-minute lock on this email." A 429
  shows the lock with the retry time.
- Saved-connection dots: probe each row once on mount with `connection_probe` (bounded to three in
  flight); ready = success colour, unreachable = destructive, unprobed = muted. Hovering shows
  "last used <relative time>". A "forget" action lives in the row's context menu.
- Cluster switcher: the titlebar centre pill opens the dropdown (`23tdn`): header SWITCH CLUSTER
  with agent count, rows with label, subtitle from `lastSummary` when known ("3 nodes · 12
  services"), status badge `active | healthy | offline`; ADD NEW AGENT goes to `/login` with an
  empty URL. Choosing another connection signs out, navigates to `/login` with that origin
  prefilled. Offline rows are dimmed and not selectable; hovering says why and when last seen
  (02.S6). A sign-in that fails after switching stays on `/login` and shows a toast with the reason
  code and request id.
- Session expired (`ME9dW`): when a refresh is rejected, render the dialog over the current page
  behind a scrim; the page stays mounted and inert, polling stops (pause queries with
  `enabled: false` derived from the auth issue, no effects). The dialog explains why and has a
  PASSWORD field with SIGN IN (email fixed to the session's) and SWITCH AGENT. Success resumes the
  page with its filters; unsaved form input survives because nothing unmounts.
- Titlebar pill shows the active connection label and a dot from the last `/health/ready` probe
  (polled every 15 s through a `controller_health` command; feeds the status bar `WS CONNECTED`).
- Settings › Connection shows label, origin, email, display name (read-only).

## Tests

- Server: register then register again → 409; login ok; wrong password → 401 with attempts
  remaining; unknown email → 401 with identical timing class; ten failures → 429 with retry;
  lock expires; sessions from password login refresh and revoke like GitHub ones (reuse the
  existing session tests through the new entry point); reset task rehashes and revokes; validation
  errors; OpenAPI drift test.
- Native client: contract tests for the four new calls.
- Tauri: login, register, restore, logout, connections list ordering and cap, forget, probe
  diagnostics mapping; fixture round-trip.
- React: sign-in ↔ create-owner switch on bootstrap; validation; 02.S2, 02.S7, lock; saved rows
  fill the form; switcher dims offline rows; session-expired dialog keeps the page mounted and
  resumes on success.

## Done when

- Against the dev controller: first launch shows Create owner → account created → app is signed in
  → quit and relaunch restores the session → sign out → sign in with the password → a wrong
  password shows 02.S7 → ten wrong attempts lock → `auth:reset-password` unlocks with a new password.
- The GitHub routes still exist and answer 503 while disabled.
- All checks pass; `PROGRESS.md` updated.

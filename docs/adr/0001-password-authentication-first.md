# 0001. Password authentication first, GitHub login later as an integration

- Status: accepted (amended by 0029)
- Date: 2026-09-28
- Spec: docs/build/04-auth-and-connect.md

## Context

The reset doc named GitHub as the only initial identity provider. The controller ships that flow
today, disabled by config, and it needs a real GitHub OAuth app plus an account allowlist before
anyone can sign in. The design also drew a GitHub App manifest setup screen. Kontainer has one user.
Forcing a GitHub app install to reach the first screen makes every local run and every test depend on
GitHub or an emulator, and the emulator does not support the manifest flow.

The design's Integrations › GitHub page already depicts GitHub as something you connect after signing
in, which is the shape deployments will need.

## Decision

- Email and password is the login method. The first registration creates the Owner and claims the
  existing `bootstrap_state` row; registration returns 409 forever after.
- Password hashing uses Loco's Argon2id helpers (`loco_rs::hash`). No new crates.
- The existing session layer (`session_families`, `refresh_credentials`, `create_session`) is the
  shared "guard". Identity providers plug into `create_session::create_in_transaction`; the password
  provider is the first, GitHub stays compiled and config-disabled as the second.
- GitHub returns as an integration attached to the signed-in Owner (Integrations › GitHub), and only
  then as an alternative login. Linking is explicit from that page, never by email matching.
- No self-service password reset. The controller binary gets an `auth reset-password` subcommand.
- Login failures return one generic error. Ten failures per email within 15 minutes lock that email
  for 15 minutes, tracked in the in-memory cache Loco already provides.

## Consequences

- Local runs and tests need no external identity service. The GitHub emulator (ADR 0005) is deferred.
- The GitHub App manifest screen in the design stays deferred; the setup-token flow is not built.
- Password credentials add one table. Reset requires host access, which is acceptable for one user
  and documented on the sign-in screen.
- When GitHub login returns it must reuse `create_session` and the linking table, not a parallel
  session model. `GitHubLoginConfig` already carries overridable authorize, token, and user URLs, so an
  emulator can be pointed at without code changes.

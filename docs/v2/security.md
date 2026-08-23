# Authentication, authorization, and security in Kontainer v2

Status: proposed architecture and production security specification

Research date: 2026-08-19

Scope: first-run bootstrap, human and service identity, authorization, sessions, public and internal transport, workload enrollment, secret and key management, audit, Docker authority, deployment hardening, supply chain, and verification for the Rust agent. Resource-specific validation and stream semantics remain in the neighboring v2 documents.

## Executive decision

Kontainer v2 must treat the agent as a privileged control plane, not as a convenience API in front of Docker. A user who can change a service image, create a secret, promote a Swarm manager, or exec as root can usually cross important security boundaries. A process that controls `/var/run/docker.sock` can normally obtain host-level authority. Authentication alone is therefore insufficient: every public and internal action needs an authenticated principal, an explicit capability, a stable resource scope, bounded credentials, and a durable audit trail.

The accepted architecture is:

- **Human identity:** pluggable authorization-code providers. Retain a minimally permissioned GitHub App for v1 compatibility and smaller installations; add generic OpenID Connect (OIDC) for production federation. An identity is the immutable `(issuer, subject)` pair, never an email address, login, or organization name.
- **Bootstrap and recovery:** a local, one-time bootstrap credential establishes the first owner and identity provider. Local access remains a break-glass recovery path, not a normal remote password database.
- **Automation identity:** opaque, high-entropy service/deploy tokens with explicit capabilities, resource scopes, expiry, owner, and rotation. They are not user sessions and cannot use interactive login or refresh flows.
- **Authorization:** deny-by-default RBAC expressed as capabilities, intersected with hierarchical resource scopes and request context. GitHub organization or OIDC group membership can admit or suspend an identity, but does not silently make that identity a Kontainer administrator.
- **Sessions:** five-minute asymmetric access JWTs plus rotating opaque refresh tokens. Server-side session state provides revocation, reuse detection, device inventory, and logout. The Tauri Rust process owns credentials; the WebView receives only non-secret profile/session handles.
- **Public transport:** HTTPS/WSS using TLS 1.3 by default. Bearer credentials travel only in `Authorization` headers. Browser-only WebSocket clients exchange an authenticated HTTPS request for a single-use, audience- and target-bound ticket; account JWTs never appear in URLs.
- **Internal identity:** mandatory mTLS with per-controller and per-node-agent X.509 identities in a Kontainer trust domain. Short-lived enrollment tokens may bootstrap certificates but are never runtime credentials. A controller forwards target-bound, short-lived signed delegation grants for sensitive user-initiated RPCs.
- **Secrets:** typed secret records, envelope encryption at rest, purpose-separated versioned keys, Docker secret/file or external KMS delivery, and no secret values in arbitrary settings, environment variables, logs, URLs, or the desktop JSON store.
- **Audit:** append-only structured security events with redaction by construction, hash chaining and externally anchored signed checkpoints. High-risk changes fail closed when the authorization/audit intent cannot be durably recorded.

No production profile may retain v1's plaintext internal gRPC, cluster-wide bearer secret, unrestricted authenticated-user role, query-string JWTs, or plaintext desktop token store. The secure identity and policy seams are foundational; postponing them would force every HTTP action, WebSocket, gRPC method, database model, and test to change again.

## What v1 actually does

The current trust path is:

```text
GitHub browser flow -> leader -> 24-hour JWT -> Tauri/WebView
                                           |-> leader HTTP/WebSocket -> local Docker socket
                                           `-> plaintext gRPC + shared secret -> follower Docker socket
```

The following findings are based on the repository, not only the deployment prose.

### Setup and GitHub App manifest

- [`server/app/helpers/setup_token.go`](../../server/app/helpers/setup_token.go) generates `ktsetup_` plus six random bytes (48 bits) when `KONTAINER_SETUP_TOKEN` is absent. [`server/routes/setup.go`](../../server/routes/setup.go) logs the complete token on every boot. A generated token changes after restart; a configured token is reusable indefinitely.
- `GET /setup/status`, `GET /setup/github/start`, and `GET /setup/github/callback` are public. Only manifest creation and setup completion use `X-Kontainer-Setup-Token`. The desktop's `setup_verify_token` sends the header to the public status route, so its apparent verification step does not actually validate the token.
- Manifest setup keeps only one in-memory `{state, token, org}` tuple for the whole process. A second attempt replaces the first, restart loses both, and no explicit TTL exists.
- The manifest start URL contains a second 128-bit token in its query string. It is consumed before the browser is posted to GitHub, but URLs can still reach browser history, proxy logs, and diagnostics.
- A random manifest `state` is sent to GitHub, but [`server/app/http/controllers/setup/callback_controller.go`](../../server/app/http/controllers/setup/callback_controller.go) never reads or validates it before exchanging the code. GitHub returns manifest `state` when supplied and requires the temporary code exchange within one hour ([GitHub App manifest flow](https://docs.github.com/en/apps/sharing-github-apps/registering-a-github-app-from-a-manifest?apiVersion=2022-11-28)); v2 must enforce both properties.
- Manifest conversion stores the GitHub client secret, private key, webhook secret, organization login, and a generated symmetric JWT secret as plaintext rows in `app_configs`. Setup completion proves only that `github_app_id` exists. It does not prove that the app was installed in the intended organization or record an installation/account ID.
- `setup_complete` remains reachable with the reusable setup token after completion. There is no atomic first-owner creation, recovery-code handoff, or disabled terminal setup state.

### GitHub login and user model

- OAuth uses an external browser, a useful property to retain. RFC 8252 recommends an external user agent instead of an embedded WebView for native applications ([OAuth 2.0 for Native Apps](https://www.rfc-editor.org/rfc/rfc8252.html)).
- Login state is a one-time 128-bit value with a ten-minute in-memory TTL, bound to an unauthenticated WebSocket session ID. This protects the ordinary callback from simple state replay, but it does not use PKCE.
- `/auth/listen` accepts every WebSocket origin and sends a complete Kontainer JWT to whichever socket owns the session ID. The browser redirect URL contains the session ID; there is no independent desktop-held transaction secret.
- The GitHub provider requests OAuth-style scopes and checks `GET /orgs/{org}/members/{login}`. GitHub App user access tokens actually use fine-grained app permissions rather than OAuth scopes, and their `scope` response is empty ([GitHub App user access tokens](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app)). The configured App `Members: read` permission is the relevant permission.
- Membership is checked only at login, by mutable organization/login strings. There is no installation check, membership state record, periodic revalidation, webhook handling, or session revocation when a user leaves the organization. GitHub's membership API supports GitHub App user and installation tokens with `Members: read`, and the detailed endpoint exposes `state` and immutable organization/user IDs ([organization membership API](https://docs.github.com/en/rest/orgs/members?apiVersion=2022-11-28)).
- [`server/app/models/user.go`](../../server/app/models/user.go) contains provider, provider ID, name, email, and avatar only. There is no active/suspended state, role, scope, invited-by field, authentication assurance, or authorization epoch.
- Every valid user JWT reaches every `/api/*` action, including exec, node promotion, image prune, secret/config mutation, settings mutation, and stack removal. [`server/app/models/setting.go`](../../server/app/models/setting.go) is an arbitrary string map, so there is no typed separation between benign preferences and security policy.

### JWT issuance, validation, and transport

- Setup generates one symmetric secret and loads it into Goravel's user guard. [`server/config/jwt.go`](../../server/config/jwt.go) defaults access-token TTL to 1,440 minutes and refresh TTL to zero. [`server/app/http/middleware/jwt.go`](../../server/app/http/middleware/jwt.go) accepts the framework parse result; the application does not explicitly pin token type, issuer, audience, algorithm, session ID, token ID, or authorization version.
- There is no session table, refresh-token rotation, token family, server logout, denylist, signing-key ID, key overlap, device inventory, or administrator revocation. Desktop “logout” deletes only the local profile; the JWT remains valid until expiry.
- All HTTP clients send a bearer token correctly in `Authorization`, but service logs, task logs, and exec put the complete JWT in `?token=`. Query credentials can leak through access logs, reverse proxies, crash reports, copied URLs, and diagnostics.
- [`server/routes/api.go`](../../server/routes/api.go) deliberately registers WebSockets outside JWT middleware, while [`server/app/services/websocket/upgrader.go`](../../server/app/services/websocket/upgrader.go) accepts every origin. RFC 6455 defines `Origin` specifically so servers can reject unauthorized browser origins; token authentication does not make accepting every origin desirable ([WebSocket protocol security](https://www.rfc-editor.org/rfc/rfc6455.html)).
- WebSockets are authenticated once. Long streams do not re-check session expiry, revocation, role changes, or target authorization. They have no per-user admission quota shared across controllers.

### Desktop credential boundary

- [`app/src-tauri/src/actions/auth/save_profile.rs`](../../app/src-tauri/src/actions/auth/save_profile.rs) and sibling actions serialize the complete JWT into `profiles.json` through `tauri-plugin-store`; the legacy path uses `session.json`. These are ordinary application data stores, not an OS credential vault.
- React receives and retains the JWT, parses its payload without signature verification to obtain `key` and `exp`, passes it through every Tauri invocation, and constructs query-string WebSocket URLs in the WebView. Server validation prevents this parser from being authoritative, but the design exposes the bearer to every compromised WebView dependency and XSS path.
- The Tauri CSP is `null`, the opener and store default capabilities are enabled, and saved profiles combine non-secret metadata with bearer tokens. This makes the desktop contract part of the agent threat model.
- URL strings are user-provided and HTTP is accepted. Production has no HTTPS requirement, custom enterprise CA policy, certificate pin policy, or warning that changing a saved URL changes the credential destination.

Platform vaults are the correct replacement: Apple Keychain is an encrypted database for small secrets ([Apple Keychain Services](https://developer.apple.com/documentation/security/keychain-services)), Windows exposes the Credential Locker to desktop applications ([Microsoft Credential Locker](https://learn.microsoft.com/en-us/windows/apps/develop/security/credential-locker)), and Linux desktops conventionally expose the Secret Service API ([freedesktop.org Secret Service](https://specifications.freedesktop.org/secret-service/latest/)).

### Internal gRPC and cluster membership

- [`server/app/services/cluster/grpc_pool.go`](../../server/app/services/cluster/grpc_pool.go) sends the same `x-kontainer-secret` on every controller/follower call and uses `insecure.NewCredentials()`, so the secret, requests, and responses are plaintext on the overlay. Any holder can call every method and claim any node identity.
- [`server/app/grpc/interceptors/secret_auth.go`](../../server/app/grpc/interceptors/secret_auth.go) covers unary RPCs globally, while bidirectional exec repeats authentication in the handler. A new streaming RPC could accidentally omit it.
- Discovery trusts task network addresses and health-response IDs but has no certificate identity. One secret compromises the whole cluster until every workload is restarted with a new value.
- The same broad service exposes health, metrics, image mutation, and exec. There is no per-method caller policy or signed end-user delegation.
- Docker's own Swarm PKI cannot be treated as Kontainer's credential: Docker uses mTLS and rotating node certificates internally, but its node private key is not a supported application credential ([Docker Swarm PKI](https://docs.docker.com/engine/swarm/how-swarm-mode-works/pki/)).

The replacement topology, discovery binding, mTLS URI identities, certificate lifecycle, and delegation envelope are specified in [`cluster-communication.md`](cluster-communication.md). This document makes those controls mandatory for the broader identity and audit system.

### Storage, deployment, and tests

- Provider credentials, JWT keys, setup state, and settings are plaintext in the same SQLite database. `APP_KEY` exists in configuration but these models do not encrypt values.
- Public HTTP defaults to plaintext; no production TLS certificate is configured. The documented reverse-proxy trust model and security headers are absent.
- The production examples use `:latest`, mount the Docker socket read/write, pass the cluster secret in an environment variable, and do not define a non-root user, read-only root filesystem, capability drops, PID/memory limits, or private/public network separation.
- [`server/Dockerfile`](../../server/Dockerfile) uses mutable builder/runtime image tags, runs `go mod tidy` during the build, runs as root, and copies `.env` from the build context into the runtime image. A production Rust image must not reproduce these patterns.
- The deployment guide claims `_FILE` support for selected values, but no implementation is present in the inspected Go source. Security documentation must be exercised by deployment tests rather than treated as implementation.
- Focused unit tests cover random token shape, OAuth state consumption, and the in-memory bridge. They do not cover route protection, manifest state, PKCE, JWT claim/algorithm confusion, role/scope decisions, revocation, WebSocket origin/expiry, gRPC secret interception, credential redaction, or deployment security.

## Security objectives and trust assumptions

### Assets

Protect at least:

- Docker/host authority and Swarm manager operations;
- service specifications, environment values, secret/config contents, logs, terminal I/O, and metrics;
- human, service, controller, and node-agent identities;
- session, provider, signing, encryption, CA, enrollment, recovery, and webhook credentials;
- authorization policy, security settings, audit records, operation/idempotency records, and backups;
- cluster availability against resource exhaustion and control-plane misuse.

### Trust boundaries

```text
System browser <-> external IdP/GitHub
       |
       v
Tauri WebView -> Tauri Rust credential/network boundary -> public ingress
                                                        -> controller + shared DB/KMS
Controller -> local manager Docker socket
Controller == mTLS + delegation ==> node agent -> local Docker socket
Build/registry/update channel -> deployed privileged binaries
```

Assumptions that must be explicit:

- Host root, a Docker daemon administrator, or a compromised daemon can control the local agent, read its mounted credentials, and alter Docker results. Kontainer can contain and detect some consequences but cannot sandbox the host from its root-equivalent control plane.
- A normal cluster workload, a process on the overlay, a remote unauthenticated client, and an authenticated low-privilege user are untrusted.
- The database and backups may be copied. Database compromise alone must not reveal reusable provider, refresh, service-token, or encryption credentials.
- Public reverse proxies are trusted only when configured by exact network identity and connected through a protected hop. Forwarded headers from any other source are attacker input.
- Identity providers authenticate users but do not define Kontainer's authorization unless a mapping is explicitly configured.
- Wall clocks can drift. Small bounded leeway is permitted, but replay windows, certificate validity, audit ordering, and database leases cannot assume perfect clocks.

### Threat model and required treatment

| Threat | Representative attack | Required treatment | Residual risk |
| --- | --- | --- | --- |
| First-run takeover | Internet scanner claims an unconfigured public controller or reuses a setup token | Locked bootstrap state; 256-bit one-time token; short window; explicit token file/local CLI; first-owner transaction; setup endpoints disabled afterward | Anyone who steals the bootstrap file during the open window can race the owner |
| OAuth login CSRF/code injection | Attacker binds their provider account to a victim desktop transaction | One-time state bound to transaction, PKCE S256, OIDC nonce, exact redirect, provider-bound callback, external browser | Compromised browser/desktop can still act for its user |
| IdP mix-up or token confusion | ID token/access token from another issuer, client, cluster, or algorithm is accepted | Exact issuer/audience/type/algorithm/key-use validation; distinct callbacks or response `iss`; mutually exclusive token validators | A trusted IdP can assert identities according to its own security |
| Stolen bearer | Token leaks from URL, JSON store, logs, or memory | Header-only short access JWT; rotating refresh token in OS vault; redaction; revocation; optional future DPoP | Malware running as the desktop user may access the vault or process |
| Stale membership | Removed GitHub/OIDC user keeps a long session | Local suspension; short access TTL; refresh-time/recent membership check; webhook/back-channel integration where reliable; max session age | Provider outage creates an availability-versus-revocation tradeoff |
| Broken object authorization | Operator guesses another stack/service/node ID | Capability + stable resource scope after authoritative resolution; filtered lists; target-bound grants; negative IDOR tests | Docker itself has coarse authority once the privileged adapter is reached |
| Confused deputy | Controller asks an agent to exec or prune without proving the user/target | mTLS workload policy plus signed request-hash/target-bound delegation and replay cache | A compromised authorized controller can mint grants until revoked |
| Internal impersonation | Overlay peer steals/replays the cluster secret or claims another node ID | Per-workload short-lived mTLS; Swarm task/node binding; no runtime shared secret; membership-driven revocation | Node compromise exposes that node's live identity until removal/expiry |
| Docker escape/host takeover | API bug permits arbitrary Engine request, mount, privileged container, or root exec | Domain allowlist; validation; role gates; no raw proxy; socket-owning process isolation; optional authz plugin/rootless/helper | Compromise of a socket-owning process remains node/manager compromise |
| Secret/database theft | SQLite/backup exposes provider private key and tokens | Envelope encryption; KMS/file KEK outside DB; per-purpose keys; encrypted backups; minimum retention | A live controller with KMS permission can decrypt secrets it needs |
| Audit tampering | Administrator deletes or rewrites evidence | Append-only authorization, HMAC chain, signed external anchors, restricted export/WORM sink | A fully compromised live controller can forge future events before detection |
| Denial of service | Login storms, huge bodies, WebSocket floods, expensive Docker fan-out | Layered rate, body, message, concurrency, byte, and time budgets; distributed limits; backpressure | Authorized destructive actions can still consume legitimate Docker resources |
| Supply-chain compromise | Malicious crate/build script/base image/update replaces privileged agent | Locked reviewed graph, isolated builds, advisory/vet gates, SBOM/provenance, signed digest-pinned images, update verification | Reviewed dependencies may contain unknown defects |

## Identity approaches considered

| Approach | Decision | Reason |
| --- | --- | --- |
| GitHub App manifest + user authorization | **Accepted for parity and as an optional provider** | Preserves the current onboarding story, supports immutable GitHub user IDs and organization membership, and can use an organization installation for durable membership checks. It is not universal enterprise identity and must no longer define authorization by itself. |
| Generic OIDC authorization code flow | **Accepted as the production federation interface** | Supports enterprise IdPs, MFA/assurance claims, discovery/JWKS rotation, and stable issuer/subject identity without adding a password database. OIDC group claims are provider-specific and require explicit mappings. |
| Local setup token | **Accepted only for first claim and recovery-window creation** | Some trust root must exist before federation. The token is local/out-of-band, short-lived, one-time, and cannot call the normal API. |
| Persistent local username/password accounts | **Rejected for the initial production release** | A password store adds enrollment, reset, MFA, breach-password, lockout, email, and help-desk obligations. It also encourages a remote break-glass account attackers can continuously probe. Passkey-based local accounts may be revisited for air-gapped installations. |
| GitHub/OIDC token used directly as Kontainer bearer | **Rejected** | Upstream tokens have the wrong audience, permissions, lifetime, revocation model, and disclosure surface. Kontainer exchanges a successful upstream authentication for its own bounded session. |
| OAuth device flow for ordinary desktop login | **Rejected by default; optional for headless CLI later** | The system-browser authorization-code flow is available and supports PKCE. GitHub warns that device flow can be abused for remote app impersonation/phishing and recommends code + PKCE when practical ([GitHub OAuth App guidance](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/best-practices-for-creating-an-oauth-app)). |
| Opaque service/deploy token | **Accepted** | It provides revocable, non-interactive identity with narrow capabilities and resource scope. It avoids pretending CI is a human and avoids long-lived JWT revocation problems. |
| Personal access token imported from GitHub | **Rejected** | It couples Kontainer authority to a broad third-party bearer, complicates ownership and revocation, and violates GitHub's recommendation to use the appropriate least-privileged App token type. |

### GitHub App provider

The v2 GitHub provider must use the GitHub App web authorization flow with `state` and PKCE S256. GitHub documents `state`, `code_challenge`, `code_challenge_method=S256`, and `code_verifier` for both OAuth Apps and GitHub App user access tokens ([GitHub web flow](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps), [GitHub App user token flow](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app)). RFC 9700 recommends PKCE even for confidential clients and requires transaction-specific binding ([OAuth 2.0 Security BCP](https://www.rfc-editor.org/rfc/rfc9700.html)).

Provider configuration rules:

1. The manifest flow is itself a bootstrap transaction with a 256-bit one-time `state`, 15-minute Kontainer TTL, exact expected organization account ID, and single-use code exchange. Reject a missing/mismatched/replayed state before contacting GitHub.
2. Require the App to be installed in the configured organization and persist immutable app ID, installation ID, organization numeric/node ID, and expected permissions. A mutable login/slug is display metadata.
3. Request only `Members: read` organization permission. Request account Email permission only if a product requirement actually uses email; email is never identity. Subscribe to no webhook by default. GitHub explicitly recommends minimum App permissions ([choosing GitHub App permissions](https://docs.github.com/en/apps/creating-github-apps/registering-a-github-app/choosing-permissions-for-a-github-app)).
4. Encrypt the client secret, App private key, and optional webhook secret as separate purpose-tagged credentials. If periodic membership checks use installation tokens, retaining the App private key is justified; otherwise discard the manifest PEM and webhook secret immediately.
5. Use the user token only to fetch the authenticated user's immutable ID and profile, then discard it unless an explicitly documented provider-refresh feature needs it. GitHub user tokens are fine-grained intersections of App and user access, not Kontainer authorization.
6. Validate membership with the configured organization installation and require `state=active`; compare returned immutable organization/user IDs. Pending invitations fail. Check at login and local-session refresh, with a short cache and a stricter freshness requirement for security administration, node role changes, secret mutation, and root exec.
7. First-run provider outage fails closed. After setup, a configurable short stale window may keep already-issued read sessions working, but no new session or high-risk action may rely on stale membership. Record and surface the degraded state.
8. If webhooks are enabled to accelerate revocation, validate `X-Hub-Signature-256` over the raw body with constant-time comparison before parsing, deduplicate delivery IDs, and still reconcile periodically. GitHub documents the HMAC validation procedure ([validating webhook deliveries](https://docs.github.com/en/webhooks/using-webhooks/validating-webhook-deliveries)).
9. Enable expiring GitHub user tokens. GitHub documents an eight-hour user token and six-month refresh token when expiration is enabled ([refreshing GitHub user tokens](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/refreshing-user-access-tokens)). These are upstream credentials and do not set Kontainer's session lifetime.
10. Do not infer a Kontainer role from GitHub organization owner/member status. The bootstrap identity becomes the first Kontainer owner because it proved the out-of-band bootstrap secret; subsequent identities default to pending or Viewer until a Kontainer administrator grants access.

GitHub recommends checking authorization “thoroughly, durably, and often,” minimum permissions, expiring tokens, and security logging ([GitHub App best practices](https://docs.github.com/en/apps/creating-github-apps/about-creating-github-apps/best-practices-for-creating-a-github-app)); the provider adapter should expose membership freshness and installation health so policy can enforce that advice.

### Generic OIDC provider

Support statically configured OIDC issuers, not arbitrary issuer URLs submitted during login. An owner provides the exact HTTPS issuer, client ID, client authentication method/credential, scopes, allowed signing algorithms, optional organization/group claim rules, and enterprise CA reference.

At configuration and refresh time:

- Fetch `/.well-known/openid-configuration`, require the returned issuer to exactly equal the configured issuer, and use its HTTPS authorization, token, and JWKS endpoints. OIDC Discovery requires an HTTPS `jwks_uri` and issuer metadata ([OIDC Discovery](https://openid.net/specs/openid-connect-discovery-1_0.html)).
- Limit response size, redirects, DNS rebinding, connection time, and JWKS key count. Private/link-local targets require an explicit enterprise-network opt-in. Owner-controlled discovery is still an outbound SSRF boundary.
- Cache keys according to HTTP metadata with a bounded stale period. An unknown `kid` triggers one rate-limited refresh, not a fetch per hostile token.
- Prefer `private_key_jwt` when the IdP supports it; otherwise store `client_secret_basic` credentials encrypted. Reject insecure token endpoint authentication methods in production.

For every login:

1. Create one transaction containing provider ID, random state, PKCE verifier/challenge, OIDC nonce, desktop transaction binding, canonical redirect URI, requested post-login action, and expiry.
2. Use authorization code flow only, `response_type=code`, PKCE S256, and an external browser. No implicit or resource-owner-password flow.
3. Bind the callback to the provider/transaction, compare state in constant time, exchange exactly once, and reject provider mix-up. Use distinct callback paths per provider when the authorization response cannot supply a trustworthy issuer.
4. Verify the ID-token signature with the configured algorithm allowlist and issuer keys; require exact `iss`, client `aud`, acceptable `azp` when present, `exp`, bounded `iat`, and the transaction nonce. OIDC Core defines these validation requirements ([OIDC Core ID-token validation](https://openid.net/specs/openid-connect-core-1_0.html#IDTokenValidation)).
5. Identify the principal by `(iss, sub)`. Treat name, email, preferred username, avatar, groups, and roles as mutable claims. Require `email_verified` only if email display or invitation matching explicitly depends on it.
6. Apply group-to-Kontainer-role mappings by exact provider/claim/value and make the resulting grant visible and auditable. Missing, truncated, or malformed group claims fail closed for that mapping; they never grant a broader fallback role.
7. Record authentication time and `amr`/`acr` when available. A high-risk policy may require a recent authentication and configured assurance, but it must not pretend an absent claim proves MFA.
8. Discard upstream access/refresh tokens after login unless a documented userinfo, logout, or lifecycle feature needs them. Any retained token is encrypted, minimally scoped, rotated, and independently revocable.

OIDC back-channel logout, SCIM deprovisioning, and provider-specific continuous-access signals are useful production integrations but cannot be mandatory for every conforming issuer. Local suspension and short access/session lifetimes remain the portable controls.

## Principal, provider, and security data model

Do not carry v1's `users`, arbitrary `app_configs`, and arbitrary `settings` tables into v2 unchanged. Use stable IDs, typed records, foreign keys, optimistic versions, and explicit secret references.

| Record | Required fields and invariants |
| --- | --- |
| `clusters` | Stable random `cluster_id`, display name, canonical public issuer/base URL, created time, security-policy version, bootstrap state. The ID survives host/controller replacement and is in every audience/scope. |
| `principals` | UUID, kind (`human`, `service`), display metadata, status (`pending`, `active`, `suspended`, `deleted`), authorization epoch, created/updated/by. Deletion tombstones identity; IDs are not reused. |
| `external_identities` | Principal ID, provider config ID, exact issuer, subject, provider immutable account ID, mutable profile fields, last authenticated/verified, membership status/freshness. Unique `(provider_config_id, issuer, subject)`. |
| `provider_configs` | Type, issuer/App/installation/account IDs, exact redirect URI, claim/membership rules, status/version, typed encrypted credential references. No generic plaintext map. |
| `roles` | Immutable system or versioned custom role, capability set, description, status. Custom roles cannot invent unknown capabilities. |
| `role_bindings` | Principal, role, cluster/resource scope, optional condition/expiry, grantor, reason, version. At least one active Owner must remain. |
| `sessions` | Session ID, principal, client installation/profile ID, provider/auth time/assurance, created/last used/idle/absolute expiry, status/revoked reason/by, refresh-family version, authorization epoch observed. |
| `refresh_tokens` | Public token ID, session/family ID, keyed hash, created/used/replaced/revoked times, optional previous-token reuse marker. Plaintext exists only at issuance and in the client vault. |
| `service_tokens` | Public token ID/prefix, principal, keyed hash, capabilities, scopes, not-before/expiry, last-used metadata, status, owner/rotation pair. No refresh flow. |
| `auth_transactions` | Hashed desktop binding secret, provider, state, PKCE/nonce secret references, redirect, expiry, attempt/consumed state. Short retention and one-time transitions. |
| `secret_envelopes` | Purpose, ciphertext, nonce, wrapped DEK, KEK ID/version, AAD/schema version, created/rotated time. The referenced owner row controls access. |
| `signing_keys` | Use (`access`, `delegation`, `audit-checkpoint`, `CA`), public key/chain, `kid`, status, not-before/after. Private material is a KMS reference or encrypted secret, never a JSON setting. |
| `enrollment_tokens` / `workload_certificates` | Hashed one-time token and verified cluster/node/task/workload scope; certificate serial/SAN/key IDs, validity, status/revocation, enrollment evidence. |
| `recovery_codes` | Salted Argon2id hash, code set/version, created, consumed, revoked. Plain codes are displayed/exported once. |
| `audit_events` / `audit_checkpoints` | Immutable canonical event plus previous/event hash; signed checkpoint and external anchor status. Access and retention are separate capabilities. |

Security configuration uses typed structures validated at startup and update. Unknown fields fail validation; secrets are references; changing canonical URL, provider issuer, trust roots, key backends, role definitions, audit retention, or proxy trust is a security action with a dedicated capability and audit event. Normal agent preferences cannot update these fields through a generic map.

## First-run bootstrap and recovery

### Locked startup state

An unclaimed controller starts **locked**:

- expose only minimal liveness/readiness and the bootstrap claim/auth transaction endpoints;
- do not expose `/api`, ordinary auth providers, WebSockets, internal certificate issuance, or Docker mutations;
- do not disclose configured providers, organization, user records, or why a guessed bootstrap credential failed;
- bind publicly only when a canonical HTTPS origin and explicit bootstrap-token source are configured. Loopback development mode is a separately named profile.

The production operator supplies a 32-byte random bootstrap token through `KONTAINER_BOOTSTRAP_TOKEN_FILE` or a local `kontainer bootstrap create` command that writes a root-readable `0600` file. Store only a keyed hash, creation time, expiry, and attempt state. Default validity is 15 minutes after the first ready controller starts; allow one explicit extension from the local CLI. Never place it in an environment variable, command argument, URL, structured log, image layer, API response, or desktop profile.

Docker Swarm secrets are encrypted in transit and at rest and are mounted only to authorized running services ([Docker secrets](https://docs.docker.com/engine/swarm/secrets/)); they are an acceptable delivery mechanism for the bootstrap file and minimum file-based KEK. They do not protect against a compromised manager or the authorized running controller.

### First-owner flow

1. Tauri Rust connects over verified HTTPS and submits the bootstrap token in a header/body excluded from logs. On success the server consumes it and returns a one-time setup-session handle plus a separate desktop-held binding secret.
2. The owner configures a provider. Provider credentials are encrypted immediately; a test/discovery result is not yet an active provider.
3. The server starts the provider authorization transaction. The system browser completes GitHub/OIDC while Tauri polls an authenticated transaction-status endpoint or holds a WSS connection authenticated by the desktop binding secret.
4. The callback authenticates the upstream identity, but returns no Kontainer bearer to the browser. It atomically marks the transaction complete and makes a one-time Kontainer authorization code available to the bound Tauri client.
5. Tauri redeems that code with the binding secret. In one database transaction the server creates/activates the principal and Owner binding, creates the session/refresh family, activates the provider, records recovery-code hashes, disables bootstrap, and commits an audit checkpoint. A crash before commit leaves the cluster unclaimed and requires a new local bootstrap token.
6. Tauri stores the refresh token in the OS vault and shows non-secret profile metadata. The operator saves recovery codes offline and confirms at least one code before setup is considered healthy.

Concurrent claims use a database compare-and-set on `bootstrap_state`; exactly one wins. All other credentials and setup sessions are revoked. Subsequent bootstrap routes return a generic 404/410 even if the old token is supplied.

### Break-glass recovery

There is no always-on remote recovery endpoint. An operator with local access runs `kontainer auth recovery-open`, which:

- requires access to the persisted database and active KEK/key backend;
- creates a 10-minute, one-use recovery window bound to the local cluster ID;
- emits the URL/nonce only to the controlling terminal or a `0600` file, never normal logs;
- requires one unused offline recovery code in addition to the local nonce;
- grants a short recovery session limited to provider repair, owner restoration, session revocation, and new recovery-code generation;
- revokes all existing recovery codes and optionally all sessions when completed.

Every attempt and local command is audited. Repeated invalid codes close the window and require a new local command. Test the recovery procedure on every supported deployment profile.

If the KEK or CA root is irretrievably lost, cryptographic recovery is impossible by design. Restore a tested encrypted backup containing the required key material or perform a documented rebootstrap that revokes sessions/workload identities and may discard encrypted provider credentials. Do not add an undocumented master password.

## Authentication transaction and desktop contract

The desktop login bridge replaces `/auth/listen`:

```text
Tauri Rust -> POST /auth/transactions
           <- transaction_id, browser_url, poll_secret, expires_at
System browser -> provider -> fixed controller callback
Controller records authenticated identity and one-time result
Tauri Rust -> POST /auth/transactions/{id}/redeem + poll_secret
           <- access token + rotating refresh token + non-secret profile
```

Rules:

- `transaction_id`, OAuth `state`, and desktop `poll_secret` are independent 256-bit random values. Store hashes for bearer-like transaction secrets. One identifier leaking through the browser cannot redeem the result.
- TTL is ten minutes; state/codes/results are single-use. Enforce a strict state machine so callback, cancel, expiry, and redeem races have one terminal result.
- The `browser_url` contains only the OAuth authorization request. It contains no Kontainer account token or poll secret.
- The callback page has no third-party resources and no token. It displays a generic “return to Kontainer” result. RFC 9700 recommends no third-party content and `Referrer-Policy: no-referrer` on authorization response pages.
- The native process performs agent API traffic and attaches credentials. React receives a random local session handle and profile/expiry/permissions summary, not the access or refresh token.
- Store refresh tokens under an application- and cluster-specific vault key. Keep access tokens in Rust memory only and clear/replace them on refresh. `profiles.json` may contain label, cluster ID, verified canonical URL, user display metadata, and vault item identifier only.
- Changing a profile URL requires reauthentication and creates a new vault namespace. Never forward an existing credential to a newly edited host.
- Production profiles require HTTPS and normal hostname/chain validation. Support an explicit per-profile enterprise CA bundle; do not expose “accept invalid certificate.” A user-approved SPKI pin can be an advanced managed option only with a rotation/recovery plan, not insecure TOFU.
- Logout first revokes the server session/refresh family, then deletes vault entries and in-memory access credentials. Local deletion failure is visible; server revocation failure queues a bounded retry and warns the user.
- Prevent secrets from crossing Tauri events, React props/state, Redux/query caches, panic strings, `Debug`, clipboard, and telemetry. Rust secret wrappers should redact formatting and zeroize on drop where practical; the `secrecy` crate is designed for explicit exposure and redacted/zeroized secret handling ([`secrecy` documentation](https://docs.rs/secrecy/latest/secrecy/)). Zeroization reduces accidental persistence but cannot defeat swapping, crash dumps, or a compromised process.

## Session and token design

### Access token

Kontainer issues a JWT access token, not an ID token:

```text
header: alg=EdDSA, kid=<active-key-id>, typ=at+jwt
claims: iss, aud=urn:kontainer:cluster:<cluster-id>, sub=<principal-id>,
        client_id=kontainer-desktop|<service-client>, sid, jti,
        iat, nbf, exp, auth_time, amr[], authorization_epoch
```

The built-in signer uses Ed25519/EdDSA with a versioned `kid`; an external KMS may use ES256 or RS256 under a separately tested algorithm profile. Never decide the verification algorithm from an untrusted header alone. RFC 8725 requires an algorithm allowlist, issuer/subject binding, and audience validation, and recommends mutually exclusive validation rules for different JWT kinds ([JWT Best Current Practices](https://www.rfc-editor.org/rfc/rfc8725.html)). RFC 9068 defines `typ=at+jwt` and required access-token claims ([JWT access-token profile](https://www.rfc-editor.org/rfc/rfc9068.html)).

Validation order:

1. Enforce header/encoded-token size and structure; reject duplicate JSON keys and unsupported critical headers.
2. Require `typ=at+jwt`, expected `kid`, and the exact algorithm configured for that key/use. Reject `none`, symmetric/asymmetric confusion, and keys meant for OIDC, delegation, audit, or another cluster.
3. Verify signature, then exact issuer and cluster audience, principal-shaped `sub`, known `client_id`, required times, bounded clock leeway (start at 30 seconds), `sid`, and unique `jti` shape.
4. Require `exp - iat <= 5 minutes` and reject a future/implausibly old `iat` or token issued before the key/session/principal not-before epoch.
5. Resolve a short-TTL cached session/principal status and authorization epoch. High-risk actions read authoritative shared state. A suspension or session-revocation event invalidates caches and closes streams across controllers.
6. Build a typed `RequestPrincipal`; handlers never parse tokens or trust role/capability claims independently.

The Rust JWT library is only a mechanism. For example, `jsonwebtoken::Validation` can pin algorithms, issuer, audience, and required claims, but its documentation makes clear that validation applies only to configured/present claims ([`jsonwebtoken::Validation`](https://docs.rs/jsonwebtoken/latest/jsonwebtoken/struct.Validation.html)); application checks for type, key use, `sid`, `jti`, lifetime, and authorization epoch remain mandatory.

### Refresh token and session lifecycle

Refresh tokens are opaque strings such as `ktr_<public-id>.<32-random-bytes>`. Store `HMAC-SHA-256(refresh_pepper, secret)` plus metadata, not the plaintext. High entropy makes a fast verifier safe; the separate pepper prevents a database-only attacker from validating guesses and is KMS/file backed.

Initial policy, subject to the open product decision:

- access token: 5 minutes;
- idle session: 7 days;
- absolute session: 30 days;
- recent-auth requirement for security/owner operations: 15 minutes;
- refresh transaction/result retention: enough to detect reuse for the 30-day family lifetime;
- session count: 20 per human principal, with oldest-session review/revocation rather than silent unlimited growth.

Every refresh rotates the token in one transaction. Returning token B invalidates token A. Reuse of A after B exists revokes the whole family, records a high-severity audit event, and requires login. Concurrent legitimate refreshes are serialized/idempotently correlated so a client retry does not look like theft.

Refresh checks principal/session status, absolute/idle expiry, authorization epoch, client ID, provider status, and any membership freshness policy. Role changes increment the authorization epoch so newly issued access tokens reflect policy immediately. Server logout, “log out all devices,” provider removal, principal suspension, recovery, signing-key compromise, and administrator action revoke appropriate session sets.

Signing-key rotation publishes old and new verification keys before issuance switches, records activation/retirement times, and retains an old public key only through the maximum token lifetime plus skew. Emergency compromise sets a key/session not-before epoch and revokes affected sessions; deleting a key file alone is not revocation.

### Service and deployment tokens

Service tokens use `kts_<public-id>.<32-random-bytes>`, the same keyed-hash storage pattern, and a dedicated authenticator. Requirements:

- owner, purpose, contact, capability set, resource scopes, issue/not-before/expiry, last-used metadata, and optional source-network constraints are mandatory;
- no built-in role and no wildcard Owner token; the creator may delegate only capabilities/scopes they are allowed to delegate;
- default maximum lifetime 90 days, with an explicit shorter policy for CI; non-expiring tokens are disabled in production;
- show plaintext once; never provide a read-back API;
- rotation can create an explicitly linked overlap token for a bounded window, after which the old token is revoked;
- revocation is immediate through the shared token-status cache/event path;
- token use has a distinct `service` principal and audit actor, not the human creator;
- interactive browser auth, refresh, WebSocket exec, security administration, owner/recovery, provider/CA management, and token creation are denied unless an explicit future machine workflow defines them. A deploy token normally cannot exec.

An optional short-lived signed workload token issued through an external CI federation is preferable to static service tokens and is deferred until a concrete CI issuer contract is designed.

## Authorization model

### Decision function

Every action calls one centralized policy service:

```text
decision = principal active
        AND session/token active and sufficiently fresh
        AND capability is granted by an active role/binding
        AND every resolved resource matches the binding scope
        AND contextual requirements (assurance, confirmation, quota) pass
        AND no explicit suspension/deny applies
```

Authentication middleware creates the principal; route/action code supplies the named capability and authoritative resource context; policy returns allow/deny plus policy version/reason for audit. Never infer authorization from HTTP method, UI visibility, provider claims, a user-supplied node ID, or possession of a cluster mTLS certificate.

Scope keys use immutable IDs:

- cluster;
- stack/environment namespace within cluster;
- service ID within cluster/stack;
- node ID;
- specific resource ID where necessary.

A cluster scope includes descendants only when the binding says so. Label/selectors may define dynamic scopes, but policy expands them against an authoritative snapshot and audits the resulting IDs. Names are display/search aliases. A service recreated under the same name is a new resource unless an explicit stable stack-scope binding includes it.

List endpoints filter results to authorized resources and return a completeness/scope marker. Single-object endpoints should use a consistent not-found response when revealing existence would be sensitive. Multi-target mutations resolve and authorize the entire target set before the first side effect; partial authorization is a denial, not silent target filtering.

### Concrete capabilities

| Domain | Capabilities | Notes |
| --- | --- | --- |
| Inventory | `resource.read`, `metrics.read`, `logs.read` | Logs are separate because application output often contains credentials/customer data. |
| Service/stack | `service.scale`, `service.restart`, `service.rollback`, `service.update.image`, `service.update.env`, `stack.remove` | Environment updates can inject secrets and may require a stricter custom role. Stack removal requires confirmation/idempotency. |
| Exec | `exec.open`, `exec.custom_shell`, `exec.custom_user`, `exec.root`, `exec.non_tty` | Checked after fresh task/container/node resolution. Root is never implied by ordinary exec. See [`exec.md`](exec.md). |
| Node | `node.availability.update`, `node.role.update`, `node.local.maintain` | Manager promotion/demotion is high risk and requires recent auth plus Admin/Owner. |
| Images | `image.read`, `image.remove`, `image.prune` | Remove/prune are node-scoped and operation/idempotency protected. |
| Docker secrets | `secret.metadata.read`, `secret.create`, `secret.remove` | Secret plaintext is write-only; there is no `secret.value.read`. Do not return create input in audit/API response. |
| Docker configs | `config.metadata.read`, `config.content.read`, `config.create`, `config.remove` | Config content is separate because it may contain sensitive material despite not being a Docker secret. |
| Agent settings | `settings.read`, `settings.update` | Only typed operational settings. Security settings use the capabilities below. |
| Access/security | `access.read`, `access.manage`, `role.manage`, `provider.manage`, `service_token.manage`, `session.revoke`, `security.policy.manage` | Delegation and last-owner constraints apply. |
| Cluster identity | `agent.read`, `agent.enroll`, `agent.remove`, `pki.rotate` | Ordinary users cannot enroll workloads or rotate trust roots. |
| Audit/recovery | `audit.read`, `audit.export`, `audit.retention.manage`, `recovery.execute` | Reading audit may reveal sensitive metadata; no role has an API capability to rewrite events. |

### System role templates

| Role | Default capabilities | Explicit exclusions |
| --- | --- | --- |
| **Owner** | All capabilities in the bound cluster. Required for provider/PKI/security policy, recovery, retention, and granting/removing Owner. | Cannot erase or rewrite audit events. Cannot remove/suspend the last active Owner. Recent authentication is required for owner-only actions. |
| **Administrator** | Inventory/logs/metrics, all ordinary Docker operations, non-root exec, node availability/role, secret/config management, settings, access/role/session/service-token administration, agent read/remove. | No Owner grant/removal, provider credential/issuer change, PKI/root rotation, recovery, audit-retention reduction, or default root exec. |
| **Operator** | `resource.read`, metrics/logs, service scale/restart/rollback/image/env update, non-root default-shell exec, image read/remove/prune, node availability within scope. | No stack removal by default, node role, root/custom exec, secret/config management, settings, access/security/agent/audit administration. |
| **Viewer** | `resource.read`, `metrics.read`, and `image.read` within scope. | No logs, config/secret metadata, exec, mutation, access, or audit. |
| **Auditor** | `resource.read`, `audit.read`, and `audit.export` within cluster; optionally metrics through a separate binding. | No logs/exec/mutation and no access/security changes. |
| **Automation** | Empty template. Token creation must choose an explicit allowed capability/scope set, commonly deploy/update/restart for named stacks. | No implicit human, exec, access, provider, PKI, recovery, or audit-retention authority. |

System roles are immutable definitions; deployments may create versioned custom roles from known capabilities. The authorization matrix is data-driven and reviewed in migrations, but code owns each capability's risk rules. A custom role cannot bypass confirmation, target resolution, recent-auth, node-side delegation, or audit requirements.

### High-risk action rules

In addition to capability/scope:

- root exec, provider/security changes, Owner changes, recovery, trust-root rotation, node role change, stack removal, secret removal, and broad prune require recent authentication;
- the UI/API provides an operation-specific confirmation token based on a fresh preview/request hash for destructive bulk operations. Confirmation is not authorization but prevents stale/click-through intent;
- sensitive mutations require an idempotency key and durable operation/audit intent before Docker dispatch;
- a principal cannot grant a role/capability/scope broader than their delegable authority;
- self-removal is allowed only when another active Owner remains; suspending the last Owner is rejected;
- policy changes do not grandfather streams: exec/log sessions are re-evaluated and closed when their required capability/session is revoked.

## WebSocket authentication and authorization

The native Tauri transport sends `Authorization: Bearer <access-token>` in the HTTP upgrade request and requests an exact versioned subprotocol. `tokio-tungstenite` supports caller-built requests; the flow is described for exec in [`exec.md`](exec.md) and logs in [`logging.md`](logging.md).

If a WebView must open a socket directly:

1. Tauri calls an authenticated HTTPS ticket endpoint through its Rust network layer.
2. Policy authorizes the exact normalized target/options and reserves concurrency.
3. The server returns a random, single-use ticket bound to subject, `sid`, capability, cluster/resource IDs, request hash, protocol, allowed origin, and 30-second expiry.
4. The browser uses only that ticket in the upgrade. Prefer a one-time subprotocol value or first protocol message over the URL; if browser API constraints force a query ticket, access logging must redact the complete query before it reaches any sink.
5. The accepting controller consumes the ticket atomically. A replay or a different target/origin fails.

For every socket:

- require WSS outside named loopback development mode;
- validate `Origin` against exact configured Tauri/Web origins when present. Native clients may omit it, but origin never replaces bearer/ticket authentication;
- authorize before `101`, then authorize the authoritative resource after resolving target IDs and before opening Docker/internal streams;
- enforce subprotocol, first-message deadline, frame/message limits, byte rates, bounded queues, concurrency, idle and maximum lifetime, ping/pong, and graceful close;
- disable compression for credentials and exec; enable it for logs only after side-channel/resource analysis;
- re-check session/principal/capability on authorization events and at least every minute for long sessions; close on expiry, logout, suspension, role/scope removal, target replacement, or node removal;
- audit open/ready/end and safe byte counts, never credentials, complete URL, log/terminal payload, secret/config values, or raw Docker errors.

An access token expiring during a socket does not automatically prove the session was revoked. The server checks the server-side session and caps the stream lifetime; the client obtains a new ticket on reconnect. Exec is never automatically replayed.

## CSRF, redirects, CORS, and public HTTP security

### OAuth and setup CSRF

- All OAuth/OIDC and GitHub manifest `state` values are high-entropy, single-use, transaction/provider/redirect-bound, short-lived, and compared before code exchange.
- PKCE uses S256 only. OIDC also uses and verifies `nonce`; these controls are complementary and simplify provider interoperability.
- Redirect URIs are preconfigured canonical constants, not derived from `Host`, arbitrary `Forwarded`, query parameters, or a desktop-supplied callback. Exact matching and no open redirectors are required by RFC 9700.
- Trust forwarded scheme/host/client IP only from configured proxy addresses or authenticated proxy mTLS identities. Strip incoming forwarded headers at direct ingress.
- Callback error pages do not echo provider error details or query parameters. Correlation IDs link to restricted logs.

Normal APIs use bearer headers and no ambient browser cookie, so classic cookie CSRF does not apply. If a future browser console introduces cookies, it must use `Secure; HttpOnly; SameSite=Lax/Strict` as appropriate plus an origin-bound CSRF token for state-changing requests; that is a distinct client profile, not a reason to add cookies to Tauri.

### CORS and headers

Disable CORS by default for the native client. A browser UI needs an exact configured origin list, exact methods/headers, correct `Vary: Origin`, and no wildcard with credentials.

Public responses set, as appropriate:

```text
Strict-Transport-Security: max-age=31536000; includeSubDomains
X-Content-Type-Options: nosniff
Referrer-Policy: no-referrer
Permissions-Policy: camera=(), microphone=(), geolocation=(), payment=(), usb=()
Cross-Origin-Opener-Policy: same-origin
Cache-Control: no-store
```

Enable HSTS only on a verified HTTPS production domain whose subdomains are safe; never on loopback development. OAuth/setup HTML additionally uses `frame-ancestors 'none'`, `base-uri 'none'`, no third-party resources, and a strict CSP. A manifest auto-submit page may allow only a nonce-bearing inline script and exact `form-action https://github.com`; ordinary callback pages use `default-src 'none'`. Remove framework/version banners and provide JSON error types without stack/daemon/database details.

### Rate and admission limits

Use layered token buckets and concurrency semaphores keyed by trusted source IP, principal/session/service token, route class, resource target, and global capacity. In HA, security-relevant limits are shared/distributed; per-process limits remain defense in depth.

Initial values to load-test:

| Surface | Initial policy |
| --- | --- |
| Bootstrap/recovery | 5 attempts/minute/source, 10/hour cluster-wide; exponential delay; close window after repeated failure |
| Auth transaction | 10 starts/minute/source, 5 outstanding/desktop installation, 100/minute cluster-wide |
| OAuth callback/code exchange | 30/minute/source/provider plus provider API backoff; one exchange per transaction |
| Refresh | 10/minute/session, 60/minute/principal, replay-family revocation |
| Read API | 300/minute/principal with endpoint cost weights and bounded fan-out |
| Mutation API | 30/minute/principal; destructive/security endpoints 5/minute and no automatic retry |
| Failed bearer/service auth | 30/minute/source and global circuit/alert; constant generic response |
| Log/exec ticket | 30/minute/principal; concurrent limits from stream specifications |
| Enrollment | 5/minute/token/source, bounded CSR/body, disabled without active window |

Return `429` with bounded `Retry-After` where disclosure is safe. Apply body/header limits before deserialization, authentication failure limits before expensive signature/provider work when possible, and authenticated resource quotas before Docker work. Do not use IP as the only identity because NATs/proxies aggregate users.

## Public TLS and proxy trust

Kontainer v2 is a new protocol and should require TLS 1.3 by default. The current IETF BCP states that new protocols using TLS must require TLS 1.3 and may retain TLS 1.2 only as a non-default deployment option ([RFC 9852](https://www.rfc-editor.org/rfc/rfc9852.html)).

Supported production modes:

1. **Controller terminates TLS:** rustls serves a configured ACME/operator certificate, validates SNI/Host against the canonical URL, and redirects or does not bind plaintext HTTP.
2. **Trusted ingress terminates public TLS:** ingress uses TLS 1.3 publicly, then TLS (preferably mTLS) to a private controller listener. The controller accepts forwarding headers only from that ingress identity/network. The controller must not be reachable directly on the same listener from untrusted networks.
3. **End-to-end passthrough:** ingress load-balances TCP and the controller terminates TLS. This simplifies application awareness but may constrain routing/health behavior.

In all modes:

- the advertised issuer/callback/base URL is one exact configured HTTPS origin;
- validate server certificate hostname and chain on Tauri, provider, KMS, database, and any remote Docker connections;
- disable TLS 0-RTT for authentication, mutation, enrollment, refresh, and ticket creation because TLS early data is replayable ([TLS 1.3 replay considerations](https://www.rfc-editor.org/info/rfc8446/));
- separate public, controller/database, and internal agent networks and ports;
- public liveness returns only generic process status; readiness/dependency detail is internal/authenticated;
- certificate issuance, renewal, expiry, handshake failures, and trust-bundle versions are observable without logging key material.

## Credential encryption and key management

### Secret classes and storage

| Secret | Storage and access |
| --- | --- |
| Provider client secret/private key/refresh token/webhook secret | Per-record envelope encryption; decrypt only in provider adapter; separate purposes and rotation metadata |
| Access/delegation/audit signing private key | External KMS/HSM preferred; encrypted `0600` file/Docker secret-backed KEK for supported small profile; separate key per use |
| Root/intermediate CA key | Root offline; online intermediate only in enrollment issuer/KMS, never node agents; see cluster identity section |
| Refresh/service/bootstrap/enrollment/poll tokens | Plaintext only at issue/use boundary; keyed hash in DB; pepper outside DB |
| Recovery code | Display once; Argon2id PHC hash with per-code salt; rate limited and used only with local recovery window |
| Workload private key | Generated on workload, non-exported where possible, permission-restricted per-node volume/keystore; never returned by enrollment |
| Docker Swarm secret/config input | Request-memory only until sent to Docker; never persisted/audited/logged by Kontainer unless a future secret broker is explicitly designed |

Envelope format:

1. Generate a random 256-bit data-encryption key (DEK) per secret record/version.
2. Encrypt with an audited AEAD, initially AES-256-GCM, using a unique 96-bit nonce and associated data containing cluster ID, table/row/field, purpose, and schema version.
3. Wrap the DEK with a versioned KEK in an external KMS/HSM. The minimum self-hosted profile loads a 32-byte KEK from a Docker secret/`0600` file and uses a separate wrapping key implementation.
4. Persist ciphertext, nonce, wrapped DEK, KEK ID/version, and AAD version. Never persist plaintext or the unwrapped DEK.
5. On rotation, new writes use the new KEK immediately; reads allow an explicit old-key set; a resumable background job rewraps DEKs without decrypting application plaintext where the KMS supports it. Verify fleet convergence before removing old keys.

Separate KEKs/peppers or KMS purposes for provider encryption, token verification, audit integrity, session signing, delegation signing, and CA issuance. One `APP_KEY` must not become a universal secret. Key access is least-privileged by controller workload identity and is itself audited by the KMS where possible.

Fail closed when a key version is unknown, ciphertext authentication fails, randomness fails, or KMS policy denies access. KMS unavailability makes new sessions/security mutations unavailable; cached resource reads may continue only without decrypting secrets and with an explicit degraded state. Do not silently regenerate keys against an existing database.

Backups contain the database, wrapped keys/ciphertexts, trust bundles, and separately protected key/KMS recovery material. Encrypt backups, restrict/monitor access, define restore ordering, and test restore plus post-restore session/certificate policy. OWASP's secrets guidance emphasizes lifecycle, envelope encryption, rotation, revocation, encrypted backups, and tested break glass ([Secrets Management Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html)).

### In-memory and error hygiene

- Use secret wrapper types whose `Debug`/`Display` are redacted; avoid `Clone`, serialization, and long-lived `String` where feasible.
- Keep decrypted values in the smallest synchronous scope; zeroize owned buffers on drop. Do not claim zeroization protects swapped pages, allocator copies, core dumps, or host-root inspection.
- Disable core dumps for production workloads where operationally acceptable; protect crash artifacts and never include request bodies/env/config.
- Build error types with safe public codes and restricted causes. Do not interpolate provider tokens, HTTP bodies, PEM, Docker env, SQL values, or headers into logs.
- Redaction is primarily type/schema based. A final pattern scanner catches common token prefixes and authorization headers, but regex masking is not the security boundary.

## Controller and node-agent identity

The full design is in [`cluster-communication.md`](cluster-communication.md). Security-critical decisions are summarized here so no public-auth implementation can substitute a weaker internal path.

### Alternatives

| Internal identity | Decision | Reason |
| --- | --- | --- |
| Plaintext gRPC + one `CLUSTER_SECRET` | **Rejected** | A captured/reused secret has cluster-wide authority, authenticates no node/workload, and is hard to rotate without full restart. |
| Overlay/network membership only | **Rejected** | Overlay peers and compromised workloads are not identities. Docker overlay encryption is optional and does not authorize application RPCs. |
| Reuse Docker Swarm node private key/certificate | **Rejected** | Unsupported coupling to daemon internals expands the blast radius of Docker's control-plane identity. Bind to Docker's node ID without reusing its key. |
| Manually provisioned per-workload mTLS | **Accepted for a non-production parity profile** | Establishes the correct runtime protocol before automated enrollment exists, but operational rotation/revocation does not scale. |
| Built-in Kontainer offline-root/online-intermediate PKI | **Accepted baseline** | Self-hosted, automatable, short-lived identities with a pluggable issuer boundary. Root recovery and issuer protection are production gates. |
| SPIFFE-shaped URI SANs without SPIRE | **Accepted identity format** | Makes trust domain/cluster/workload/node policy precise while keeping the baseline deployable. |
| SPIFFE/SPIRE or `step-ca` issuer | **Accepted integration, deployment choice** | Higher-assurance workload API/attestation/rotation for operators already running it; must produce the same verified identity model. |
| Join/enrollment bearer | **Accepted for bootstrap only** | A one-time, short-lived, scope-bound token can authorize CSR issuance when combined with authoritative Swarm task/node evidence. It is never accepted by normal RPCs. |

### Workload identities and certificate policy

Use URI SAN identities such as:

```text
spiffe://kontainer.internal/swarm/<cluster-id>/controller/<instance-id>
spiffe://kontainer.internal/swarm/<cluster-id>/node/<node-id>/agent
```

Certificates carry only verified identities, correct client/server EKUs, and stable DNS SANs needed for TLS name verification. Authorization uses the verified URI SAN, trust chain, cluster ID, workload kind, node ID, certificate status, and current Swarm membership—not CN, source IP alone, hostname, protobuf claims, or service name.

Normal gRPC requires TLS 1.3, ALPN `h2`, server and client certificates, chain/name verification, explicit message/concurrency limits, and no insecure verifier in production. Node-agent ports remain private/unpublished. Controllers and agents use separate issuance policies and intermediates where practical.

Leaf lifetime starts at 24 hours and renews after roughly two-thirds with jitter. Renewal requires a still-valid mTLS identity, proof of the new key, and fresh task/node membership. Root rotation overlaps trust bundles until the whole fleet uses new leaves. Removal or quarantine denies new calls immediately through membership policy, closes sensitive streams, deny-lists the serial where needed, and lets short expiry bound offline exposure.

### Enrollment and join credentials

Initial enrollment runs on a separate private server-authenticated TLS endpoint exposing only `Enroll`; it is disabled when no window/ticket exists. The agent creates its key and CSR locally and supplies cluster/node/task/workload claims, boot nonce, protocol range, and token.

A production issuer must establish the claimed node by one of:

- a one-time ticket pre-bound to the discovered task/node;
- verified direct source-address-to-Swarm-task binding on the dedicated DNSRR control network, plus token/nonce and deployment tests proving source preservation;
- an external SPIRE/TPM/cloud workload attestation accepted by policy.

A reusable cluster enrollment secret without task binding is not a production gate. Tokens are 256-bit, hashed at rest, single-use, valid at most ten minutes, scoped to cluster/workload/service generation and node/task when known, delivered as Docker secrets, and deleted after issuance. The resulting certificate—not the token—authenticates normal RPC.

### Delegated end-user authority

mTLS proves “this is an authorized controller”; it does not prove “Alice may exec as root in service X.” For exec, image delete/prune, node-local mutation, and other host-affecting calls, the controller signs a `DelegationGrant` using a key distinct from TLS:

```text
typ=kontainer-delegation+jwt, kid, issuer_controller_id,
principal_id, session_id, capability, cluster_id, node_id,
resource_ids, request_id, operation_id, request_hash,
iat, nbf, exp (<=30 seconds), nonce, policy_version
```

The agent verifies the configured token type/algorithm/key use, that authenticated mTLS controller equals issuer, its own cluster/node is the target, capability/method/request hash/resource agree, time is valid, controller and agent membership are current, and nonce was not consumed. Maintain a bounded replay cache through grant expiry. Never forward roles as trusted metadata or give an agent a general public-access JWT validator.

### Removal and compromise

Removing an agent/node:

1. mark membership draining/removed from authoritative Swarm reconciliation;
2. reject new calls and delegation immediately on every controller;
3. cancel high-risk streams, drain safe short reads, and remove channels;
4. revoke/deny-list certificate serial and prevent renewal;
5. remove service task/secret access and best-effort erase local keys/journals;
6. retain identity/audit tombstones; rejoin requires a new task-bound enrollment and certificate.

On controller compromise, revoke its certificate and delegation-signing key, remove the workload, stop accepting its grants, rotate any accessible provider/session/encryption credentials according to incident scope, and inspect audit/operation records. Rotating a join token alone does not revoke an enrolled workload, just as Docker notes that rotating a Swarm join token does not change existing membership ([Docker Swarm join tokens](https://docs.docker.com/engine/swarm/swarm-mode/)).

## Docker socket threat model and least privilege

Docker documents that only trusted users should control the daemon and warns that a web/API parameterization bug can create arbitrary containers/mounts; remote unauthenticated daemon access can grant host root ([Docker Engine security](https://docs.docker.com/engine/security/), [protect daemon access](https://docs.docker.com/engine/security/protect-access/)). A read-only bind mount of the Unix socket does not make Engine methods read-only.

Mandatory controls:

- Keep the controller on managers for Swarm-scoped APIs and a node agent on each node for local APIs. Expose a typed Kontainer domain allowlist, never raw Docker method/path/headers/body or a socket tunnel.
- Resolve resource ownership from fresh manager state. A client never chooses Docker host, agent address, arbitrary Unix path, task IP, or raw container name when an immutable ID exists.
- Validate and normalize every Docker option. Maintain per-action allowlists for image references, environment mutation, shell/argv/user, prune filters, labels, and pagination; reject unknown fields.
- Split read and mutation adapters/interfaces and gate methods with capabilities, even though the underlying socket remains powerful. Keep node-local methods off controllers where not needed and Swarm manager mutations off agents.
- Run as a dedicated non-root UID when socket group access permits; drop all Linux capabilities, set `no-new-privileges`, read-only root filesystem, explicit writable volumes/tmpfs, seccomp/AppArmor/SELinux profile, PID/memory/CPU/FD limits, and no host PID/network/privileged mode.
- Mount only the local socket/named pipe. Do not enable `tcp://*:2375`; remote Engine is a separately approved profile with mTLS, exact hosts, and a threat review.
- Do not mount host root, `/proc`, or namespaces except a narrow read-only metrics source explicitly enabled. A metrics feature cannot silently expand the exec/API process's authority.
- Treat registry credentials, Docker auth headers, environment values, secret/config inputs, terminal/log payloads, and raw daemon errors as sensitive.
- Make confirmation/idempotency/audit controls precede destructive Engine calls; cancellation does not imply Docker rolled back an action.
- Detect daemon API version/capabilities and fail closed for unsupported semantics instead of sending a fallback raw request.

Evaluate Docker rootless mode for supported environments; Docker states that both daemon and containers then run without root privileges ([Docker rootless mode](https://docs.docker.com/engine/security/rootless/)). Swarm feature/operational limitations must be tested before claiming support. A Docker authorization plugin can fail closed on daemon requests ([Docker authorization plugins](https://docs.docker.com/engine/extend/plugins_authorization/)); it is defense in depth because it sees the powerful Kontainer service identity, not the end-user policy unless a safe attribution channel is designed.

A future small local privileged helper may own the socket while the public controller is sandboxed. It must still expose typed operations over local mTLS/Unix peer credentials; a generic local proxy merely moves the vulnerability.

## Audit architecture

### Events

Audit is distinct from debug/application logs. Record at least:

- bootstrap, recovery, provider setup/change/test, login/callback/refresh/logout, failed auth, session/token create/rotate/revoke/reuse, principal/role/scope/status change;
- enrollment/renewal/revocation/quarantine, trust/signing/encryption key change, proxy/TLS/security policy change;
- every authorization denial for sensitive actions and sampled/rate-controlled denials for noisy reads;
- every mutation request, authorization/confirmation decision, operation dispatch, per-target result/uncertainty, and completion;
- exec/log start/end, target route, effective shell/user/TTY, byte counts/duration/exit reason, but never I/O content;
- audit query/export/retention change and every attempt to access/decrypt a protected credential.

Canonical fields include event schema/type/version, event/request/operation/session IDs, occurred/received timestamps, controller/process identity, actor principal/identity/client/token public ID, source address after trusted-proxy processing, authentication method/assurance, capability, policy version/decision/reason, cluster/resource IDs and safe names, request hash, before/after safe metadata, target workload/certificate IDs, outcome/error code, byte/count/duration, and trace ID.

Do not record:

- authorization/refresh/service/bootstrap/enrollment/recovery/poll/WebSocket tickets;
- passwords, provider tokens, client secrets, private keys, webhook secrets, PEM, Docker registry auth;
- Docker secret values, config content by default, service environment values, log lines, terminal input/output;
- complete URLs/query strings, request/response bodies, raw provider/daemon/database errors;
- unnecessary email/IP/profile data beyond the documented retention/privacy purpose.

Use typed redaction and safe summaries before serialization. Escape control characters and bound every user-controlled field to prevent log injection and disk exhaustion.

### Integrity, availability, and retention

Within each cluster/partition compute:

```text
event_hash = HMAC(audit_integrity_key_version,
                  canonical_event_bytes || previous_event_hash)
```

Canonical encoding/version is fixed and test-vector driven. At least every five minutes and at clean shutdown, sign the current sequence/hash checkpoint using a separate audit-checkpoint key and export the checkpoint to an external operator-controlled sink. For higher assurance export events continuously to append-only/WORM storage. A chain without an external anchor cannot detect an attacker who rewrites both events and local chain state.

Authorization/audit intent for a high-risk mutation is committed transactionally with its operation record before dispatch. If durable audit/operation storage is unavailable, security administration and mutations fail closed. Read-only inventory may continue with a bounded local audit spool and visible degraded status; authentication failures and recovery never silently drop. Alert on sequence gaps, signature/chain failure, exporter lag, disk threshold, clock anomaly, and audit writer failure.

Initial retention tiers:

- authentication, access/security policy, recovery, enrollment/key, mutation/operation, exec metadata: 365 days;
- audit reads/exports and successful resource reads: 90 days or sampled according to policy;
- high-volume operational diagnostics: 30 days, in a separate non-audit stream.

Operators may increase these for legal/incident needs. Reducing retention requires Owner + recent auth, takes effect prospectively after a cooling period, and is audited/exported before deletion. Legal/privacy requirements can change the values; release defaults must never be represented as compliance certification. OWASP recommends tamper detection, read-only copies, restricted/monitored access, secure transport, redaction, and retention-aware disposal ([Logging Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html)).

## Rust implementation security

Prefer safe Rust throughout the auth, parser, policy, crypto-envelope, protocol, and audit crates. Set `#![forbid(unsafe_code)]` where dependencies/interfaces allow it. Any necessary unsafe/FFI belongs in a small private adapter with a written invariant, dedicated reviewers, Miri/sanitizer tests where applicable, and no untrusted data crossing unchecked bounds. Rust's own documentation emphasizes that unsafe operations carry contracts the compiler cannot verify and that soundness can depend on surrounding safe code ([Rust `unsafe`](https://doc.rust-lang.org/stable/std/keyword.unsafe.html), [Rustonomicon](https://doc.rust-lang.org/stable/nomicon/working-with-unsafe.html)).

Implementation boundaries:

- one authentication middleware builds typed principals for Axum/Tower; handlers cannot construct one from headers;
- one policy service owns capability/resource/context decisions for HTTP and stream admission;
- separate validators and keys/types for OIDC ID tokens, Kontainer access tokens, delegation grants, tickets, and audit checkpoints;
- secrets use non-serializable redacted wrappers; public DTOs cannot contain secret types;
- SQL queries use typed parameters and transactions; unique/check/foreign-key constraints enforce one-time and state-machine invariants;
- cryptographic randomness comes from the OS CSPRNG and failure is fatal for issuance;
- canonical URL/resource/claim parsing uses reviewed libraries and exact comparisons, not string prefix tests;
- async tasks are owned by cancellation trees; no detached auth/provider/audit/stream task can outlive its session or retain secrets indefinitely;
- panic at a request/task boundary becomes a safe internal error and alert; production is not compiled with `panic=abort` until cleanup/secret/audit consequences are reviewed.

## Supply-chain and release security

Rust's security policy explicitly says the toolchain assumes source and dependencies are trusted and reviewed; compiling malicious dependencies is not treated as a Rust vulnerability ([Rust security policy](https://rust-lang.org/policies/security/)). Memory safety does not remove dependency or build-script trust.

Production gates:

- commit `Cargo.lock`, pin the supported Rust toolchain, and build/test with `--locked`; Cargo documents that the lockfile records exact dependency resolution and `--locked` prevents silent changes ([Cargo dependency resolution](https://doc.rust-lang.org/cargo/reference/resolver.html)).
- allow crates.io plus an approved internal mirror only; pin git dependencies to reviewed commits or reject them; review features, proc macros, native code, and build scripts as executable build-time trust;
- run `cargo audit` against RustSec on every change and scheduled daily. RustSec is the Rust Secure Code WG advisory database and documents `cargo-audit`/`cargo-deny` integration ([RustSec](https://rustsec.org/)); define a time-bounded, owner-approved exception process rather than ignoring advisories globally;
- use `cargo deny` for advisories, licenses, duplicate-risk, and source allowlists; introduce `cargo vet` or equivalent reviewed attestations for privileged/security-critical dependencies;
- minimize/default-disable dependency features and prefer rustls over platform OpenSSL ambiguity unless a documented/FIPS profile requires another backend;
- run secret scanning, static analysis, tests/fuzzers, license review, and dependency diffs in an isolated CI job with least-privileged credentials and no production secrets;
- use a multi-stage, network-isolated final build from digest-pinned builder/base images; never run dependency mutation (`cargo update`) or copy `.env`, VCS credentials, or build secrets into an image;
- ship a minimal non-root image, read-only filesystem defaults, OCI/CycloneDX/SPDX SBOM, embedded dependency metadata, provenance/attestation, and image signature. Deploy by immutable digest and verify signature/policy before rollout;
- sign desktop installers/update metadata and agent images with separately protected release keys; support rollback prevention, staged rollout, and emergency revocation;
- publish a vulnerability-reporting channel, supported-version policy, security-advisory intake, and credential/image rotation runbook.

`cargo vendor`/source replacement can support hermetic builds, but Cargo notes its directory checksum protects against accidental modification and is not itself a malicious-change security mechanism ([Cargo source replacement](https://doc.rust-lang.org/cargo/reference/source-replacement.html)). Provenance, review, and isolated builders remain necessary.

## Verification strategy

### Unit and property tests

- bootstrap/recovery/token generators meet entropy/format requirements and fail on RNG errors;
- every state machine rejects invalid, concurrent, replayed, expired, and restart transitions;
- JWT validators reject `none`, wrong algorithm family, wrong `typ`/key use/`kid`, bad signature, duplicate claims, missing/incorrect issuer/audience/subject/session/token/time, excessive lifetime, cluster-A token at cluster B, ID token as access token, and delegation/access cross-use;
- refresh rotation/retry/reuse, family revocation, authorization epoch, idle/absolute expiry, signing-key overlap, clock leeway, and concurrent refresh have deterministic tests;
- policy tests generate every role x capability x scope x resource combination, explicit last-owner/delegation/recent-auth rules, list filtering, and multi-target all-or-nothing authorization;
- envelope tests cover AAD substitution, ciphertext/nonce/key version corruption, rotation/rewrap, KMS denial/unavailability, and no plaintext in serialized models;
- audit canonicalization/hash/signature/checkpoint tests use stable fixtures and detect edit/delete/reorder/splice/key confusion;
- redaction property tests feed every secret/token/header prefix and control character through error/log/audit paths.

### Provider integration tests

Run fake provider servers plus opt-in real-provider suites:

- state/nonce/PKCE success, mismatch, missing, replay, expiry, callback race, authorization denial, code reuse, provider mix-up, redirect/host injection, and callback cache/header policy;
- OIDC discovery issuer mismatch, HTTPS/redirect/SSRF limits, JWKS rotation and bounded unknown-`kid` refresh, signature/algorithm/audience/azp/time/nonce errors, malformed/oversize claims, group mapping and assurance;
- GitHub manifest state and one-hour code behavior, installation/account mismatch, insufficient permission, private active member, pending/non-member, org/user rename with stable ID, token expiry, API/rate-limit/outage stale policy, and webhook signature/delivery replay when enabled;
- prove upstream tokens never become Kontainer bearers and do not remain in DB/logs unless an explicitly enabled encrypted feature requires them.

### HTTP, desktop, and WebSocket tests

- every non-health route is covered by authentication and a named capability; route enumeration CI fails for unclassified new routes/RPCs;
- CORS, trusted proxy, canonical Host, TLS redirect, headers, CSP, cache, no-referrer, body/header/rate limits, and safe error contracts;
- a real Tauri test proves refresh credentials are in Keychain/Credential Locker/Secret Service, absent from `profiles.json`, WebView state/events, URLs, logs, and crash output; logout revokes server then removes vault entry;
- WSS header and ticket flows, single use, target/request/origin binding, wrong subprotocol, frame/byte/rate limits, session expiry/revocation/role change, node removal, and no account token/query logging;
- origin checks cover permitted Tauri origins, hostile browser origin, missing native origin, and spoofed origin from a non-browser client without treating origin as authentication.

### Internal identity and cluster tests

- TLS 1.3/mTLS handshake succeeds only for correct trust domain, cluster, workload kind, node ID, DNS SAN, EKU, validity, and allowed serial;
- reject CN-only identity, wrong CA/cluster/node/task, expired/not-yet-valid/revoked/quarantined cert, stale task IP reuse, source/claim mismatch, and production insecure verifier/shared secret;
- enrollment token one-use/expiry/scope/race, CSR proof, source-task binding or external attestation, renewal membership check, root/leaf rotation, issuer outage, and node removal/rejoin;
- delegation type/key/controller-peer/target/capability/request-hash/time/nonce validation and replay-cache bounds;
- revoke a live controller/agent/session during unary, logs, and exec and verify prompt policy behavior plus audit completion;
- run the chaos/security cases in [`cluster-communication.md`](cluster-communication.md), including partitions, rolling duplicate tasks, IP reuse, clock skew, and key rotation.

### Docker and adversarial tests

- fake daemon asserts every public action maps only to an approved Engine method/options and no client-controlled host/path/header/body passes through;
- IDOR/stale-resource tests recreate services/tasks/containers under names and IDs, move tasks, and change nodes between preview, authorization, delegation, and execution;
- real isolated Swarm tests run every role/scope, root/custom exec, secret/config mutation, node role, prune, cancellation/uncertain outcome, and audit record;
- fuzz OAuth/JWT/JWK/URL, JSON/protobuf, policy selectors, Docker multiplexed frames, WebSocket messages, audit canonical encoding, and archive/config parsers with size limits enabled;
- load/soak authentication failures, refresh storms, provider/JWKS outage, audit/KMS/DB pressure, slow sockets, and high-cardinality resource scopes; memory/tasks/FD/disk remain bounded.

### Supply-chain and deployment tests

- CI fails on unlocked dependency drift, disallowed source/license, unreviewed advisory exception, secret in image/history/SBOM/log, unsigned artifact, mutable production image reference, root user, writable rootfs, excess capability, or public agent gRPC port;
- inspect final images for `.env`, source/VCS metadata, package managers, shells where unnecessary, private keys, token fixtures, and unexpected setuid/capability binaries;
- deploy through supported direct-TLS and ingress-TLS profiles; prove forwarding headers are ignored from untrusted sources and proxy-to-controller protection works;
- encrypted backup/restore retains cluster identity/audit verification, forces documented session/certificate behavior, and works during key version overlap;
- commission an external penetration test before the production gate, focusing on bootstrap, OAuth/OIDC, IDOR/RBAC, WebSockets, SSRF, Docker option injection, secret leakage, and cluster identity.

## V1 parity, production gates, and deferred enhancements

### Secure v1-parity milestone

This milestone reproduces current product functions but is explicitly a non-HA compatibility profile:

- first-run GitHub App setup and organization-gated login, correct manifest/OAuth state, PKCE, one first Owner, recovery codes;
- principal/session models, five-minute access tokens, rotating refresh, server logout/revocation, Tauri OS-vault storage, header-only credentials;
- all HTTP/WebSocket actions classified and enforced, initially with system roles/scopes sufficient for the UI;
- authenticated WSS or single-use tickets for logs/exec, origin and lifecycle checks;
- manually provisioned per-workload mTLS for internal gRPC, correct identity/method policy, no plaintext shared secret;
- encrypted provider/signing credentials with a file/Docker-secret KEK, basic key rotation, typed security config;
- structured authorization/mutation/exec audit with local chain/checkpoint;
- single controller and SQLite allowed only with a visible non-production/non-HA warning.

Security correctness items above are parity foundations, not optional polish. Functional parity does not mean reproducing known vulnerabilities.

### V2 production release gates

- [ ] Generic OIDC plus the GitHub provider pass conformance/security tests; provider lifecycle and membership freshness are observable.
- [ ] Full role/capability/resource-scope matrix, custom roles, delegation limits, recent-auth/confirmation, and negative IDOR suite pass for every route/RPC.
- [ ] Shared production data store supports atomic sessions, revocation propagation, rate limits, operation/audit intent, HA controllers, backup/restore, and last-owner invariants.
- [ ] Public TLS 1.3/canonical origin/trusted ingress configuration, exact CORS/origin rules, security headers, and no insecure production switch are verified.
- [ ] Automated mTLS enrollment, renewal, revocation, root/intermediate rotation, authoritative membership binding, target-bound delegation, and removal/rejoin are complete.
- [ ] External KMS/HSM or explicitly supported hardened file-KEK profile, envelope rotation, key recovery, encrypted backups, and secret-leak tests pass.
- [ ] Refresh reuse detection, administrative/user/session/provider revocation, desktop vault behavior on macOS/Windows/Linux, and long-stream revocation pass.
- [ ] Audit redaction/integrity/checkpoint/export/retention, alerts, and high-risk fail-closed behavior pass; production supports an external append-only sink.
- [ ] Docker domain allowlist, workload sandbox, private networks, digest-pinned signed images, SBOM/provenance, dependency/advisory/vet gates, and vulnerability process are in release automation.
- [ ] Rate/resource/load/chaos/fuzz and external penetration-test findings meet accepted severity/SLO thresholds.
- [ ] Operator runbooks cover bootstrap, IdP/provider outage, owner recovery, session/service-token compromise, node/controller compromise, key/CA rotation, database/KMS/audit outage, backup restore, and security upgrade rollback.

### Deferred enhancements

- SPIRE/TPM/cloud workload attestation as an alternative to built-in Swarm-task-bound enrollment;
- SCIM, OIDC back-channel logout, GitHub/team webhooks, and continuous-access evaluation for faster deprovisioning;
- passkey/WebAuthn local accounts for air-gapped deployments, with no fallback password by default;
- DPoP/device-bound access tokens. RFC 9700 recommends sender-constrained tokens and identifies DPoP/mTLS ([OAuth 2.0 Security BCP](https://www.rfc-editor.org/rfc/rfc9700.html)); complexity and desktop key-recovery UX require a separate design;
- just-in-time elevation, dual approval, maintenance windows, PAM integration, and session recording metadata for highly regulated root exec (never record terminal content by default);
- policy-as-code (Cedar/OPA), only if the fixed capability/scope model proves insufficient and deterministic policy/version/audit behavior is preserved;
- external CI workload-identity federation and very short deploy credentials;
- privileged local Docker helper, rootless/profile-specific support, and Docker authorization-plugin end-user attribution;
- remote site/NAT agent identity/relay, post-quantum TLS/PKI profile, and hardware-backed desktop token keys;
- external transparency log/WORM audit service with independently witnessed checkpoints.

## Acceptance checklist

### Identity and bootstrap

- [ ] An unclaimed Internet-facing controller exposes no Docker/auth/internal capability and cannot be claimed without the local one-time credential.
- [ ] Bootstrap has at least 256 bits of entropy, is never logged/URL/env/image persisted, expires/consumes once, survives only according to documented restart behavior, and creates exactly one first Owner transactionally.
- [ ] Recovery requires local action plus one offline code, is time/attempt limited, revokes/reissues codes, and is regularly exercised.
- [ ] Principals use immutable issuer/subject IDs; email/login/org names are never identity keys.
- [ ] GitHub installation/account/active membership and OIDC issuer/audience/nonce/assurance rules fail closed as specified.

### Tokens, sessions, and desktop

- [ ] Access validators pin token type, key use, algorithm, issuer, cluster audience, subject, session/token IDs, times, lifetime, and authorization epoch.
- [ ] Refresh tokens rotate once, detect reuse, revoke a family, honor idle/absolute limits, and can be revoked per device/user/provider/cluster.
- [ ] Service tokens are opaque, hashed+peppered, one-time displayed, expiring, scoped, owned, rotatable, revocable, non-interactive, and separately audited.
- [ ] Tauri Rust owns all bearers; refresh tokens use the OS vault; access tokens remain in Rust memory; no token exists in WebView state, JSON profile, URL, clipboard, or logs.
- [ ] Logout and administrator/provider/security events revoke server state and close affected long streams within the documented bound.

### Authorization and transport

- [ ] Every HTTP route, WebSocket action/message, scheduled action, and gRPC method declares a capability, resource resolver, risk rules, and audit class.
- [ ] All system roles/custom scopes and IDOR/multi-target/recreation races have negative tests; UI hiding is never the only enforcement.
- [ ] High-risk actions require recent authentication, confirmation where defined, operation/idempotency identity, and durable audit intent.
- [ ] HTTPS/WSS and internal gRPC use TLS 1.3 by default; no plaintext/shared-secret/insecure-verifier production path exists.
- [ ] WebSocket account JWTs never occur in queries; tickets are single-use/short/target-bound; origin, subprotocol, limits, revocation, and audit are enforced.
- [ ] Proxy forwarding and source IP are trusted only from configured protected ingress; direct spoofing tests pass.

### Keys, cluster, Docker, and audit

- [ ] Database/backups do not reveal provider, signing, token, recovery, enrollment, or CA secrets without the separate KMS/KEK.
- [ ] Encryption/signing/CA/audit/token keys are purpose-separated, versioned, rotatable, recoverable, least-privileged, and never silently regenerated.
- [ ] Workload certificates bind trust domain/cluster/workload/node to current Swarm membership; enrollment, renewal, removal, revocation, and root rotation pass live tests.
- [ ] Sensitive node RPCs verify mTLS controller plus target/request-bound delegation and replay prevention.
- [ ] The public/internal API cannot select an Engine endpoint or express a raw Docker request; socket-owning workloads use documented hardening and private networks.
- [ ] Audit covers all required events, contains no forbidden data, detects tampering/gaps, anchors externally, enforces retention/access, and fails closed for high-risk writes.
- [ ] Release artifacts are locked, reviewed/scanned, SBOM/provenance-bearing, signed, digest-pinned, minimal, non-root, and pass supported deployment/restore/incident drills.

## Open decisions suitable for GitHub issues

1. **Ratify launch identity providers and GitHub App ownership model.** Decide whether production requires generic OIDC, whether per-cluster GitHub App manifests remain supported, whether Kontainer hosts a shared App, and exactly which App credentials/webhooks are retained. Blocks provider schema and setup UX.
2. **Choose OIDC conformance and group/lifecycle profile.** Define supported client authentication/signing algorithms, discovery/enterprise CA/SSRF policy, group claim mappings, assurance claims, back-channel logout, SCIM, and provider-outage freshness. Blocks enterprise support claims.
3. **Approve role/capability/scope matrix.** Ratify system roles, custom roles, stack/service/node hierarchy, logs/config/secret visibility, Operator env/image permissions, root exec, node-role and delegation rules. Blocks API authorization tests and migration.
4. **Set session and service-token policy.** Approve five-minute access, idle/absolute/recent-auth limits, maximum devices, provider revalidation, refresh retry/reuse semantics, service-token maximum/overlap, and whether DPoP is deferred. Blocks stable auth contract.
5. **Select access/delegation signing algorithms and FIPS profile.** Choose Ed25519 versus ES256/RS256 KMS profiles, libraries/backends, key publication/rotation, and cross-token typing. Blocks crypto implementation and compliance posture.
6. **Select production KMS/secret backends.** Define supported cloud KMS/Vault/file-KEK providers, envelope format, HA permissions, backup/root-key recovery, rotation SLO, and behavior during KMS outage. Blocks credential migration and production gate.
7. **Finalize bootstrap and break-glass UX.** Define Docker secret/local CLI delivery, TTL/restart semantics, recovery-code count/format, two-person or hardware requirements, ownerless repair, and destructive rebootstrap behavior. Blocks first-run deployment docs.
8. **Choose workload issuer and enrollment assurance.** Align with [`cluster-communication.md`](cluster-communication.md): built-in CA versus `step-ca`/SPIRE, task-source binding versus per-node tickets, dynamic node delivery, certificate lifetimes, CRL/denylist, CA backup/recovery, optional TPM. Blocks production agent rollout.
9. **Ratify public ingress/TLS support.** Choose controller TLS versus trusted proxy/passthrough, TLS 1.2 exception policy, ACME, forwarded-header identity, enterprise CA desktop UX, routing mesh, and public health exposure. Blocks production manifests.
10. **Choose audit sink, integrity, privacy, and retention.** Approve canonical encoding/chain/checkpoint key, external append-only/WORM targets, fail-closed classes, default tiers, PII handling, export format, and incident/legal hold. Blocks production operations and compliance statements.
11. **Choose shared rate-limit/revocation infrastructure.** Determine whether PostgreSQL alone is sufficient or Redis/event transport is required for HA cache invalidation, distributed admission, WebSocket closure, and abuse response. Blocks multi-controller security SLOs.
12. **Define Docker isolation support.** Decide supported rootless Engine configurations, authorization plugin, controller/node-agent sandbox, future privileged helper, Windows named-pipe support, and which socket-driven features are incompatible. Blocks hardened deployment matrix.
13. **Choose desktop credential-vault implementation and migration.** Select a maintained Rust abstraction for macOS/Windows/Linux, vault accessibility/user-presence policy, headless Linux behavior, plaintext v1 token deletion, profile URL migration, update signing, and test harness. Blocks secure session migration.
14. **Approve supply-chain policy and vulnerability SLA.** Define advisory severities/timelines, cargo-vet criteria/exceptions, approved registries/git/native/build dependencies, SBOM/provenance/signing tools, release-key custody, supported versions, and disclosure channel. Blocks release gate.

## Definition of done

The Rust agent security rewrite is complete only when an operator can securely bootstrap and recover a cluster; humans and automation receive distinct, revocable, scoped identities; every Docker-affecting action is authorized at both public and internal trust boundaries; tokens, provider credentials, CA/signing keys, and desktop refresh credentials have tested storage/rotation/revocation; removed users/nodes/controllers lose authority promptly; audit evidence is useful without containing protected payloads and has externally verifiable integrity; and the full provider, authorization, WebSocket, mTLS, Docker, chaos, restore, supply-chain, and penetration-test gates pass on every supported production deployment profile.

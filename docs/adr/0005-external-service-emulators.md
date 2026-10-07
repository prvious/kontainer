# 0005. External services are emulated, not mocked, when their features arrive

- Status: accepted
- Date: 2026-09-28
- Spec: none (applies to future specs)

## Context

The design integrates with GitHub (OAuth, GitHub App, GHCR, OIDC deploy policies), Slack, Discord,
outbound webhooks, and Traefik. All of those are deferred by this build plan. When they arrive, tests
that are as close as possible to the real service matter more than convenience. Vercel Labs publishes
`emulate` (v0.12.0, 2026-09-24), a stateful local emulator for GitHub, Slack, and others that speaks
the real protocols and delivers real signed webhooks.

## Decision

Per service, the test double to use:

| Service | Double | Notes |
|---|---|---|
| GitHub OAuth, GitHub Apps, installations, repo webhooks | `npx emulate --service github --seed dev/emulate.config.yaml` | Not supported: App manifests, device flow, GHCR, OIDC id-tokens. `GitHubLoginConfig` already exposes the three URLs to point at it. |
| Slack | `emulate --service slack` | Web API, OAuth v2, incoming webhooks, signed events. |
| Discord | `fauxcord` (tomacheese/fauxcord) or `minicord` | Stateful REST and gateway; pick when the feature is specified. |
| OIDC issuer for GitHub Actions deploy policies | `navikt/mock-oauth2-server` container | Only if the Actions OIDC exchange is built. |
| Container registry (GHCR stand-in) | `registry:2` service in the DinD stack | Speaks the same Registry HTTP API v2. |
| Outbound webhooks | An in-test `axum` listener | Zero dependencies; assert on received requests. |
| Traefik | A real Traefik service in the DinD stack with Prometheus metrics and JSON access log | No emulator needed. |

Rules: Node is acceptable inside Rust integration tests that spawn `emulate`, and such tests skip
with a printed reason when `node` is absent. Emulator seeds live in `dev/`. No hand-written HTTP mocks
of third-party APIs beyond a handful of lines.

## Consequences

- GitHub issue #130 (local GitHub emulator) is answered by this ADR and closes with the build plan.
- Nothing is installed now. The first spec that touches one of these services adds its double and
  the `pnpm dev:<service>` script.

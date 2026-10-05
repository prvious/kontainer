# 0028. Integrations hub and GitHub page as placeholders, and a kit breadcrumb

- Status: accepted (amended by 0041)
- Date: 2026-09-28
- Spec: docs/build/18-integrations.md

## Context

Spec 18 makes the Integrations hub and the GitHub integration page reachable from Settings.
Every integration behind them is deferred (ADR 0010), so both pages keep the design's layout
and show no data. The pages need the `Settings › Integrations` breadcrumb from `lXH8d`, and
the app had no breadcrumb yet. Design gap G15 suggests a disabled CONNECT button on the GitHub
card, while the spec and ADR 0010 ask for skeleton lines where each card's description and
footer would be.

## Decision

- **Routes.** The pages are `/settings/integrations` and `/settings/integrations/github`.
  Settings' INTEGRATIONS row now opens the hub (it was restricted in spec 17).
- **Hub.**
  - Four cards, GitHub, Slack, Discord, and Webhooks. Each has its design icon (lucide
    `github`, `hash`, `message-circle`, `webhook`), its name, and the `NOT CONNECTED` badge.
  - The description and footer are static skeleton lines. Only the GitHub card is a link.
  - The Traefik panel has its `route` tile, title, `NOT CONFIGURED`, and a skeleton block
    for the chart and footer.
  - The spec and ADR 0010 win over G15's disabled CONNECT: the footer is a skeleton, so the
    card offers nothing it can't do.
- **GitHub page.**
  - The header carries `NOT CONNECTED`, followed by the one info Alert
    `GitHub connects when deployments arrive.` (G23).
  - OAUTH CONNECTION, CONTAINER REGISTRY (GHCR), OIDC DEPLOY POLICIES, and DANGER ZONE
    (red, as designed) show their headers over skeleton rows. There are no buttons.
- Neither page calls a Tauri command. The tests render them outside the signed-in shell to
  prove it.
- **Breadcrumb in the kit, not Shadcn.**
  - The Shadcn CLI's current `breadcrumb` registry item for this style lists an npm package
    named `cn` as a dependency, and imports `cn` from it. The project's own CLI tried to
    `pnpm add cn` and failed. The latest CLI wrote the file with that import.
  - Adding an unknown package breaks the dependency rule and is a supply-chain risk. Fixing
    the generated file by hand breaks the rule for `components/ui`.
  - So `components/kit/breadcrumbs.tsx` is a small `nav` / `ol` built on TanStack `Link`,
    styled from `lXH8d`. It can be swapped for the Shadcn component once the registry item
    is fixed.

## Consequences

- Promoting any integration follows ADR 0010: a design pass for its gap, a spec, and an ADR
  that replaces the placeholder.
- No server work: the controller has no integration routes.

# 18. Integrations

## Goal

The Integrations hub and the GitHub integration page as honest placeholders, reachable from
Settings, so the navigation and layout exist for when GitHub, Slack, Discord, Webhooks, and
Traefik are built. No server work.

Design: hub `r9tns` (light `r7MNZ2`); GitHub `kx4iP` (light `WOSLC`). Deferred entirely with no
routes: Slack `7Q6xY`, Discord `xInGN`, Webhooks `TZXLI`, Traefik `lXH8d`, and states `SVasd`,
`lpHG9`, `pYEjL`, `N33N3`, `LsFKQ`, `Cb3jH`, `UHYJB`, `LwQE9`. Gaps G14, G15, G23. Decisions:
ADR 0005, ADR 0010.

## Desktop

- Routes `/settings/integrations` and `/settings/integrations/github`, both with the breadcrumb
  `Settings › Integrations` (› `GitHub`) from `lXH8d`.
- Hub: header `INTEGRATIONS`, `// manage external service connections`. Cards grid (`Xmpv8`)
  with GitHub, Slack, Discord, Webhooks: icon tile and name as designed, badge `NOT CONNECTED`
  (`hcuLC`), and skeleton lines where the description and footer would be (ADR 0010). Only the
  GitHub card is clickable. Below, the Traefik panel (`xu4By`) with its identity tile and title,
  badge `NOT CONFIGURED`, and a skeleton block where the chart and footer sit.
- GitHub page: header `GITHUB INTEGRATION`, `// repository access & deployment policies`, badge
  `NOT CONNECTED`; one info Alert `GitHub connects when deployments arrive.`; the four sections
  (OAUTH CONNECTION, CONTAINER REGISTRY (GHCR), OIDC DEPLOY POLICIES, DANGER ZONE) render their
  section headers with skeleton rows beneath. No buttons.
- Icons: use lucide `github` for GitHub; for Slack, Discord, and Webhooks use the design's tiles
  exported as SVG through `Export(..., "html-css")` or a plain lucide fallback (`hash`,
  `message-circle`, `webhook`) when the exported vectors are not clean.

## Tests

- Both routes render with the breadcrumb; the hub shows four cards with `NOT CONNECTED`; only the
  GitHub card navigates; no Tauri command is invoked by either page.

## Done when

- Settings → Integrations → GitHub is navigable and matches the design's structure with skeleton
  content; all checks pass; `PROGRESS.md` updated. This is the last spec: run the full check set
  from `00-goal.md` rule 8 and record the output in `PROGRESS.md`.

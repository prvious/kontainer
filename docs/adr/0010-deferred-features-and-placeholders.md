# 0010. Deferred features and how their placeholders render

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/00-goal.md, docs/build/design-gaps.md

## Context

The design draws the whole product, including sections the reset doc defers and sections that need
external services. The owner wants a usable desktop with every basic operator feature now, and honest
placeholders for the rest without "coming soon" marketing copy.

## Decision

Deferred entirely, no navigation entry, no route:

- Deployments (overview, details, history, realtime) and everything that depends on GitHub push.
- Secrets & Vault, plus the old Swarm secrets and configs pages.
- Deploy stack, create service, container and service detail pages (no design).
- Node enrollment (real certificate issuance); drain, availability, and role actions.
- Volume Explorer and File Preview backend.
- Traefik integration page and scraping.
- GitHub login and the GitHub App manifest screen (ADR 0001).
- Roles, capabilities, recent-auth (ADR 0002).

Rendered as placeholders inside live pages:

- A deferred panel renders the Foundations skeleton components (`tLPEH` stat, `ZZAe7` rows) in
  its designed slot, permanently, with no shimmer animation and no copy.
- A deferred button renders disabled with a plain tooltip: "Not built yet".
- The Integrations hub shows GitHub, Slack, Discord, and Webhooks cards as skeletons except the badge
  text NOT CONNECTED; the Traefik panel is a skeleton.
- The Overview Traefik banner renders its skeleton variant (`PsbU8` state) when Traefik is not
  configured, which is always in this phase.

## Consequences

- The app never shows fake numbers. Placeholders are visibly unfinished, which is acceptable for one
  user.
- Promoting a deferred feature means: a design pass if `design-gaps.md` lists a gap, a spec in
  `docs/build/`, and an ADR superseding the relevant line here.

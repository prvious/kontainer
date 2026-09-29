# 0024. The Overview, controller reachability, and paused changes

- Status: accepted
- Date: 2026-09-28
- Spec: docs/build/14-overview.md

## Context

Spec 14 makes `/` the Cluster Overview, built from earlier specs' queries. It also adds an
app-wide notion of "controller unreachable": cached figures stay on screen, dimmed, and every
change is paused until a probe answers again. The spec defines unreachable as the health probe
query's `failureCount >= 3`. TanStack Query v5 resets `failureCount` at the start of every fetch,
so with `retry: false` it never passes 1.

## Decision

- **Reachability.**
  - The health probe (`/health/ready`, every 15 s) now throws when the controller is
    unreachable, and retries twice, 1 s apart. "Three failures" therefore means three attempts
    of one fetch.
  - The query then stays in error, including while later fetches retry, until a probe answers.
  - A `ConnectivityProvider` in `_authenticated` exposes `reachable`, the latest probe,
    `reconnectedAt`, and `retry`.
  - The titlebar dot turns red on the first failed attempt. RECONNECTING and the cached-data
    state wait for the third.
  - `reconnectedAt` is recorded while rendering the flip back ("storing information from
    previous renders"), not in an effect.
  - An outage shows about 17 s after the last good probe at worst. The dev cluster showed it
    well inside the spec's 20 s.
- **Paused changes.**
  - `ActionButton` and `RowActions` entries take `mutates`. While unreachable, those entries are
    restricted and a Tooltip Rich says "Changes are paused while offline".
  - `ConfirmDialog` is always treated as a change, and its Enter-to-submit is gated too.
  - A row menu with changes in it shows the same line as a label, because menu items can't
    carry tooltips.
  - These are marked: header CREATE / PULL buttons, their dialog submits, bulk container
    actions, and every row action that changes state.
- **Overview.**
  - The subtitle reads `for <label> · N nodes · swarm active|degraded · synced 12s ago`. When
    unreachable it reads `stale — last synced HH:MM:SS` in amber, as the design draws it.
  - While unreachable, the warning Alert uses the spec's copy with RETRY NOW. The design's
    "retrying in 8s" countdown isn't built, because the query's interval timer isn't
    observable.
  - A success Alert shows for 5 s after `reconnectedAt`, derived from the shared clock.
- **Cards.**
  - CONTAINERS: live rows by state. CPU: the summary's mean. "across N nodes" counts the online
    sampled nodes that mean is taken over. The peak is the busiest bucket of the `1h` / `60s`
    cluster series.
  - MEMORY comes from the summary. NETWORK I/O sums receive and transmit over online nodes'
    latest samples. `up` is transmit and `down` is receive.
  - The CPU ring's tooltip lists every node. A node without a live sample reads `unreachable`.
- **Traefik.** The banner is always the skeleton variant, with its tile and title (ADR 0010).
- **Table.**
  - `ContainersTable` gained a title, optional selection, a header slot, and a dimmed state. Its
    remove dialog and action runner are now shared with the Containers page.
  - The Overview filters by `?q=` and `?state=`, orders busiest CPU first, then caps at 50. The
    count badge reads `50 OF N` when capped.
  - VIEW ALL opens `/containers` with the same search and states.
  - Headers don't re-sort: sorting the capped 50 would misrepresent the rest.
  - There is no bulk selection.
- **Hover.** The page draws no sparkline outside the Traefik skeleton. SparkBars hover stays as
  built on the Containers and Nodes strips, and the page's hover state is the CPU ring.

## Consequences

- Other pages keep their own error states. Only mutations are gated app-wide, as the spec asks.
- The root path no longer redirects to `/containers`, so the sidebar's Overview link is live.

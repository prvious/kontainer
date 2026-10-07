# 14. Overview

## Goal

The landing page: four metric cards, the Traefik banner as a skeleton, and the running containers
table, with first-load skeletons, stale-data handling when the controller is unreachable, and chart
hover. Everything on it is composed from earlier specs.

Design: Overview `WCBzg` (light `gQVTN`); states `PsbU8` (03.S1 first load), `fG7X1` (03.S2
controller unreachable, stale data), `UWkEX` (03.S3 chart and metric hover); captions `I2yVr`,
`QrqBZ`, `s6MKQX`. Chart rules `EjLoT`, `B64BE`.

## Data

Cluster summary (spec 13), cluster samples (spec 05), containers (spec 07), the controller health
probe (spec 04). No new server work.

## Page (`/`)

- Header: `CLUSTER OVERVIEW`, subtitle `for <connection label> · N nodes · swarm active · synced
  12s ago` (relative to the summary query's `dataUpdatedAt`, ticking through a `useNow` clock
  hook), REFRESH (spins while any of the page's queries refetch), DEPLOY disabled with tooltip
  "Not built yet".
- Metrics row (`ypkH5`), four cards:
  - CONTAINERS: `running / total`, a StackedTrack of running / paused / exited with the legend
    `21 RUNNING · 2 PAUSED · 1 EXITED`.
  - CPU USAGE: `47 %` with a Ring, subtitle `across N nodes · peak X%` where peak is the maximum
    bucket of the last hour of cluster samples.
  - MEMORY: `12.4 GB / 24 GB`, subtitle `52% allocated · 11.6 GB free`.
  - NETWORK I/O: `1.2 GB/s` (rx + tx summed over online nodes), subtitle `up X · down Y`.
- Traefik banner (`Ykc5k`): always the skeleton variant from `PsbU8` in this phase (ADR 0010),
  keeping the identity tile and the `TRAEFIK` title so the layout matches.
- Container table section (`O2CiT`): title `RUNNING CONTAINERS` with the count badge, search box,
  an `ALL STATES` filter button (state filter only), no bulk selection, the same DataTable and row
  actions as spec 07, sorted by CPU descending, capped at 50 rows with a `VIEW ALL` link to
  `/containers`.
- Sidebar CONTAINERS badge equals the total from the containers query.

## States

- First load (03.S1): only on first load or right after switching connection (the query cache is
  cleared on login already): SkeletonStat × 4, the Traefik skeleton, SkeletonRow × 6, subtitle
  `syncing cluster state…`. Background refetches never blank the page: every query uses
  `placeholderData: keepPreviousData` and only the REFRESH icon spins.
- Controller unreachable (03.S2): a `ConnectivityProvider` in `_authenticated` derives
  `reachable` from the health probe query (spec 04) with `failureCount >= 3` meaning unreachable,
  and records `reconnectedAt` when it flips back. While unreachable: a warning Alert at the top of
  the page reads `Showing cached data — controller unreachable` with `Last successful sync
  14:32:05 (2m ago). Changes are paused until the connection returns.` and RETRY NOW (refetches
  the probe immediately); every figure is dimmed and stamped `as of 14:32`; the status bar reads
  `RECONNECTING`; mutating actions across the app are disabled with a Tooltip Rich that says why.
  For five seconds after `reconnectedAt` a success Alert replaces it (derived from `useNow`, no
  timers in effects).
- Hover (03.S3): SparkBars snap a crosshair to the nearest bucket, keep that bar in the accent
  colour and drop the rest to `--primary-deep`, and show a ChartTooltip with the bucket time and
  values; keyboard left and right move the crosshair. Hovering the CPU ring shows a per-node
  breakdown tooltip where unreachable nodes say `unreachable`, never `0%`. Every icon-only action
  has a label tooltip after 400 ms.

## Tests

- Card math from a fixed summary and sample fixture (peak, free memory, network sum).
- First-load skeletons vs refetch spinner; stale state after three failures dims figures and
  disables a mutation button; success alert appears after reconnect and is gone after 5 s of
  clock advance; ring tooltip shows `unreachable` for an offline node; table cap and VIEW ALL.

## Done when

- The dev cluster renders live cards and the table; stopping the controller for 20 s shows the
  cached-data state and recovery shows the success alert; all checks pass; `PROGRESS.md` updated.

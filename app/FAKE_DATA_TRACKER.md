# Fake & Placeholder Data Tracker

Places in the dashboard UI using fake, placeholder, or hardcoded data instead of real backend data.

## 1. Activity Feed — `components/dashboard/activity-feed.tsx:23-33`

8 hardcoded placeholder events (service updates, errors, restarts, etc.). Comment on line 23 says _"this feature doesn't have backend support yet"_. No real data is fetched at all.

## 2. Resource Charts — CPU/Memory History — `components/dashboard/resource-charts.tsx:13-30`

`generateFakeTimeSeries()` creates 24 fake data points using `Math.random()` jitter on real current values. Used as fallback when `history.length <= 1` (line 79).

## 3. Resource Charts — Network I/O — `components/dashboard/resource-charts.tsx:32-45`

`generateFakeNetworkData()` generates 12 random inbound/outbound data points. This is **always** fake — there's no real network metrics collection. The chart subtitle even says _"placeholder data"_ (line 168).

## 4. Container Table — `components/dashboard/container-table.tsx:53-81`

`buildPlaceholderRows()` generates fake container rows with random IDs (`task-001`...), random CPU/memory via `Math.random()`, and fake node assignments. Used as fallback when no agent metrics are available (line 100-105).

## 5. Stats Cards — Network I/O — `components/dashboard/stats-cards.tsx:194-201`

The 4th stat card shows `"—"` as value, `"Coming soon"` as trend, and `"not yet collected"` as detail. No backend support for network I/O metrics.

## 6. Logs Page — `routes/_authenticated/logs/index.tsx:18-23`

Entire page is a placeholder — just shows _"Centralized logging coming soon"_ with an icon. No real functionality.

## 7. Settings Page — `routes/_authenticated/settings/index.tsx:18-19`

Same — just _"Settings coming soon"_ with a placeholder card. No real functionality.

---

## Real vs Fake Summary

| Component                       | Real Data                        | Fake Data                            |
| ------------------------------- | -------------------------------- | ------------------------------------ |
| Stats Cards (Nodes/CPU/Memory)  | Nodes, CPU, Memory from hooks    | Network I/O card                     |
| Resource Charts                 | CPU/Memory current snapshot      | Historical time-series, all Net I/O  |
| Container Table                 | Task counts from `useTaskStats`  | Individual container rows (fallback) |
| Services List                   | All real from `useServices`      | —                                    |
| Activity Feed                   | —                                | Everything                           |
| Logs page                       | —                                | Everything                           |
| Settings page                   | —                                | Everything                           |

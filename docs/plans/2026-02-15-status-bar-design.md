# Status Bar Design

## Goal

Add a persistent bottom status bar to the Kontainer desktop app showing cluster health and resource utilization at a glance — making the app feel like a real desktop infrastructure tool.

## Placement

A thin (~28px) bar pinned to the very bottom of the window, spanning full width below the sidebar + content area. Uses `bg-card` with a top `border-border` for subtle separation. Same pattern as VS Code.

```
┌─────────────────────────────────────────────────────────────────┐
│ [Sidebar]  │  [Header]                                         │
│            ├────────────────────────────────────────────────────┤
│            │                                                    │
│ Navigation │              Main Content (Outlet)                 │
│            │                                                    │
├────────────┴────────────────────────────────────────────────────┤
│ * production-swarm  |  3 nodes  |  42/42 tasks   CPU 34% MEM 61% │
└─────────────────────────────────────────────────────────────────┘
```

## Content

### Left side — Cluster Health (interactive)

| Item | Display | Click | Warning state |
|------|---------|-------|---------------|
| Cluster name | Colored dot + label | Opens cluster switcher popover | Dot red if agent unreachable |
| Nodes | `3 nodes` with icon | Navigate to `/nodes` | Amber + `2/3 nodes` if any down/draining |
| Tasks | `42/42 tasks` with icon | Navigate to `/tasks` | Amber if running < desired, red if failures |

### Right side — Resource Summary (interactive)

| Item | Display | Click |
|------|---------|-------|
| CPU | `CPU 34%` | Navigate to dashboard `/` |
| Memory | `MEM 61%` | Navigate to dashboard `/` |

CPU and MEM are aggregated across all nodes. Text color shifts: default <70%, amber 70-90%, red >90%.

## Data Sources

All data comes from existing React Query hooks — no new backend endpoints needed.

- **Cluster name/status**: `useAuth()` — `activeProfileId`, `agentUrl`, `label`
- **Nodes**: `useNodes()` — count, filter by status/availability
- **Tasks**: `useTaskStats()` — `running`, `desiredRunning`, `failed`, `total`
- **CPU/MEM**: `useClusterMetrics()` — aggregate `cpuPercent` and `memoryPercent` across all agent snapshots

## Behavior

- Updates automatically via React Query 5s refetch interval
- Shows skeleton pulse during loading states
- Shows `--` for values when disconnected or data unavailable
- Interactive: each segment is clickable to navigate to the relevant page

## Non-goals (v1)

- Terminal session count
- User avatar / profile info
- Refresh interval indicator
- Disk usage gauge

# Dashboard (Cluster Overview) Redesign — Plan 3a of 5

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the dashboard page to match the new "Cluster Overview" design — metric cards, traefik banner, and a running containers table with live CPU/MEM data.

**Architecture:** Replace the current dashboard (StatsCards + ResourceCharts + ServiceHealthGrid + ActivityFeed) with the new layout: page header with action buttons, 4 metric cards, a traefik status banner (placeholder), and a running containers table. The table joins `useTasks()` data with `useClusterMetrics()` container metrics for live CPU/MEM per container. Some features are NOOP/placeholder (see `docs/missing.md`).

**Tech Stack:** React, TanStack Query, Tailwind CSS v4, Lucide icons

**Design reference:** `docs/design.pen` — screen `WCBzg` (Desktop App / Cluster Overview)

**What's real vs placeholder:**

| Element | Real data | Placeholder |
|---------|-----------|-------------|
| Containers count + status breakdown | ✅ `useTaskStats()` | |
| CPU % + bar | ✅ `useClusterMetrics()` avg | |
| Memory GB + bar | ✅ `useClusterMetrics()` sum | |
| Network I/O card | | ✅ Static "1.2 GB/s" |
| Traefik banner | | ✅ Static (no traefik integration) |
| REFRESH button | ✅ `queryClient.invalidateQueries()` | |
| DEPLOY button | | ✅ NOOP |
| Table: status, name, image | ✅ `useTasks()` | |
| Table: CPU, MEM | ✅ `useClusterMetrics().containers` joined by containerId | |
| Table: ports | | ✅ Shows "—" |
| Table: uptime | ✅ Derived from task `timestamp` | |

---

## File Structure

### Files to delete
- `app/src/components/dashboard/stats-cards.tsx` — Replaced by new metric cards
- `app/src/components/dashboard/resource-charts.tsx` — Replaced (no charts in new design)
- `app/src/components/dashboard/service-health-grid.tsx` — Replaced (no service grid)
- `app/src/components/dashboard/activity-feed.tsx` — Replaced (no activity feed)

### Files to create
- `app/src/components/dashboard/metric-card.tsx` — Reusable metric card component (icon, label, value, subtitle, bar, badges)
- `app/src/components/dashboard/traefik-banner.tsx` — Static placeholder traefik status banner
- `app/src/components/dashboard/containers-table.tsx` — Running containers table with live metrics

### Files to modify
- `app/src/routes/_authenticated/index.tsx` — Rewrite dashboard page layout
- `app/tests/unit/dashboard-overview.spec.tsx` — Update tests for new dashboard

---

### Task 1: Create MetricCard Component

**Files:**
- Create: `app/src/components/dashboard/metric-card.tsx`

A reusable card for the 4 stats at the top. Each card has: colored icon + label, large value + subtitle, optional progress bar, optional status badges.

- [ ] **Step 1: Create metric-card.tsx**

```tsx
import type { LucideIcon } from "lucide-react";

interface MetricBadge {
    label: string;
    color: string; // tailwind bg class e.g. "bg-accent-green-20"
    textColor: string; // tailwind text class e.g. "text-accent-green"
}

interface MetricCardProps {
    icon: LucideIcon;
    iconColor: string;
    label: string;
    value: string;
    subtitle?: string;
    bar?: { percent: number; color: string };
    badges?: MetricBadge[];
}

export function MetricCard({ icon: Icon, iconColor, label, value, subtitle, bar, badges }: MetricCardProps) {
    return (
        <div className='flex h-[117px] flex-col gap-2 border border-border bg-card p-4'>
            {/* Label */}
            <div className='flex items-center gap-1.5'>
                <Icon className={`size-3 ${iconColor}`} />
                <span className='font-mono text-[9px] font-bold tracking-[0.5px] text-muted-foreground'>{label}</span>
            </div>

            {/* Value + subtitle */}
            <div className='flex items-end gap-2'>
                <span className='text-[32px] font-bold leading-none tracking-tight text-foreground'>{value}</span>
                {subtitle && <span className='mb-1 font-mono text-[11px] tracking-[0.3px] text-muted-foreground'>{subtitle}</span>}
            </div>

            {/* Progress bar OR badges */}
            {bar && (
                <div className='h-1 w-full bg-bg-elevated'>
                    <div className={`h-full ${bar.color}`} style={{ width: `${Math.min(bar.percent, 100)}%` }} />
                </div>
            )}
            {badges && (
                <div className='flex gap-2'>
                    {badges.map((badge) => (
                        <span key={badge.label} className={`rounded px-1.5 py-0.5 font-mono text-[10px] font-medium ${badge.color} ${badge.textColor}`}>
                            {badge.label}
                        </span>
                    ))}
                </div>
            )}
        </div>
    );
}
```

- [ ] **Step 2: Verify types pass**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 3: Commit**

```bash
git add app/src/components/dashboard/metric-card.tsx
git commit -m "feat: add reusable MetricCard component for dashboard"
```

---

### Task 2: Create TraefikBanner Component (Placeholder)

**Files:**
- Create: `app/src/components/dashboard/traefik-banner.tsx`

Static placeholder — no real data. Shows traefik proxy status with hardcoded stats. Marked in `docs/missing.md`.

- [ ] **Step 1: Create traefik-banner.tsx**

```tsx
import { Route, ArrowRight } from "lucide-react";

export function TraefikBanner() {
    return (
        <div className='flex items-center justify-between border border-border bg-card px-4 py-3'>
            <div className='flex items-center gap-2.5'>
                <Route className='size-4 text-accent-cyan' />
                <span className='text-[13px] font-semibold text-foreground'>Traefik Proxy</span>
                <span className='size-1.5 rounded-full bg-muted-foreground' />
                <span className='font-mono text-[10px] font-semibold text-muted-foreground'>--</span>
            </div>
            <div className='flex items-center gap-4'>
                <span className='font-mono text-[11px] text-muted-foreground'>-- req/s</span>
                <span className='font-mono text-[11px] text-muted-foreground'>-- p95</span>
                <span className='font-mono text-[11px] text-muted-foreground'>-- err</span>
                <ArrowRight className='size-3.5 text-muted-foreground' />
            </div>
        </div>
    );
}
```

Note: All values are muted placeholders (`--`) since traefik integration doesn't exist yet. See `docs/missing.md`.

- [ ] **Step 2: Verify types pass**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 3: Commit**

```bash
git add app/src/components/dashboard/traefik-banner.tsx
git commit -m "feat: add TraefikBanner placeholder component"
```

---

### Task 3: Create ContainersTable Component

**Files:**
- Create: `app/src/components/dashboard/containers-table.tsx`

The main table showing running containers with live metrics. Joins task data with container metrics by `containerId`.

Design columns: checkbox (decorative), STATUS, NAME, IMAGE, CPU, MEM, PORTS, UPTIME, Actions.

- [ ] **Step 1: Create containers-table.tsx**

This is the largest new component. It should:
- Accept `tasks: TaskItem[]` and `containerMetrics: Map<string, ContainerMetrics>` as props
- Accept `search: string` for filtering
- Show column headers in mono uppercase (9px, muted)
- For each task row:
  - Status: colored dot (green=running, orange=paused/preparing, red=failed/exited)
  - Name: container/service name in mono
  - Image: image name in mono muted
  - CPU: from containerMetrics lookup by containerId, show "—" if not found
  - MEM: from containerMetrics lookup, format as "128 MB", show "—" if not found
  - Ports: show "—" (placeholder, see `docs/missing.md`)
  - Uptime: derive from task `timestamp` as relative time
  - Actions: terminal + logs icon buttons (use existing `useTerminal().openExec` for terminal)
- Filter rows by search string (match name or image)
- Error rows (state=failed/rejected) should have subtle red background `bg-accent-red/5`

Important: CPU values above 8% should show in `text-accent-orange`. Memory values above 1GB should show in `text-accent-orange`. Otherwise use `text-text-secondary`.

Format memory helper:
```typescript
function formatMemory(bytes: number): string {
    if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
    if (bytes >= 1e6) return `${Math.round(bytes / 1e6)} MB`;
    return `${Math.round(bytes / 1e3)} KB`;
}
```

Uptime helper:
```typescript
function formatUptime(timestamp: string): string {
    const seconds = Math.floor((Date.now() - new Date(timestamp).getTime()) / 1000);
    if (seconds < 60) return "just now";
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ${minutes % 60}m`;
    const days = Math.floor(hours / 24);
    return `${days}d ${hours % 24}h`;
}
```

- [ ] **Step 2: Verify types pass**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 3: Commit**

```bash
git add app/src/components/dashboard/containers-table.tsx
git commit -m "feat: add ContainersTable with live CPU/MEM metrics"
```

---

### Task 4: Rewrite Dashboard Page

**Files:**
- Modify: `app/src/routes/_authenticated/index.tsx`
- Delete: `app/src/components/dashboard/stats-cards.tsx`
- Delete: `app/src/components/dashboard/resource-charts.tsx`
- Delete: `app/src/components/dashboard/service-health-grid.tsx`
- Delete: `app/src/components/dashboard/activity-feed.tsx`

Replace the entire dashboard with the new layout.

- [ ] **Step 1: Rewrite the dashboard page**

The new dashboard should:

1. **Fetch data** from: `useTasks()`, `useTaskStats()`, `useClusterMetrics()`, `useNodes()`, `useAuth()` (for cluster name via active profile label)

   Cluster name lookup:
   ```ts
   const { profiles, activeProfileId } = useAuth();
   const clusterName = profiles.find(p => p.id === activeProfileId)?.label ?? "Cluster";
   ```
2. **Build a container metrics map**: Flatten `metricsData.agents[].containers[]` into a `Map<containerId, ContainerMetrics>` for O(1) lookup
3. **Page Header**: "CLUSTER OVERVIEW" (24px bold) + subtitle with cluster name, node count, container count + REFRESH button (invalidates queries) + DEPLOY button (NOOP, disabled style)
4. **Metrics Row**: 4 `<MetricCard>` components:
   - Containers: value=running count, subtitle="/ {total} slots", badges for running/paused/exited counts
   - CPU: value=avg CPU%, subtitle="avg across {n} nodes", bar with cyan color
   - Memory: value=total used GB, subtitle="/ {total} GB", bar with purple color
   - Network I/O: value="--", subtitle="placeholder", bar with orange color at 0%
5. **Traefik Banner**: `<TraefikBanner />` placeholder
6. **Containers Table Section**: Card wrapper with header ("RUNNING CONTAINERS" + count badge + search input + FILTER button NOOP) + `<ContainersTable>` with the tasks and metrics map
7. **Loading state**: Skeleton matching the new layout (4 card skeletons + table skeleton)
8. **Error state**: Same error component with retry button

- [ ] **Step 2: Delete old dashboard components**

Delete these files (they are no longer imported anywhere):
- `app/src/components/dashboard/stats-cards.tsx`
- `app/src/components/dashboard/resource-charts.tsx`
- `app/src/components/dashboard/service-health-grid.tsx`
- `app/src/components/dashboard/activity-feed.tsx`

- [ ] **Step 3: Verify types pass**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 4: Commit**

```bash
git add app/src/routes/_authenticated/index.tsx app/src/components/dashboard/
git commit -m "feat: redesign dashboard with metric cards, traefik banner, containers table"
```

---

### Task 5: Update Dashboard Tests

**Files:**
- Modify: `app/tests/unit/dashboard-overview.spec.tsx`

The current tests check for "Infrastructure Overview" title, node/service/task counts, StatsCards, ResourceCharts, ServiceHealthGrid, ActivityFeed. All of those are gone.

- [ ] **Step 1: Read the current test file**

Read `app/tests/unit/dashboard-overview.spec.tsx` to understand the test structure and mocks.

- [ ] **Step 2: Rewrite tests for new dashboard**

Key tests to include:
- **Renders page title "CLUSTER OVERVIEW"**
- **Renders subtitle with node and container counts**
- **Renders 4 metric cards** (check for "CONTAINERS", "CPU USAGE", "MEMORY", "NETWORK I/O" labels)
- **Renders traefik banner** (check for "Traefik Proxy" text)
- **Renders containers table header** (check for "RUNNING CONTAINERS" text)
- **Renders container rows from tasks data** (mock tasks + metrics, check container names appear)
- **Shows loading skeleton when data is loading**
- **Shows error state with retry button**
- **REFRESH button invalidates queries** (check `queryClient.invalidateQueries` is called)

Update mocks to include `list_tasks` alongside existing `list_nodes`, `list_task_stats`, `list_cluster_metrics` mocks. The `list_tasks` mock should return sample tasks with containerIds that match the container metrics mock data.

- [ ] **Step 3: Run tests**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app test -- tests/unit/dashboard-overview.spec.tsx
```

Fix any failures.

- [ ] **Step 4: Commit**

```bash
git add app/tests/unit/dashboard-overview.spec.tsx
git commit -m "test: update dashboard tests for new cluster overview layout"
```

---

### Task 6: Final Verification

- [ ] **Step 1: Run type check**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 2: Run linter**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm lint
```

- [ ] **Step 3: Run formatter**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm fmt
```

- [ ] **Step 4: Run all tests**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app test
```

- [ ] **Step 5: Verify no orphan imports**

Check that no file still imports the deleted dashboard components:

```bash
grep -r "stats-cards\|resource-charts\|service-health-grid\|activity-feed" app/src/ --include="*.tsx" --include="*.ts"
```

Should return no results.

- [ ] **Step 6: Fix any issues and commit**

```bash
git add app/src/ app/tests/
git commit -m "fix: resolve lint, format, and test issues from dashboard redesign"
```

# List Pages Redesign — Plan 3b of 5

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build shared list page components and redesign all 7 list pages (Services, Containers, Nodes, Stacks, Networks, Volumes, Images) to match the new dark design.

**Architecture:** Create 3 shared components (`PageHeader`, `TableShell`, `ColumnHeaders`) that handle the common chrome pattern across all list pages. Each page defines its own columns and row rendering. Nodes uses a card layout instead of a table. All pages use existing data hooks — no backend changes needed.

**Tech Stack:** React, TanStack Query, TanStack Router, Tailwind CSS v4, Lucide icons, Shadcn UI

**Design reference:** `docs/design.pen` — screens `LMjiS` (Services), `2rYBA` (Containers), `psaVB` (Nodes), `twXyi` (Stacks), `isFfK` (Networks), `jigga` (Volumes), `OJDW6` (Images)

---

## Shared Pattern Analysis

Every list page (except Nodes) follows this structure:

```
┌─────────────────────────────────────────────────┐
│ PAGE HEADER                                      │
│ Title (24px bold)          [Action Buttons]       │
│ subtitle (mono, muted)                           │
├─────────────────────────────────────────────────┤
│ ┌─ TABLE CARD (bg-card, border) ──────────────┐ │
│ │ SECTION TITLE  [count]     [search] [filter] │ │
│ ├──────────────────────────────────────────────┤ │
│ │ COL1    COL2    COL3    (bg-elevated, mono)  │ │
│ ├──────────────────────────────────────────────┤ │
│ │ row data...                                  │ │
│ │ row data...                                  │ │
│ │ row data (error: subtle red bg)              │ │
│ └──────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────┘
```

---

## File Structure

### New shared components
- `app/src/components/page-header.tsx` — Title + subtitle + action buttons (right slot)
- `app/src/components/table-shell.tsx` — Card wrapper with section header (title + count + search + filter slot)
- `app/src/components/column-headers.tsx` — Elevated column header row

### Utility file
- `app/src/lib/task-utils.ts` — Add `buildContainerMetricsMap(agents: AgentMetricsSnapshot[]): Map<string, ContainerMetrics>` utility to flatten agent container metrics into a lookup map by containerId. Used by both the dashboard (Plan 3a) and the containers page.

### Pages to modify (redesign)
- `app/src/routes/_authenticated/services/index.tsx` — Services list
- `app/src/routes/_authenticated/tasks/index.tsx` — Containers list
- `app/src/routes/_authenticated/nodes/index.tsx` — Nodes cards
- `app/src/routes/_authenticated/stacks/index.tsx` — Stacks list
- `app/src/routes/_authenticated/networks/index.tsx` — Networks list
- `app/src/routes/_authenticated/volumes/index.tsx` — Volumes list
- `app/src/routes/_authenticated/images/index.tsx` — Images list

### Tests to update
- `app/tests/unit/services-list.spec.tsx`
- `app/tests/unit/tasks-page.spec.tsx`
- `app/tests/unit/nodes-list.spec.tsx`
- `app/tests/unit/list-stacks.spec.tsx`
- `app/tests/unit/networks-list.spec.tsx`
- `app/tests/unit/volumes-list.spec.tsx`
- `app/tests/unit/images-page.spec.tsx`

---

### Task 1: Create PageHeader Component

**Files:**
- Create: `app/src/components/page-header.tsx`

Reusable page header with title, subtitle, and right-side action buttons slot.

- [ ] **Step 1: Create page-header.tsx**

```tsx
interface PageHeaderProps {
    title: string;
    subtitle: string;
    children?: React.ReactNode; // right-side slot for action buttons
}

export function PageHeader({ title, subtitle, children }: PageHeaderProps) {
    return (
        <div className='flex items-center justify-between'>
            <div className='flex flex-col gap-1'>
                <h1 className='text-2xl font-bold tracking-tight text-foreground'>{title}</h1>
                <p className='font-mono text-[11px] tracking-[0.3px] text-muted-foreground'>{subtitle}</p>
            </div>
            {children && <div className='flex items-center gap-2'>{children}</div>}
        </div>
    );
}
```

- [ ] **Step 2: Verify types**
```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 3: Commit**
```bash
git add app/src/components/page-header.tsx
git commit -m "feat: add reusable PageHeader component"
```

---

### Task 2: Create TableShell Component

**Files:**
- Create: `app/src/components/table-shell.tsx`

Card wrapper with section header: title + count badge + search input + optional filter slot. Children render inside the card body.

- [ ] **Step 1: Create table-shell.tsx**

The parent owns search state and passes it down. `TableShell` is a pure visual wrapper — no internal state.

```tsx
import { Search } from "lucide-react";
import { Input } from "@/components/ui/input";

interface TableShellProps {
    title: string;
    count: number;
    searchValue: string;
    onSearchChange: (value: string) => void;
    searchPlaceholder?: string;
    filterSlot?: React.ReactNode;
    children: React.ReactNode;
}

export function TableShell({ title, count, searchValue, onSearchChange, searchPlaceholder, filterSlot, children }: TableShellProps) {
    return (
        <div className='flex flex-1 flex-col overflow-hidden border border-border bg-card'>
            {/* Table header */}
            <div className='flex items-center justify-between px-4 py-3 border-b border-border'>
                <div className='flex items-center gap-3'>
                    <span className='font-mono text-[11px] font-bold tracking-[0.5px] text-foreground'>{title}</span>
                    <span className='rounded bg-bg-elevated px-2 py-0.5 font-mono text-[10px] text-muted-foreground'>{count}</span>
                </div>
                <div className='flex items-center gap-2'>
                    <div className='relative'>
                        <Search className='absolute top-1/2 left-2.5 size-3 -translate-y-1/2 text-muted-foreground' />
                        <Input
                            placeholder={searchPlaceholder ?? "Search..."}
                            value={searchValue}
                            onChange={(e) => onSearchChange(e.target.value)}
                            className='h-7 w-[200px] bg-transparent pl-8 font-mono text-[11px]'
                        />
                    </div>
                    {filterSlot}
                </div>
            </div>

            {/* Table content */}
            <div className='flex-1 overflow-y-auto'>
                {children}
            </div>
        </div>
    );
}
```

- [ ] **Step 2: Verify types**
```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 3: Commit**
```bash
git add app/src/components/table-shell.tsx
git commit -m "feat: add reusable TableShell component with search and filter slot"
```

---

### Task 3: Create ColumnHeaders Component

**Files:**
- Create: `app/src/components/column-headers.tsx`

Renders the elevated column header row. Accepts column definitions with name, width, and alignment.

- [ ] **Step 1: Create column-headers.tsx**

```tsx
interface Column {
    label: string;
    width?: string | number; // CSS width value (e.g. "120px", "10%") or number (px)
    align?: "left" | "right" | "center";
}

interface ColumnHeadersProps {
    columns: Column[];
}

export function ColumnHeaders({ columns }: ColumnHeadersProps) {
    return (
        <div className='flex items-center bg-bg-elevated px-4 py-2 border-b border-border'>
            {columns.map((col) => (
                <span
                    key={col.label}
                    className={`font-mono text-[9px] font-bold tracking-[0.5px] text-muted-foreground ${
                        col.align === "right" ? "text-right" : ""
                    }`}
                    style={{ width: col.width ?? "auto", flex: col.width ? undefined : 1 }}
                >
                    {col.label}
                </span>
            ))}
        </div>
    );
}
```

- [ ] **Step 2: Verify types**
```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 3: Commit**
```bash
git add app/src/components/column-headers.tsx
git commit -m "feat: add reusable ColumnHeaders component"
```

---

### Task 4: Redesign Services Page

**Files:**
- Modify: `app/src/routes/_authenticated/services/index.tsx`

Design columns: STATUS (dot + label), NAME (linked), MODE (replicated/global), IMAGE, REPLICAS (badge), STACK, PORTS (placeholder "—"), UPDATED + row actions.

Use `PageHeader` with title "SERVICES", subtitle with counts, and a "CREATE SERVICE" button (NOOP — add to `docs/missing.md`).

Use `TableShell` with "SWARM SERVICES" title + count + search.

Use `ColumnHeaders` for the header row.

Keep existing `useServices()` hook and filtering logic. Restyle rows to match the design: mono font, muted secondary text, colored status dots, accent-colored badges for replicas.

The page already has most of the logic — this is primarily a visual restyling using the shared components.

- [ ] **Step 1: Read the current file and redesign**
- [ ] **Step 2: Update `app/tests/unit/services-list.spec.tsx`** — adjust selectors for new text (e.g. "SERVICES" instead of "Services", "SWARM SERVICES" section title)
- [ ] **Step 3: Run tests and verify**
- [ ] **Step 4: Commit**

```bash
git add app/src/routes/_authenticated/services/index.tsx app/tests/unit/services-list.spec.tsx
git commit -m "feat: redesign services page with shared list components"
```

---

### Task 5: Redesign Containers Page

**Files:**
- Modify: `app/src/routes/_authenticated/tasks/index.tsx`

Design columns: STATUS, NAME, IMAGE, CPU, MEM, PORTS (placeholder), UPTIME + row actions.

Use `PageHeader` with title "CONTAINERS", subtitle with running/total counts, and optional action buttons.

Use `TableShell` with "ALL CONTAINERS" title + count + search.

This page should join `useTasks()` with `useClusterMetrics()` containers for live CPU/MEM (same pattern as the dashboard containers table from Plan 3a). Create a utility function for the join that can be shared.

Filter bar above the table: search + status filter (Image/All dropdown) + sort dropdown. The dropdowns can be NOOP for now (just visual, no actual filtering beyond search). Add to `docs/missing.md`.

- [ ] **Step 1: Read the current file and redesign**
- [ ] **Step 2: Update `app/tests/unit/tasks-page.spec.tsx`**
- [ ] **Step 3: Run tests and verify**
- [ ] **Step 4: Commit**

```bash
git add app/src/routes/_authenticated/tasks/index.tsx app/tests/unit/tasks-page.spec.tsx
git commit -m "feat: redesign containers page with live CPU/MEM metrics"
```

---

### Task 6: Redesign Nodes Page (Cards Layout)

**Files:**
- Modify: `app/src/routes/_authenticated/nodes/index.tsx`

**This page uses CARDS, not a table.** Each node is a card showing:
- Hostname + status badges (LEADER/MANAGER/WORKER, Active/Drain)
- IP address, OS, Docker version
- Metrics: CPU bar + MEM bar (from `useClusterMetrics()`)
- Running tasks count
- Warning banner for unhealthy nodes

Use `PageHeader` with title "NODES", subtitle with counts, and "ADD NODE" button (NOOP — add to `docs/missing.md`).

Cards arranged in a horizontal row with `flex gap-4`.

Join node data with cluster metrics by `nodeId` to get CPU/MEM per node.

- [ ] **Step 1: Read the current file and redesign**
- [ ] **Step 2: Update `app/tests/unit/nodes-list.spec.tsx`**
- [ ] **Step 3: Run tests and verify**
- [ ] **Step 4: Commit**

```bash
git add app/src/routes/_authenticated/nodes/index.tsx app/tests/unit/nodes-list.spec.tsx
git commit -m "feat: redesign nodes page with card layout and live metrics"
```

---

### Task 7: Redesign Stacks Page

**Files:**
- Modify: `app/src/routes/_authenticated/stacks/index.tsx`

Design columns: STATUS, NAME (+ description), SERVICES, REPLICAS, CPU, MEM (aggregate), LAST DEPLOY + row actions.

Use shared components. "DEPLOY STACK" button (NOOP — add to `docs/missing.md`).

CPU/MEM per stack: show "—" for now and add to `docs/missing.md`. The aggregation requires a two-hop join (`ContainerMetrics.serviceName` → `ServiceItem.name` → `ServiceItem.stack`) which adds complexity without much value at this stage.

- [ ] **Step 1: Read the current file and redesign**
- [ ] **Step 2: Update `app/tests/unit/list-stacks.spec.tsx`**
- [ ] **Step 3: Run tests and verify**
- [ ] **Step 4: Commit**

```bash
git add app/src/routes/_authenticated/stacks/index.tsx app/tests/unit/list-stacks.spec.tsx
git commit -m "feat: redesign stacks page with shared list components"
```

---

### Task 8: Redesign Networks Page

**Files:**
- Modify: `app/src/routes/_authenticated/networks/index.tsx`

Design columns: NAME, ID (truncated), DRIVER, SCOPE, SUBNET + row actions.

Use shared components. "CREATE NETWORK" button (NOOP — add to `docs/missing.md`).

Simple table — all data comes from `useNetworks()`.

- [ ] **Step 1: Read the current file and redesign**
- [ ] **Step 2: Update `app/tests/unit/networks-list.spec.tsx`**
- [ ] **Step 3: Run tests and verify**
- [ ] **Step 4: Commit**

```bash
git add app/src/routes/_authenticated/networks/index.tsx app/tests/unit/networks-list.spec.tsx
git commit -m "feat: redesign networks page with shared list components"
```

---

### Task 9: Redesign Volumes Page

**Files:**
- Modify: `app/src/routes/_authenticated/volumes/index.tsx`

Design columns: NAME, DRIVER, MOUNTPOINT, SIZE, CREATED + row actions.

Use shared components. "CREATE VOLUME" button (NOOP — add to `docs/missing.md`).

Simple table — all data comes from `useVolumes()`.

- [ ] **Step 1: Read the current file and redesign**
- [ ] **Step 2: Update `app/tests/unit/volumes-list.spec.tsx`**
- [ ] **Step 3: Run tests and verify**
- [ ] **Step 4: Commit**

```bash
git add app/src/routes/_authenticated/volumes/index.tsx app/tests/unit/volumes-list.spec.tsx
git commit -m "feat: redesign volumes page with shared list components"
```

---

### Task 10: Redesign Images Page

**Files:**
- Modify: `app/src/routes/_authenticated/images/index.tsx`

Design columns: REPOSITORY, TAG, IMAGE ID (truncated), SIZE, NODE, CREATED + row actions.

Use shared components. "PULL IMAGE" button (NOOP — add to `docs/missing.md`).

The current images page groups by node. The new design shows a flat list. Keep the `useImages()` hook but flatten the node-grouped response into a single list.

- [ ] **Step 1: Read the current file and redesign**
- [ ] **Step 2: Update `app/tests/unit/images-page.spec.tsx`**
- [ ] **Step 3: Run tests and verify**
- [ ] **Step 4: Commit**

```bash
git add app/src/routes/_authenticated/images/index.tsx app/tests/unit/images-page.spec.tsx
git commit -m "feat: redesign images page with shared list components"
```

---

### Task 11: Update docs/missing.md

**Files:**
- Modify: `docs/missing.md`

Add entries for all NOOP buttons and placeholder data introduced in this plan:

- CREATE SERVICE button (services page)
- Container status/sort filter dropdowns (containers page)
- ADD NODE button (nodes page)
- DEPLOY STACK button (stacks page)
- CREATE NETWORK button (networks page)
- CREATE VOLUME button (volumes page)
- PULL IMAGE button (images page)
- Stack-level CPU/MEM aggregation (if skipped)

- [ ] **Step 1: Append to docs/missing.md**
- [ ] **Step 2: Commit**
```bash
git add docs/missing.md
git commit -m "docs: update missing.md with list page NOOP features"
```

---

### Task 12: Final Verification

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

- [ ] **Step 5: Fix any issues and commit**
```bash
git add app/src/ app/tests/
git commit -m "fix: resolve lint, format, and test issues from list pages redesign"
```

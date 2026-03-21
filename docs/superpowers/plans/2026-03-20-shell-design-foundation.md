# Shell & Design Foundation Migration — Plan 1 of 5

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate the app shell (sidebar, titlebar, status bar, layout, color tokens) to match the new dark-themed design in `docs/design.pen`, so every page immediately looks like the redesign.

**Architecture:** Replace the dual light/dark theme with a single dark palette using the design's hex tokens. Rewrite the three shell components (sidebar, titlebar, status bar) and the authenticated layout to match the new design. Keep all data hooks, contexts, and page content unchanged — those are handled in later plans.

**Tech Stack:** React, Tailwind CSS v4, Shadcn UI, Lucide icons, TanStack Router, JetBrains Mono + Space Grotesk fonts

**Design reference:** `docs/design.pen` — screens `WCBzg` (Desktop App), components `YJkf8` (Sidebar), `iHwhS` (Titlebar), `KYxT3` (StatusBar), `09e1f` (Accent Line)

**Important context:**
- The new design is **dark-only**. There is no light theme. The appearance context's theme toggle will be simplified.
- The design uses **Space Grotesk** (display) + **JetBrains Mono** (monospace). Space Grotesk needs to be installed.
- Sidebar sections change from (Cluster, Workloads, Infrastructure, Configuration, Observability) → (INFRASTRUCTURE, ORCHESTRATION, SYSTEM).
- The titlebar replaces DashboardHeader. It contains traffic lights + "KONTAINER" logo + cluster selector + utility icons.
- The status bar is completely redesigned with new segments: git branch, Docker version, CPU, MEM, Tasks, Alerts on left; Terminals, Log Streams, WS status on right.
- The "Tasks" route is now called "Containers" in the UI (route path stays `/tasks`).

**Subsequent plans:**
- Plan 2: Auth Flow (splash, connect, authenticating screens)
- Plan 3: Dashboard & List Pages (cluster overview + all table pages)
- Plan 4: System Pages (exec, logs, settings)
- Plan 5: New Features (integrations, deployments, vault, volume explorer)

---

## File Structure

### Modified files
- `app/src/app.css` — Replace color tokens with dark-only palette, add new custom properties
- `app/src/components/sidebar-nav.tsx` — Rewrite with new section structure and mono styling
- `app/src/components/status-bar.tsx` — Rewrite with new segment layout
- `app/src/components/dashboard-header.tsx` — Replace with Titlebar component
- `app/src/components/logo.tsx` — Update to new design (container icon + "KONTAINER")
- `app/src/components/cluster-switcher.tsx` — Move into titlebar center, minor style updates
- `app/src/routes/_authenticated.tsx` — Update layout structure (accent line → titlebar → sidebar+content → status bar)
- `app/src/lib/fonts.ts` — Add Space Grotesk, set as default display font
- `app/src/contexts/appearance-context.tsx` — Remove light theme, force dark mode
- `app/tests/unit/status-bar.spec.tsx` — Update test expectations for new status bar structure

### New files
- `app/src/components/accent-line.tsx` — Gradient line component at top of authenticated layout

### Packages to install (requires approval)
- `@fontsource-variable/space-grotesk` — Display font used throughout the new design

---

## Design Token Reference

From `docs/design.pen` variables:

| Token | Value | Maps to |
|-------|-------|---------|
| `--bg-deep` | `#0C0C0C` | `--background` |
| `--bg-card` | `#0A0A0A` | `--card` |
| `--bg-elevated` | `#141414` | `--muted`, `--accent` |
| `--bg-subtle` | `#1A1A1A` | `--secondary` |
| `--bg-sidebar` | `#080808` | `--sidebar` |
| `--border` | `#2f2f2f` | `--border`, `--input` |
| `--border-light` | `#3f3f3f` | (new) `--border-light` |
| `--text-primary` | `#FFFFFF` | `--foreground` |
| `--text-secondary` | `#8a8a8a` | (new) `--text-secondary` |
| `--text-muted` | `#6a6a6a` | `--muted-foreground` |
| `--accent-green` | `#00FF88` | `--primary`, `--success` |
| `--accent-cyan` | `#00D4FF` | (new) `--accent-cyan` |
| `--accent-purple` | `#A78BFA` | (new) `--accent-purple` |
| `--accent-orange` | `#FF8800` | `--warning` |
| `--accent-red` | `#FF4444` | `--destructive` |
| `--titlebar-bg` | `#060606` | (new) `--titlebar-bg` |
| `--statusbar-bg` | `#060606` | (new) `--statusbar-bg` |

---

### Task 1: Install Space Grotesk Font

**Files:**
- Modify: `app/src/lib/fonts.ts`
- Modify: `app/src/app.css` (import line)

- [ ] **Step 1: Install the font package**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app add @fontsource-variable/space-grotesk
```

- [ ] **Step 2: Add the CSS import**

In `app/src/app.css`, add after the existing font imports:

```css
@import "@fontsource-variable/space-grotesk";
```

- [ ] **Step 3: Add Space Grotesk to fonts.ts**

In `app/src/lib/fonts.ts`, add to the `fonts` array (first position, since it's the new default):

```typescript
{
    id: "space-grotesk",
    label: "Space Grotesk",
    cssFamily: '"Space Grotesk Variable", sans-serif',
    load: () => Promise.resolve(), // loaded via CSS import
},
```

Update `DEFAULT_FONT_ID`:

```typescript
export const DEFAULT_FONT_ID = "space-grotesk";
```

- [ ] **Step 4: Update the CSS `@theme` font-sans**

In `app/src/app.css`, inside `@theme inline`, change:

```css
--font-sans: "Space Grotesk Variable", sans-serif;
```

- [ ] **Step 5: Verify the app still compiles**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 6: Commit**

```bash
git add app/src/app.css app/src/lib/fonts.ts app/package.json pnpm-lock.yaml
git commit -m "feat: add Space Grotesk as default display font"
```

---

### Task 2: Replace CSS Tokens with Dark-Only Palette

**Files:**
- Modify: `app/src/app.css`

This is the most impactful change — it sets the visual tone for the entire app.

- [ ] **Step 1: Replace the `:root` and `.dark` blocks with a single dark palette**

Replace everything from `:root {` through the closing `}` of `.dark {` with a single `:root` block. The new design is dark-only, so we collapse both blocks.

New `:root` block:

```css
:root {
    --titlebar-inset: 35px;

    /* Core surfaces */
    --background: #0C0C0C;
    --foreground: #FFFFFF;
    --card: #0A0A0A;
    --card-foreground: #FFFFFF;
    --popover: #0A0A0A;
    --popover-foreground: #FFFFFF;

    /* Primary = accent green */
    --primary: #00FF88;
    --primary-foreground: #000000;

    /* Secondary & muted */
    --secondary: #1A1A1A;
    --secondary-foreground: #FFFFFF;
    --muted: #141414;
    --muted-foreground: #6a6a6a;
    --accent: #141414;
    --accent-foreground: #FFFFFF;

    /* Destructive */
    --destructive: #FF4444;

    /* Borders & input */
    --border: #2f2f2f;
    --input: #2f2f2f;
    --ring: #00FF88;

    /* Charts */
    --chart-1: #00D4FF;
    --chart-2: #00FF88;
    --chart-3: #A78BFA;
    --chart-4: #FF8800;
    --chart-5: #FF4444;

    --radius: 0.5rem;

    /* Sidebar */
    --sidebar: #080808;
    --sidebar-foreground: #FFFFFF;
    --sidebar-primary: #00FF88;
    --sidebar-primary-foreground: #000000;
    --sidebar-accent: #FFFFFF0D;
    --sidebar-accent-foreground: #FFFFFF;
    --sidebar-border: #2f2f2f;
    --sidebar-ring: #00FF88;

    /* Semantic status */
    --success: #00FF88;
    --success-foreground: #000000;
    --warning: #FF8800;
    --warning-foreground: #000000;

    /* Design system extensions */
    --accent-green: #00FF88;
    --accent-green-20: #00FF8833;
    --accent-cyan: #00D4FF;
    --accent-cyan-20: #00D4FF33;
    --accent-purple: #A78BFA;
    --accent-purple-20: #A78BFA33;
    --accent-orange: #FF8800;
    --accent-orange-20: #FF880033;
    --accent-red: #FF4444;
    --text-secondary: #8a8a8a;
    --bg-elevated: #141414;
    --bg-subtle: #1A1A1A;
    --border-light: #3f3f3f;
    --titlebar-bg: #060606;
    --statusbar-bg: #060606;
}
```

- [ ] **Step 2: Add the new tokens to `@theme inline`**

Add these lines inside `@theme inline` (after the existing Shadcn mappings):

```css
--color-accent-green: var(--accent-green);
--color-accent-green-20: var(--accent-green-20);
--color-accent-cyan: var(--accent-cyan);
--color-accent-cyan-20: var(--accent-cyan-20);
--color-accent-purple: var(--accent-purple);
--color-accent-purple-20: var(--accent-purple-20);
--color-accent-orange: var(--accent-orange);
--color-accent-orange-20: var(--accent-orange-20);
--color-accent-red: var(--accent-red);
--color-text-secondary: var(--text-secondary);
--color-bg-elevated: var(--bg-elevated);
--color-bg-subtle: var(--bg-subtle);
--color-border-light: var(--border-light);
--color-titlebar-bg: var(--titlebar-bg);
--color-statusbar-bg: var(--statusbar-bg);
```

- [ ] **Step 3: Set dark color-scheme on body**

In `@layer base`, update body styles to use dark color-scheme (the `.dark` class is applied by the appearance context in Task 3):

```css
html,
body {
    @apply font-sans text-foreground antialiased;
    background: var(--background);
    color-scheme: dark;
}
```

Note: Do NOT use `@apply dark;` — `dark` is a Tailwind v4 custom variant, not a utility class. The `.dark` class is managed by the appearance context's `syncDOM()` function.

- [ ] **Step 4: Verify types still pass**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 5: Commit**

```bash
git add app/src/app.css
git commit -m "feat: replace dual theme with dark-only design palette"
```

---

### Task 3: Force Dark Mode in Appearance Context

**Files:**
- Modify: `app/src/contexts/appearance-context.tsx`

The appearance context currently supports light/dark/system. We need to simplify it to always be dark.

- [ ] **Step 1: Read the current appearance context**

Read `app/src/contexts/appearance-context.tsx` to understand the current implementation.

- [ ] **Step 2: Force dark mode**

Find where the context applies the `.dark` class to the document element and ensure it always adds `.dark` regardless of the `scheme` setting. The simplest change: in the effect that toggles the class, always add `.dark` and never remove it.

Look for code like:
```typescript
document.documentElement.classList.toggle("dark", ...)
```
Replace with:
```typescript
document.documentElement.classList.add("dark");
```

Leave the scheme state/setting in place for now (removing it fully would break the settings UI which is Plan 4). Just make the effect always apply dark.

- [ ] **Step 3: Verify types pass**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 4: Commit**

```bash
git add app/src/contexts/appearance-context.tsx
git commit -m "feat: force dark mode, remove light theme support"
```

---

### Task 4: Update Logo Component

**Files:**
- Modify: `app/src/components/logo.tsx`

The new logo is a green container icon + "KONTAINER" in mono uppercase with letter-spacing.

- [ ] **Step 1: Rewrite logo.tsx**

```tsx
import { Container } from "lucide-react";

export function Logo() {
    return (
        <div className="flex items-center gap-2">
            <Container className="size-4 text-accent-green" />
            <span className="font-mono text-[13px] font-bold tracking-[1.5px] text-foreground">
                KONTAINER
            </span>
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
git add app/src/components/logo.tsx
git commit -m "feat: update logo to new design (green container + KONTAINER)"
```

---

### Task 5: Create Accent Line Component

**Files:**
- Create: `app/src/components/accent-line.tsx`

A thin 2px gradient bar at the very top of the app window. Green → Cyan → Purple gradient.

- [ ] **Step 1: Create the component**

```tsx
export function AccentLine() {
    return (
        <div
            className="h-0.5 w-full shrink-0"
            style={{
                background: "linear-gradient(90deg, #00FF88, #00D4FF 50%, #A78BFA)",
            }}
        />
    );
}
```

- [ ] **Step 2: Verify types pass**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 3: Commit**

```bash
git add app/src/components/accent-line.tsx
git commit -m "feat: add accent gradient line component"
```

---

### Task 6: Rewrite Dashboard Header → Titlebar

**Files:**
- Modify: `app/src/components/dashboard-header.tsx`

The current DashboardHeader is just an empty spacer (`<header data-tauri-drag-region className='h-(--titlebar-inset) shrink-0' />`). The new design has a full titlebar with traffic lights, logo, cluster selector, and utility icons.

- [ ] **Step 1: Rewrite dashboard-header.tsx as Titlebar**

```tsx
import { Search, Bell, ChevronDown } from "lucide-react";
import { Logo } from "@/components/logo";
import { useAuth } from "@/hooks/use-auth";
import { useCommandPalette } from "@/hooks/use-command-palette";

export function DashboardHeader() {
    const { profiles, activeProfileId } = useAuth();
    const { setOpen } = useCommandPalette();
    const activeProfile = profiles.find((p) => p.id === activeProfileId);
    const clusterName = activeProfile?.label ?? "No cluster";

    return (
        <header
            data-tauri-drag-region
            className="flex h-[38px] shrink-0 items-center justify-between border-b border-border bg-titlebar-bg/90 px-4"
        >
            {/* Left: traffic lights spacer + logo */}
            <div className="flex items-center gap-3">
                <div className="flex items-center gap-2">
                    <span className="size-3 rounded-full bg-[#FF5F57]" />
                    <span className="size-3 rounded-full bg-[#FEBC2E]" />
                    <span className="size-3 rounded-full bg-[#28C840]" />
                </div>
                <Logo />
            </div>

            {/* Center: cluster selector */}
            <button
                className="flex items-center gap-2 text-text-secondary transition-colors hover:text-foreground"
                onClick={() => setOpen(true)}
            >
                <span className="size-[7px] shrink-0 rounded-full bg-accent-green animate-pulse-dot" />
                <span className="font-mono text-[11px] font-medium tracking-[0.5px]">
                    {clusterName}
                </span>
                <ChevronDown className="size-3 text-muted-foreground" />
            </button>

            {/* Right: utility icons */}
            <div className="flex items-center gap-4">
                <button
                    className="text-muted-foreground transition-colors hover:text-foreground"
                    onClick={() => setOpen(true)}
                >
                    <Search className="size-3.5" />
                </button>
                <button className="text-muted-foreground transition-colors hover:text-foreground">
                    <Bell className="size-3.5" />
                </button>
            </div>
        </header>
    );
}
```

Notes:
- We keep the export name `DashboardHeader` to avoid renaming across the codebase. The component is now the titlebar conceptually.
- **Traffic lights**: These decorative dots are intentional for the design aesthetic. On macOS, the native traffic lights are overlaid by the OS at the same position — the CSS `--titlebar-inset` padding is no longer needed since the titlebar is now a full component. If the native buttons overlap, adjust with `pl-[76px]` on the left section to leave space for them and remove the fake dots. This is a visual tuning step during verification.
- **ClusterSwitcher**: The old sidebar rendered `<ClusterSwitcher>` with full profile management (add/switch/remove/disconnect). This titlebar replaces it with a simplified cluster name display that opens the command palette. The `ClusterSwitcher` component remains available and will be integrated into the command palette or a dedicated overlay in Plan 2 (Auth Flow). For now, users can still switch profiles via the command palette's existing cluster actions.

- [ ] **Step 2: Verify types pass**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 3: Commit**

```bash
git add app/src/components/dashboard-header.tsx
git commit -m "feat: replace empty header with full titlebar (logo, cluster, utilities)"
```

---

### Task 7: Rewrite Sidebar Navigation

**Files:**
- Modify: `app/src/components/sidebar-nav.tsx`

New sidebar structure from the design:
- **INFRASTRUCTURE**: Overview, Containers, Images, Volumes, Networks
- **ORCHESTRATION**: Stacks, Nodes, Services
- **SYSTEM** (bottom-pinned): Exec, Logs, Settings
- Agent info card at very bottom

Styling: mono font, uppercase section labels, letter-spacing, icon + label per item, count badges with colored backgrounds.

- [ ] **Step 1: Rewrite sidebar-nav.tsx**

```tsx
import { Link, useMatches } from "@tanstack/react-router";
import {
    Activity, Box, HardDrive,
    FileStack, LayoutDashboard, Layers, Network, Server,
    Settings, Terminal, ScrollText,
} from "lucide-react";
import { useNodes } from "@/hooks/use-nodes";
import { useTaskStats } from "@/hooks/use-task-stats";

type NavItem = {
    label: string;
    to: string;
    icon: typeof LayoutDashboard;
    countKey?: string;
    badgeColor?: string;
};
type NavSection = { heading: string; items: NavItem[] };

const topSections: NavSection[] = [
    {
        heading: "INFRASTRUCTURE",
        items: [
            { label: "OVERVIEW", to: "/", icon: LayoutDashboard },
            { label: "CONTAINERS", to: "/tasks", icon: Box, countKey: "tasks", badgeColor: "bg-accent-green-20 text-accent-green" },
            { label: "IMAGES", to: "/images", icon: Layers },
            { label: "VOLUMES", to: "/volumes", icon: HardDrive },
            { label: "NETWORKS", to: "/networks", icon: Network },
        ],
    },
    {
        heading: "ORCHESTRATION",
        items: [
            { label: "STACKS", to: "/stacks", icon: FileStack },
            { label: "NODES", to: "/nodes", icon: Server, countKey: "nodes", badgeColor: "bg-accent-orange-20 text-accent-orange" },
            { label: "SERVICES", to: "/services", icon: Activity },
        ],
    },
];

const bottomSection: NavSection = {
    heading: "SYSTEM",
    items: [
        { label: "EXEC", to: "/exec", icon: Terminal },
        { label: "LOGS", to: "/logs", icon: ScrollText },
        { label: "SETTINGS", to: "/settings", icon: Settings },
    ],
};

export function SidebarNav() {
    const matches = useMatches();
    const currentPath = matches[matches.length - 1]?.fullPath ?? "/";

    const { data: nodes } = useNodes();
    const { data: taskStats } = useTaskStats();

    const counts: Record<string, number> = {
        tasks: taskStats?.running ?? 0,
        nodes: nodes?.length ?? 0,
    };

    function isActive(to: string): boolean {
        if (to === "/") return currentPath === "/";
        return currentPath.startsWith(to);
    }

    function renderSection(section: NavSection) {
        return (
            <div key={section.heading}>
                <p className="px-3 font-mono text-[9px] font-semibold tracking-[1px] text-muted-foreground">
                    // {section.heading}
                </p>
                <div className="mt-1 space-y-0.5">
                    {section.items.map((item) => {
                        const active = isActive(item.to);
                        const count = item.countKey ? counts[item.countKey] : undefined;

                        return (
                            <Link
                                key={item.to}
                                to={item.to}
                                className={`flex h-[34px] items-center gap-2.5 rounded px-3 font-mono text-xs font-medium tracking-[0.5px] transition-colors ${
                                    active
                                        ? "bg-sidebar-accent text-foreground"
                                        : "text-text-secondary hover:text-foreground"
                                }`}
                            >
                                <item.icon className="size-4 shrink-0 text-muted-foreground" />
                                <span className="flex-1">{item.label}</span>
                                {count !== undefined && count > 0 && (
                                    <span
                                        className={`rounded px-1.5 py-0.5 font-mono text-[10px] leading-none ${
                                            item.badgeColor ?? "bg-sidebar-accent text-muted-foreground"
                                        }`}
                                    >
                                        {count}
                                    </span>
                                )}
                            </Link>
                        );
                    })}
                </div>
            </div>
        );
    }

    return (
        <aside className="flex w-[220px] shrink-0 flex-col border-r border-border bg-sidebar/90">
            {/* Top: main navigation */}
            <nav className="flex-1 space-y-4 overflow-y-auto px-2 py-3">
                {topSections.map(renderSection)}
            </nav>

            {/* Bottom: system + agent info */}
            <div className="space-y-3 px-2 pb-3">
                {renderSection(bottomSection)}
                <div className="rounded bg-bg-elevated px-3 py-2.5 border border-border">
                    <p className="font-mono text-[9px] font-bold tracking-[0.5px] text-accent-green">
                        AGENT v0.4.2
                    </p>
                    <p className="font-mono text-[9px] font-medium tracking-[0.5px] text-muted-foreground">
                        uptime: --
                    </p>
                </div>
            </div>
        </aside>
    );
}
```

Note: The agent version/uptime is hardcoded for now — a hook to fetch this from the agent API can be added later. The exec route `/exec` doesn't exist yet — it currently lives in the terminal panel. We'll add a placeholder route in a later plan; for now it's a dead link in the sidebar which is fine during migration.

- [ ] **Step 2: Remove unused imports**

The old sidebar imported `ClusterSwitcher`, `useServices`, `useSecrets`, `useConfigs`, `useStacks`, `useImages`. The new one uses a smaller subset. Verify no import errors.

- [ ] **Step 3: Verify types pass**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 4: Commit**

```bash
git add app/src/components/sidebar-nav.tsx
git commit -m "feat: rewrite sidebar with new design sections and mono styling"
```

---

### Task 8: Rewrite Status Bar

**Files:**
- Modify: `app/src/components/status-bar.tsx`
- Modify: `app/tests/unit/status-bar.spec.tsx`

New status bar layout from design:
- **Left**: git branch, Docker version, CPU %, MEM, Tasks, Alerts
- **Right**: Terminal sessions, Log Streams, WS status
- All in mono font, 10px, colored icons per segment, pipe separators

- [ ] **Step 1: Rewrite status-bar.tsx**

```tsx
import { useNavigate } from "@tanstack/react-router";
import {
    GitBranch, Container, Cpu, MemoryStick, Layers,
    TriangleAlert, Terminal, ScrollText, ChevronUp, X,
} from "lucide-react";
import { useAuth } from "@/hooks/use-auth";
import { useNodes } from "@/hooks/use-nodes";
import { useTaskStats } from "@/hooks/use-task-stats";
import { useClusterMetrics } from "@/hooks/use-cluster-metrics";
import { useTerminal } from "@/hooks/use-terminal";

function Separator() {
    return <span className="h-full w-px bg-border" />;
}

function Segment({
    icon: Icon,
    iconColor,
    label,
    textColor,
    onClick,
    testId,
    valueTestId,
}: {
    icon: typeof Cpu;
    iconColor: string;
    label: string;
    textColor?: string;
    onClick?: () => void;
    testId?: string;
    valueTestId?: string;
}) {
    const Wrapper = onClick ? "button" : "div";
    return (
        <Wrapper
            className={`flex items-center gap-[5px] px-2.5 h-full ${textColor ?? "text-text-secondary"} ${onClick ? "hover:text-foreground transition-colors" : ""}`}
            onClick={onClick}
            data-testid={testId}
        >
            <Icon className={`size-[11px] ${iconColor}`} />
            <span className="font-mono text-[10px] font-medium" data-testid={valueTestId}>{label}</span>
        </Wrapper>
    );
}

export function StatusBar() {
    const navigate = useNavigate();
    const { data: nodes, isLoading: nodesLoading } = useNodes();
    const { data: taskStats, isLoading: taskStatsLoading } = useTaskStats();
    const { data: metrics, isLoading: metricsLoading } = useClusterMetrics();

    const { sessions, activeSessionId, isMinimized, setMinimized, switchSession, closeSession } = useTerminal();

    // Metrics
    const agents = metrics?.agents ?? [];
    const avgCpu = agents.length > 0 ? Math.round(agents.reduce((sum, a) => sum + a.node.cpuPercent, 0) / agents.length) : null;
    const totalMem = agents.reduce((sum, a) => sum + a.node.memoryUsed, 0);
    const totalMemMax = agents.reduce((sum, a) => sum + a.node.memoryTotal, 0);
    const memDisplay = metricsLoading || totalMemMax === 0 ? "--" : `${(totalMem / 1e9).toFixed(1)}/${(totalMemMax / 1e9).toFixed(0)}G`;

    // Tasks
    const taskCount = taskStatsLoading ? "--" : `${taskStats?.running ?? 0} TASKS`;
    const taskColor = (taskStats?.failed ?? 0) > 0 ? "text-accent-red" : "";

    // CPU display
    const cpuDisplay = metricsLoading || avgCpu === null ? "--" : `CPU ${avgCpu}%`;

    // Alerts (failed tasks)
    const alertCount = taskStats?.failed ?? 0;

    return (
        <div className="flex h-7 shrink-0 items-center border-t border-border bg-statusbar-bg/95 text-xs">
            {/* Left segments */}
            <div className="flex h-full items-center">
                <Segment icon={GitBranch} iconColor="text-accent-green" label="main" />
                <Separator />
                <Segment icon={Container} iconColor="text-text-secondary" label="Docker 24.0.7" />
                <Separator />
                <Segment
                    icon={Cpu}
                    iconColor="text-accent-cyan"
                    label={cpuDisplay}
                    testId="status-cpu"
                    valueTestId="cpu-value"
                    onClick={() => navigate({ to: "/" })}
                />
                <Separator />
                <Segment
                    icon={MemoryStick}
                    iconColor="text-accent-purple"
                    label={`MEM ${memDisplay}`}
                    testId="status-mem"
                    valueTestId="mem-value"
                    onClick={() => navigate({ to: "/" })}
                />
                <Separator />
                <Segment
                    icon={Layers}
                    iconColor="text-accent-orange"
                    label={taskCount}
                    textColor={taskColor}
                    testId="status-tasks"
                    onClick={() => navigate({ to: "/tasks" })}
                />
                <Separator />
                {alertCount > 0 && (
                    <Segment
                        icon={TriangleAlert}
                        iconColor="text-accent-red"
                        label={`${alertCount} ALERT${alertCount > 1 ? "S" : ""}`}
                        textColor="text-accent-red"
                    />
                )}
            </div>

            {/* Spacer */}
            <div className="flex-1" />

            {/* Right segments */}
            <div className="flex h-full items-center">
                {/* Terminal sessions */}
                {sessions.length > 0 ? (
                    <>
                        <Separator />
                        <div className="flex h-full items-center gap-1 px-2.5">
                            {sessions.map((session) => {
                                const name = session.target.containerName || session.target.containerId.slice(0, 12);
                                const isActive = session.id === activeSessionId;
                                return (
                                    <button
                                        key={session.id}
                                        className={`group flex items-center gap-1 rounded px-1.5 py-0.5 font-mono text-[10px] transition-colors ${isActive ? "bg-bg-elevated text-foreground" : "text-muted-foreground hover:text-foreground"}`}
                                        onClick={() => {
                                            switchSession(session.id);
                                            setMinimized(false);
                                        }}
                                    >
                                        <Terminal className="size-[11px] text-accent-green" />
                                        <span className="max-w-[80px] truncate">{name}</span>
                                        <span
                                            className="shrink-0 rounded p-0.5 opacity-0 group-hover:opacity-100 hover:bg-muted"
                                            onClick={(e) => {
                                                e.stopPropagation();
                                                closeSession(session.id);
                                            }}
                                            role="button"
                                            tabIndex={0}
                                        >
                                            <X className="size-2.5" />
                                        </span>
                                    </button>
                                );
                            })}
                            {isMinimized && (
                                <button
                                    className="rounded p-0.5 text-muted-foreground hover:text-foreground"
                                    onClick={() => setMinimized(false)}
                                >
                                    <ChevronUp className="size-3.5" />
                                </button>
                            )}
                        </div>
                    </>
                ) : (
                    <>
                        <Separator />
                        <Segment icon={Terminal} iconColor="text-accent-green" label={`${sessions.length} TERMINALS`} />
                    </>
                )}
                <Separator />
                <Segment icon={ScrollText} iconColor="text-accent-cyan" label="0 LOG STREAMS" />
                <Separator />
                <div className="flex items-center gap-[5px] px-2.5 h-full">
                    <span className="size-1.5 rounded-full bg-accent-green animate-pulse-dot" />
                    <span className="font-mono text-[10px] font-medium text-accent-green">WS CONNECTED</span>
                </div>
            </div>
        </div>
    );
}
```

Note: Git branch, Docker version, log stream count, and WS status are hardcoded for now. We'll add real hooks for these as the agent API surfaces them. The important thing is the layout and visual structure match the design.

- [ ] **Step 2: Update status-bar.spec.tsx test expectations**

The new status bar has different text formats and removes node count. Update these specific tests:

**a) "renders cluster name from active profile"** — DELETE this test. Cluster name is now in the titlebar, not the status bar.

**b) "renders node count"** — DELETE this test. Node count is now a sidebar badge.

**c) "renders task count as running/desired"** — Update expected text:
```typescript
it("renders task count", async () => {
    mockAllInvokes();
    renderWithProviders();
    await waitFor(() => {
        expect(screen.getByTestId("status-tasks")).toHaveTextContent("42 TASKS");
    });
});
```

**d) "renders aggregated CPU and MEM percentages"** — Update expected text format:
```typescript
it("renders aggregated CPU and MEM", async () => {
    mockAllInvokes();
    renderWithProviders();
    await waitFor(() => {
        expect(screen.getByTestId("cpu-value")).toHaveTextContent("CPU 35%");
    });
    // MEM: (3G + 10G) / (8G + 16G) = 13G used, 24G total → "MEM 13.0/24G"
    expect(screen.getByTestId("mem-value")).toHaveTextContent("MEM 13.0/24G");
});
```

**e) "shows warning style when nodes are down"** — DELETE this test. Nodes are no longer in the status bar.

**f) "shows warning style when tasks running < desired"** — Update to check for task text only (no warning class — the new status bar uses `text-accent-red` only for failed tasks):
```typescript
it("renders degraded task count", async () => {
    const degradedTasks: TaskStats = { total: 50, running: 35, failed: 0, preparing: 5, complete: 10, desiredRunning: 42 };
    mockAllInvokes(sampleNodes, degradedTasks);
    renderWithProviders();
    await waitFor(() => {
        expect(screen.getByTestId("status-tasks")).toHaveTextContent("35 TASKS");
    });
});
```

**g) "shows destructive style when tasks have failures"** — Update class check:
```typescript
it("shows red when tasks have failures", async () => {
    const failedTasks: TaskStats = { total: 50, running: 40, failed: 5, preparing: 0, complete: 5, desiredRunning: 42 };
    mockAllInvokes(sampleNodes, failedTasks);
    renderWithProviders();
    await waitFor(() => {
        expect(screen.getByTestId("status-tasks")).toHaveTextContent("40 TASKS");
    });
    expect(screen.getByTestId("status-tasks")).toHaveClass("text-accent-red");
});
```

**h) "shows dashes when data is loading"** — Update:
```typescript
it("shows dashes when data is loading", async () => {
    vi.mocked(invoke).mockImplementation(() => new Promise(() => {}));
    renderWithProviders();
    await waitFor(() => {
        expect(screen.getByTestId("cpu-value")).toHaveTextContent("--");
    });
    expect(screen.getByTestId("mem-value")).toHaveTextContent("--");
    expect(screen.getByTestId("status-tasks")).toHaveTextContent("--");
});
```

**i) Navigation tests for CPU/MEM** — Keep these tests, they should still work since `data-testid="status-cpu"` and `data-testid="status-mem"` are preserved.

**j) "navigates to /nodes when clicking node count"** — DELETE. Nodes are no longer in the status bar.

**k) "navigates to /tasks when clicking task count"** — Keep, update selector to `status-tasks` and wait for `"42 TASKS"` text.

- [ ] **Step 3: Run tests**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app test -- tests/unit/status-bar.spec.tsx
```

Fix any failures.

- [ ] **Step 4: Verify types pass**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 5: Commit**

```bash
git add app/src/components/status-bar.tsx app/tests/unit/status-bar.spec.tsx
git commit -m "feat: rewrite status bar with new segment layout and design tokens"
```

---

### Task 9: Update Authenticated Layout

**Files:**
- Modify: `app/src/routes/_authenticated.tsx`

New layout structure (top to bottom):
1. AccentLine (2px gradient)
2. DashboardHeader (titlebar — 38px)
3. Sidebar (left) + Main Content (right) — flex row, takes remaining height
4. StatusBar (28px)

The current layout has: sidebar left (with drag region/titlebar inset), then content area with DashboardHeader spacer, then status bar. We need to restructure so the titlebar spans the full width above both sidebar and content.

- [ ] **Step 1: Update the authenticated layout**

```tsx
import { Outlet, createFileRoute, redirect, useRouter } from "@tanstack/react-router";
import { SidebarNav } from "@/components/sidebar-nav";
import { DashboardHeader } from "@/components/dashboard-header";
import { StatusBar } from "@/components/status-bar";
import { AccentLine } from "@/components/accent-line";
import { TerminalProvider } from "@/contexts/terminal-context";
import { CommandPaletteProvider } from "@/contexts/command-palette-context";
import { CommandPalette } from "@/components/command-palette";
import { useAuth } from "@/hooks/use-auth";
import { useAgentStatus } from "@/hooks/use-agent-status";

export const Route = createFileRoute("/_authenticated")({
    beforeLoad: ({ context, location }) => {
        if (!context.auth.user) {
            throw redirect({
                to: "/login",
                search: { redirect: location.href },
            });
        }
    },
    component: AuthenticatedLayout,
});

function AuthenticatedLayout() {
    const { agentUrl, token, logout } = useAuth();
    const router = useRouter();

    useAgentStatus(agentUrl, token, () => {
        logout().then(() => router.navigate({ to: "/login", search: { redirect: undefined } }));
    });

    return (
        <TerminalProvider>
            <CommandPaletteProvider>
                <div className="flex h-screen flex-col overflow-hidden bg-background">
                    <AccentLine />
                    <DashboardHeader />
                    <div className="flex flex-1 overflow-hidden">
                        <SidebarNav />
                        <main className="flex-1 overflow-y-auto">
                            <Outlet />
                        </main>
                    </div>
                    <StatusBar />
                </div>
                <CommandPalette />
            </CommandPaletteProvider>
        </TerminalProvider>
    );
}
```

Key changes:
- Removed the `grain` class and rounded-corner inner panel — the new design uses flat surfaces
- Removed `p-2 pb-0 pl-0` padding
- Added `AccentLine` at the very top
- `DashboardHeader` (titlebar) spans full width above the sidebar+content row
- Background is `bg-background` (#0C0C0C)
- `data-tauri-drag-region` moved from sidebar to titlebar (already done in Task 6)

- [ ] **Step 2: Verify types pass**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 3: Run all frontend tests**

```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app test
```

Fix any failures caused by layout changes.

- [ ] **Step 4: Commit**

```bash
git add app/src/routes/_authenticated.tsx
git commit -m "feat: restructure authenticated layout with accent line, full-width titlebar"
```

---

### Task 10: Final Verification

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

If there are lint/format/test issues, fix them and commit the specific affected files:

```bash
git add app/src/ app/tests/
git commit -m "fix: resolve lint, format, and test issues from shell migration"
```

- [ ] **Step 6: Visual verification**

Build and run the app to visually verify:
```bash
cd /Users/clovismuneza/Apps/kontainer/app && pnpm tauri dev
```

Check:
- Accent gradient line visible at top
- Titlebar shows traffic lights, "KONTAINER" logo, cluster name, search/bell icons
- Sidebar has correct sections (INFRASTRUCTURE, ORCHESTRATION, SYSTEM)
- Sidebar items use mono font, uppercase, correct icons
- Status bar shows all segments with correct colors
- Overall dark theme with #0C0C0C background
- No visual regressions on existing pages (dashboard, services, etc.)

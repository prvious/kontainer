# Logs Page Redesign — Multi-Stream Tabs

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the logs page with multi-tab log streaming — users can open multiple service log streams in tabs, switch between them without losing data, and add new streams via a service picker dialog.

**Architecture:** Create a `LogStreamManager` that holds multiple active log streams in React state (not context — page-local). Each stream has its own `useLogStream` hook instance, tab UI, and buffered logs. The "+" button opens a command-palette-style dialog listing services from `useServices()`. The log viewer renders the active tab's logs with TIMESTAMP, LEVEL (stdout/stderr badge), and MESSAGE columns.

**Tech Stack:** React, TanStack Query, WebSocket (via existing `useLogStream` hook), Tailwind CSS v4, Lucide icons

**Design reference:** `docs/design.pen` — screen `ZkgXL` (Logs)

**What's real vs placeholder:**

| Element | Real | Placeholder |
|---------|------|-------------|
| Multi-tab log streams | ✅ `useLogStream` per tab | |
| Service picker dialog | ✅ `useServices()` for list | |
| Log entries (timestamp, line) | ✅ WebSocket data | |
| LEVEL column (parsed log levels) | | ✅ Shows "stdout"/"stderr" — see `docs/missing.md` |
| Filter logs search | ✅ Client-side text filter on log lines | |
| "New Stream" button | ✅ Opens picker dialog | |
| Tab close (X) | ✅ Disconnects WebSocket | |
| Tab switching | ✅ Buffered — logs preserved in memory | |

---

## File Structure

### New files
- `app/src/components/logs/log-stream-tabs.tsx` — Tab bar with active/inactive tabs + close buttons + "+" add button
- `app/src/components/logs/log-entry-row.tsx` — Single log line: timestamp + level badge + message
- `app/src/components/logs/stream-picker-dialog.tsx` — Command-palette-style dialog for picking a service to stream
- `app/src/components/logs/log-panel.tsx` — The log viewer panel for a single stream (column headers + scrollable log entries)

### Modified files
- `app/src/routes/_authenticated/logs/index.tsx` — Rewrite with multi-tab architecture
- `app/src/hooks/use-log-stream.ts` — No changes needed (already supports any path)

### Existing files used as-is
- `app/src/hooks/use-log-stream.ts` — WebSocket log stream hook
- `app/src/hooks/use-service-logs.ts` — Wrapper for service log path
- `app/src/hooks/use-services.ts` — Service list for the picker

---

## Architecture: Multi-Stream State

The logs page manages multiple streams using local state (not a context — this state is page-scoped):

```typescript
interface LogStream {
    id: string;              // unique tab ID (crypto.randomUUID)
    serviceId: string;       // which service to stream
    serviceName: string;     // display name for the tab
    color: string;           // rotating dot color (green, cyan, orange, purple, red)
}

// Page state:
const [streams, setStreams] = useState<LogStream[]>([]);
const [activeStreamId, setActiveStreamId] = useState<string | null>(null);
```

Each stream tab renders a hidden `<LogPanel>` that stays mounted (preserving its `useLogStream` connection and buffered logs). Only the active tab's panel is visible (`display: block` vs `display: none`). This keeps WebSocket connections alive and logs buffered for all tabs.

When a stream is closed (X), its `<LogPanel>` unmounts, which triggers `useLogStream`'s cleanup effect to close the WebSocket.

---

### Task 1: Create LogEntryRow Component

**Files:**
- Create: `app/src/components/logs/log-entry-row.tsx`

A single log line with 3 columns: timestamp (100px, muted mono), level badge (70px, stdout=green/stderr=red), message (flex, mono white).

- [ ] **Step 1: Create the component**

```tsx
import type { LogEntry } from "@/hooks/use-log-stream";

function formatTimestamp(ts: string): string {
    try {
        const d = new Date(ts);
        if (Number.isNaN(d.getTime())) return ts;
        return d.toISOString().slice(11, 23); // "HH:mm:ss.SSS"
    } catch {
        return ts;
    }
}

export function LogEntryRow({ entry }: { entry: LogEntry }) {
    return (
        <div className='flex items-start border-b border-border px-4 py-1.5'>
            {/* Timestamp */}
            <span className='w-[100px] shrink-0 font-mono text-[11px] text-muted-foreground'>
                {entry.timestamp ? formatTimestamp(entry.timestamp) : ""}
            </span>

            {/* Level badge */}
            <span className='w-[70px] shrink-0'>
                <span
                    className={`inline-block rounded px-1.5 py-0.5 font-mono text-[9px] font-bold ${
                        entry.stream === "stderr"
                            ? "bg-accent-red/15 text-accent-red"
                            : "bg-accent-green/15 text-accent-green"
                    }`}
                >
                    {entry.stream === "stderr" ? "ERR" : "OUT"}
                </span>
            </span>

            {/* Message */}
            <span className='min-w-0 flex-1 break-all font-mono text-[11px] text-foreground'>{entry.line}</span>
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
git add app/src/components/logs/log-entry-row.tsx
git commit -m "feat: add LogEntryRow component for log viewer"
```

---

### Task 2: Create LogPanel Component

**Files:**
- Create: `app/src/components/logs/log-panel.tsx`

The log viewer for a single stream. Uses `useLogStream` internally. Shows column headers (TIMESTAMP, LEVEL, MESSAGE) + scrollable log entries with auto-scroll.

- [ ] **Step 1: Create the component**

Props:
```typescript
interface LogPanelProps {
    serviceId: string;
    visible: boolean; // controls display:block/none for tab switching
}
```

Internally:
- Calls `useLogStream(`/api/services/${serviceId}/logs`, options)` with default options
- Renders column header row (bg-elevated, mono uppercase)
- Renders `<LogEntryRow>` for each log entry
- Auto-scrolls to bottom when new logs arrive (reuse the scroll pattern from existing `LogViewer`)
- Shows connection status indicator (small dot) and line count
- Shows "Connecting..." / "Waiting for logs..." / "No logs" empty states

The component stays mounted even when `visible=false` to keep the WebSocket alive and buffer logs.

```tsx
<div style={{ display: visible ? "flex" : "none" }} className='flex-1 flex-col'>
    {/* Column headers */}
    {/* Scrollable log entries */}
</div>
```

- [ ] **Step 2: Verify types**
- [ ] **Step 3: Commit**
```bash
git add app/src/components/logs/log-panel.tsx
git commit -m "feat: add LogPanel component with WebSocket log streaming"
```

---

### Task 3: Create LogStreamTabs Component

**Files:**
- Create: `app/src/components/logs/log-stream-tabs.tsx`

Tab bar with active/inactive stream tabs, close buttons, and a "+" button.

- [ ] **Step 1: Create the component**

Props:
```typescript
interface LogStreamTabsProps {
    streams: LogStream[];
    activeStreamId: string | null;
    onSelect: (id: string) => void;
    onClose: (id: string) => void;
    onAdd: () => void;
}
```

Design:
- Horizontal tab bar with `bg-card`, `border`, rounded top corners
- Each tab: colored dot + scroll-text icon + service name (mono) + X close button
- Active tab: green bottom border (2px), elevated bg, green text
- Inactive tabs: muted text, border-right separator
- "+" button at the end: muted, 36px wide
- Tab dot colors rotate through: `["bg-accent-green", "bg-accent-cyan", "bg-accent-orange", "bg-accent-purple", "bg-accent-red"]`

- [ ] **Step 2: Verify types**
- [ ] **Step 3: Commit**
```bash
git add app/src/components/logs/log-stream-tabs.tsx
git commit -m "feat: add LogStreamTabs with active/inactive states and close"
```

---

### Task 4: Create StreamPickerDialog Component

**Files:**
- Create: `app/src/components/logs/stream-picker-dialog.tsx`

A command-palette-style dialog that lists services from `useServices()`. The user searches and clicks a service to start streaming.

- [ ] **Step 1: Create the component**

Props:
```typescript
interface StreamPickerDialogProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    onSelect: (service: { id: string; name: string }) => void;
    excludeServiceIds?: string[]; // services already open in tabs
}
```

Design:
- Uses Shadcn `Dialog` component
- Search input at top with magnifying glass icon
- Scrollable list of services below
- Each service row: status dot + service name (mono bold) + image name (mono muted) + replicas badge
- Services already open in tabs are dimmed/disabled (via `excludeServiceIds`)
- Click a service → calls `onSelect` → dialog closes
- Empty state: "No services match" or "No services found"

Uses `useServices()` hook internally to fetch the list.

- [ ] **Step 2: Verify types**
- [ ] **Step 3: Commit**
```bash
git add app/src/components/logs/stream-picker-dialog.tsx
git commit -m "feat: add StreamPickerDialog for selecting service log streams"
```

---

### Task 5: Rewrite Logs Page

**Files:**
- Modify: `app/src/routes/_authenticated/logs/index.tsx`

Replace the "coming soon" placeholder with the full multi-tab log streaming page.

- [ ] **Step 1: Rewrite the page**

Structure:
```
PageHeader ("LOGS" + "// per-service log streams" + search input + "New Stream" button)
LogStreamTabs (tab bar)
LogPanel × N (one per stream, only active is visible)
StreamPickerDialog (opened by "New Stream" button or "+" tab)
```

Page state:
```typescript
const [streams, setStreams] = useState<LogStream[]>([]);
const [activeStreamId, setActiveStreamId] = useState<string | null>(null);
const [pickerOpen, setPickerOpen] = useState(false);
const [logFilter, setLogFilter] = useState("");
```

Handlers:
- `handleAddStream(service)` — adds a new `LogStream` to state, sets it as active
- `handleCloseStream(id)` — removes stream, selects previous/next tab
- `handleSelectTab(id)` — sets active stream
- `handlePickerOpen()` — opens the stream picker dialog

Color rotation: assign colors from a constant array based on stream index.

The "Filter logs..." search input in the header filters log lines in the active panel (pass `logFilter` down to the active `LogPanel`).

Empty state (no streams open): Show a centered message "No log streams active" with a button to open the picker.

- [ ] **Step 2: Verify types**
```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app types
```

- [ ] **Step 3: Commit**
```bash
git add app/src/routes/_authenticated/logs/index.tsx
git commit -m "feat: redesign logs page with multi-tab service log streaming"
```

---

### Task 6: Write Tests for Logs Page

**Files:**
- Create: `app/tests/unit/logs-page.spec.tsx`

Tests to write:

- [ ] **Step 1: Create test file with these tests**

**a) "renders empty state when no streams are open"** — No streams, shows "No log streams active" message + button to add stream

**b) "opens stream picker dialog when clicking New Stream"** — Click "New Stream" button, assert dialog opens with service list

**c) "adds a new stream tab when selecting a service"** — Open picker, click a service, assert tab appears with service name

**d) "switches between tabs"** — Open 2 streams, click inactive tab, assert it becomes active

**e) "closes a stream tab"** — Open a stream, click X on tab, assert stream is removed and empty state shows

**f) "renders page header with title LOGS"** — Assert "LOGS" heading exists

Mock `useLogStream` to avoid real WebSocket connections in tests. Mock `useServices` to return sample services for the picker.

The `useLogStream` hook can be mocked at the module level:
```typescript
vi.mock("@/hooks/use-log-stream", () => ({
    useLogStream: () => ({
        logs: [],
        status: "connected" as const,
        isPaused: false,
        clearLogs: vi.fn(),
        pause: vi.fn(),
        resume: vi.fn(),
        reconnect: vi.fn(),
    }),
}));
```

- [ ] **Step 2: Run tests**
```bash
cd /Users/clovismuneza/Apps/kontainer && pnpm --filter app test -- tests/unit/logs-page.spec.tsx
```

- [ ] **Step 3: Commit**
```bash
git add app/tests/unit/logs-page.spec.tsx
git commit -m "test: add logs page tests for multi-tab streaming"
```

---

### Task 7: Update docs/missing.md

**Files:**
- Modify: `docs/missing.md`

- [ ] **Step 1: Add logs section**

```markdown
## Logs Page

- **Structured log level parsing** — The LEVEL column shows "OUT"/"ERR" (stdout/stderr). Parsing log levels (INFO, WARN, ERROR, DEBUG) from log lines requires structured logging or pattern matching. Needs either structured log output from services or a configurable log format parser.
```

- [ ] **Step 2: Commit**
```bash
git add docs/missing.md
git commit -m "docs: add logs page missing features to missing.md"
```

---

### Task 8: Final Verification

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
git add app/src/ app/tests/ docs/
git commit -m "fix: resolve lint, format, and test issues from logs redesign"
```

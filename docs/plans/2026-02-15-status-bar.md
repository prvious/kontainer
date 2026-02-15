# Status Bar Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a persistent bottom status bar showing cluster health (name, nodes, tasks) and resource utilization (CPU, MEM) — making the app feel like a real desktop tool.

**Architecture:** Single `StatusBar` component placed in the authenticated layout (`_authenticated.tsx`), below the sidebar+content area. Consumes existing hooks (`useAuth`, `useNodes`, `useTaskStats`, `useClusterMetrics`) — no backend changes. Each segment is a clickable button navigating to its relevant page via TanStack Router's `useNavigate`.

**Tech Stack:** React, TanStack Router, TanStack Query (existing hooks), Tailwind CSS, Lucide icons, Shadcn Tooltip

---

### Task 1: Write failing test for StatusBar component

**Files:**
- Create: `app/tests/unit/status-bar.spec.tsx`

**Step 1: Write the test file**

This test covers: rendering cluster name, node count, task count, CPU/MEM gauges, click navigation, and warning states.

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import type { NodeItem } from "@/types/nodes";
import type { TaskStats } from "@/types/task-stats";
import type { ClusterMetricsResponse } from "@/types/metrics";
import { StatusBar } from "@/components/status-bar";

function createTestQueryClient() {
    return new QueryClient({
        defaultOptions: {
            queries: { retry: false, gcTime: 0 },
        },
    });
}

const mockAuth: AuthContextValue = {
    user: { id: "1", name: "Test", email: "test@test.com", avatarUrl: "", provider: "github" },
    token: "test-jwt",
    expiresAt: Math.floor(Date.now() / 1000) + 9999,
    agentUrl: "http://localhost:8080",
    isLoading: false,
    login: vi.fn(),
    logout: vi.fn(),
    profiles: [
        {
            id: "profile-1",
            label: "production-swarm",
            agentUrl: "http://localhost:8080",
            token: "test-jwt",
            expiresAt: Math.floor(Date.now() / 1000) + 9999,
            userId: "1",
            userName: "Test",
            userEmail: "test@test.com",
            userAvatarUrl: "",
            userProvider: "github",
        },
    ],
    activeProfileId: "profile-1",
    switchProfile: vi.fn(),
    removeProfile: vi.fn(),
    updateProfile: vi.fn(),
};

const sampleNodes: NodeItem[] = [
    {
        id: "node1",
        hostname: "manager-1",
        role: "manager",
        status: "ready",
        availability: "active",
        engineVersion: "24.0.7",
        os: "linux",
        architecture: "x86_64",
        ip: "10.0.0.1",
        leader: true,
        updatedAt: new Date().toISOString(),
    },
    {
        id: "node2",
        hostname: "worker-1",
        role: "worker",
        status: "ready",
        availability: "active",
        engineVersion: "24.0.7",
        os: "linux",
        architecture: "aarch64",
        ip: "10.0.0.2",
        leader: false,
        updatedAt: new Date().toISOString(),
    },
    {
        id: "node3",
        hostname: "worker-2",
        role: "worker",
        status: "ready",
        availability: "active",
        engineVersion: "24.0.7",
        os: "linux",
        architecture: "aarch64",
        ip: "10.0.0.3",
        leader: false,
        updatedAt: new Date().toISOString(),
    },
];

const sampleTaskStats: TaskStats = {
    total: 50,
    running: 42,
    failed: 0,
    preparing: 2,
    complete: 6,
    desiredRunning: 42,
};

const sampleMetrics: ClusterMetricsResponse = {
    agents: [
        {
            timestamp: new Date().toISOString(),
            nodeId: "node1",
            hostname: "manager-1",
            node: { cpus: 4, memoryTotal: 8e9, memoryUsed: 3e9, memoryPercent: 37.5, cpuPercent: 25, diskTotal: 100e9, diskUsed: 40e9, diskPercent: 40 },
            containers: [],
        },
        {
            timestamp: new Date().toISOString(),
            nodeId: "node2",
            hostname: "worker-1",
            node: { cpus: 4, memoryTotal: 8e9, memoryUsed: 5e9, memoryPercent: 62.5, cpuPercent: 45, diskTotal: 100e9, diskUsed: 60e9, diskPercent: 60 },
            containers: [],
        },
    ],
    history: [],
};

function mockInvokes(
    nodes: NodeItem[] = sampleNodes,
    taskStats: TaskStats = sampleTaskStats,
    metrics: ClusterMetricsResponse = sampleMetrics,
) {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
        if (cmd === "list_nodes") return { nodes };
        if (cmd === "list_task_stats") return { taskStats };
        if (cmd === "list_cluster_metrics") return metrics;
        return undefined;
    });
}

let navigatedTo: string | null = null;

function createTestRouter() {
    navigatedTo = null;
    const rootRoute = createRootRoute({});
    const indexRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/",
        component: () => <StatusBar />,
    });
    // Catch-all route to track navigation
    const catchAllRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "$",
        component: () => {
            // We just need to track navigation happened
            return <div data-testid="navigated" />;
        },
    });
    const nodesRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/nodes",
        component: () => <div data-testid="nodes-page" />,
    });
    const tasksRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/tasks",
        component: () => <div data-testid="tasks-page" />,
    });
    const routeTree = rootRoute.addChildren([indexRoute, nodesRoute, tasksRoute, catchAllRoute]);
    return createRouter({
        routeTree,
        history: createMemoryHistory({ initialEntries: ["/"] }),
    });
}

function renderStatusBar(authOverrides: Partial<AuthContextValue> = {}) {
    const queryClient = createTestQueryClient();
    const auth = { ...mockAuth, ...authOverrides };
    const router = createTestRouter();

    return render(
        <AuthContext value={auth}>
            <QueryClientProvider client={queryClient}>
                <RouterProvider router={router} />
            </QueryClientProvider>
        </AuthContext>,
    );
}

describe("StatusBar", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("renders cluster name from active profile", async () => {
        mockInvokes();
        renderStatusBar();

        await waitFor(() => {
            expect(screen.getByText("production-swarm")).toBeInTheDocument();
        });
    });

    it("renders node count", async () => {
        mockInvokes();
        renderStatusBar();

        await waitFor(() => {
            expect(screen.getByText("3 nodes")).toBeInTheDocument();
        });
    });

    it("renders task count as running/desired", async () => {
        mockInvokes();
        renderStatusBar();

        await waitFor(() => {
            expect(screen.getByText("42/42 tasks")).toBeInTheDocument();
        });
    });

    it("renders aggregated CPU and MEM percentages", async () => {
        mockInvokes();
        renderStatusBar();

        // Average CPU: (25 + 45) / 2 = 35%
        await waitFor(() => {
            expect(screen.getByText("CPU 35%")).toBeInTheDocument();
        });
        // Average MEM: (37.5 + 62.5) / 2 = 50%
        expect(screen.getByText("MEM 50%")).toBeInTheDocument();
    });

    it("shows warning style when nodes are down", async () => {
        const nodesWithDown = [
            ...sampleNodes.slice(0, 2),
            { ...sampleNodes[2], status: "down" },
        ];
        mockInvokes(nodesWithDown);
        renderStatusBar();

        await waitFor(() => {
            expect(screen.getByText("2/3 nodes")).toBeInTheDocument();
        });
    });

    it("shows warning style when tasks running < desired", async () => {
        const unhealthyTasks: TaskStats = {
            ...sampleTaskStats,
            running: 35,
            desiredRunning: 42,
        };
        mockInvokes(sampleNodes, unhealthyTasks);
        renderStatusBar();

        await waitFor(() => {
            expect(screen.getByText("35/42 tasks")).toBeInTheDocument();
        });
    });

    it("shows dashes when data is loading", () => {
        vi.mocked(invoke).mockImplementation(() => new Promise(() => {}));
        renderStatusBar();

        // Should show placeholder dashes for numeric values
        const dashes = screen.getAllByText("--");
        expect(dashes.length).toBeGreaterThanOrEqual(1);
    });

    it("navigates to /nodes when clicking node count", async () => {
        mockInvokes();
        const user = userEvent.setup();
        renderStatusBar();

        await waitFor(() => {
            expect(screen.getByText("3 nodes")).toBeInTheDocument();
        });

        await user.click(screen.getByText("3 nodes"));

        await waitFor(() => {
            expect(screen.getByTestId("nodes-page")).toBeInTheDocument();
        });
    });

    it("navigates to /tasks when clicking task count", async () => {
        mockInvokes();
        const user = userEvent.setup();
        renderStatusBar();

        await waitFor(() => {
            expect(screen.getByText("42/42 tasks")).toBeInTheDocument();
        });

        await user.click(screen.getByText("42/42 tasks"));

        await waitFor(() => {
            expect(screen.getByTestId("tasks-page")).toBeInTheDocument();
        });
    });
});
```

**Step 2: Run the test to verify it fails**

Run: `pnpm --filter app test tests/unit/status-bar.spec.tsx`
Expected: FAIL — `Cannot find module '@/components/status-bar'`

---

### Task 2: Create the StatusBar component

**Files:**
- Create: `app/src/components/status-bar.tsx`

**Step 1: Install shadcn tooltip if not present**

Run: `ls app/src/components/ui/tooltip.tsx || pnpm --filter app dlx shadcn@latest add tooltip`

**Step 2: Write the StatusBar component**

```tsx
import { useNavigate } from "@tanstack/react-router";
import { useAuth } from "@/hooks/use-auth";
import { useNodes } from "@/hooks/use-nodes";
import { useTaskStats } from "@/hooks/use-task-stats";
import { useClusterMetrics } from "@/hooks/use-cluster-metrics";
import { Server, ListChecks, Cpu, MemoryStick } from "lucide-react";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";

function useAggregatedMetrics() {
    const { data } = useClusterMetrics();
    if (!data?.agents?.length) return null;

    const agents = data.agents;
    const avgCpu = Math.round(agents.reduce((sum, a) => sum + a.node.cpuPercent, 0) / agents.length);
    const avgMem = Math.round(agents.reduce((sum, a) => sum + a.node.memoryPercent, 0) / agents.length);
    return { avgCpu, avgMem };
}

function percentColor(pct: number): string {
    if (pct >= 90) return "text-destructive";
    if (pct >= 70) return "text-warning";
    return "text-muted-foreground";
}

function StatusBarButton({
    onClick,
    children,
    tooltip,
}: {
    onClick: () => void;
    children: React.ReactNode;
    tooltip: string;
}) {
    return (
        <Tooltip>
            <TooltipTrigger
                render={
                    <button
                        onClick={onClick}
                        className="flex items-center gap-1.5 px-2.5 py-1 text-xs text-muted-foreground transition-colors hover:text-foreground"
                    />
                }
            >
                {children}
            </TooltipTrigger>
            <TooltipContent side="top">
                <p>{tooltip}</p>
            </TooltipContent>
        </Tooltip>
    );
}

export function StatusBar() {
    const navigate = useNavigate();
    const { profiles, activeProfileId } = useAuth();
    const { data: nodes } = useNodes();
    const { data: taskStats } = useTaskStats();
    const metrics = useAggregatedMetrics();

    const activeProfile = profiles.find((p) => p.id === activeProfileId);
    const clusterName = activeProfile?.label ?? "No cluster";

    // Node health
    const totalNodes = nodes?.length;
    const readyNodes = nodes?.filter((n) => n.status === "ready").length;
    const hasNodeIssue = totalNodes !== undefined && readyNodes !== undefined && readyNodes < totalNodes;
    const nodeText = totalNodes === undefined ? "--" : hasNodeIssue ? `${readyNodes}/${totalNodes} nodes` : `${totalNodes} nodes`;

    // Task health
    const running = taskStats?.running;
    const desired = taskStats?.desiredRunning;
    const hasTaskIssue = running !== undefined && desired !== undefined && running < desired;
    const taskText =
        running === undefined || desired === undefined ? "--" : `${running}/${desired} tasks`;

    return (
        <TooltipProvider delayDuration={300}>
            <div className="flex h-7 shrink-0 items-center justify-between border-t border-border bg-card px-1">
                <div className="flex items-center">
                    {/* Cluster name */}
                    <div className="flex items-center gap-1.5 px-2.5 text-xs font-medium text-foreground">
                        <span className="size-1.5 shrink-0 animate-pulse-dot rounded-full bg-primary" />
                        {clusterName}
                    </div>

                    <span className="h-3 w-px bg-border" />

                    {/* Nodes */}
                    <StatusBarButton onClick={() => navigate({ to: "/nodes" })} tooltip="View nodes">
                        <Server className="size-3" />
                        <span className={hasNodeIssue ? "text-warning" : ""}>{nodeText}</span>
                    </StatusBarButton>

                    <span className="h-3 w-px bg-border" />

                    {/* Tasks */}
                    <StatusBarButton onClick={() => navigate({ to: "/tasks" })} tooltip="View tasks">
                        <ListChecks className="size-3" />
                        <span className={hasTaskIssue ? "text-warning" : taskStats?.failed ? "text-destructive" : ""}>
                            {taskText}
                        </span>
                    </StatusBarButton>
                </div>

                <div className="flex items-center">
                    {/* CPU */}
                    <StatusBarButton onClick={() => navigate({ to: "/" })} tooltip="View dashboard">
                        <Cpu className="size-3" />
                        <span className={metrics ? percentColor(metrics.avgCpu) : ""}>
                            {metrics ? `CPU ${metrics.avgCpu}%` : "CPU --"}
                        </span>
                    </StatusBarButton>

                    <span className="h-3 w-px bg-border" />

                    {/* Memory */}
                    <StatusBarButton onClick={() => navigate({ to: "/" })} tooltip="View dashboard">
                        <MemoryStick className="size-3" />
                        <span className={metrics ? percentColor(metrics.avgMem) : ""}>
                            {metrics ? `MEM ${metrics.avgMem}%` : "MEM --"}
                        </span>
                    </StatusBarButton>
                </div>
            </div>
        </TooltipProvider>
    );
}
```

**Step 3: Run test to verify it passes**

Run: `pnpm --filter app test tests/unit/status-bar.spec.tsx`
Expected: All tests PASS

**Step 4: Commit**

```bash
git add app/src/components/status-bar.tsx app/tests/unit/status-bar.spec.tsx
git commit -m "feat: add StatusBar component with cluster health and resource gauges"
```

---

### Task 3: Wire StatusBar into authenticated layout

**Files:**
- Modify: `app/src/routes/_authenticated.tsx`

**Step 1: Add StatusBar to layout**

In `_authenticated.tsx`, import and render `<StatusBar />` below the content area, outside the sidebar/content flex but inside the outermost container. The layout becomes:

```tsx
import { StatusBar } from "@/components/status-bar";

function AuthenticatedLayout() {
    return (
        <TerminalProvider>
            <CommandPaletteProvider>
                <div className="flex h-screen flex-col overflow-hidden">
                    <div className="flex flex-1 overflow-hidden p-2 pl-0">
                        <SidebarNav />
                        <div className="flex flex-1 flex-col overflow-hidden">
                            <DashboardHeader />
                            <div className="grain flex flex-1 flex-col overflow-hidden rounded-xl bg-background shadow-sm">
                                <main className="flex-1 overflow-y-auto">
                                    <Outlet />
                                </main>
                            </div>
                        </div>
                    </div>
                    <StatusBar />
                </div>
                <CommandPalette />
            </CommandPaletteProvider>
        </TerminalProvider>
    );
}
```

Key change: wrap the existing `flex h-screen` div in a new `flex h-screen flex-col` container. The existing sidebar+content becomes `flex-1 overflow-hidden`, and `<StatusBar />` sits at the bottom with `shrink-0`.

**Step 2: Verify visually**

Run: `pnpm dev:app`
Expected: Status bar visible at the bottom of the window. Cluster name, node count, task count, CPU%, MEM% all rendering. Clicking items navigates to correct pages.

**Step 3: Run all tests**

Run: `pnpm --filter app test`
Expected: All tests PASS (no existing tests broken by layout change)

**Step 4: Run lint, types, format**

Run: `pnpm --filter app run types && pnpm --filter app run lint && pnpm --filter app fmt`
Expected: All clean

**Step 5: Commit**

```bash
git add app/src/routes/_authenticated.tsx
git commit -m "feat: wire StatusBar into authenticated layout"
```

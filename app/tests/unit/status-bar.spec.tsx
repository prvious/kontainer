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
        architecture: "x86_64",
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
            node: {
                cpus: 4,
                memoryTotal: 8_000_000_000,
                memoryUsed: 3_000_000_000,
                memoryPercent: 37.5,
                cpuPercent: 25,
                diskTotal: 100_000_000_000,
                diskUsed: 50_000_000_000,
                diskPercent: 50,
            },
            containers: [],
        },
        {
            timestamp: new Date().toISOString(),
            nodeId: "node2",
            hostname: "worker-1",
            node: {
                cpus: 8,
                memoryTotal: 16_000_000_000,
                memoryUsed: 10_000_000_000,
                memoryPercent: 62.5,
                cpuPercent: 45,
                diskTotal: 200_000_000_000,
                diskUsed: 120_000_000_000,
                diskPercent: 60,
            },
            containers: [],
        },
    ],
    history: [],
};

let navigatedTo: string | null = null;

function createTestRouter(component: () => React.JSX.Element) {
    navigatedTo = null;
    const rootRoute = createRootRoute({});
    const indexRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/",
        component,
    });
    const nodesRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/nodes",
        component: () => {
            navigatedTo = "/nodes";
            return <div>Nodes Page</div>;
        },
    });
    const tasksRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/tasks",
        component: () => {
            navigatedTo = "/tasks";
            return <div>Tasks Page</div>;
        },
    });
    const routeTree = rootRoute.addChildren([indexRoute, nodesRoute, tasksRoute]);
    return createRouter({
        routeTree,
        history: createMemoryHistory({ initialEntries: ["/"] }),
    });
}

function renderWithProviders(authOverrides: Partial<AuthContextValue> = {}) {
    const queryClient = createTestQueryClient();
    const auth = { ...mockAuth, ...authOverrides };
    const router = createTestRouter(StatusBar);

    return render(
        <AuthContext value={auth}>
            <QueryClientProvider client={queryClient}>
                <RouterProvider router={router} />
            </QueryClientProvider>
        </AuthContext>,
    );
}

function mockAllInvokes(nodesData: NodeItem[] = sampleNodes, taskStatsData: TaskStats = sampleTaskStats, metricsData: ClusterMetricsResponse = sampleMetrics) {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
        if (cmd === "list_nodes") return { nodes: nodesData };
        if (cmd === "list_task_stats") return { taskStats: taskStatsData };
        if (cmd === "list_cluster_metrics") return metricsData;
        return undefined;
    });
}

describe("StatusBar", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        navigatedTo = null;
    });

    it("renders cluster name from active profile", async () => {
        mockAllInvokes();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("production-swarm")).toBeInTheDocument();
        });
    });

    it("renders node count", async () => {
        mockAllInvokes();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("3 nodes")).toBeInTheDocument();
        });
    });

    it("renders task count as running/desired", async () => {
        mockAllInvokes();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("42/42 tasks")).toBeInTheDocument();
        });
    });

    it("renders aggregated CPU and MEM percentages", async () => {
        mockAllInvokes();
        renderWithProviders();

        // Average CPU: (25 + 45) / 2 = 35%
        // Average MEM: (37.5 + 62.5) / 2 = 50%
        await waitFor(() => {
            expect(screen.getByTestId("cpu-value")).toHaveTextContent("35%");
        });
        expect(screen.getByTestId("mem-value")).toHaveTextContent("50%");
    });

    it("shows warning style when nodes are down", async () => {
        const nodesWithDown: NodeItem[] = [
            ...sampleNodes.slice(0, 2),
            {
                id: "node3",
                hostname: "worker-2",
                role: "worker",
                status: "down",
                availability: "active",
                engineVersion: "24.0.7",
                os: "linux",
                architecture: "x86_64",
                ip: "10.0.0.3",
                leader: false,
                updatedAt: new Date().toISOString(),
            },
        ];

        mockAllInvokes(nodesWithDown);
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("2/3 nodes")).toBeInTheDocument();
        });

        const nodeButton = screen.getByTestId("status-nodes");
        expect(nodeButton).toHaveClass("text-warning");
    });

    it("shows warning style when tasks running < desired", async () => {
        const degradedTasks: TaskStats = {
            total: 50,
            running: 35,
            failed: 0,
            preparing: 5,
            complete: 10,
            desiredRunning: 42,
        };

        mockAllInvokes(sampleNodes, degradedTasks);
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("35/42 tasks")).toBeInTheDocument();
        });

        const taskButton = screen.getByTestId("status-tasks");
        expect(taskButton).toHaveClass("text-warning");
    });

    it("shows destructive style when tasks have failures", async () => {
        const failedTasks: TaskStats = {
            total: 50,
            running: 40,
            failed: 5,
            preparing: 0,
            complete: 5,
            desiredRunning: 42,
        };

        mockAllInvokes(sampleNodes, failedTasks);
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("40/42 tasks")).toBeInTheDocument();
        });

        const taskButton = screen.getByTestId("status-tasks");
        expect(taskButton).toHaveClass("text-destructive");
    });

    it("shows dashes when data is loading", async () => {
        vi.mocked(invoke).mockImplementation(() => new Promise(() => {}));
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByTestId("cpu-value")).toHaveTextContent("--");
        });
        expect(screen.getByTestId("mem-value")).toHaveTextContent("--");
        expect(screen.getByTestId("status-nodes")).toHaveTextContent("--");
        expect(screen.getByTestId("status-tasks")).toHaveTextContent("--");
    });

    it("navigates to /nodes when clicking node count", async () => {
        mockAllInvokes();
        const user = userEvent.setup();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("3 nodes")).toBeInTheDocument();
        });

        await user.click(screen.getByTestId("status-nodes"));

        await waitFor(() => {
            expect(navigatedTo).toBe("/nodes");
        });
    });

    it("navigates to /tasks when clicking task count", async () => {
        mockAllInvokes();
        const user = userEvent.setup();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("42/42 tasks")).toBeInTheDocument();
        });

        await user.click(screen.getByTestId("status-tasks"));

        await waitFor(() => {
            expect(navigatedTo).toBe("/tasks");
        });
    });

    it("navigates to / when clicking CPU segment", async () => {
        mockAllInvokes();
        const user = userEvent.setup();
        // Start from /nodes so we can detect navigation to /
        const queryClient = createTestQueryClient();
        const auth = { ...mockAuth };
        const rootRoute = createRootRoute({});
        const indexRoute = createRoute({
            getParentRoute: () => rootRoute,
            path: "/",
            component: () => <div data-testid='dashboard-page'>Dashboard</div>,
        });
        const nodesRoute = createRoute({
            getParentRoute: () => rootRoute,
            path: "/nodes",
            component: StatusBar,
        });
        const tasksRoute = createRoute({
            getParentRoute: () => rootRoute,
            path: "/tasks",
            component: () => <div>Tasks Page</div>,
        });
        const routeTree = rootRoute.addChildren([indexRoute, nodesRoute, tasksRoute]);
        const router = createRouter({
            routeTree,
            history: createMemoryHistory({ initialEntries: ["/nodes"] }),
        });

        render(
            <AuthContext value={auth}>
                <QueryClientProvider client={queryClient}>
                    <RouterProvider router={router} />
                </QueryClientProvider>
            </AuthContext>,
        );

        await waitFor(() => {
            expect(screen.getByTestId("cpu-value")).toBeInTheDocument();
        });

        await user.click(screen.getByTestId("status-cpu"));

        await waitFor(() => {
            expect(screen.getByTestId("dashboard-page")).toBeInTheDocument();
        });
    });

    it("navigates to / when clicking MEM segment", async () => {
        mockAllInvokes();
        const user = userEvent.setup();
        // Start from /nodes so we can detect navigation to /
        const queryClient = createTestQueryClient();
        const auth = { ...mockAuth };
        const rootRoute = createRootRoute({});
        const indexRoute = createRoute({
            getParentRoute: () => rootRoute,
            path: "/",
            component: () => <div data-testid='dashboard-page'>Dashboard</div>,
        });
        const nodesRoute = createRoute({
            getParentRoute: () => rootRoute,
            path: "/nodes",
            component: StatusBar,
        });
        const tasksRoute = createRoute({
            getParentRoute: () => rootRoute,
            path: "/tasks",
            component: () => <div>Tasks Page</div>,
        });
        const routeTree = rootRoute.addChildren([indexRoute, nodesRoute, tasksRoute]);
        const router = createRouter({
            routeTree,
            history: createMemoryHistory({ initialEntries: ["/nodes"] }),
        });

        render(
            <AuthContext value={auth}>
                <QueryClientProvider client={queryClient}>
                    <RouterProvider router={router} />
                </QueryClientProvider>
            </AuthContext>,
        );

        await waitFor(() => {
            expect(screen.getByTestId("mem-value")).toBeInTheDocument();
        });

        await user.click(screen.getByTestId("status-mem"));

        await waitFor(() => {
            expect(screen.getByTestId("dashboard-page")).toBeInTheDocument();
        });
    });
});

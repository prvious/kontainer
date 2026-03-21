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
import type { TaskItem } from "@/types/tasks";
import type { ClusterMetricsResponse } from "@/types/metrics";

// Import the actual dashboard page component from the route file
import { Route as DashboardRoute } from "@/routes/_authenticated/index";

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
    agentUrl: "http://localhost:3000",
    isLoading: false,
    profiles: [
        {
            id: "profile-1",
            label: "My Cluster",
            agentUrl: "http://localhost:3000",
            userId: "1",
            userName: "Test",
            userEmail: "test@test.com",
            userAvatarUrl: "",
            userProvider: "github",
            token: "test-jwt",
            expiresAt: Math.floor(Date.now() / 1000) + 9999,
        },
    ],
    activeProfileId: "profile-1",
    login: vi.fn(),
    logout: vi.fn(),
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
];

const sampleTaskStats: TaskStats = {
    total: 10,
    running: 7,
    failed: 1,
    preparing: 1,
    complete: 1,
    desiredRunning: 8,
};

const sampleTasks: TaskItem[] = [
    {
        id: "task-1",
        serviceId: "svc-1",
        serviceName: "nginx-proxy",
        slot: 1,
        nodeId: "node1",
        nodeName: "manager-1",
        state: "running",
        desiredState: "running",
        exitCode: null,
        error: "",
        timestamp: new Date().toISOString(),
        containerId: "container-abc123",
        image: "nginx:1.25-alpine",
    },
    {
        id: "task-2",
        serviceId: "svc-2",
        serviceName: "api-gateway",
        slot: 1,
        nodeId: "node1",
        nodeName: "manager-1",
        state: "running",
        desiredState: "running",
        exitCode: null,
        error: "",
        timestamp: new Date().toISOString(),
        containerId: "container-def456",
        image: "kontainer/gateway:2.1",
    },
];

const sampleClusterMetrics: ClusterMetricsResponse = {
    agents: [],
    history: [],
};

function DashboardPageComponent() {
    // Re-export the route's component for test rendering
    const Component = DashboardRoute.options.component!;
    return <Component />;
}

function createTestRouter(component: () => JSX.Element) {
    const rootRoute = createRootRoute({});
    const indexRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/",
        component,
    });
    const routeTree = rootRoute.addChildren([indexRoute]);
    return createRouter({
        routeTree,
        history: createMemoryHistory({ initialEntries: ["/"] }),
    });
}

let testQueryClient: QueryClient;

function renderWithProviders(authOverrides: Partial<AuthContextValue> = {}) {
    testQueryClient = createTestQueryClient();
    const auth = { ...mockAuth, ...authOverrides };
    const router = createTestRouter(DashboardPageComponent);

    return render(
        <AuthContext value={auth}>
            <QueryClientProvider client={testQueryClient}>
                <RouterProvider router={router} />
            </QueryClientProvider>
        </AuthContext>,
    );
}

function mockAllInvokes(
    nodesData: NodeItem[] = sampleNodes,
    taskStatsData: TaskStats = sampleTaskStats,
    tasksData: TaskItem[] = sampleTasks,
    metricsData: ClusterMetricsResponse = sampleClusterMetrics,
) {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
        if (cmd === "list_nodes") return { nodes: nodesData };
        if (cmd === "list_task_stats") return { taskStats: taskStatsData };
        if (cmd === "list_tasks") return { tasks: tasksData };
        if (cmd === "list_cluster_metrics") return metricsData;
        return undefined;
    });
}

describe("DashboardOverview", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("renders CLUSTER OVERVIEW title", async () => {
        mockAllInvokes();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("CLUSTER OVERVIEW")).toBeInTheDocument();
        });
    });

    it("renders subtitle with node and container counts", async () => {
        mockAllInvokes();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("CLUSTER OVERVIEW")).toBeInTheDocument();
        });

        // Subtitle should contain cluster name, node count, and running container count
        // "My Cluster · 2 nodes · 7 containers"
        const subtitle = screen.getByText(/My Cluster/);
        expect(subtitle).toBeInTheDocument();
        expect(subtitle.textContent).toContain("2 nodes");
        expect(subtitle.textContent).toContain("7 containers");
    });

    it("renders 4 metric cards", async () => {
        mockAllInvokes();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("CONTAINERS")).toBeInTheDocument();
        });

        expect(screen.getByText("CPU USAGE")).toBeInTheDocument();
        expect(screen.getByText("MEMORY")).toBeInTheDocument();
        expect(screen.getByText("NETWORK I/O")).toBeInTheDocument();
    });

    it("renders traefik banner", async () => {
        mockAllInvokes();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Traefik Proxy")).toBeInTheDocument();
        });
    });

    it("renders containers table header", async () => {
        mockAllInvokes();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("RUNNING CONTAINERS")).toBeInTheDocument();
        });
    });

    it("renders container rows from tasks data", async () => {
        mockAllInvokes();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("nginx-proxy")).toBeInTheDocument();
        });

        expect(screen.getByText("api-gateway")).toBeInTheDocument();
    });

    it("shows loading skeleton when data is loading", async () => {
        // Mock invoke to never resolve, keeping queries in loading state
        vi.mocked(invoke).mockImplementation(() => new Promise(() => {}));
        renderWithProviders();

        // The LoadingSkeleton renders skeleton elements; check for their presence
        await waitFor(() => {
            const skeletons = document.querySelectorAll('[data-slot="skeleton"]');
            expect(skeletons.length).toBeGreaterThan(0);
        });

        // The main content should NOT be present
        expect(screen.queryByText("CLUSTER OVERVIEW")).not.toBeInTheDocument();
    });

    it("shows error state with retry button", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") throw new Error("network error");
            if (cmd === "list_nodes") return { nodes: sampleNodes };
            if (cmd === "list_task_stats") return { taskStats: sampleTaskStats };
            if (cmd === "list_cluster_metrics") return sampleClusterMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Failed to connect to agent")).toBeInTheDocument();
        });

        expect(screen.getByRole("button", { name: /retry/i })).toBeInTheDocument();
    });

    it("REFRESH button invalidates queries", async () => {
        mockAllInvokes();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("CLUSTER OVERVIEW")).toBeInTheDocument();
        });

        // Clear mock call history so we can detect new calls
        vi.mocked(invoke).mockClear();
        mockAllInvokes();

        const refreshButton = screen.getByRole("button", { name: /refresh/i });
        await userEvent.click(refreshButton);

        // After clicking refresh, queries should be invalidated, causing new invoke calls
        await waitFor(() => {
            expect(vi.mocked(invoke)).toHaveBeenCalled();
        });
    });
});

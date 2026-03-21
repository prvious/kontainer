import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import { SettingsContext } from "@/contexts/settings-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import type { SettingsContextValue } from "@/contexts/settings-context-value";
import type { TaskItem } from "@/types/tasks";
import type { ClusterMetricsResponse } from "@/types/metrics";

import { Route as TasksRoute } from "@/routes/_authenticated/tasks/index";

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
    login: vi.fn(),
    logout: vi.fn(),
};

const mockSettings: SettingsContextValue = {
    refreshInterval: 5000,
    setRefreshInterval: vi.fn(),
};

function createTestRouter() {
    const rootRoute = createRootRoute({});
    const tasksRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/tasks",
        component: TasksRoute.options.component,
    });
    const routeTree = rootRoute.addChildren([tasksRoute]);
    return createRouter({
        routeTree,
        history: createMemoryHistory({ initialEntries: ["/tasks"] }),
    });
}

function renderWithProviders(authOverrides: Partial<AuthContextValue> = {}, settingsOverrides: Partial<SettingsContextValue> = {}) {
    const queryClient = createTestQueryClient();
    const auth = { ...mockAuth, ...authOverrides };
    const settings = { ...mockSettings, ...settingsOverrides };
    const router = createTestRouter();

    return render(
        <AuthContext value={auth}>
            <SettingsContext value={settings}>
                <QueryClientProvider client={queryClient}>
                    <RouterProvider router={router} />
                </QueryClientProvider>
            </SettingsContext>
        </AuthContext>,
    );
}

const sampleTasks: TaskItem[] = [
    {
        id: "task1",
        serviceId: "svc1",
        serviceName: "web",
        slot: 1,
        nodeId: "node1",
        nodeName: "worker-1",
        state: "running",
        desiredState: "running",
        exitCode: null,
        error: "",
        timestamp: new Date(Date.now() - 60000).toISOString(),
        containerId: "abc123",
        image: "nginx:latest",
    },
    {
        id: "task2",
        serviceId: "svc2",
        serviceName: "api",
        slot: 1,
        nodeId: "node2",
        nodeName: "worker-2",
        state: "failed",
        desiredState: "running",
        exitCode: 1,
        error: "container exited",
        timestamp: new Date(Date.now() - 3600000).toISOString(),
        containerId: "def456",
        image: "node:20",
    },
    {
        id: "task3",
        serviceId: "svc3",
        serviceName: "database",
        slot: 1,
        nodeId: "node3",
        nodeName: "worker-3",
        state: "complete",
        desiredState: "shutdown",
        exitCode: 0,
        error: "",
        timestamp: new Date(Date.now() - 86400000).toISOString(),
        containerId: "ghi789",
        image: "postgres:16",
    },
];

const emptyMetrics: ClusterMetricsResponse = { agents: [], history: [] };

const sampleMetrics: ClusterMetricsResponse = {
    agents: [
        {
            timestamp: new Date().toISOString(),
            nodeId: "node1",
            hostname: "worker-1",
            node: { cpus: 4, memoryTotal: 8e9, memoryUsed: 4e9, memoryPercent: 50, cpuPercent: 25, diskTotal: 100e9, diskUsed: 50e9, diskPercent: 50 },
            containers: [
                {
                    containerId: "abc123",
                    name: "web.1",
                    serviceName: "web",
                    cpuPercent: 12.5,
                    memoryUsed: 2e9,
                    memoryLimit: 4e9,
                    memoryPercent: 50,
                    createdAt: Date.now() / 1000 - 3600,
                },
            ],
        },
    ],
    history: [],
};

describe("TasksPage", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("shows loading skeleton initially", async () => {
        vi.mocked(invoke).mockImplementation(() => new Promise(() => {}));
        renderWithProviders();

        await waitFor(() => {
            const skeletons = document.querySelectorAll('[data-slot="skeleton"]');
            expect(skeletons.length).toBeGreaterThan(0);
        });
    });

    it("renders containers table after loading", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: sampleTasks };
            if (cmd === "list_nodes") return { nodes: [] };
            if (cmd === "list_cluster_metrics") return emptyMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("web")).toBeInTheDocument();
        });

        expect(screen.getByText("api")).toBeInTheDocument();
        expect(screen.getByText("database")).toBeInTheDocument();
        expect(screen.getByText("nginx:latest")).toBeInTheDocument();
        expect(screen.getByText("node:20")).toBeInTheDocument();
    });

    it("shows page header with title and subtitle", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: sampleTasks };
            if (cmd === "list_nodes") return { nodes: [{ id: "n1" }, { id: "n2" }] };
            if (cmd === "list_cluster_metrics") return emptyMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("CONTAINERS")).toBeInTheDocument();
        });

        expect(screen.getByText("ALL CONTAINERS")).toBeInTheDocument();
    });

    it("shows container count in table shell", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: sampleTasks };
            if (cmd === "list_nodes") return { nodes: [] };
            if (cmd === "list_cluster_metrics") return emptyMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("3")).toBeInTheDocument();
        });
    });

    it("shows status labels (UP, EXIT) instead of raw state", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: sampleTasks };
            if (cmd === "list_nodes") return { nodes: [] };
            if (cmd === "list_cluster_metrics") return emptyMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("UP")).toBeInTheDocument();
        });

        const exitLabels = screen.getAllByText("EXIT");
        expect(exitLabels.length).toBe(2);
    });

    it("shows empty state when no containers", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: [] };
            if (cmd === "list_nodes") return { nodes: [] };
            if (cmd === "list_cluster_metrics") return emptyMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("No containers found")).toBeInTheDocument();
        });
    });

    it("shows error state when fetch fails", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") throw new Error("network error");
            if (cmd === "list_nodes") return { nodes: [] };
            if (cmd === "list_cluster_metrics") return emptyMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Failed to load tasks")).toBeInTheDocument();
        });
    });

    it("filters containers by search query", async () => {
        const user = userEvent.setup();
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: sampleTasks };
            if (cmd === "list_nodes") return { nodes: [] };
            if (cmd === "list_cluster_metrics") return emptyMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("web")).toBeInTheDocument();
        });

        const searchInput = screen.getByPlaceholderText("Search containers...");
        await user.type(searchInput, "api");

        expect(screen.getByText("api")).toBeInTheDocument();
        expect(screen.queryByText("web")).not.toBeInTheDocument();
        expect(screen.queryByText("database")).not.toBeInTheDocument();
    });

    it("shows no results message when search has no matches", async () => {
        const user = userEvent.setup();
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: sampleTasks };
            if (cmd === "list_nodes") return { nodes: [] };
            if (cmd === "list_cluster_metrics") return emptyMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("web")).toBeInTheDocument();
        });

        const searchInput = screen.getByPlaceholderText("Search containers...");
        await user.type(searchInput, "nonexistent");

        await waitFor(() => {
            expect(screen.getByText("No containers match your filter")).toBeInTheDocument();
        });
    });

    it("renders CPU and MEM from cluster metrics", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: [sampleTasks[0]] };
            if (cmd === "list_nodes") return { nodes: [] };
            if (cmd === "list_cluster_metrics") return sampleMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("12.5%")).toBeInTheDocument();
        });

        expect(screen.getByText("2.0 GB")).toBeInTheDocument();
    });

    it("highlights CPU > 8% with orange", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: [sampleTasks[0]] };
            if (cmd === "list_nodes") return { nodes: [] };
            if (cmd === "list_cluster_metrics") return sampleMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            const cpuEl = screen.getByText("12.5%");
            expect(cpuEl.className).toContain("text-accent-orange");
        });
    });

    it("highlights MEM >= 1GB with orange", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: [sampleTasks[0]] };
            if (cmd === "list_nodes") return { nodes: [] };
            if (cmd === "list_cluster_metrics") return sampleMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            const memEl = screen.getByText("2.0 GB");
            expect(memEl.className).toContain("text-accent-orange");
        });
    });

    it("applies error background for failed tasks", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: [sampleTasks[1]] };
            if (cmd === "list_nodes") return { nodes: [] };
            if (cmd === "list_cluster_metrics") return emptyMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("EXIT")).toBeInTheDocument();
        });

        const row = screen.getByText("EXIT").closest('[class*="border-b"]');
        expect(row?.className).toContain("bg-accent-red/5");
    });

    it("does not fetch when auth is missing", () => {
        vi.mocked(invoke).mockImplementation(async () => ({ tasks: [] }));

        renderWithProviders({ token: null, agentUrl: null });

        expect(invoke).not.toHaveBeenCalledWith("list_tasks", expect.anything());
    });
});

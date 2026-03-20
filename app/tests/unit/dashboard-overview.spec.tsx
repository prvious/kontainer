import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import type { ServiceItem } from "@/types/services";
import type { NodeItem } from "@/types/nodes";
import type { TaskStats } from "@/types/task-stats";
import { useServices } from "@/hooks/use-services";
import { useNodes } from "@/hooks/use-nodes";
import { useTaskStats } from "@/hooks/use-task-stats";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { Progress } from "@/components/ui/progress";
import { AlertTriangle } from "lucide-react";

function computeServiceStats(services: ServiceItem[]) {
    const total = services.length;
    const running = services.filter((s) => s.replicasRunning > 0).length;
    const healthPct = total > 0 ? Math.round((running / total) * 100) : 0;
    return { total, running, healthPct };
}

function computeNodeStats(nodes: NodeItem[]) {
    const total = nodes.length;
    const managers = nodes.filter((n) => n.role === "manager").length;
    const workers = nodes.filter((n) => n.role === "worker").length;
    const down = nodes.filter((n) => n.status === "down" || n.status === "disconnected").length;
    const ready = nodes.filter((n) => n.status === "ready").length;
    const healthPct = total > 0 ? Math.round((ready / total) * 100) : 0;
    return { total, managers, workers, down, ready, healthPct };
}

function StatCard({ label, value, accent }: { label: string; value: string | number; accent?: string }) {
    return (
        <Card>
            <CardContent className='flex items-center gap-4 p-4'>
                <div className={`flex size-10 items-center justify-center rounded-lg ${accent ?? "bg-muted"}`} />
                <div>
                    <p className='text-2xl font-semibold tracking-tight'>{value}</p>
                    <p className='text-xs text-muted-foreground'>{label}</p>
                </div>
            </CardContent>
        </Card>
    );
}

function LoadingSkeleton() {
    return (
        <div data-testid='loading-skeleton' className='space-y-6 p-6'>
            <div className='grid gap-4 sm:grid-cols-2 lg:grid-cols-4'>
                {[1, 2, 3, 4].map((i) => (
                    <Card key={i}>
                        <CardContent className='flex items-center gap-4 p-4'>
                            <Skeleton className='size-10 rounded-lg' />
                            <div className='space-y-1.5'>
                                <Skeleton className='h-6 w-12' />
                                <Skeleton className='h-3 w-20' />
                            </div>
                        </CardContent>
                    </Card>
                ))}
            </div>
        </div>
    );
}

function DashboardTestComponent() {
    const { data: services, isLoading: servicesLoading, error: servicesError } = useServices();
    const { data: nodes, isLoading: nodesLoading, error: nodesError } = useNodes();
    const { data: taskStats, isLoading: taskStatsLoading, error: taskStatsError } = useTaskStats();

    const isLoading = servicesLoading || nodesLoading || taskStatsLoading;
    const error = servicesError || nodesError || taskStatsError;

    if (isLoading) return <LoadingSkeleton />;

    if (error) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
                <AlertTriangle className='size-5' />
                <p>Failed to connect to agent</p>
            </div>
        );
    }

    const serviceItems = services ?? [];
    const nodeItems = nodes ?? [];
    const stats: TaskStats = taskStats ?? { total: 0, running: 0, failed: 0, preparing: 0, complete: 0, desiredRunning: 0 };
    const svcStats = computeServiceStats(serviceItems);
    const nStats = computeNodeStats(nodeItems);
    const taskHealthPct = stats.desiredRunning > 0 ? Math.round((stats.running / stats.desiredRunning) * 100) : 0;
    const composite = Math.round((svcStats.healthPct + nStats.healthPct + taskHealthPct) / 3);

    return (
        <div>
            {/* Stat Cards */}
            <StatCard label='Total Services' value={svcStats.total} />
            <StatCard label='Total Nodes' value={nStats.total} />
            <StatCard label='Running Tasks' value={stats.running} />
            <StatCard label='Failed Tasks' value={stats.failed} accent={stats.failed > 0 ? "bg-destructive/10 text-destructive" : "bg-muted"} />

            {/* Cluster Health */}
            <Card>
                <CardHeader>
                    <CardTitle>Cluster Health</CardTitle>
                    <span data-testid='health-pct'>{composite}%</span>
                </CardHeader>
                <CardContent>
                    <Progress value={composite} className='h-2' />
                </CardContent>
            </Card>

            {/* Nodes Overview */}
            <Card>
                <CardHeader>
                    <CardTitle>Nodes Overview</CardTitle>
                </CardHeader>
                <CardContent>
                    <div className='grid grid-cols-3 gap-4 text-center'>
                        <div>
                            <p data-testid='managers-count'>{nStats.managers}</p>
                            <p>Managers</p>
                        </div>
                        <div>
                            <p data-testid='workers-count'>{nStats.workers}</p>
                            <p>Workers</p>
                        </div>
                        <div>
                            <p data-testid='down-count' className={nStats.down > 0 ? "text-destructive" : ""}>
                                {nStats.down}
                            </p>
                            <p>Down</p>
                        </div>
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}

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

function createTestRouter() {
    const rootRoute = createRootRoute({});
    const indexRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/",
        component: DashboardTestComponent,
    });
    const routeTree = rootRoute.addChildren([indexRoute]);
    return createRouter({
        routeTree,
        history: createMemoryHistory({ initialEntries: ["/"] }),
    });
}

function renderWithProviders(authOverrides: Partial<AuthContextValue> = {}) {
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

const sampleServices: ServiceItem[] = [
    {
        id: "svc1",
        name: "web-app",
        image: "nginx:latest",
        stack: "frontend",
        replicasRunning: 3,
        replicasDesired: 3,
        updatedAt: new Date().toISOString(),
    },
    {
        id: "svc2",
        name: "api-server",
        image: "node:18",
        stack: "backend",
        replicasRunning: 2,
        replicasDesired: 2,
        updatedAt: new Date().toISOString(),
    },
];

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

function mockAllInvokes(servicesData: ServiceItem[] = sampleServices, nodesData: NodeItem[] = sampleNodes, taskStatsData: TaskStats = sampleTaskStats) {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
        if (cmd === "list_services") return { services: servicesData };
        if (cmd === "list_nodes") return { nodes: nodesData };
        if (cmd === "list_task_stats") return { taskStats: taskStatsData };
        return undefined;
    });
}

describe("DashboardOverview", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("shows loading skeleton initially", async () => {
        vi.mocked(invoke).mockImplementation(() => new Promise(() => {}));
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByTestId("loading-skeleton")).toBeInTheDocument();
        });
    });

    it("renders all stat cards with correct values", async () => {
        mockAllInvokes();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Total Services")).toBeInTheDocument();
        });

        // Check that labels exist
        expect(screen.getByText("Total Nodes")).toBeInTheDocument();
        expect(screen.getByText("Running Tasks")).toBeInTheDocument();
        expect(screen.getByText("Failed Tasks")).toBeInTheDocument();

        // Check stat card values via their sibling labels
        const totalServicesLabel = screen.getByText("Total Services");
        expect(totalServicesLabel.parentElement?.querySelector(".text-2xl")).toHaveTextContent("2");

        const runningTasksLabel = screen.getByText("Running Tasks");
        expect(runningTasksLabel.parentElement?.querySelector(".text-2xl")).toHaveTextContent("7");

        const failedTasksLabel = screen.getByText("Failed Tasks");
        expect(failedTasksLabel.parentElement?.querySelector(".text-2xl")).toHaveTextContent("1");
    });

    it("shows node overview with managers/workers/down counts", async () => {
        mockAllInvokes();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Nodes Overview")).toBeInTheDocument();
        });

        expect(screen.getByTestId("managers-count")).toHaveTextContent("1");
        expect(screen.getByTestId("workers-count")).toHaveTextContent("1");
        expect(screen.getByTestId("down-count")).toHaveTextContent("0");
        expect(screen.getByText("Managers")).toBeInTheDocument();
        expect(screen.getByText("Workers")).toBeInTheDocument();
        expect(screen.getByText("Down")).toBeInTheDocument();
    });

    it("displays cluster health composite percentage", async () => {
        mockAllInvokes();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Cluster Health")).toBeInTheDocument();
        });

        // Service health: 2/2 = 100%, Node health: 2/2 = 100%, Task health: 7/8 = 88%
        // Composite: (100 + 100 + 88) / 3 = 96%
        expect(screen.getByTestId("health-pct")).toHaveTextContent("96%");
    });

    it("shows error state when any hook fails", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_services") throw new Error("network error");
            if (cmd === "list_nodes") return { nodes: sampleNodes };
            if (cmd === "list_task_stats") return { taskStats: sampleTaskStats };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Failed to connect to agent")).toBeInTheDocument();
        });
    });

    it("handles empty/zero data gracefully", async () => {
        const emptyTaskStats: TaskStats = {
            total: 0,
            running: 0,
            failed: 0,
            preparing: 0,
            complete: 0,
            desiredRunning: 0,
        };
        mockAllInvokes([], [], emptyTaskStats);
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Total Services")).toBeInTheDocument();
        });

        // All zeros should render
        const zeros = screen.getAllByText("0");
        expect(zeros.length).toBeGreaterThan(0);

        // Health should be 0%
        expect(screen.getByTestId("health-pct")).toHaveTextContent("0%");
    });

    it("shows destructive accent on failed tasks when > 0", async () => {
        mockAllInvokes();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Failed Tasks")).toBeInTheDocument();
        });

        // Find the Failed Tasks stat card's icon container
        const failedLabel = screen.getByText("Failed Tasks");
        const card = failedLabel.closest('[data-slot="card-content"]');
        expect(card).toBeInTheDocument();

        const iconContainer = card?.querySelector(".bg-destructive\\/10");
        expect(iconContainer).toBeInTheDocument();
    });

    it("does not show destructive accent on failed tasks when 0", async () => {
        const zeroFailed: TaskStats = { ...sampleTaskStats, failed: 0 };
        mockAllInvokes(sampleServices, sampleNodes, zeroFailed);
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Failed Tasks")).toBeInTheDocument();
        });

        const failedLabel = screen.getByText("Failed Tasks");
        const card = failedLabel.closest('[data-slot="card-content"]');
        const iconContainer = card?.querySelector(".bg-destructive\\/10");
        expect(iconContainer).toBeNull();
    });

    it("shows down nodes with destructive styling", async () => {
        const nodesWithDown: NodeItem[] = [
            ...sampleNodes,
            {
                id: "node3",
                hostname: "down-node",
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

        mockAllInvokes(sampleServices, nodesWithDown, sampleTaskStats);
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByTestId("down-count")).toHaveTextContent("1");
        });

        expect(screen.getByTestId("down-count")).toHaveClass("text-destructive");
    });
});

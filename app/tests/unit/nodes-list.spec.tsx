import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import type { NodeItem } from "@/types/nodes";
import { useNodes } from "@/hooks/use-nodes";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { AlertCircle, Server } from "lucide-react";

function RoleBadges({ node }: { node: NodeItem }) {
    if (node.role === "manager" && node.leader) {
        return <Badge className='h-4 border-0 bg-emerald-500/15 px-1.5 font-mono text-[10px] text-emerald-500 hover:bg-emerald-500/15'>LEADER</Badge>;
    }
    if (node.role === "manager") {
        return <Badge className='h-4 border-0 bg-cyan-500/15 px-1.5 font-mono text-[10px] text-cyan-500 hover:bg-cyan-500/15'>MANAGER</Badge>;
    }
    return <Badge className='h-4 border-0 bg-muted px-1.5 font-mono text-[10px] text-muted-foreground hover:bg-muted'>WORKER</Badge>;
}

function AvailabilityBadge({ availability }: { availability: string }) {
    if (availability === "drain") {
        return <span className='font-mono text-[10px] font-medium text-orange-500'>DRAIN</span>;
    }
    if (availability === "pause") {
        return <span className='font-mono text-[10px] font-medium text-amber-500'>PAUSE</span>;
    }
    return <span className='font-mono text-[10px] font-medium text-emerald-500'>ACTIVE</span>;
}

function NodeCard({ node }: { node: NodeItem }) {
    return (
        <div className='flex min-w-[300px] flex-1 flex-col gap-3 border border-border bg-card p-4' data-testid='node-card'>
            <div className='flex items-start justify-between'>
                <div className='flex flex-col gap-1.5'>
                    <span className='font-mono text-sm font-bold text-foreground'>{node.hostname}</span>
                    <div className='flex items-center gap-1.5'>
                        <RoleBadges node={node} />
                        <AvailabilityBadge availability={node.availability} />
                    </div>
                </div>
            </div>
            <div className='grid grid-cols-2 gap-x-4 gap-y-1.5'>
                <div className='flex flex-col'>
                    <span className='font-mono text-[9px] font-bold tracking-[0.5px] text-muted-foreground uppercase'>IP</span>
                    <span className='font-mono text-[11px] text-foreground'>{node.ip}</span>
                </div>
                <div className='flex flex-col'>
                    <span className='font-mono text-[9px] font-bold tracking-[0.5px] text-muted-foreground uppercase'>Docker</span>
                    <span className='font-mono text-[11px] text-foreground'>{node.engineVersion}</span>
                </div>
                <div className='flex flex-col'>
                    <span className='font-mono text-[9px] font-bold tracking-[0.5px] text-muted-foreground uppercase'>OS</span>
                    <span className='font-mono text-[11px] text-foreground'>{node.os}</span>
                </div>
                <div className='flex flex-col'>
                    <span className='font-mono text-[9px] font-bold tracking-[0.5px] text-muted-foreground uppercase'>Arch</span>
                    <span className='font-mono text-[11px] text-foreground'>{node.architecture}</span>
                </div>
            </div>
        </div>
    );
}

function LoadingSkeleton() {
    return (
        <div className='flex flex-col gap-6 p-6'>
            <div className='flex items-center justify-between'>
                <div className='flex flex-col gap-1'>
                    <Skeleton className='h-7 w-24' />
                    <Skeleton className='h-3 w-56' />
                </div>
                <Skeleton className='h-7 w-28' />
            </div>
            <div className='flex flex-wrap gap-4'>
                {[1, 2, 3].map((i) => (
                    <div key={i} className='flex min-w-[300px] flex-1 flex-col gap-3 border border-border bg-card p-4'>
                        <Skeleton className='h-4 w-32' />
                        <Skeleton className='h-4 w-16 rounded-full' />
                    </div>
                ))}
            </div>
        </div>
    );
}

function NodesTestComponent() {
    const { data: nodes, isLoading, error } = useNodes();

    if (isLoading) {
        return <LoadingSkeleton />;
    }

    if (error) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-xs text-destructive'>
                <AlertCircle className='size-4' />
                <p>Failed to load nodes</p>
            </div>
        );
    }

    if (!nodes || nodes.length === 0) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-xs text-muted-foreground'>
                <Server className='size-4' />
                <p>No nodes found</p>
            </div>
        );
    }

    return (
        <div className='flex flex-wrap gap-4'>
            {nodes.map((node) => (
                <NodeCard key={node.id} node={node} />
            ))}
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
        component: NodesTestComponent,
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

describe("NodesList", () => {
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

    it("renders node cards after loading", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_nodes") return { nodes: sampleNodes };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("manager-1")).toBeInTheDocument();
        });

        expect(screen.getByText("worker-1")).toBeInTheDocument();
        expect(screen.getByText("10.0.0.1")).toBeInTheDocument();
        expect(screen.getByText("10.0.0.2")).toBeInTheDocument();

        const cards = document.querySelectorAll('[data-testid="node-card"]');
        expect(cards.length).toBe(2);
    });

    it("renders role badges correctly", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_nodes") return { nodes: sampleNodes };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("LEADER")).toBeInTheDocument();
        });

        expect(screen.getByText("WORKER")).toBeInTheDocument();
    });

    it("shows empty state when no nodes", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_nodes") return { nodes: [] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("No nodes found")).toBeInTheDocument();
        });
    });

    it("shows error state when fetch fails", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_nodes") throw new Error("network error");
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Failed to load nodes")).toBeInTheDocument();
        });
    });

    it("does not fetch when auth is missing", () => {
        vi.mocked(invoke).mockImplementation(async () => ({ nodes: [] }));

        renderWithProviders({ token: null, agentUrl: null });

        expect(invoke).not.toHaveBeenCalledWith("list_nodes", expect.anything());
    });

    it("renders LEADER badge for manager leader node", async () => {
        const leaderNode: NodeItem = {
            id: "leader-node",
            hostname: "leader-host",
            role: "manager",
            status: "ready",
            availability: "active",
            engineVersion: "24.0.7",
            os: "linux",
            architecture: "x86_64",
            ip: "10.0.0.10",
            leader: true,
            updatedAt: new Date().toISOString(),
        };

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_nodes") return { nodes: [leaderNode] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("LEADER")).toBeInTheDocument();
        });
    });

    it("renders DRAIN availability badge", async () => {
        const drainNode: NodeItem = {
            id: "drain-node",
            hostname: "drain-host",
            role: "worker",
            status: "ready",
            availability: "drain",
            engineVersion: "24.0.7",
            os: "linux",
            architecture: "x86_64",
            ip: "10.0.0.20",
            leader: false,
            updatedAt: new Date().toISOString(),
        };

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_nodes") return { nodes: [drainNode] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("DRAIN")).toBeInTheDocument();
        });
    });

    it("renders hostname and node info on card", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_nodes") return { nodes: [sampleNodes[0]] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("manager-1")).toBeInTheDocument();
        });

        expect(screen.getByText("10.0.0.1")).toBeInTheDocument();
        expect(screen.getByText("24.0.7")).toBeInTheDocument();
        expect(screen.getByText("linux")).toBeInTheDocument();
        expect(screen.getByText("x86_64")).toBeInTheDocument();
    });
});

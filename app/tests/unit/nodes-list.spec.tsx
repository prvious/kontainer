import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import type { NodeItem } from "@/types/nodes";
import { useNodes } from "@/hooks/use-nodes";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { AlertCircle, Server } from "lucide-react";

function StatusDot({ node }: { node: NodeItem }) {
    if (node.status === "down" || node.status === "disconnected") {
        return <span className='animate-pulse-dot size-2 rounded-full bg-destructive' title={node.status} />;
    }
    if (node.availability === "drain" || node.availability === "pause") {
        return <span className='size-2 rounded-full bg-amber-500' title={node.availability} />;
    }
    return <span className='size-2 rounded-full bg-success' title='ready' />;
}

function RoleBadge({ node }: { node: NodeItem }) {
    if (node.role === "manager") {
        return <Badge variant='default'>{node.leader ? "manager (leader)" : "manager"}</Badge>;
    }
    return <Badge variant='secondary'>worker</Badge>;
}

function AvailabilityBadge({ availability }: { availability: string }) {
    if (availability === "drain") {
        return <Badge variant='destructive'>drain</Badge>;
    }
    if (availability === "pause") {
        return <Badge variant='secondary'>pause</Badge>;
    }
    return <Badge variant='default'>active</Badge>;
}

function NodesTable({ nodes }: { nodes: NodeItem[] }) {
    return (
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead className='w-8' />
                    <TableHead>Hostname</TableHead>
                    <TableHead>Role</TableHead>
                    <TableHead>Availability</TableHead>
                    <TableHead>IP</TableHead>
                    <TableHead>Engine</TableHead>
                    <TableHead>OS / Arch</TableHead>
                    <TableHead className='text-right'>Updated</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {nodes.map((node) => (
                    <TableRow key={node.id}>
                        <TableCell>
                            <StatusDot node={node} />
                        </TableCell>
                        <TableCell className='font-medium'>{node.hostname}</TableCell>
                        <TableCell>
                            <RoleBadge node={node} />
                        </TableCell>
                        <TableCell>
                            <AvailabilityBadge availability={node.availability} />
                        </TableCell>
                        <TableCell>
                            <span className='font-mono text-xs text-muted-foreground'>{node.ip}</span>
                        </TableCell>
                        <TableCell>
                            <span className='font-mono text-xs text-muted-foreground'>{node.engineVersion}</span>
                        </TableCell>
                        <TableCell>
                            <span className='text-xs text-muted-foreground'>
                                {node.os}/{node.architecture}
                            </span>
                        </TableCell>
                        <TableCell className='text-right'>
                            <span className='text-xs text-muted-foreground'>just now</span>
                        </TableCell>
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}

function LoadingSkeleton() {
    return (
        <div className='p-6'>
            <Card>
                <CardContent className='p-0'>
                    <div className='space-y-0'>
                        {[1, 2, 3, 4, 5].map((i) => (
                            <div key={i} className='flex items-center gap-4 border-b px-4 py-3 last:border-0'>
                                <Skeleton className='size-2 rounded-full' />
                                <Skeleton className='h-4 w-32' />
                                <Skeleton className='h-5 w-20' />
                                <Skeleton className='h-5 w-16' />
                                <Skeleton className='h-4 w-24' />
                                <Skeleton className='ml-auto h-4 w-16' />
                            </div>
                        ))}
                    </div>
                </CardContent>
            </Card>
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
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
                <AlertCircle className='size-5' />
                <p>Failed to load nodes</p>
            </div>
        );
    }

    if (!nodes || nodes.length === 0) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-muted-foreground'>
                <Server className='size-5' />
                <p>No nodes found</p>
            </div>
        );
    }

    return (
        <Card>
            <CardContent className='p-0'>
                <NodesTable nodes={nodes} />
            </CardContent>
        </Card>
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
    agentUrl: "http://localhost:8080",
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

    it("renders nodes after loading", async () => {
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
        expect(screen.getByText("manager (leader)")).toBeInTheDocument();
        expect(screen.getByText("worker")).toBeInTheDocument();
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

    it("renders leader badge for manager leader node", async () => {
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
            expect(screen.getByText("manager (leader)")).toBeInTheDocument();
        });
    });

    it("renders drain availability badge", async () => {
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
            expect(screen.getByText("drain")).toBeInTheDocument();
        });
    });
});

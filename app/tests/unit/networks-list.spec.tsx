import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import type { NetworkItem } from "@/types/networks";
import { useNetworks } from "@/hooks/use-networks";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { AlertCircle, Network } from "lucide-react";

function NetworksTable({ networks }: { networks: NetworkItem[] }) {
    return (
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead>Name</TableHead>
                    <TableHead>Driver</TableHead>
                    <TableHead>Scope</TableHead>
                    <TableHead>Flags</TableHead>
                    <TableHead>Services</TableHead>
                    <TableHead className='text-right'>Created</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {networks.map((net) => (
                    <TableRow key={net.id}>
                        <TableCell className='font-medium'>
                            <div className='flex items-center gap-2'>
                                {net.name}
                                {net.ingress && (
                                    <Badge variant='secondary' className='text-[10px]'>
                                        ingress
                                    </Badge>
                                )}
                            </div>
                        </TableCell>
                        <TableCell>
                            <span className='font-mono text-xs text-muted-foreground'>{net.driver}</span>
                        </TableCell>
                        <TableCell>
                            <span className='text-xs text-muted-foreground'>{net.scope}</span>
                        </TableCell>
                        <TableCell>
                            <div className='flex items-center gap-1'>
                                {net.internal && <Badge variant='secondary'>internal</Badge>}
                                {net.attachable && <Badge variant='secondary'>attachable</Badge>}
                            </div>
                        </TableCell>
                        <TableCell>
                            {net.services.length === 0 ? (
                                <span className='text-xs text-muted-foreground'>-</span>
                            ) : (
                                <div className='flex flex-wrap gap-1'>
                                    {net.services.map((svc) => (
                                        <Badge key={svc.id} variant='default'>
                                            {svc.name}
                                        </Badge>
                                    ))}
                                </div>
                            )}
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
                                <Skeleton className='h-4 w-32' />
                                <Skeleton className='h-4 w-16' />
                                <Skeleton className='h-4 w-16' />
                                <Skeleton className='h-5 w-20' />
                                <Skeleton className='ml-auto h-4 w-16' />
                            </div>
                        ))}
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}

function NetworksTestComponent() {
    const { data: networks, isLoading, error } = useNetworks();

    if (isLoading) {
        return <LoadingSkeleton />;
    }

    if (error) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
                <AlertCircle className='size-5' />
                <p>Failed to load networks</p>
            </div>
        );
    }

    if (!networks || networks.length === 0) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-muted-foreground'>
                <Network className='size-5' />
                <p>No networks found</p>
            </div>
        );
    }

    return (
        <Card>
            <CardContent className='p-0'>
                <NetworksTable networks={networks} />
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
        component: NetworksTestComponent,
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

const sampleNetworks: NetworkItem[] = [
    {
        id: "net1",
        name: "my-overlay",
        driver: "overlay",
        scope: "swarm",
        internal: false,
        attachable: true,
        ingress: false,
        services: [
            { id: "svc1", name: "web" },
            { id: "svc2", name: "api" },
        ],
        createdAt: new Date().toISOString(),
    },
    {
        id: "net-ingress",
        name: "ingress",
        driver: "overlay",
        scope: "swarm",
        internal: false,
        attachable: false,
        ingress: true,
        services: [],
        createdAt: new Date().toISOString(),
    },
];

describe("NetworksList", () => {
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

    it("renders networks after loading", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_networks") return { networks: sampleNetworks };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("my-overlay")).toBeInTheDocument();
        });

        // "ingress" appears as both network name and badge, so use getAllByText
        expect(screen.getAllByText("ingress").length).toBeGreaterThanOrEqual(1);
        expect(screen.getByText("web")).toBeInTheDocument();
        expect(screen.getByText("api")).toBeInTheDocument();
    });

    it("shows attached service badges for networks", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_networks") return { networks: sampleNetworks };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("web")).toBeInTheDocument();
        });

        expect(screen.getByText("api")).toBeInTheDocument();
    });

    it("shows empty state when no networks", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_networks") return { networks: [] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("No networks found")).toBeInTheDocument();
        });
    });

    it("shows error state when fetch fails", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_networks") throw new Error("network error");
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Failed to load networks")).toBeInTheDocument();
        });
    });

    it("does not fetch when auth is missing", () => {
        vi.mocked(invoke).mockImplementation(async () => ({ networks: [] }));

        renderWithProviders({ token: null, agentUrl: null });

        expect(invoke).not.toHaveBeenCalledWith("list_networks", expect.anything());
    });

    it("shows attachable badge for attachable networks", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_networks") return { networks: sampleNetworks };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("attachable")).toBeInTheDocument();
        });
    });

    it("renders ingress badge on ingress network", async () => {
        const ingressNet: NetworkItem = {
            id: "net-ingress",
            name: "ingress",
            driver: "overlay",
            scope: "swarm",
            internal: false,
            attachable: false,
            ingress: true,
            services: [],
            createdAt: new Date().toISOString(),
        };

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_networks") return { networks: [ingressNet] };
            return undefined;
        });

        renderWithProviders();

        // "ingress" appears as network name + as badge text
        await waitFor(() => {
            const matches = screen.getAllByText("ingress");
            expect(matches.length).toBeGreaterThanOrEqual(2);
        });
    });
});

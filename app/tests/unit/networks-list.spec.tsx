import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import type { NetworkItem } from "@/types/networks";
import { useNetworks } from "@/hooks/use-networks";
import { Skeleton } from "@/components/ui/skeleton";
import { PageHeader } from "@/components/page-header";
import { TableShell } from "@/components/table-shell";
import { ColumnHeaders } from "@/components/column-headers";
import { AlertCircle, Network } from "lucide-react";
import { useState } from "react";

const NETWORK_COLUMNS = [
    { label: "STATUS", width: 70 },
    { label: "NAME" },
    { label: "ID", width: 150 },
    { label: "DRIVER", width: 100 },
    { label: "SCOPE", width: 80 },
    { label: "SUBNET", width: 120 },
    { label: "", width: 60 },
];

function NetworksTestComponent() {
    const { data: networks, isLoading, error } = useNetworks();
    const [search, setSearch] = useState("");

    const filtered = (networks ?? []).filter(
        (net) =>
            net.name.toLowerCase().includes(search.toLowerCase()) ||
            net.driver.toLowerCase().includes(search.toLowerCase()) ||
            net.scope.toLowerCase().includes(search.toLowerCase()),
    );

    if (isLoading) {
        return (
            <div className='flex flex-col gap-6 p-6'>
                <Skeleton className='h-7 w-28' />
                <div className='space-y-0'>
                    {[1, 2, 3].map((i) => (
                        <div key={i} className='flex items-center gap-3 border-b px-4 py-2.5'>
                            <Skeleton className='size-1.5 rounded-full' />
                            <Skeleton className='h-3 w-28' />
                        </div>
                    ))}
                </div>
            </div>
        );
    }

    if (error) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-xs text-destructive'>
                <AlertCircle className='size-4' />
                <p>Failed to load networks</p>
            </div>
        );
    }

    const networkCount = networks?.length ?? 0;
    const overlayCount = (networks ?? []).filter((n) => n.driver === "overlay").length;
    const bridgeCount = (networks ?? []).filter((n) => n.driver === "bridge").length;
    const subtitle = `${networkCount} ${networkCount === 1 ? "network" : "networks"} · ${overlayCount} overlay · ${bridgeCount} bridge`;

    return (
        <div className='flex h-full flex-col gap-6 p-6'>
            <PageHeader title='NETWORKS' subtitle={subtitle} />

            <TableShell
                title='ALL NETWORKS'
                count={filtered.length}
                searchValue={search}
                onSearchChange={setSearch}
                searchPlaceholder='Search networks...'
            >
                <ColumnHeaders columns={NETWORK_COLUMNS} />
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                        <Network className='size-4' />
                        <p>{search ? "No networks match your filter" : "No networks found"}</p>
                    </div>
                ) : (
                    <div>
                        {filtered.map((net) => (
                            <div key={net.id} className='flex items-center border-b border-border px-4 py-2.5'>
                                <span className='size-1.5 rounded-full bg-success' />
                                <span className='ml-4 font-mono text-xs font-medium'>{net.name}</span>
                                <span className='ml-4 font-mono text-[11px] text-muted-foreground'>{net.id.slice(0, 12)}</span>
                                <span className='ml-4 font-mono text-[11px] text-muted-foreground'>{net.driver}</span>
                                <span className='ml-4 font-mono text-[11px]'>{net.scope}</span>
                            </div>
                        ))}
                    </div>
                )}
            </TableShell>
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
        id: "net1aabbccddee",
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
        id: "net2aabbccddee",
        name: "bridge0",
        driver: "bridge",
        scope: "local",
        internal: false,
        attachable: false,
        ingress: false,
        services: [],
        createdAt: new Date().toISOString(),
    },
    {
        id: "net-ingress-id1",
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

    it("renders page header and table shell after loading", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_networks") return { networks: sampleNetworks };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("NETWORKS")).toBeInTheDocument();
        });

        expect(screen.getByText("ALL NETWORKS")).toBeInTheDocument();
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

        expect(screen.getByText("bridge0")).toBeInTheDocument();
        expect(screen.getByText("ingress")).toBeInTheDocument();
    });

    it("shows subtitle with driver counts", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_networks") return { networks: sampleNetworks };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText(/3 networks · 2 overlay · 1 bridge/)).toBeInTheDocument();
        });
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

    it("renders column headers", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_networks") return { networks: sampleNetworks };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("STATUS")).toBeInTheDocument();
        });

        expect(screen.getByText("NAME")).toBeInTheDocument();
        expect(screen.getByText("DRIVER")).toBeInTheDocument();
        expect(screen.getByText("SCOPE")).toBeInTheDocument();
        expect(screen.getByText("SUBNET")).toBeInTheDocument();
    });

    it("renders swarm scope for overlay network", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_networks") return { networks: sampleNetworks };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("my-overlay")).toBeInTheDocument();
        });

        expect(screen.getAllByText("swarm").length).toBeGreaterThanOrEqual(1);
        expect(screen.getByText("local")).toBeInTheDocument();
    });
});

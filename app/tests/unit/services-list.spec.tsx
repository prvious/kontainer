import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import { useServices } from "@/hooks/use-services";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { PageHeader } from "@/components/page-header";
import { TableShell } from "@/components/table-shell";
import { ColumnHeaders } from "@/components/column-headers";
import { AlertCircle, Container } from "lucide-react";
import { useState } from "react";
import type { ServiceItem } from "@/types/services";

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

function ReplicasBadge({ service }: { service: ServiceItem }) {
    if (service.replicasDesired === -1) {
        return (
            <Badge variant='secondary' className='h-4 px-1.5 font-mono text-[10px]'>
                global
            </Badge>
        );
    }
    const healthy = service.replicasRunning >= service.replicasDesired && service.replicasDesired > 0;
    return (
        <Badge variant={healthy ? "default" : "destructive"} className='h-4 px-1.5 font-mono text-[10px]'>
            {service.replicasRunning}/{service.replicasDesired}
        </Badge>
    );
}

const SERVICE_COLUMNS = [
    { label: "STATUS", width: 70 },
    { label: "NAME" },
    { label: "IMAGE", width: 200 },
    { label: "STACK", width: 100 },
    { label: "REPLICAS", width: 80 },
    { label: "UPDATED", width: 100, align: "right" as const },
    { label: "", width: 60 },
];

function ServicesTestComponent() {
    const { data: services, isLoading, error } = useServices();
    const [search, setSearch] = useState("");

    const filtered = (services ?? []).filter(
        (svc) =>
            svc.name.toLowerCase().includes(search.toLowerCase()) ||
            svc.image.toLowerCase().includes(search.toLowerCase()) ||
            (svc.stack && svc.stack.toLowerCase().includes(search.toLowerCase())),
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
                <p>Failed to load services</p>
            </div>
        );
    }

    const serviceCount = services?.length ?? 0;
    const stackCount = new Set((services ?? []).map((s) => s.stack).filter(Boolean)).size;
    const totalReplicas = (services ?? []).reduce((sum, s) => sum + s.replicasRunning, 0);
    const subtitle = `${serviceCount} ${serviceCount === 1 ? "service" : "services"} · ${stackCount} ${stackCount === 1 ? "stack" : "stacks"} · ${totalReplicas} replicas running`;

    return (
        <div className='flex h-full flex-col gap-6 p-6'>
            <PageHeader title='SERVICES' subtitle={subtitle} />

            <TableShell title='SWARM SERVICES' count={filtered.length} searchValue={search} onSearchChange={setSearch} searchPlaceholder='Search services...'>
                <ColumnHeaders columns={SERVICE_COLUMNS} />
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                        <Container className='size-4' />
                        <p>{search ? "No services match your filter" : "No services found"}</p>
                    </div>
                ) : (
                    <div>
                        {filtered.map((svc) => (
                            <div key={svc.id} className='flex items-center border-b border-border px-4 py-2.5'>
                                <span className='font-mono text-xs'>{svc.name}</span>
                                <span className='ml-4 font-mono text-[11px] text-muted-foreground'>{svc.image}</span>
                                <span className='ml-4 font-mono text-[11px] text-muted-foreground'>{svc.stack}</span>
                                <div className='ml-4'>
                                    <ReplicasBadge service={svc} />
                                </div>
                            </div>
                        ))}
                    </div>
                )}
            </TableShell>
        </div>
    );
}

function createTestRouter() {
    const rootRoute = createRootRoute({});
    const indexRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/",
        component: ServicesTestComponent,
    });
    const serviceDetailRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/services/$serviceId",
        component: () => <div>detail</div>,
    });
    const routeTree = rootRoute.addChildren([indexRoute, serviceDetailRoute]);
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
        name: "web",
        image: "nginx:latest",
        stack: "myapp",
        replicasRunning: 3,
        replicasDesired: 3,
        updatedAt: new Date().toISOString(),
    },
    {
        id: "svc2",
        name: "api",
        image: "node:20",
        stack: "myapp",
        replicasRunning: 1,
        replicasDesired: 2,
        updatedAt: new Date().toISOString(),
    },
    {
        id: "svc3",
        name: "standalone-db",
        image: "postgres:16",
        stack: "",
        replicasRunning: 1,
        replicasDesired: 1,
        updatedAt: new Date().toISOString(),
    },
];

describe("ServicesList", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("shows loading skeleton initially", async () => {
        vi.mocked(invoke).mockImplementation(() => new Promise(() => {})); // never resolves
        renderWithProviders();

        // Wait for router to render, then check skeletons
        await waitFor(() => {
            const skeletons = document.querySelectorAll('[data-slot="skeleton"]');
            expect(skeletons.length).toBeGreaterThan(0);
        });
    });

    it("renders services after loading", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_services") return { services: sampleServices };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("SERVICES")).toBeInTheDocument();
        });

        expect(screen.getByText("SWARM SERVICES")).toBeInTheDocument();
        expect(screen.getByText("web")).toBeInTheDocument();
        expect(screen.getByText("api")).toBeInTheDocument();
        expect(screen.getByText("standalone-db")).toBeInTheDocument();
        expect(screen.getByText("nginx:latest")).toBeInTheDocument();
        expect(screen.getByText("3/3")).toBeInTheDocument();
        expect(screen.getByText("1/2")).toBeInTheDocument();
    });

    it("shows empty state when no services", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_services") return { services: [] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("No services found")).toBeInTheDocument();
        });
    });

    it("shows error state when fetch fails", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_services") throw new Error("network error");
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Failed to load services")).toBeInTheDocument();
        });
    });

    it("does not fetch when auth is missing", () => {
        vi.mocked(invoke).mockImplementation(async () => ({ services: [] }));

        renderWithProviders({ token: null, agentUrl: null });

        // invoke should not be called for list_services since query is disabled
        expect(invoke).not.toHaveBeenCalledWith("list_services", expect.anything());
    });

    it("renders global mode services with global badge", async () => {
        const globalService: ServiceItem = {
            id: "svc-global",
            name: "monitoring",
            image: "prometheus:v2",
            stack: "",
            replicasRunning: 2,
            replicasDesired: -1,
            updatedAt: new Date().toISOString(),
        };

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_services") return { services: [globalService] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("monitoring")).toBeInTheDocument();
        });

        expect(screen.getByText("global")).toBeInTheDocument();
    });
});

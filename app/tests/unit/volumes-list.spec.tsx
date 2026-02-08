import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import type { VolumeItem } from "@/types/volumes";
import { useVolumes } from "@/hooks/use-volumes";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { AlertCircle, HardDrive } from "lucide-react";

function formatBytes(bytes: number): string {
    if (bytes < 0) return "-";
    if (bytes === 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    const value = bytes / Math.pow(1024, i);
    return `${value.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

function VolumesTable({ volumes }: { volumes: VolumeItem[] }) {
    return (
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead>Name</TableHead>
                    <TableHead>Driver</TableHead>
                    <TableHead>Scope</TableHead>
                    <TableHead>Stack</TableHead>
                    <TableHead>Size</TableHead>
                    <TableHead>Refs</TableHead>
                    <TableHead className='text-right'>Created</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {volumes.map((vol) => (
                    <TableRow key={vol.name}>
                        <TableCell className='font-medium'>
                            <div className='flex flex-col gap-0.5'>
                                <span>{vol.name}</span>
                                <span className='truncate font-mono text-[10px] text-muted-foreground'>{vol.mountpoint}</span>
                            </div>
                        </TableCell>
                        <TableCell>
                            <span className='font-mono text-xs text-muted-foreground'>{vol.driver}</span>
                        </TableCell>
                        <TableCell>
                            <span className='text-xs text-muted-foreground'>{vol.scope}</span>
                        </TableCell>
                        <TableCell>
                            {vol.stack ? <Badge variant='secondary'>{vol.stack}</Badge> : <span className='text-xs text-muted-foreground'>-</span>}
                        </TableCell>
                        <TableCell>
                            <span className='font-mono text-xs text-muted-foreground'>{formatBytes(vol.sizeBytes)}</span>
                        </TableCell>
                        <TableCell>
                            <span className='font-mono text-xs text-muted-foreground'>{vol.refCount >= 0 ? vol.refCount : "-"}</span>
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
                                <Skeleton className='h-4 w-40' />
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

function VolumesTestComponent() {
    const { data: volumes, isLoading, error } = useVolumes();

    if (isLoading) {
        return <LoadingSkeleton />;
    }

    if (error) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
                <AlertCircle className='size-5' />
                <p>Failed to load volumes</p>
            </div>
        );
    }

    if (!volumes || volumes.length === 0) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-muted-foreground'>
                <HardDrive className='size-5' />
                <p>No volumes found</p>
            </div>
        );
    }

    return (
        <Card>
            <CardContent className='p-0'>
                <VolumesTable volumes={volumes} />
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
        component: VolumesTestComponent,
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

const sampleVolumes: VolumeItem[] = [
    {
        name: "db-data",
        driver: "local",
        scope: "local",
        mountpoint: "/var/lib/docker/volumes/db-data/_data",
        labels: { "com.docker.stack.namespace": "myapp" },
        options: {},
        createdAt: new Date().toISOString(),
        refCount: 2,
        sizeBytes: 1048576,
        stack: "myapp",
    },
    {
        name: "cache-vol",
        driver: "local",
        scope: "local",
        mountpoint: "/var/lib/docker/volumes/cache-vol/_data",
        labels: {},
        options: {},
        createdAt: new Date().toISOString(),
        refCount: -1,
        sizeBytes: -1,
        stack: "",
    },
];

describe("VolumesList", () => {
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

    it("renders volumes after loading", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_volumes") return { volumes: sampleVolumes };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("db-data")).toBeInTheDocument();
        });

        expect(screen.getByText("cache-vol")).toBeInTheDocument();
        expect(screen.getByText("myapp")).toBeInTheDocument();
    });

    it("shows size and ref count for volumes with usage data", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_volumes") return { volumes: sampleVolumes };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("db-data")).toBeInTheDocument();
        });

        expect(screen.getByText("1.0 MB")).toBeInTheDocument();
        expect(screen.getByText("2")).toBeInTheDocument();
    });

    it("shows dash for volumes without usage data", async () => {
        const noUsage: VolumeItem[] = [
            {
                name: "no-usage-vol",
                driver: "local",
                scope: "local",
                mountpoint: "/var/lib/docker/volumes/no-usage-vol/_data",
                labels: {},
                options: {},
                createdAt: new Date().toISOString(),
                refCount: -1,
                sizeBytes: -1,
                stack: "",
            },
        ];

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_volumes") return { volumes: noUsage };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("no-usage-vol")).toBeInTheDocument();
        });

        // Both size and refs should show "-"
        const dashes = screen.getAllByText("-");
        expect(dashes.length).toBeGreaterThanOrEqual(2);
    });

    it("shows empty state when no volumes", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_volumes") return { volumes: [] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("No volumes found")).toBeInTheDocument();
        });
    });

    it("shows error state when fetch fails", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_volumes") throw new Error("network error");
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Failed to load volumes")).toBeInTheDocument();
        });
    });

    it("does not fetch when auth is missing", () => {
        vi.mocked(invoke).mockImplementation(async () => ({ volumes: [] }));

        renderWithProviders({ token: null, agentUrl: null });

        expect(invoke).not.toHaveBeenCalledWith("list_volumes", expect.anything());
    });

    it("shows stack badge for volumes with stack label", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_volumes") return { volumes: sampleVolumes };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("myapp")).toBeInTheDocument();
        });
    });
});

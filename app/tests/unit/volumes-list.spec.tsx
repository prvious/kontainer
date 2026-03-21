import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import type { VolumeItem } from "@/types/volumes";
import { useVolumes } from "@/hooks/use-volumes";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { PageHeader } from "@/components/page-header";
import { TableShell } from "@/components/table-shell";
import { ColumnHeaders } from "@/components/column-headers";
import { AlertCircle, HardDrive } from "lucide-react";
import { useState } from "react";

function formatBytes(bytes: number): string {
    if (bytes < 0) return "-";
    if (bytes === 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    const value = bytes / Math.pow(1024, i);
    return `${value.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

const VOLUME_COLUMNS = [
    { label: "NAME" },
    { label: "DRIVER", width: 100 },
    { label: "MOUNTPOINT" },
    { label: "SIZE", width: 80, align: "right" as const },
    { label: "CREATED", width: 100, align: "right" as const },
    { label: "", width: 60 },
];

function VolumesTestComponent() {
    const { data: volumes, isLoading, error } = useVolumes();
    const [search, setSearch] = useState("");

    const filtered = (volumes ?? []).filter(
        (vol) =>
            vol.name.toLowerCase().includes(search.toLowerCase()) ||
            vol.driver.toLowerCase().includes(search.toLowerCase()) ||
            vol.stack.toLowerCase().includes(search.toLowerCase()),
    );

    if (isLoading) {
        return (
            <div className='flex flex-col gap-6 p-6'>
                <Skeleton className='h-7 w-28' />
                <div className='space-y-0'>
                    {[1, 2, 3].map((i) => (
                        <div key={i} className='flex items-center gap-3 border-b px-4 py-2.5'>
                            <Skeleton className='h-3 w-32' />
                            <Skeleton className='h-3 w-16' />
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
                <p>Failed to load volumes</p>
            </div>
        );
    }

    const volumeCount = volumes?.length ?? 0;
    const driverCount = new Set((volumes ?? []).map((v) => v.driver).filter(Boolean)).size;
    const subtitle = `${volumeCount} ${volumeCount === 1 ? "volume" : "volumes"} · ${driverCount} ${driverCount === 1 ? "driver" : "drivers"}`;

    return (
        <div className='flex h-full flex-col gap-6 p-6'>
            <PageHeader title='VOLUMES' subtitle={subtitle}>
                <Button variant='outline' size='sm' disabled className='h-7 font-mono text-[11px] text-muted-foreground'>
                    CREATE VOLUME
                </Button>
            </PageHeader>

            <TableShell title='ALL VOLUMES' count={filtered.length} searchValue={search} onSearchChange={setSearch} searchPlaceholder='Search volumes...'>
                <ColumnHeaders columns={VOLUME_COLUMNS} />
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                        <HardDrive className='size-4' />
                        <p>{search ? "No volumes match your filter" : "No volumes found"}</p>
                    </div>
                ) : (
                    <div>
                        {filtered.map((vol) => (
                            <div key={vol.name} className='flex items-center border-b border-border px-4 py-2.5'>
                                <div style={{ flex: 1 }}>
                                    <span className='font-mono text-xs font-medium'>{vol.name}</span>
                                </div>
                                <div style={{ width: 100 }}>
                                    <span className='font-mono text-[11px] text-muted-foreground'>{vol.driver}</span>
                                </div>
                                <div style={{ flex: 1 }} className='min-w-0 pr-4'>
                                    <span className='block truncate font-mono text-[11px] text-muted-foreground'>{vol.mountpoint}</span>
                                </div>
                                <div style={{ width: 80 }} className='text-right'>
                                    <span className='font-mono text-[11px] text-muted-foreground'>{formatBytes(vol.sizeBytes)}</span>
                                </div>
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
            expect(screen.getByText("VOLUMES")).toBeInTheDocument();
        });

        expect(screen.getByText("ALL VOLUMES")).toBeInTheDocument();
        expect(screen.getByText("db-data")).toBeInTheDocument();
        expect(screen.getByText("cache-vol")).toBeInTheDocument();
    });

    it("shows size for volumes with usage data", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_volumes") return { volumes: sampleVolumes };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("db-data")).toBeInTheDocument();
        });

        expect(screen.getByText("1.0 MB")).toBeInTheDocument();
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

        const dashes = screen.getAllByText("-");
        expect(dashes.length).toBeGreaterThanOrEqual(1);
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

    it("shows column headers for volume table", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_volumes") return { volumes: sampleVolumes };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("NAME")).toBeInTheDocument();
        });

        expect(screen.getByText("DRIVER")).toBeInTheDocument();
        expect(screen.getByText("MOUNTPOINT")).toBeInTheDocument();
        expect(screen.getByText("SIZE")).toBeInTheDocument();
        expect(screen.getByText("CREATED")).toBeInTheDocument();
    });

    it("shows subtitle with volume and driver counts", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_volumes") return { volumes: sampleVolumes };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText(/2 volumes/)).toBeInTheDocument();
        });

        expect(screen.getByText(/1 driver/)).toBeInTheDocument();
    });
});

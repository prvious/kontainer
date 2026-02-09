import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import { useState, useMemo } from "react";
import { useImages } from "@/hooks/use-images";
import { useRemoveImage } from "@/hooks/use-remove-image";
import { usePruneImages } from "@/hooks/use-prune-images";
import { useClusterMetrics } from "@/hooks/use-cluster-metrics";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Checkbox } from "@/components/ui/checkbox";
import { Progress } from "@/components/ui/progress";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import {
    AlertDialog,
    AlertDialogAction,
    AlertDialogCancel,
    AlertDialogContent,
    AlertDialogDescription,
    AlertDialogFooter,
    AlertDialogHeader,
    AlertDialogTitle,
    AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { ButtonGroup, ButtonGroupSeparator } from "@/components/ui/button-group";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { AlertCircle, ChevronDown, HardDrive, Search, Trash2, Eraser } from "lucide-react";
import type { ImageItem, ClusterImagesResponse } from "@/types/images";
import type { ClusterMetricsResponse } from "@/types/metrics";

function formatBytes(bytes: number): string {
    if (bytes <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    const value = bytes / Math.pow(1024, i);
    return `${value.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

function formatRelativeTime(unix: number): string {
    const seconds = Math.floor(Date.now() / 1000 - unix);
    if (seconds < 60) return "just now";
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
}

function LoadingSkeleton() {
    return (
        <Card>
            <CardContent className='p-0'>
                <div className='space-y-0'>
                    {[1, 2, 3, 4, 5].map((i) => (
                        <div key={i} className='flex items-center gap-4 border-b px-4 py-3 last:border-0'>
                            <Skeleton className='h-4 w-4' />
                            <Skeleton className='h-4 w-40' />
                        </div>
                    ))}
                </div>
            </CardContent>
        </Card>
    );
}

function ImagesTestComponent() {
    const [search, setSearch] = useState("");
    const [selected, setSelected] = useState<Set<string>>(new Set());
    const [forceDeleteTarget, setForceDeleteTarget] = useState<ImageItem | null>(null);
    const { data: imagesData, isLoading, error } = useImages();
    const { data: metricsData } = useClusterMetrics();
    const removeImage = useRemoveImage();
    const pruneImages = usePruneImages();

    const allImages = useMemo(() => {
        if (!imagesData?.nodes) return [];
        return imagesData.nodes.flatMap((node) => node.images);
    }, [imagesData]);

    const filteredImages = useMemo(() => {
        if (!search) return allImages;
        const q = search.toLowerCase();
        return allImages.filter(
            (img) => img.tags.some((t) => t.toLowerCase().includes(q)) || img.id.toLowerCase().includes(q) || img.hostname.toLowerCase().includes(q),
        );
    }, [allImages, search]);

    const agents = metricsData?.agents ?? [];

    function toggleSelect(key: string) {
        setSelected((prev) => {
            const next = new Set(prev);
            if (next.has(key)) next.delete(key);
            else next.add(key);
            return next;
        });
    }

    function toggleSelectAll() {
        if (selected.size === filteredImages.length) {
            setSelected(new Set());
        } else {
            setSelected(new Set(filteredImages.map((img) => `${img.nodeId}:${img.id}`)));
        }
    }

    function handleDelete(img: ImageItem, force: boolean) {
        removeImage.mutate({ imageId: img.id, nodeId: img.nodeId, force });
    }

    function handleBulkDelete(force: boolean) {
        for (const key of selected) {
            const [nodeId, imageId] = key.split(":");
            removeImage.mutate({ imageId, nodeId, force });
        }
        setSelected(new Set());
    }

    if (isLoading) return <LoadingSkeleton />;

    if (error) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
                <AlertCircle className='size-5' />
                <p>Failed to load images</p>
            </div>
        );
    }

    return (
        <div className='p-6'>
            <div className='mb-6 flex items-center justify-between'>
                <div>
                    <h1 className='text-xl font-semibold tracking-tight'>Images</h1>
                    <p className='mt-1 text-sm text-muted-foreground'>
                        {allImages.length} images across {imagesData?.nodes?.length ?? 0} nodes
                    </p>
                </div>
                <div className='flex items-center gap-2'>
                    <div className='relative'>
                        <Search className='absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground' />
                        <Input placeholder='Search images...' value={search} onChange={(e) => setSearch(e.target.value)} className='h-8 w-56 pl-8 text-sm' />
                    </div>
                    <AlertDialog>
                        <AlertDialogTrigger render={<Button variant='outline' size='sm' data-testid='prune-btn' />}>
                            <Eraser className='size-3.5' />
                            Prune
                        </AlertDialogTrigger>
                        <AlertDialogContent>
                            <AlertDialogHeader>
                                <AlertDialogTitle>Prune unused images?</AlertDialogTitle>
                                <AlertDialogDescription>This will remove all unused images to reclaim disk space.</AlertDialogDescription>
                            </AlertDialogHeader>
                            <AlertDialogFooter>
                                <AlertDialogCancel>Cancel</AlertDialogCancel>
                                <AlertDialogAction onClick={() => pruneImages.mutate()} render={<Button variant='destructive' />}>
                                    Prune All
                                </AlertDialogAction>
                            </AlertDialogFooter>
                        </AlertDialogContent>
                    </AlertDialog>
                </div>
            </div>

            {/* Storage cards */}
            {agents.length > 0 && (
                <div className='mb-6 grid gap-3' data-testid='storage-cards'>
                    {agents.map((agent) => (
                        <Card key={agent.nodeId}>
                            <CardContent className='p-3'>
                                <span className='text-xs font-medium'>{agent.hostname}</span>
                                <Progress value={agent.node.diskPercent} className='my-1 h-1.5' />
                                <p className='text-[10px] text-muted-foreground'>
                                    {formatBytes(agent.node.diskUsed)} / {formatBytes(agent.node.diskTotal)}
                                </p>
                            </CardContent>
                        </Card>
                    ))}
                </div>
            )}

            {/* Bulk actions */}
            {selected.size > 0 && (
                <div className='mb-4 flex items-center gap-3 rounded-lg border bg-muted/50 px-4 py-2' data-testid='bulk-bar'>
                    <span className='text-sm font-medium'>{selected.size} selected</span>
                    <div className='ml-auto flex items-center gap-2'>
                        <AlertDialog>
                            <AlertDialogTrigger render={<Button variant='outline' size='sm' data-testid='bulk-delete-btn' />}>
                                <Trash2 className='size-3.5' />
                                Delete Selected
                            </AlertDialogTrigger>
                            <AlertDialogContent>
                                <AlertDialogHeader>
                                    <AlertDialogTitle>Delete {selected.size} images?</AlertDialogTitle>
                                    <AlertDialogDescription>This will remove the selected images.</AlertDialogDescription>
                                </AlertDialogHeader>
                                <AlertDialogFooter>
                                    <AlertDialogCancel>Cancel</AlertDialogCancel>
                                    <AlertDialogAction onClick={() => handleBulkDelete(false)} render={<Button variant='destructive' />}>
                                        Delete
                                    </AlertDialogAction>
                                </AlertDialogFooter>
                            </AlertDialogContent>
                        </AlertDialog>
                    </div>
                </div>
            )}

            {filteredImages.length === 0 ? (
                <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-muted-foreground'>
                    <HardDrive className='size-5' />
                    <p>{search ? "No images match your search" : "No images found"}</p>
                </div>
            ) : (
                <Card>
                    <CardContent className='p-0'>
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead className='w-10'>
                                        <Checkbox
                                            checked={selected.size === filteredImages.length && filteredImages.length > 0}
                                            onCheckedChange={toggleSelectAll}
                                            data-testid='select-all'
                                        />
                                    </TableHead>
                                    <TableHead>Image</TableHead>
                                    <TableHead>Size</TableHead>
                                    <TableHead>Created</TableHead>
                                    <TableHead>Node</TableHead>
                                    <TableHead className='w-24' />
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {filteredImages.map((img) => {
                                    const key = `${img.nodeId}:${img.id}`;
                                    const fullTag = img.tags.length > 0 ? img.tags[0] : null;
                                    const colonIdx = fullTag ? fullTag.lastIndexOf(":") : -1;
                                    const imageName = fullTag ? (colonIdx > 0 ? fullTag.slice(0, colonIdx) : fullTag) : "<none>";
                                    const imageTag = fullTag && colonIdx > 0 ? fullTag.slice(colonIdx + 1) : null;
                                    return (
                                        <TableRow key={key}>
                                            <TableCell>
                                                <Checkbox
                                                    checked={selected.has(key)}
                                                    onCheckedChange={() => toggleSelect(key)}
                                                    data-testid={`select-${img.id}`}
                                                />
                                            </TableCell>
                                            <TableCell>
                                                <div className='flex flex-col gap-0.5'>
                                                    <span className='text-sm font-medium'>{imageName}</span>
                                                    <span className='font-mono text-[11px] text-muted-foreground'>{imageTag ?? img.id}</span>
                                                </div>
                                            </TableCell>
                                            <TableCell>
                                                <span className='font-mono text-xs'>{formatBytes(img.size)}</span>
                                            </TableCell>
                                            <TableCell>
                                                <span className='text-xs'>{formatRelativeTime(img.created)}</span>
                                            </TableCell>
                                            <TableCell>
                                                <div className='flex flex-col gap-0.5'>
                                                    <span className='text-sm font-medium'>{img.hostname}</span>
                                                    <span className='font-mono text-[11px] text-muted-foreground'>{img.nodeId}</span>
                                                </div>
                                            </TableCell>
                                            <TableCell>
                                                <ButtonGroup>
                                                    <AlertDialog>
                                                        <AlertDialogTrigger
                                                            render={<Button variant='destructive' size='xs' data-testid={`delete-${img.id}`} />}
                                                        >
                                                            Delete
                                                        </AlertDialogTrigger>
                                                        <AlertDialogContent>
                                                            <AlertDialogHeader>
                                                                <AlertDialogTitle>Delete image?</AlertDialogTitle>
                                                                <AlertDialogDescription>
                                                                    Remove {imageName} from {img.hostname}.
                                                                </AlertDialogDescription>
                                                            </AlertDialogHeader>
                                                            <AlertDialogFooter>
                                                                <AlertDialogCancel>Cancel</AlertDialogCancel>
                                                                <AlertDialogAction
                                                                    onClick={() => handleDelete(img, false)}
                                                                    render={<Button variant='destructive' />}
                                                                >
                                                                    Delete
                                                                </AlertDialogAction>
                                                            </AlertDialogFooter>
                                                        </AlertDialogContent>
                                                    </AlertDialog>
                                                    <ButtonGroupSeparator />
                                                    <DropdownMenu>
                                                        <DropdownMenuTrigger render={<Button variant='destructive' size='icon-xs' />}>
                                                            <ChevronDown className='size-3' />
                                                        </DropdownMenuTrigger>
                                                        <DropdownMenuContent align='end'>
                                                            <DropdownMenuItem variant='destructive' onClick={() => setForceDeleteTarget(img)}>
                                                                <Trash2 />
                                                                Force Delete
                                                            </DropdownMenuItem>
                                                        </DropdownMenuContent>
                                                    </DropdownMenu>
                                                </ButtonGroup>
                                            </TableCell>
                                        </TableRow>
                                    );
                                })}
                            </TableBody>
                        </Table>
                    </CardContent>
                </Card>
            )}

            {/* Force Delete confirmation dialog (controlled) */}
            <AlertDialog open={!!forceDeleteTarget} onOpenChange={(open) => !open && setForceDeleteTarget(null)}>
                <AlertDialogContent>
                    <AlertDialogHeader>
                        <AlertDialogTitle>Force delete image?</AlertDialogTitle>
                        <AlertDialogDescription>
                            Force remove {forceDeleteTarget?.tags[0] ?? forceDeleteTarget?.id} from {forceDeleteTarget?.hostname}. This may affect running
                            containers.
                        </AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                        <AlertDialogCancel>Cancel</AlertDialogCancel>
                        <AlertDialogAction
                            onClick={() => {
                                if (forceDeleteTarget) handleDelete(forceDeleteTarget, true);
                                setForceDeleteTarget(null);
                            }}
                            render={<Button variant='destructive' />}
                        >
                            Force Delete
                        </AlertDialogAction>
                    </AlertDialogFooter>
                </AlertDialogContent>
            </AlertDialog>
        </div>
    );
}

const sampleImages: ClusterImagesResponse = {
    nodes: [
        {
            nodeId: "node-1",
            hostname: "manager-1",
            images: [
                {
                    id: "abc123def456",
                    tags: ["nginx:latest", "nginx:1.25"],
                    size: 142000000,
                    created: Math.floor(Date.now() / 1000) - 3600,
                    nodeId: "node-1",
                    hostname: "manager-1",
                },
                {
                    id: "def456abc789",
                    tags: ["redis:7"],
                    size: 45000000,
                    created: Math.floor(Date.now() / 1000) - 86400,
                    nodeId: "node-1",
                    hostname: "manager-1",
                },
            ],
        },
        {
            nodeId: "node-2",
            hostname: "worker-1",
            images: [{ id: "ghi789jkl012", tags: [], size: 30000000, created: Math.floor(Date.now() / 1000) - 7200, nodeId: "node-2", hostname: "worker-1" }],
        },
    ],
};

const sampleMetrics: ClusterMetricsResponse = {
    agents: [
        {
            timestamp: new Date().toISOString(),
            nodeId: "node-1",
            hostname: "manager-1",
            node: {
                cpus: 4,
                memoryTotal: 8000000000,
                memoryUsed: 4000000000,
                memoryPercent: 50,
                cpuPercent: 25,
                diskTotal: 100000000000,
                diskUsed: 45000000000,
                diskPercent: 45,
            },
            containers: [],
        },
        {
            timestamp: new Date().toISOString(),
            nodeId: "node-2",
            hostname: "worker-1",
            node: {
                cpus: 2,
                memoryTotal: 4000000000,
                memoryUsed: 2000000000,
                memoryPercent: 50,
                cpuPercent: 10,
                diskTotal: 50000000000,
                diskUsed: 30000000000,
                diskPercent: 60,
            },
            containers: [],
        },
    ],
    history: [],
};

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
        component: ImagesTestComponent,
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

describe("ImagesPage", () => {
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

    it("renders image list with mock data", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") return sampleImages;
            if (cmd === "list_cluster_metrics") return sampleMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("nginx")).toBeInTheDocument();
        });

        expect(screen.getByText("redis")).toBeInTheDocument();
        expect(screen.getByText("<none>")).toBeInTheDocument();
        // Tag is shown as accent text under the name
        expect(screen.getByText("latest")).toBeInTheDocument();
        // Untagged images show the ID as accent text
        expect(screen.getByText("ghi789jkl012")).toBeInTheDocument();
    });

    it("shows total image count", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") return sampleImages;
            if (cmd === "list_cluster_metrics") return sampleMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("3 images across 2 nodes")).toBeInTheDocument();
        });
    });

    it("search filters images by tag", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") return sampleImages;
            if (cmd === "list_cluster_metrics") return sampleMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("nginx")).toBeInTheDocument();
        });

        const user = userEvent.setup();
        const searchInput = screen.getByPlaceholderText("Search images...");
        await user.type(searchInput, "nginx");

        expect(screen.getByText("nginx")).toBeInTheDocument();
        expect(screen.queryByText("redis")).not.toBeInTheDocument();
    });

    it("storage stats cards render per-node disk info", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") return sampleImages;
            if (cmd === "list_cluster_metrics") return sampleMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByTestId("storage-cards")).toBeInTheDocument();
        });

        // Storage cards should contain disk usage text
        const storageCards = screen.getByTestId("storage-cards");
        expect(storageCards).toBeInTheDocument();
        // Both node hostnames appear in the cards
        const allManager = screen.getAllByText("manager-1");
        expect(allManager.length).toBeGreaterThanOrEqual(1);
        const allWorker = screen.getAllByText("worker-1");
        expect(allWorker.length).toBeGreaterThanOrEqual(1);
    });

    it("select single image shows bulk bar", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") return sampleImages;
            if (cmd === "list_cluster_metrics") return sampleMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("nginx")).toBeInTheDocument();
        });

        const user = userEvent.setup();
        const checkbox = screen.getByTestId("select-abc123def456");
        await user.click(checkbox);

        await waitFor(() => {
            expect(screen.getByTestId("bulk-bar")).toBeInTheDocument();
            expect(screen.getByText("1 selected")).toBeInTheDocument();
        });
    });

    it("select all updates count", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") return sampleImages;
            if (cmd === "list_cluster_metrics") return sampleMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("nginx")).toBeInTheDocument();
        });

        const user = userEvent.setup();
        const selectAll = screen.getByTestId("select-all");
        await user.click(selectAll);

        await waitFor(() => {
            expect(screen.getByText("3 selected")).toBeInTheDocument();
        });
    });

    it("shows empty state when no images", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") return { nodes: [] };
            if (cmd === "list_cluster_metrics") return { agents: [], history: [] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("No images found")).toBeInTheDocument();
        });
    });

    it("shows error state when fetch fails", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") throw new Error("network error");
            if (cmd === "list_cluster_metrics") return { agents: [], history: [] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Failed to load images")).toBeInTheDocument();
        });
    });

    it("does not fetch when auth is missing", () => {
        vi.mocked(invoke).mockImplementation(async () => ({ nodes: [] }));

        renderWithProviders({ token: null, agentUrl: null });

        expect(invoke).not.toHaveBeenCalledWith("list_images", expect.anything());
    });

    it("delete confirmation dialog appears", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") return sampleImages;
            if (cmd === "list_cluster_metrics") return sampleMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("nginx")).toBeInTheDocument();
        });

        const user = userEvent.setup();
        const deleteBtn = screen.getByTestId("delete-abc123def456");
        await user.click(deleteBtn);

        await waitFor(() => {
            expect(screen.getByText("Delete image?")).toBeInTheDocument();
        });
    });

    it("prune confirmation dialog appears", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") return sampleImages;
            if (cmd === "list_cluster_metrics") return sampleMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("nginx")).toBeInTheDocument();
        });

        const user = userEvent.setup();
        const pruneBtn = screen.getByTestId("prune-btn");
        await user.click(pruneBtn);

        await waitFor(() => {
            expect(screen.getByText("Prune unused images?")).toBeInTheDocument();
        });
    });

    it("shows node hostname and nodeId", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") return sampleImages;
            if (cmd === "list_cluster_metrics") return sampleMetrics;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("nginx")).toBeInTheDocument();
        });

        // hostname appears in storage cards and table rows
        const managerEntries = screen.getAllByText("manager-1");
        expect(managerEntries.length).toBeGreaterThanOrEqual(1);
        // nodeId appears under hostname in the table
        const nodeIds = screen.getAllByText("node-1");
        expect(nodeIds.length).toBeGreaterThanOrEqual(1);
    });
});

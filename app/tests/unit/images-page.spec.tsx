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
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { PageHeader } from "@/components/page-header";
import { TableShell } from "@/components/table-shell";
import { ColumnHeaders } from "@/components/column-headers";
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
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { ButtonGroup, ButtonGroupSeparator } from "@/components/ui/button-group";
import { AlertCircle, ChevronDown, HardDrive, Trash2 } from "lucide-react";
import type { ImageItem, ClusterImagesResponse } from "@/types/images";

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

function shortId(id: string): string {
    const raw = id.startsWith("sha256:") ? id.slice(7) : id;
    return raw.slice(0, 12);
}

const IMAGE_COLUMNS = [
    { label: "REPOSITORY" },
    { label: "TAG", width: 100 },
    { label: "IMAGE ID", width: 120 },
    { label: "SIZE", width: 80, align: "right" as const },
    { label: "NODE", width: 100 },
    { label: "CREATED", width: 100, align: "right" as const },
    { label: "", width: 60 },
];

function LoadingSkeleton() {
    return (
        <div className='flex flex-col gap-6 p-6'>
            <div className='flex items-center justify-between'>
                <div className='flex flex-col gap-1'>
                    <Skeleton className='h-7 w-24' />
                    <Skeleton className='h-3 w-40' />
                </div>
                <Skeleton className='h-7 w-28' />
            </div>
            <div className='flex flex-col overflow-hidden border border-border'>
                <div className='flex items-center justify-between border-b border-border px-4 py-3'>
                    <Skeleton className='h-4 w-32' />
                    <Skeleton className='h-7 w-48' />
                </div>
                <div className='space-y-0'>
                    {[1, 2, 3, 4, 5].map((i) => (
                        <div key={i} className='flex items-center gap-3 border-b border-border px-4 py-2.5 last:border-0'>
                            <Skeleton className='h-3 w-40' />
                            <Skeleton className='h-3 w-16' />
                            <Skeleton className='h-3 w-24' />
                            <Skeleton className='ml-auto h-3 w-14' />
                        </div>
                    ))}
                </div>
            </div>
        </div>
    );
}

interface ImageRowProps {
    img: ImageItem;
    onDelete: (img: ImageItem, force: boolean) => void;
    onForceDelete: (img: ImageItem) => void;
}

function ImageRow({ img, onDelete, onForceDelete }: ImageRowProps) {
    const fullTag = img.tags.length > 0 ? img.tags[0] : null;
    const colonIdx = fullTag ? fullTag.lastIndexOf(":") : -1;
    const repository = fullTag ? (colonIdx > 0 ? fullTag.slice(0, colonIdx) : fullTag) : "<none>";
    const tag = fullTag && colonIdx > 0 ? fullTag.slice(colonIdx + 1) : null;

    return (
        <div className='flex items-center border-b border-border px-4 py-2.5 hover:bg-bg-elevated/50'>
            <div style={{ flex: 1 }}>
                <span className='font-mono text-xs'>{repository}</span>
            </div>
            <div style={{ width: 100 }}>
                <span className='font-mono text-[11px] text-muted-foreground'>{tag ?? <span className='opacity-40'>—</span>}</span>
            </div>
            <div style={{ width: 120 }}>
                <span className='truncate font-mono text-[11px] text-muted-foreground' title={img.id}>
                    {shortId(img.id)}
                </span>
            </div>
            <div style={{ width: 80 }} className='text-right'>
                <span className='font-mono text-[11px] text-muted-foreground'>{formatBytes(img.size)}</span>
            </div>
            <div style={{ width: 100 }}>
                <span className='font-mono text-[11px] text-muted-foreground'>{img.hostname}</span>
            </div>
            <div style={{ width: 100 }} className='text-right'>
                <span className='font-mono text-[11px] text-muted-foreground'>{formatRelativeTime(img.created)}</span>
            </div>
            <div style={{ width: 60 }} className='flex items-center justify-end'>
                <ButtonGroup>
                    <AlertDialog>
                        <AlertDialogTrigger render={<Button variant='destructive' size='xs' data-testid={`delete-${img.id}`} />}>Delete</AlertDialogTrigger>
                        <AlertDialogContent>
                            <AlertDialogHeader>
                                <AlertDialogTitle>Delete image?</AlertDialogTitle>
                                <AlertDialogDescription>
                                    Remove {repository} from {img.hostname}.
                                </AlertDialogDescription>
                            </AlertDialogHeader>
                            <AlertDialogFooter>
                                <AlertDialogCancel>Cancel</AlertDialogCancel>
                                <AlertDialogAction onClick={() => onDelete(img, false)} render={<Button variant='destructive' />}>
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
                            <DropdownMenuItem variant='destructive' onClick={() => onForceDelete(img)}>
                                <Trash2 />
                                Force Delete
                            </DropdownMenuItem>
                        </DropdownMenuContent>
                    </DropdownMenu>
                </ButtonGroup>
            </div>
        </div>
    );
}

function ImagesTestComponent() {
    const [search, setSearch] = useState("");
    const [forceDeleteTarget, setForceDeleteTarget] = useState<ImageItem | null>(null);
    const { data: imagesData, isLoading, error } = useImages();
    const removeImage = useRemoveImage();
    const pruneImages = usePruneImages();

    const allImages = useMemo(() => {
        if (!imagesData?.nodes) return [];
        return imagesData.nodes.flatMap((node) => node.images);
    }, [imagesData]);

    const filteredImages = useMemo(() => {
        if (!search) return allImages;
        const q = search.toLowerCase();
        return allImages.filter((img) => img.tags.some((t) => t.toLowerCase().includes(q)) || img.hostname.toLowerCase().includes(q));
    }, [allImages, search]);

    function handleDelete(img: ImageItem, force: boolean) {
        removeImage.mutate({ imageId: img.id, nodeId: img.nodeId, force });
    }

    const nodeCount = imagesData?.nodes?.length ?? 0;
    const subtitle = `${allImages.length} ${allImages.length === 1 ? "image" : "images"} across ${nodeCount} ${nodeCount === 1 ? "node" : "nodes"}`;

    if (isLoading) return <LoadingSkeleton />;

    if (error) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-xs text-destructive'>
                <AlertCircle className='size-4' />
                <p>Failed to load images</p>
            </div>
        );
    }

    return (
        <div className='flex h-full flex-col gap-6 p-6'>
            <PageHeader title='IMAGES' subtitle={subtitle}>
                <Button variant='outline' size='sm' disabled className='h-7 font-mono text-[11px] text-muted-foreground'>
                    PULL IMAGE
                </Button>
            </PageHeader>

            <TableShell
                title='LOCAL IMAGES'
                count={filteredImages.length}
                searchValue={search}
                onSearchChange={setSearch}
                searchPlaceholder='Search images...'
                filterSlot={
                    <AlertDialog>
                        <AlertDialogTrigger
                            render={<Button variant='outline' size='sm' data-testid='prune-btn' className='h-7 font-mono text-[11px] text-muted-foreground' />}
                        >
                            PRUNE
                        </AlertDialogTrigger>
                        <AlertDialogContent>
                            <AlertDialogHeader>
                                <AlertDialogTitle>Prune unused images?</AlertDialogTitle>
                                <AlertDialogDescription>
                                    This will remove all unused images across all nodes to reclaim disk space. This action cannot be undone.
                                </AlertDialogDescription>
                            </AlertDialogHeader>
                            <AlertDialogFooter>
                                <AlertDialogCancel>Cancel</AlertDialogCancel>
                                <AlertDialogAction onClick={() => pruneImages.mutate()} render={<Button variant='destructive' />}>
                                    {pruneImages.isPending ? "Pruning..." : "Prune All"}
                                </AlertDialogAction>
                            </AlertDialogFooter>
                        </AlertDialogContent>
                    </AlertDialog>
                }
            >
                <ColumnHeaders columns={IMAGE_COLUMNS} />
                {filteredImages.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                        <HardDrive className='size-4' />
                        <p>{search ? "No images match your filter" : "No images found"}</p>
                    </div>
                ) : (
                    <div>
                        {filteredImages.map((img) => (
                            <ImageRow key={`${img.nodeId}:${img.id}`} img={img} onDelete={handleDelete} onForceDelete={setForceDeleteTarget} />
                        ))}
                    </div>
                )}
            </TableShell>

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

    it("renders flat image list with repository and tag columns", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") return sampleImages;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("nginx")).toBeInTheDocument();
        });

        expect(screen.getByText("redis")).toBeInTheDocument();
        expect(screen.getByText("<none>")).toBeInTheDocument();
        // Tag column shows the tag portion
        expect(screen.getByText("latest")).toBeInTheDocument();
        expect(screen.getByText("7")).toBeInTheDocument();
    });

    it("shows image ID truncated to 12 chars without sha256: prefix", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") return sampleImages;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("nginx")).toBeInTheDocument();
        });

        // shortId("abc123def456") = "abc123def456" (already 12 chars)
        expect(screen.getByText("abc123def456")).toBeInTheDocument();
        // sha256: prefix should not appear
        expect(screen.queryByText(/sha256:/)).not.toBeInTheDocument();
    });

    it("shows subtitle with total image and node counts", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") return sampleImages;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("3 images across 2 nodes")).toBeInTheDocument();
        });
    });

    it("shows node hostname in NODE column", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") return sampleImages;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("nginx")).toBeInTheDocument();
        });

        const managerEntries = screen.getAllByText("manager-1");
        expect(managerEntries.length).toBeGreaterThanOrEqual(1);
        const workerEntries = screen.getAllByText("worker-1");
        expect(workerEntries.length).toBeGreaterThanOrEqual(1);
    });

    it("search filters images by tag", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") return sampleImages;
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

    it("shows empty state when no images", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") return { nodes: [] };
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

    it("column headers are rendered", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") return sampleImages;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("nginx")).toBeInTheDocument();
        });

        expect(screen.getByText("REPOSITORY")).toBeInTheDocument();
        expect(screen.getByText("TAG")).toBeInTheDocument();
        expect(screen.getByText("IMAGE ID")).toBeInTheDocument();
        expect(screen.getByText("SIZE")).toBeInTheDocument();
        expect(screen.getByText("NODE")).toBeInTheDocument();
        expect(screen.getByText("CREATED")).toBeInTheDocument();
    });

    it("PULL IMAGE button is rendered and disabled", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_images") return sampleImages;
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("nginx")).toBeInTheDocument();
        });

        const pullBtn = screen.getByText("PULL IMAGE");
        expect(pullBtn).toBeInTheDocument();
        expect(pullBtn.closest("button")).toBeDisabled();
    });
});

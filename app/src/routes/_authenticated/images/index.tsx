import { createFileRoute } from "@tanstack/react-router";
import { useState, useMemo } from "react";
import { useImages } from "@/hooks/use-images";
import { useRemoveImage } from "@/hooks/use-remove-image";
import { usePruneImages } from "@/hooks/use-prune-images";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { PageHeader } from "@/components/page-header";
import { TableShell } from "@/components/table-shell";
import { TableToolbar } from "@/components/table-toolbar";
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
import { RefreshButton } from "@/components/refresh-button";
import { AlertCircle, ChevronDown, HardDrive, Trash2 } from "lucide-react";
import type { ImageItem } from "@/types/images";

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
                                    Remove <strong>{repository}</strong> from {img.hostname}. This will fail if the image is in use.
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

function ImagesPage() {
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

    if (isLoading) {
        return <LoadingSkeleton />;
    }

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
                <RefreshButton queryKeys={[["images"]]} />
                <Button variant='outline' size='sm' disabled className='h-7 font-mono text-[11px] text-muted-foreground'>
                    PULL IMAGE
                </Button>
            </PageHeader>

            <TableToolbar searchValue={search} onSearchChange={setSearch} searchPlaceholder='Search images...' />

            <TableShell
                title='LOCAL IMAGES'
                count={filteredImages.length}
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
                            Force remove <strong>{forceDeleteTarget?.tags[0] ?? forceDeleteTarget?.id}</strong> from {forceDeleteTarget?.hostname}. This may
                            affect running containers.
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

export const Route = createFileRoute("/_authenticated/images/")({
    component: ImagesPage,
});

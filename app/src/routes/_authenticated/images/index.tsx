import { createFileRoute } from "@tanstack/react-router";
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
import { AlertCircle, ChevronDown, Eraser, HardDrive, Search, Trash2 } from "lucide-react";
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

function LoadingSkeleton() {
    return (
        <div className='flex flex-col'>
            <div className='flex h-10 items-center gap-3 border-b border-border px-3'>
                <Skeleton className='h-4 w-14' />
                <Skeleton className='h-4 w-8 rounded-full' />
                <div className='flex-1' />
                <Skeleton className='h-6 w-40' />
                <Skeleton className='h-6 w-16' />
            </div>
            <div className='space-y-0'>
                {[1, 2, 3, 4, 5].map((i) => (
                    <div key={i} className='flex items-center gap-3 border-b border-border px-3 py-2 last:border-0'>
                        <Skeleton className='size-3' />
                        <Skeleton className='h-3 w-36' />
                        <Skeleton className='h-3 w-16' />
                        <Skeleton className='h-3 w-14' />
                        <Skeleton className='ml-auto h-3 w-14' />
                    </div>
                ))}
            </div>
        </div>
    );
}

function ImagesPage() {
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
        <div className='flex h-full flex-col'>
            {/* Toolbar */}
            <div className='flex h-10 shrink-0 items-center gap-3 border-b border-border px-3'>
                <h1 className='text-xs font-semibold'>Images</h1>
                <span className='rounded-md bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground'>{allImages.length}</span>
                <div className='flex-1' />
                <div className='relative'>
                    <Search className='absolute top-1/2 left-2 size-3 -translate-y-1/2 text-muted-foreground' />
                    <Input
                        placeholder='Filter images...'
                        value={search}
                        onChange={(e) => setSearch(e.target.value)}
                        className='h-6 w-48 rounded-sm border-border bg-transparent pl-7 text-[11px] focus-visible:ring-1'
                    />
                </div>
                <AlertDialog>
                    <AlertDialogTrigger render={<Button variant='ghost' size='sm' className='h-6 px-2 text-[11px]' />}>
                        <Eraser className='size-3' />
                        Prune
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
            </div>

            <div className='flex-1 overflow-y-auto'>
                {/* Storage cards */}
                {agents.length > 0 && (
                    <div className='border-b border-border px-3 py-2'>
                        <div className='grid gap-2' style={{ gridTemplateColumns: `repeat(${Math.min(agents.length, 4)}, minmax(0, 1fr))` }}>
                            {agents.map((agent) => (
                                <Card key={agent.nodeId}>
                                    <CardContent className='p-2'>
                                        <div className='mb-1 flex items-center justify-between'>
                                            <span className='truncate text-[11px] font-medium'>{agent.hostname}</span>
                                            <span className='font-mono text-[10px] text-muted-foreground'>{Math.round(agent.node.diskPercent)}%</span>
                                        </div>
                                        <Progress value={agent.node.diskPercent} className='mb-1 h-1' />
                                        <p className='text-[10px] text-muted-foreground'>
                                            {formatBytes(agent.node.diskUsed)} / {formatBytes(agent.node.diskTotal)}
                                        </p>
                                    </CardContent>
                                </Card>
                            ))}
                        </div>
                    </div>
                )}

                {/* Bulk action bar */}
                {selected.size > 0 && (
                    <div className='flex items-center gap-3 border-b border-border bg-muted/30 px-3 py-1.5'>
                        <span className='text-[11px] font-medium'>{selected.size} selected</span>
                        <div className='ml-auto flex items-center gap-2'>
                            <AlertDialog>
                                <AlertDialogTrigger render={<Button variant='outline' size='sm' className='h-6 px-2 text-[11px]' />}>
                                    <Trash2 className='size-3' />
                                    Delete
                                </AlertDialogTrigger>
                                <AlertDialogContent>
                                    <AlertDialogHeader>
                                        <AlertDialogTitle>Delete {selected.size} images?</AlertDialogTitle>
                                        <AlertDialogDescription>
                                            This will remove the selected images. Images in use by containers will fail to delete.
                                        </AlertDialogDescription>
                                    </AlertDialogHeader>
                                    <AlertDialogFooter>
                                        <AlertDialogCancel>Cancel</AlertDialogCancel>
                                        <AlertDialogAction onClick={() => handleBulkDelete(false)} render={<Button variant='destructive' />}>
                                            Delete
                                        </AlertDialogAction>
                                    </AlertDialogFooter>
                                </AlertDialogContent>
                            </AlertDialog>
                            <AlertDialog>
                                <AlertDialogTrigger render={<Button variant='destructive' size='sm' className='h-6 px-2 text-[11px]' />}>
                                    <Trash2 className='size-3' />
                                    Force
                                </AlertDialogTrigger>
                                <AlertDialogContent>
                                    <AlertDialogHeader>
                                        <AlertDialogTitle>Force delete {selected.size} images?</AlertDialogTitle>
                                        <AlertDialogDescription>
                                            This will forcefully remove the selected images, even if they are being used by containers. This may break running
                                            containers.
                                        </AlertDialogDescription>
                                    </AlertDialogHeader>
                                    <AlertDialogFooter>
                                        <AlertDialogCancel>Cancel</AlertDialogCancel>
                                        <AlertDialogAction onClick={() => handleBulkDelete(true)} render={<Button variant='destructive' />}>
                                            Force Delete
                                        </AlertDialogAction>
                                    </AlertDialogFooter>
                                </AlertDialogContent>
                            </AlertDialog>
                            <Button variant='ghost' size='sm' className='h-6 px-2 text-[11px]' onClick={() => setSelected(new Set())}>
                                Clear
                            </Button>
                        </div>
                    </div>
                )}

                {/* Table */}
                {filteredImages.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                        <HardDrive className='size-4' />
                        <p>{search ? "No images match your filter" : "No images found"}</p>
                    </div>
                ) : (
                    <Table>
                        <TableHeader>
                            <TableRow className='bg-muted/40 hover:bg-muted/40'>
                                <TableHead className='h-7 w-8'>
                                    <Checkbox
                                        checked={selected.size === filteredImages.length && filteredImages.length > 0}
                                        onCheckedChange={toggleSelectAll}
                                    />
                                </TableHead>
                                <TableHead className='h-7 text-[11px]'>Image</TableHead>
                                <TableHead className='h-7 text-[11px]'>Size</TableHead>
                                <TableHead className='h-7 text-[11px]'>Created</TableHead>
                                <TableHead className='h-7 text-[11px]'>Node</TableHead>
                                <TableHead className='h-7 w-20' />
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
                                        <TableCell className='py-1.5'>
                                            <Checkbox checked={selected.has(key)} onCheckedChange={() => toggleSelect(key)} />
                                        </TableCell>
                                        <TableCell className='py-1.5'>
                                            <div className='flex flex-col gap-0'>
                                                <span className='text-xs font-medium'>{imageName}</span>
                                                <span className='font-mono text-[10px] text-muted-foreground'>{imageTag ?? img.id}</span>
                                            </div>
                                        </TableCell>
                                        <TableCell className='py-1.5'>
                                            <span className='font-mono text-[11px] text-muted-foreground'>{formatBytes(img.size)}</span>
                                        </TableCell>
                                        <TableCell className='py-1.5'>
                                            <span className='font-mono text-[11px] text-muted-foreground'>{formatRelativeTime(img.created)}</span>
                                        </TableCell>
                                        <TableCell className='py-1.5'>
                                            <div className='flex flex-col gap-0'>
                                                <span className='text-xs font-medium'>{img.hostname}</span>
                                                <span className='font-mono text-[10px] text-muted-foreground'>{img.nodeId}</span>
                                            </div>
                                        </TableCell>
                                        <TableCell className='py-1.5'>
                                            <ButtonGroup>
                                                <AlertDialog>
                                                    <AlertDialogTrigger render={<Button variant='destructive' size='xs' />}>Delete</AlertDialogTrigger>
                                                    <AlertDialogContent>
                                                        <AlertDialogHeader>
                                                            <AlertDialogTitle>Delete image?</AlertDialogTitle>
                                                            <AlertDialogDescription>
                                                                Remove <strong>{imageName}</strong> from {img.hostname}. This will fail if the image is in use.
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
                )}
            </div>

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

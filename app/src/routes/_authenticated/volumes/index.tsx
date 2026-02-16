import { createFileRoute } from "@tanstack/react-router";
import { useVolumes } from "@/hooks/use-volumes";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { AlertCircle, HardDrive, Search } from "lucide-react";
import { useState } from "react";
import type { VolumeItem } from "@/types/volumes";

function formatRelativeTime(iso: string): string {
    if (!iso) return "-";
    const seconds = Math.floor((Date.now() - new Date(iso).getTime()) / 1000);
    if (seconds < 60) return "just now";
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
}

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
                <TableRow className='bg-muted/40 hover:bg-muted/40'>
                    <TableHead className='h-7 text-[11px]'>Name</TableHead>
                    <TableHead className='h-7 text-[11px]'>Driver</TableHead>
                    <TableHead className='h-7 text-[11px]'>Scope</TableHead>
                    <TableHead className='h-7 text-[11px]'>Stack</TableHead>
                    <TableHead className='h-7 text-[11px]'>Size</TableHead>
                    <TableHead className='h-7 text-[11px]'>Refs</TableHead>
                    <TableHead className='h-7 text-right text-[11px]'>Created</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {volumes.map((vol) => (
                    <TableRow key={vol.name}>
                        <TableCell className='py-1.5'>
                            <div className='flex flex-col gap-0'>
                                <span className='text-xs font-medium'>{vol.name}</span>
                                <span className='truncate font-mono text-[10px] text-muted-foreground'>{vol.mountpoint}</span>
                            </div>
                        </TableCell>
                        <TableCell className='py-1.5'>
                            <span className='font-mono text-[11px] text-muted-foreground'>{vol.driver}</span>
                        </TableCell>
                        <TableCell className='py-1.5'>
                            <span className='text-[11px] text-muted-foreground'>{vol.scope}</span>
                        </TableCell>
                        <TableCell className='py-1.5'>
                            {vol.stack ? (
                                <Badge variant='secondary' className='h-4 px-1.5 text-[10px]'>
                                    {vol.stack}
                                </Badge>
                            ) : (
                                <span className='text-[11px] text-muted-foreground'>-</span>
                            )}
                        </TableCell>
                        <TableCell className='py-1.5'>
                            <span className='font-mono text-[11px] text-muted-foreground'>{formatBytes(vol.sizeBytes)}</span>
                        </TableCell>
                        <TableCell className='py-1.5'>
                            <span className='font-mono text-[11px] text-muted-foreground'>{vol.refCount >= 0 ? vol.refCount : "-"}</span>
                        </TableCell>
                        <TableCell className='py-1.5 text-right'>
                            <span className='font-mono text-[11px] text-muted-foreground'>{formatRelativeTime(vol.createdAt)}</span>
                        </TableCell>
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}

function LoadingSkeleton() {
    return (
        <div className='flex flex-col'>
            <div className='flex h-10 items-center gap-3 border-b border-border px-3'>
                <Skeleton className='h-4 w-16' />
                <Skeleton className='h-4 w-8 rounded-full' />
                <div className='flex-1' />
                <Skeleton className='h-6 w-40' />
            </div>
            <div className='space-y-0'>
                {[1, 2, 3, 4, 5].map((i) => (
                    <div key={i} className='flex items-center gap-3 border-b border-border px-3 py-2 last:border-0'>
                        <Skeleton className='h-3 w-32' />
                        <Skeleton className='h-3 w-14' />
                        <Skeleton className='h-3 w-14' />
                        <Skeleton className='h-4 w-16 rounded-full' />
                        <Skeleton className='ml-auto h-3 w-14' />
                    </div>
                ))}
            </div>
        </div>
    );
}

function VolumesPage() {
    const { data: volumes, isLoading, error } = useVolumes();
    const [search, setSearch] = useState("");

    const filtered = (volumes ?? []).filter(
        (vol) =>
            vol.name.toLowerCase().includes(search.toLowerCase()) ||
            vol.driver.toLowerCase().includes(search.toLowerCase()) ||
            vol.stack.toLowerCase().includes(search.toLowerCase()),
    );

    if (isLoading) {
        return <LoadingSkeleton />;
    }

    if (error) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-xs text-destructive'>
                <AlertCircle className='size-4' />
                <p>Failed to load volumes</p>
            </div>
        );
    }

    return (
        <div className='flex h-full flex-col'>
            <div className='flex h-10 shrink-0 items-center gap-3 border-b border-border px-3'>
                <h1 className='text-xs font-semibold'>Volumes</h1>
                <span className='rounded-md bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground'>{volumes?.length ?? 0}</span>
                <div className='flex-1' />
                <div className='relative'>
                    <Search className='absolute top-1/2 left-2 size-3 -translate-y-1/2 text-muted-foreground' />
                    <Input
                        placeholder='Filter volumes...'
                        value={search}
                        onChange={(e) => setSearch(e.target.value)}
                        className='h-6 w-48 rounded-sm border-border bg-transparent pl-7 text-[11px] focus-visible:ring-1'
                    />
                </div>
            </div>
            <div className='flex-1 overflow-y-auto'>
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                        <HardDrive className='size-4' />
                        <p>{search ? "No volumes match your filter" : "No volumes found"}</p>
                    </div>
                ) : (
                    <VolumesTable volumes={filtered} />
                )}
            </div>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/volumes/")({
    component: VolumesPage,
});

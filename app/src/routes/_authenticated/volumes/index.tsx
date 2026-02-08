import { createFileRoute } from "@tanstack/react-router";
import { useVolumes } from "@/hooks/use-volumes";
import { PageHeader } from "@/components/page-header";
import { Card, CardContent } from "@/components/ui/card";
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
                            <span className='text-xs text-muted-foreground'>{formatRelativeTime(vol.createdAt)}</span>
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
        return (
            <>
                <PageHeader title='Volumes' />
                <LoadingSkeleton />
            </>
        );
    }

    if (error) {
        return (
            <>
                <PageHeader title='Volumes' />
                <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
                    <AlertCircle className='size-5' />
                    <p>Failed to load volumes</p>
                </div>
            </>
        );
    }

    return (
        <>
            <PageHeader
                title='Volumes'
                description={`${volumes?.length ?? 0} volumes`}
                actions={
                    <div className='relative'>
                        <Search className='absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground' />
                        <Input placeholder='Search volumes...' value={search} onChange={(e) => setSearch(e.target.value)} className='h-8 w-56 pl-8 text-sm' />
                    </div>
                }
            />

            <div className='p-6'>
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-muted-foreground'>
                        <HardDrive className='size-5' />
                        <p>{search ? "No volumes match your search" : "No volumes found"}</p>
                    </div>
                ) : (
                    <Card>
                        <CardContent className='p-0'>
                            <VolumesTable volumes={filtered} />
                        </CardContent>
                    </Card>
                )}
            </div>
        </>
    );
}

export const Route = createFileRoute("/_authenticated/volumes/")({
    component: VolumesPage,
});

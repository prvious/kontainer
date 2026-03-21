import { createFileRoute } from "@tanstack/react-router";
import { useVolumes } from "@/hooks/use-volumes";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { PageHeader } from "@/components/page-header";
import { TableShell } from "@/components/table-shell";
import { TableToolbar } from "@/components/table-toolbar";
import { ColumnHeaders } from "@/components/column-headers";
import { RefreshButton } from "@/components/refresh-button";
import { AlertCircle, HardDrive, MoreHorizontal } from "lucide-react";
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

const VOLUME_COLUMNS = [
    { label: "NAME" },
    { label: "DRIVER", width: 100 },
    { label: "MOUNTPOINT" },
    { label: "SIZE", width: 80, align: "right" as const },
    { label: "CREATED", width: 100, align: "right" as const },
    { label: "", width: 60 },
];

function VolumeRow({ volume }: { volume: VolumeItem }) {
    return (
        <div className='flex items-center border-b border-border px-4 py-2.5 hover:bg-bg-elevated/50'>
            <div style={{ flex: 1 }} className='flex items-center'>
                <span className='font-mono text-xs font-medium'>{volume.name}</span>
            </div>
            <div style={{ width: 100 }}>
                <span className='font-mono text-[11px] text-muted-foreground'>{volume.driver}</span>
            </div>
            <div style={{ flex: 1 }} className='min-w-0 pr-4'>
                <span className='block truncate font-mono text-[11px] text-muted-foreground'>{volume.mountpoint}</span>
            </div>
            <div style={{ width: 80 }} className='text-right'>
                <span className='font-mono text-[11px] text-muted-foreground'>{formatBytes(volume.sizeBytes)}</span>
            </div>
            <div style={{ width: 100 }} className='text-right'>
                <span className='font-mono text-[11px] text-muted-foreground'>{formatRelativeTime(volume.createdAt)}</span>
            </div>
            <div style={{ width: 60 }} className='flex items-center justify-end'>
                <button className='flex size-6 items-center justify-center rounded text-muted-foreground opacity-0 transition-opacity hover:bg-bg-elevated hover:opacity-100 focus:opacity-100'>
                    <MoreHorizontal className='size-3.5' />
                </button>
            </div>
        </div>
    );
}

function LoadingSkeleton() {
    return (
        <div className='flex flex-col gap-6 p-6'>
            <div className='flex items-center justify-between'>
                <div className='flex flex-col gap-1'>
                    <Skeleton className='h-7 w-28' />
                    <Skeleton className='h-3 w-48' />
                </div>
                <Skeleton className='h-7 w-36' />
            </div>
            <div className='flex flex-col overflow-hidden border border-border'>
                <div className='flex items-center justify-between border-b border-border px-4 py-3'>
                    <Skeleton className='h-4 w-32' />
                    <Skeleton className='h-7 w-48' />
                </div>
                <div className='space-y-0'>
                    {[1, 2, 3, 4, 5].map((i) => (
                        <div key={i} className='flex items-center gap-3 border-b border-border px-4 py-2.5 last:border-0'>
                            <Skeleton className='h-3 w-32' />
                            <Skeleton className='h-3 w-16' />
                            <Skeleton className='h-3 w-40' />
                            <Skeleton className='ml-auto h-3 w-12' />
                        </div>
                    ))}
                </div>
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

    const volumeCount = volumes?.length ?? 0;
    const driverCount = new Set((volumes ?? []).map((v) => v.driver).filter(Boolean)).size;
    const subtitle = `${volumeCount} ${volumeCount === 1 ? "volume" : "volumes"} · ${driverCount} ${driverCount === 1 ? "driver" : "drivers"}`;

    return (
        <div className='flex h-full flex-col gap-6 p-6'>
            <PageHeader title='VOLUMES' subtitle={subtitle}>
                <RefreshButton queryKeys={[["volumes"]]} />
                <Button variant='outline' size='sm' disabled className='h-7 font-mono text-[11px] text-muted-foreground'>
                    CREATE VOLUME
                </Button>
            </PageHeader>

            <TableToolbar searchValue={search} onSearchChange={setSearch} searchPlaceholder='Search volumes...' />

            <TableShell title='ALL VOLUMES' count={filtered.length}>
                <ColumnHeaders columns={VOLUME_COLUMNS} />
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                        <HardDrive className='size-4' />
                        <p>{search ? "No volumes match your filter" : "No volumes found"}</p>
                    </div>
                ) : (
                    <div>
                        {filtered.map((vol) => (
                            <VolumeRow key={vol.name} volume={vol} />
                        ))}
                    </div>
                )}
            </TableShell>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/volumes/")({
    component: VolumesPage,
});

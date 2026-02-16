import { createFileRoute } from "@tanstack/react-router";
import { useNetworks } from "@/hooks/use-networks";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { AlertCircle, Network, Search } from "lucide-react";
import { useState } from "react";
import type { NetworkItem } from "@/types/networks";

function formatRelativeTime(iso: string): string {
    const seconds = Math.floor((Date.now() - new Date(iso).getTime()) / 1000);
    if (seconds < 60) return "just now";
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
}

function NetworksTable({ networks }: { networks: NetworkItem[] }) {
    return (
        <Table>
            <TableHeader>
                <TableRow className='bg-muted/40 hover:bg-muted/40'>
                    <TableHead className='h-7 text-[11px]'>Name</TableHead>
                    <TableHead className='h-7 text-[11px]'>Driver</TableHead>
                    <TableHead className='h-7 text-[11px]'>Scope</TableHead>
                    <TableHead className='h-7 text-[11px]'>Flags</TableHead>
                    <TableHead className='h-7 text-[11px]'>Services</TableHead>
                    <TableHead className='h-7 text-right text-[11px]'>Created</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {networks.map((net) => (
                    <TableRow key={net.id}>
                        <TableCell className='py-1.5'>
                            <div className='flex items-center gap-1.5'>
                                <span className='text-xs font-medium'>{net.name}</span>
                                {net.ingress && (
                                    <Badge variant='secondary' className='h-4 px-1.5 text-[10px]'>
                                        ingress
                                    </Badge>
                                )}
                            </div>
                        </TableCell>
                        <TableCell className='py-1.5'>
                            <span className='font-mono text-[11px] text-muted-foreground'>{net.driver}</span>
                        </TableCell>
                        <TableCell className='py-1.5'>
                            <span className='text-[11px] text-muted-foreground'>{net.scope}</span>
                        </TableCell>
                        <TableCell className='py-1.5'>
                            <div className='flex items-center gap-1'>
                                {net.internal && (
                                    <Badge variant='secondary' className='h-4 px-1.5 text-[10px]'>
                                        internal
                                    </Badge>
                                )}
                                {net.attachable && (
                                    <Badge variant='secondary' className='h-4 px-1.5 text-[10px]'>
                                        attachable
                                    </Badge>
                                )}
                            </div>
                        </TableCell>
                        <TableCell className='py-1.5'>
                            {net.services.length === 0 ? (
                                <span className='text-[11px] text-muted-foreground'>-</span>
                            ) : (
                                <div className='flex flex-wrap gap-1'>
                                    {net.services.map((svc) => (
                                        <Badge key={svc.id} variant='default' className='h-4 px-1.5 text-[10px]'>
                                            {svc.name}
                                        </Badge>
                                    ))}
                                </div>
                            )}
                        </TableCell>
                        <TableCell className='py-1.5 text-right'>
                            <span className='font-mono text-[11px] text-muted-foreground'>{formatRelativeTime(net.createdAt)}</span>
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
                <Skeleton className='h-4 w-18' />
                <Skeleton className='h-4 w-8 rounded-full' />
                <div className='flex-1' />
                <Skeleton className='h-6 w-40' />
            </div>
            <div className='space-y-0'>
                {[1, 2, 3, 4, 5].map((i) => (
                    <div key={i} className='flex items-center gap-3 border-b border-border px-3 py-2 last:border-0'>
                        <Skeleton className='h-3 w-28' />
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

function NetworksPage() {
    const { data: networks, isLoading, error } = useNetworks();
    const [search, setSearch] = useState("");

    const filtered = (networks ?? []).filter(
        (net) =>
            net.name.toLowerCase().includes(search.toLowerCase()) ||
            net.driver.toLowerCase().includes(search.toLowerCase()) ||
            net.services.some((svc) => svc.name.toLowerCase().includes(search.toLowerCase())),
    );

    if (isLoading) {
        return <LoadingSkeleton />;
    }

    if (error) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-xs text-destructive'>
                <AlertCircle className='size-4' />
                <p>Failed to load networks</p>
            </div>
        );
    }

    return (
        <div className='flex h-full flex-col'>
            <div className='flex h-10 shrink-0 items-center gap-3 border-b border-border px-3'>
                <h1 className='text-xs font-semibold'>Networks</h1>
                <span className='rounded-md bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground'>{networks?.length ?? 0}</span>
                <div className='flex-1' />
                <div className='relative'>
                    <Search className='absolute top-1/2 left-2 size-3 -translate-y-1/2 text-muted-foreground' />
                    <Input
                        placeholder='Filter networks...'
                        value={search}
                        onChange={(e) => setSearch(e.target.value)}
                        className='h-6 w-48 rounded-sm border-border bg-transparent pl-7 text-[11px] focus-visible:ring-1'
                    />
                </div>
            </div>
            <div className='flex-1 overflow-y-auto'>
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                        <Network className='size-4' />
                        <p>{search ? "No networks match your filter" : "No networks found"}</p>
                    </div>
                ) : (
                    <NetworksTable networks={filtered} />
                )}
            </div>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/networks/")({
    component: NetworksPage,
});

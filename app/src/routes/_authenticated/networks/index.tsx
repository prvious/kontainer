import { createFileRoute } from "@tanstack/react-router";
import { useNetworks } from "@/hooks/use-networks";
import { PageHeader } from "@/components/page-header";
import { Card, CardContent } from "@/components/ui/card";
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
                <TableRow>
                    <TableHead>Name</TableHead>
                    <TableHead>Driver</TableHead>
                    <TableHead>Scope</TableHead>
                    <TableHead>Flags</TableHead>
                    <TableHead>Services</TableHead>
                    <TableHead className='text-right'>Created</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {networks.map((net) => (
                    <TableRow key={net.id}>
                        <TableCell className='font-medium'>
                            <div className='flex items-center gap-2'>
                                {net.name}
                                {net.ingress && (
                                    <Badge variant='secondary' className='text-[10px]'>
                                        ingress
                                    </Badge>
                                )}
                            </div>
                        </TableCell>
                        <TableCell>
                            <span className='font-mono text-xs text-muted-foreground'>{net.driver}</span>
                        </TableCell>
                        <TableCell>
                            <span className='text-xs text-muted-foreground'>{net.scope}</span>
                        </TableCell>
                        <TableCell>
                            <div className='flex items-center gap-1'>
                                {net.internal && <Badge variant='secondary'>internal</Badge>}
                                {net.attachable && <Badge variant='secondary'>attachable</Badge>}
                            </div>
                        </TableCell>
                        <TableCell>
                            {net.services.length === 0 ? (
                                <span className='text-xs text-muted-foreground'>-</span>
                            ) : (
                                <div className='flex flex-wrap gap-1'>
                                    {net.services.map((svc) => (
                                        <Badge key={svc.id} variant='default'>
                                            {svc.name}
                                        </Badge>
                                    ))}
                                </div>
                            )}
                        </TableCell>
                        <TableCell className='text-right'>
                            <span className='text-xs text-muted-foreground'>{formatRelativeTime(net.createdAt)}</span>
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
                                <Skeleton className='h-4 w-32' />
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
        return (
            <>
                <PageHeader title='Networks' />
                <LoadingSkeleton />
            </>
        );
    }

    if (error) {
        return (
            <>
                <PageHeader title='Networks' />
                <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
                    <AlertCircle className='size-5' />
                    <p>Failed to load networks</p>
                </div>
            </>
        );
    }

    return (
        <>
            <PageHeader
                title='Networks'
                description={`${networks?.length ?? 0} overlay networks`}
                actions={
                    <div className='relative'>
                        <Search className='absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground' />
                        <Input placeholder='Search networks...' value={search} onChange={(e) => setSearch(e.target.value)} className='h-8 w-56 pl-8 text-sm' />
                    </div>
                }
            />

            <div className='p-6'>
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-muted-foreground'>
                        <Network className='size-5' />
                        <p>{search ? "No networks match your search" : "No networks found"}</p>
                    </div>
                ) : (
                    <Card>
                        <CardContent className='p-0'>
                            <NetworksTable networks={filtered} />
                        </CardContent>
                    </Card>
                )}
            </div>
        </>
    );
}

export const Route = createFileRoute("/_authenticated/networks/")({
    component: NetworksPage,
});

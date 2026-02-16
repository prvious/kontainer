import { createFileRoute, Link } from "@tanstack/react-router";
import { useServices } from "@/hooks/use-services";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { AlertCircle, ArrowUpRight, Container, Layers, Search } from "lucide-react";
import { useState } from "react";
import type { ServiceItem } from "@/types/services";

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

function StatusDot({ service }: { service: ServiceItem }) {
    if (service.replicasDesired === -1) {
        return <span className='size-1.5 rounded-full bg-muted-foreground' title='global mode' />;
    }
    const healthy = service.replicasRunning >= service.replicasDesired && service.replicasDesired > 0;
    return <span className={`size-1.5 rounded-full ${healthy ? "bg-success" : 'animate-pulse-dot bg-destructive'}`} />;
}

function ReplicasBadge({ service }: { service: ServiceItem }) {
    if (service.replicasDesired === -1) {
        return (
            <Badge variant='secondary' className='h-4 px-1.5 text-[10px]'>
                global
            </Badge>
        );
    }
    const healthy = service.replicasRunning === service.replicasDesired && service.replicasDesired > 0;
    return (
        <Badge variant={healthy ? "default" : "destructive"} className='h-4 px-1.5 font-mono text-[10px]'>
            {service.replicasRunning}/{service.replicasDesired}
        </Badge>
    );
}

function ServicesTable({ services }: { services: ServiceItem[] }) {
    return (
        <Table>
            <TableHeader>
                <TableRow className='bg-muted/40 hover:bg-muted/40'>
                    <TableHead className='h-7 w-6 pr-0 pl-3' />
                    <TableHead className='h-7 text-[11px]'>Name</TableHead>
                    <TableHead className='h-7 text-[11px]'>Image</TableHead>
                    <TableHead className='h-7 text-[11px]'>Stack</TableHead>
                    <TableHead className='h-7 text-[11px]'>Replicas</TableHead>
                    <TableHead className='h-7 text-right text-[11px]'>Updated</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {services.map((svc) => (
                    <TableRow key={svc.id} className='group'>
                        <TableCell className='py-1.5 pr-0 pl-3'>
                            <StatusDot service={svc} />
                        </TableCell>
                        <TableCell className='py-1.5'>
                            <Link
                                to='/services/$serviceId'
                                params={{ serviceId: svc.id }}
                                className='inline-flex items-center gap-1 text-xs font-medium hover:underline'
                            >
                                {svc.name}
                                <ArrowUpRight className='size-3 opacity-0 transition-opacity group-hover:opacity-100' />
                            </Link>
                        </TableCell>
                        <TableCell className='py-1.5'>
                            <span className='font-mono text-[11px] text-muted-foreground'>{svc.image}</span>
                        </TableCell>
                        <TableCell className='py-1.5'>
                            {svc.stack ? (
                                <span className='inline-flex items-center gap-1 text-[11px]'>
                                    <Layers className='size-3 text-muted-foreground' />
                                    {svc.stack}
                                </span>
                            ) : (
                                <span className='text-[11px] text-muted-foreground'>-</span>
                            )}
                        </TableCell>
                        <TableCell className='py-1.5'>
                            <ReplicasBadge service={svc} />
                        </TableCell>
                        <TableCell className='py-1.5 text-right'>
                            <span className='font-mono text-[11px] text-muted-foreground'>{formatRelativeTime(svc.updatedAt)}</span>
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
                        <Skeleton className='size-1.5 rounded-full' />
                        <Skeleton className='h-3 w-28' />
                        <Skeleton className='h-3 w-40' />
                        <Skeleton className='h-3 w-16' />
                        <Skeleton className='ml-auto h-4 w-12 rounded-full' />
                    </div>
                ))}
            </div>
        </div>
    );
}

function ServicesPage() {
    const { data: services, isLoading, error } = useServices();
    const [search, setSearch] = useState("");

    const filtered = (services ?? []).filter(
        (svc) =>
            svc.name.toLowerCase().includes(search.toLowerCase()) ||
            svc.image.toLowerCase().includes(search.toLowerCase()) ||
            (svc.stack && svc.stack.toLowerCase().includes(search.toLowerCase())),
    );

    if (isLoading) {
        return <LoadingSkeleton />;
    }

    if (error) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-xs text-destructive'>
                <AlertCircle className='size-4' />
                <p>Failed to load services</p>
            </div>
        );
    }

    return (
        <div className='flex h-full flex-col'>
            <div className='flex h-10 shrink-0 items-center gap-3 border-b border-border px-3'>
                <h1 className='text-xs font-semibold'>Services</h1>
                <span className='rounded-md bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground'>{services?.length ?? 0}</span>
                <div className='flex-1' />
                <div className='relative'>
                    <Search className='absolute top-1/2 left-2 size-3 -translate-y-1/2 text-muted-foreground' />
                    <Input
                        placeholder='Filter services...'
                        value={search}
                        onChange={(e) => setSearch(e.target.value)}
                        className='h-6 w-48 rounded-sm border-border bg-transparent pl-7 text-[11px] focus-visible:ring-1'
                    />
                </div>
            </div>
            <div className='flex-1 overflow-y-auto'>
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                        <Container className='size-4' />
                        <p>{search ? "No services match your filter" : "No services found"}</p>
                    </div>
                ) : (
                    <ServicesTable services={filtered} />
                )}
            </div>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/services/")({
    component: ServicesPage,
});

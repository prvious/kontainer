import { createFileRoute, Link } from "@tanstack/react-router";
import { useServices } from "@/hooks/use-services";
import { PageHeader } from "@/components/page-header";
import { Card, CardContent } from "@/components/ui/card";
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
        return <span className='size-2 rounded-full bg-muted-foreground' title='global mode' />;
    }
    const healthy = service.replicasRunning >= service.replicasDesired && service.replicasDesired > 0;
    return <span className={`size-2 rounded-full ${healthy ? "bg-success" : 'animate-pulse-dot bg-destructive'}`} />;
}

function ReplicasBadge({ service }: { service: ServiceItem }) {
    if (service.replicasDesired === -1) {
        return <Badge variant='secondary'>global</Badge>;
    }
    const healthy = service.replicasRunning === service.replicasDesired && service.replicasDesired > 0;
    return (
        <Badge variant={healthy ? "default" : "destructive"} className='font-mono'>
            {service.replicasRunning}/{service.replicasDesired}
        </Badge>
    );
}

function ServicesTable({ services }: { services: ServiceItem[] }) {
    return (
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead className='w-8' />
                    <TableHead>Name</TableHead>
                    <TableHead>Image</TableHead>
                    <TableHead>Stack</TableHead>
                    <TableHead>Replicas</TableHead>
                    <TableHead className='text-right'>Updated</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {services.map((svc) => (
                    <TableRow key={svc.id} className='group'>
                        <TableCell>
                            <StatusDot service={svc} />
                        </TableCell>
                        <TableCell>
                            <Link
                                to='/services/$serviceId'
                                params={{ serviceId: svc.id }}
                                className='inline-flex items-center gap-1.5 font-medium hover:underline'
                            >
                                {svc.name}
                                <ArrowUpRight className='size-3 opacity-0 transition-opacity group-hover:opacity-100' />
                            </Link>
                        </TableCell>
                        <TableCell>
                            <span className='font-mono text-xs text-muted-foreground'>{svc.image}</span>
                        </TableCell>
                        <TableCell>
                            {svc.stack ? (
                                <span className='inline-flex items-center gap-1.5 text-sm'>
                                    <Layers className='size-3 text-muted-foreground' />
                                    {svc.stack}
                                </span>
                            ) : (
                                <span className='text-sm text-muted-foreground'>-</span>
                            )}
                        </TableCell>
                        <TableCell>
                            <ReplicasBadge service={svc} />
                        </TableCell>
                        <TableCell className='text-right'>
                            <span className='text-xs text-muted-foreground'>{formatRelativeTime(svc.updatedAt)}</span>
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
                                <Skeleton className='size-2 rounded-full' />
                                <Skeleton className='h-4 w-32' />
                                <Skeleton className='h-4 w-48' />
                                <Skeleton className='h-4 w-20' />
                                <Skeleton className='ml-auto h-5 w-12' />
                            </div>
                        ))}
                    </div>
                </CardContent>
            </Card>
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
        return (
            <>
                <PageHeader title='Services' />
                <LoadingSkeleton />
            </>
        );
    }

    if (error) {
        return (
            <>
                <PageHeader title='Services' />
                <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
                    <AlertCircle className='size-5' />
                    <p>Failed to load services</p>
                </div>
            </>
        );
    }

    return (
        <>
            <PageHeader
                title='Services'
                description={`${services?.length ?? 0} services`}
                actions={
                    <div className='relative'>
                        <Search className='absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground' />
                        <Input placeholder='Search services...' value={search} onChange={(e) => setSearch(e.target.value)} className='h-8 w-56 pl-8 text-sm' />
                    </div>
                }
            />

            <div className='p-6'>
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-muted-foreground'>
                        <Container className='size-5' />
                        <p>{search ? "No services match your search" : "No services found"}</p>
                    </div>
                ) : (
                    <Card>
                        <CardContent className='p-0'>
                            <ServicesTable services={filtered} />
                        </CardContent>
                    </Card>
                )}
            </div>
        </>
    );
}

export const Route = createFileRoute("/_authenticated/services/")({
    component: ServicesPage,
});

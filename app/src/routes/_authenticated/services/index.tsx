import { createFileRoute, Link } from "@tanstack/react-router";
import { useServices } from "@/hooks/use-services";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { PageHeader } from "@/components/page-header";
import { TableShell } from "@/components/table-shell";
import { ColumnHeaders } from "@/components/column-headers";
import { AlertCircle, ArrowUpRight, Container, MoreHorizontal } from "lucide-react";
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
            <Badge variant='secondary' className='h-4 px-1.5 font-mono text-[10px]'>
                global
            </Badge>
        );
    }
    const healthy = service.replicasRunning >= service.replicasDesired && service.replicasDesired > 0;
    return (
        <Badge
            className={`h-4 px-1.5 font-mono text-[10px] ${healthy ? 'bg-success/20 text-success hover:bg-success/20' : 'bg-destructive/20 text-destructive hover:bg-destructive/20'}`}
        >
            {service.replicasRunning}/{service.replicasDesired}
        </Badge>
    );
}

const SERVICE_COLUMNS = [
    { label: "STATUS", width: 70 },
    { label: "NAME" },
    { label: "IMAGE", width: 200 },
    { label: "STACK", width: 100 },
    { label: "REPLICAS", width: 80 },
    { label: "UPDATED", width: 100, align: "right" as const },
    { label: "", width: 60 },
];

function ServiceRow({ service }: { service: ServiceItem }) {
    return (
        <div className='flex items-center border-b border-border px-4 py-2.5 hover:bg-bg-elevated/50'>
            <div style={{ width: 70 }} className='flex items-center'>
                <StatusDot service={service} />
            </div>
            <div style={{ flex: 1 }} className='flex items-center'>
                <Link
                    to='/services/$serviceId'
                    params={{ serviceId: service.id }}
                    className='group inline-flex items-center gap-1 font-mono text-xs font-medium hover:underline'
                >
                    {service.name}
                    <ArrowUpRight className='size-3 opacity-0 transition-opacity group-hover:opacity-100' />
                </Link>
            </div>
            <div style={{ width: 200 }}>
                <span className='font-mono text-[11px] text-muted-foreground'>{service.image}</span>
            </div>
            <div style={{ width: 100 }}>
                <span className='font-mono text-[11px] text-muted-foreground'>{service.stack || <span className='opacity-30'>—</span>}</span>
            </div>
            <div style={{ width: 80 }}>
                <ReplicasBadge service={service} />
            </div>
            <div style={{ width: 100 }} className='text-right'>
                <span className='font-mono text-[11px] text-muted-foreground'>{formatRelativeTime(service.updatedAt)}</span>
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
                <Skeleton className='h-7 w-32' />
            </div>
            <div className='flex flex-col overflow-hidden border border-border'>
                <div className='flex items-center justify-between border-b border-border px-4 py-3'>
                    <Skeleton className='h-4 w-32' />
                    <Skeleton className='h-7 w-48' />
                </div>
                <div className='space-y-0'>
                    {[1, 2, 3, 4, 5].map((i) => (
                        <div key={i} className='flex items-center gap-3 border-b border-border px-4 py-2.5 last:border-0'>
                            <Skeleton className='size-1.5 rounded-full' />
                            <Skeleton className='h-3 w-28' />
                            <Skeleton className='h-3 w-40' />
                            <Skeleton className='h-3 w-16' />
                            <Skeleton className='ml-auto h-4 w-12 rounded-full' />
                        </div>
                    ))}
                </div>
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

    const serviceCount = services?.length ?? 0;
    const stackCount = new Set((services ?? []).map((s) => s.stack).filter(Boolean)).size;
    const totalReplicas = (services ?? []).reduce((sum, s) => sum + (s.replicasDesired === -1 ? s.replicasRunning : s.replicasRunning), 0);
    const subtitle = `${serviceCount} ${serviceCount === 1 ? "service" : "services"} · ${stackCount} ${stackCount === 1 ? "stack" : "stacks"} · ${totalReplicas} replicas running`;

    return (
        <div className='flex h-full flex-col gap-6 p-6'>
            <PageHeader title='SERVICES' subtitle={subtitle}>
                <Button variant='outline' size='sm' disabled className='h-7 font-mono text-[11px] text-muted-foreground'>
                    CREATE SERVICE
                </Button>
            </PageHeader>

            <TableShell
                title='SWARM SERVICES'
                count={filtered.length}
                searchValue={search}
                onSearchChange={setSearch}
                searchPlaceholder='Search services...'
                filterSlot={
                    <Button variant='outline' size='sm' disabled className='h-7 font-mono text-[11px] text-muted-foreground'>
                        FILTER
                    </Button>
                }
            >
                <ColumnHeaders columns={SERVICE_COLUMNS} />
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                        <Container className='size-4' />
                        <p>{search ? "No services match your filter" : "No services found"}</p>
                    </div>
                ) : (
                    <div>
                        {filtered.map((svc) => (
                            <ServiceRow key={svc.id} service={svc} />
                        ))}
                    </div>
                )}
            </TableShell>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/services/")({
    component: ServicesPage,
});

import { Link } from "@tanstack/react-router";
import { useServices } from "@/hooks/use-services";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { AlertCircle, Container, Layers } from "lucide-react";
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

function ReplicasBadge({ service }: { service: ServiceItem }) {
    if (service.replicasDesired === -1) {
        return <Badge variant='secondary'>global</Badge>;
    }

    const healthy = service.replicasRunning === service.replicasDesired && service.replicasDesired > 0;

    return (
        <Badge variant={healthy ? "default" : "destructive"}>
            {service.replicasRunning}/{service.replicasDesired}
        </Badge>
    );
}

function ServiceRow({ service }: { service: ServiceItem }) {
    return (
        <Link
            to='/services/$serviceId'
            params={{ serviceId: service.id }}
            className='-mx-2 flex items-center justify-between gap-4 rounded-md px-2 py-2 transition-colors hover:bg-muted/50'
        >
            <div className='flex items-center gap-3 overflow-hidden'>
                <Container className='size-4 shrink-0 text-muted-foreground' />
                <div className='min-w-0'>
                    <p className='truncate text-sm font-medium'>{service.name}</p>
                    <p className='truncate text-xs text-muted-foreground'>{service.image}</p>
                </div>
            </div>
            <div className='flex shrink-0 items-center gap-3'>
                <span className='text-xs text-muted-foreground'>{formatRelativeTime(service.updatedAt)}</span>
                <ReplicasBadge service={service} />
            </div>
        </Link>
    );
}

function groupByStack(services: ServiceItem[]): Record<string, ServiceItem[]> {
    const groups: Record<string, ServiceItem[]> = {};
    for (const svc of services) {
        const key = svc.stack || "Standalone";
        if (!groups[key]) groups[key] = [];
        groups[key].push(svc);
    }
    return groups;
}

function LoadingSkeleton() {
    return (
        <div className='space-y-4'>
            {[1, 2].map((i) => (
                <Card key={i}>
                    <CardHeader>
                        <Skeleton className='h-5 w-32' />
                    </CardHeader>
                    <CardContent>
                        <div className='space-y-3'>
                            {[1, 2, 3].map((j) => (
                                <div key={j} className='flex items-center justify-between'>
                                    <div className='flex items-center gap-3'>
                                        <Skeleton className='size-4 rounded' />
                                        <div className='space-y-1'>
                                            <Skeleton className='h-4 w-40' />
                                            <Skeleton className='h-3 w-56' />
                                        </div>
                                    </div>
                                    <Skeleton className='h-5 w-12 rounded-full' />
                                </div>
                            ))}
                        </div>
                    </CardContent>
                </Card>
            ))}
        </div>
    );
}

export function ServicesList() {
    const { data: services, isLoading, error } = useServices();

    if (isLoading) {
        return <LoadingSkeleton />;
    }

    if (error) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
                <AlertCircle className='size-5' />
                <p>Failed to load services</p>
            </div>
        );
    }

    if (!services || services.length === 0) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-muted-foreground'>
                <Container className='size-5' />
                <p>No services found</p>
            </div>
        );
    }

    const groups = groupByStack(services);
    const sortedKeys = Object.keys(groups).sort((a, b) => {
        if (a === "Standalone") return 1;
        if (b === "Standalone") return -1;
        return a.localeCompare(b);
    });

    return (
        <div className='space-y-4'>
            {sortedKeys.map((stack) => (
                <Card key={stack}>
                    <CardHeader>
                        <CardTitle className='flex items-center gap-2'>
                            <Layers className='size-4' />
                            {stack}
                            <Badge variant='secondary'>{groups[stack].length}</Badge>
                        </CardTitle>
                    </CardHeader>
                    <CardContent>
                        <div className='divide-y'>
                            {groups[stack].map((svc) => (
                                <ServiceRow key={svc.id} service={svc} />
                            ))}
                        </div>
                    </CardContent>
                </Card>
            ))}
        </div>
    );
}

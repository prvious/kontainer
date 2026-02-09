import { createFileRoute, Link } from "@tanstack/react-router";
import { useServices } from "@/hooks/use-services";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { AlertCircle, ArrowLeft, ArrowUpRight, Container } from "lucide-react";
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

function StackServicesTable({ services }: { services: ServiceItem[] }) {
    return (
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead className='w-8' />
                    <TableHead>Name</TableHead>
                    <TableHead>Image</TableHead>
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
            <div className='mb-6'>
                <Skeleton className='h-4 w-20' />
                <Skeleton className='mt-3 h-6 w-48' />
                <Skeleton className='mt-2 h-4 w-32' />
            </div>
            <Card>
                <CardContent className='p-0'>
                    <div className='space-y-0'>
                        {[1, 2, 3].map((i) => (
                            <div key={i} className='flex items-center gap-4 border-b px-4 py-3 last:border-0'>
                                <Skeleton className='size-2 rounded-full' />
                                <Skeleton className='h-4 w-32' />
                                <Skeleton className='h-4 w-48' />
                                <Skeleton className='ml-auto h-5 w-12' />
                            </div>
                        ))}
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}

function StackDetailPage() {
    const { stackName } = Route.useParams();
    const { data: services, isLoading, error } = useServices(stackName);

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

    const totalRunning = (services ?? []).reduce((sum, s) => sum + s.replicasRunning, 0);
    const totalDesired = (services ?? []).reduce((sum, s) => sum + (s.replicasDesired === -1 ? s.replicasRunning : s.replicasDesired), 0);

    return (
        <div className='p-6'>
            <div className='mb-6'>
                <Link to='/stacks' className='inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground'>
                    <ArrowLeft className='size-3.5' />
                    Stacks
                </Link>
                <h1 className='mt-2 text-xl font-semibold tracking-tight'>{stackName}</h1>
                <p className='mt-1 text-sm text-muted-foreground'>
                    {services?.length ?? 0} services &middot; {totalRunning}/{totalDesired} replicas
                </p>
            </div>
            {(services ?? []).length === 0 ? (
                <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-muted-foreground'>
                    <Container className='size-5' />
                    <p>No services in this stack</p>
                </div>
            ) : (
                <Card>
                    <CardContent className='p-0'>
                        <StackServicesTable services={services!} />
                    </CardContent>
                </Card>
            )}
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/stacks/$stackName")({
    component: StackDetailPage,
});

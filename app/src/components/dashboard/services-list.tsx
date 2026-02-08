import { Link } from "@tanstack/react-router";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ArrowRight, CheckCircle, Clock, Layers, Loader2, RefreshCw, TriangleAlert } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { ServiceItem } from "@/types/services";

interface ServicesListProps {
    services: ServiceItem[];
}

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

function getServiceStatus(service: ServiceItem) {
    if (service.replicasDesired === -1) {
        return { icon: RefreshCw, color: "text-chart-2", label: "global" };
    }
    if (service.replicasRunning >= service.replicasDesired && service.replicasDesired > 0) {
        return { icon: CheckCircle, color: "text-primary", label: "running" };
    }
    if (service.replicasRunning > 0) {
        return { icon: TriangleAlert, color: "text-warning", label: "partial" };
    }
    return { icon: Clock, color: "text-muted-foreground", label: "pending" };
}

export function ServicesList({ services }: ServicesListProps) {
    const sorted = [...services].sort((a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime()).slice(0, 8);

    return (
        <Card className='animate-fade-in-up' style={{ animationDelay: "350ms" }}>
            <CardHeader className='pb-3'>
                <div className='flex items-center justify-between'>
                    <div className='flex items-center gap-2'>
                        <h3 className='text-sm font-medium'>Swarm Services</h3>
                        <span className='rounded-full bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground'>{services.length}</span>
                    </div>
                    <Link to='/services'>
                        <Button variant='ghost' size='sm' className='h-auto gap-1 px-2 py-1 text-xs text-muted-foreground'>
                            View all <ArrowRight className='size-3' />
                        </Button>
                    </Link>
                </div>
            </CardHeader>
            <CardContent className='p-0'>
                {sorted.length === 0 ? (
                    <div className='py-8 text-center text-sm text-muted-foreground'>No services found</div>
                ) : (
                    <div className='divide-y divide-border'>
                        {sorted.map((svc) => {
                            const status = getServiceStatus(svc);
                            const StatusIcon = status.icon;

                            return (
                                <Link
                                    key={svc.id}
                                    to='/services/$serviceId'
                                    params={{ serviceId: svc.id }}
                                    className='flex items-center gap-3 px-4 py-2.5 transition-colors hover:bg-accent/50'
                                >
                                    <StatusIcon className={`size-4 shrink-0 ${status.color}`} />
                                    <div className='flex-1 overflow-hidden'>
                                        <div className='flex items-center gap-2'>
                                            <span className='truncate text-sm font-medium'>{svc.name}</span>
                                            {svc.stack && (
                                                <Badge variant='secondary' className='gap-1 text-[10px]'>
                                                    <Layers className='size-2.5' />
                                                    {svc.stack}
                                                </Badge>
                                            )}
                                        </div>
                                        <p className='truncate font-mono text-[11px] text-muted-foreground'>{svc.image}</p>
                                    </div>
                                    <div className='flex shrink-0 items-center gap-3'>
                                        {svc.replicasDesired === -1 ? (
                                            <div className='flex items-center gap-1'>
                                                <Loader2 className='size-3 text-muted-foreground' />
                                                <span className='text-xs text-muted-foreground'>global</span>
                                            </div>
                                        ) : (
                                            <span className='font-mono text-xs text-muted-foreground'>
                                                {svc.replicasRunning}/{svc.replicasDesired}
                                            </span>
                                        )}
                                        <span className='text-[11px] text-muted-foreground'>{formatRelativeTime(svc.updatedAt)}</span>
                                    </div>
                                </Link>
                            );
                        })}
                    </div>
                )}
            </CardContent>
        </Card>
    );
}

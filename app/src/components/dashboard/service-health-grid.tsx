import { Link } from "@tanstack/react-router";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ArrowRight } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { ServiceItem } from "@/types/services";

interface ServiceHealthGridProps {
    services: ServiceItem[];
}

function getStatusColor(service: ServiceItem): string {
    if (service.replicasDesired === -1) {
        // global mode — always healthy
        return "bg-primary";
    }
    if (service.replicasRunning >= service.replicasDesired && service.replicasDesired > 0) {
        return "bg-primary";
    }
    if (service.replicasRunning === 0) {
        return "bg-destructive";
    }
    return "bg-warning";
}

function getReplicaLabel(service: ServiceItem): string {
    if (service.replicasDesired === -1) {
        return "global";
    }
    return `${service.replicasRunning}/${service.replicasDesired}`;
}

export function ServiceHealthGrid({ services }: ServiceHealthGridProps) {
    // Sort: unhealthy first, then alphabetical
    const sorted = [...services].sort((a, b) => {
        const aHealthy = a.replicasDesired === -1 || (a.replicasRunning >= a.replicasDesired && a.replicasDesired > 0);
        const bHealthy = b.replicasDesired === -1 || (b.replicasRunning >= b.replicasDesired && b.replicasDesired > 0);
        if (aHealthy !== bHealthy) return aHealthy ? 1 : -1;
        return a.name.localeCompare(b.name);
    });

    return (
        <Card className='animate-fade-in-up' style={{ animationDelay: "350ms" }}>
            <CardHeader className='pb-3'>
                <div className='flex items-center justify-between'>
                    <div className='flex items-center gap-2'>
                        <h3 className='text-sm font-medium'>Service Health</h3>
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
                        {sorted.map((svc) => (
                            <Link
                                key={svc.id}
                                to='/services/$serviceId'
                                params={{ serviceId: svc.id }}
                                className='flex items-center gap-3 px-4 py-2 transition-colors hover:bg-accent/50'
                            >
                                <span className={`size-2 shrink-0 rounded-full ${getStatusColor(svc)}`} />
                                <span className='min-w-0 flex-1 truncate text-sm'>{svc.name}</span>
                                <Badge variant='secondary' className='shrink-0 font-mono text-[10px]'>
                                    {getReplicaLabel(svc)}
                                </Badge>
                            </Link>
                        ))}
                    </div>
                )}
            </CardContent>
        </Card>
    );
}

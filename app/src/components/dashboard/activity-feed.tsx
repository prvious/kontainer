import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { ArrowRight, ArrowUpCircle, AlertCircle, Plus, RefreshCw, Server, Trash2, Zap } from "lucide-react";

interface ActivityEvent {
    id: string;
    type: "service_update" | "error" | "task_restart" | "scale" | "node_join" | "service_create" | "service_remove";
    target: string;
    message: string;
    time: string;
}

const eventConfig: Record<ActivityEvent["type"], { icon: React.ComponentType<{ className?: string }>; color: string }> = {
    service_update: { icon: ArrowUpCircle, color: "text-chart-2 bg-chart-2/10" },
    error: { icon: AlertCircle, color: "text-destructive bg-destructive/10" },
    task_restart: { icon: RefreshCw, color: "text-warning bg-warning/10" },
    scale: { icon: Zap, color: "text-primary bg-primary/10" },
    node_join: { icon: Server, color: "text-primary bg-primary/10" },
    service_create: { icon: Plus, color: "text-primary bg-primary/10" },
    service_remove: { icon: Trash2, color: "text-muted-foreground bg-muted" },
};

// Placeholder events - this feature doesn't have backend support yet
const placeholderEvents: ActivityEvent[] = [
    { id: "1", type: "service_update", target: "api-gateway", message: "Image updated to v2.4.1", time: "2m ago" },
    { id: "2", type: "scale", target: "worker-service", message: "Scaled from 3 to 5 replicas", time: "8m ago" },
    { id: "3", type: "error", target: "redis-cache", message: "Task failed: OOM killed", time: "15m ago" },
    { id: "4", type: "task_restart", target: "nginx-proxy", message: "Task restarted automatically", time: "22m ago" },
    { id: "5", type: "node_join", target: "worker-03", message: "Node joined the cluster", time: "45m ago" },
    { id: "6", type: "service_create", target: "monitoring-stack", message: "Stack deployed with 4 services", time: "1h ago" },
    { id: "7", type: "service_update", target: "auth-service", message: "Environment variables updated", time: "2h ago" },
    { id: "8", type: "service_remove", target: "legacy-api", message: "Service removed from cluster", time: "3h ago" },
];

export function ActivityFeed() {
    return (
        <Card className='animate-fade-in-up' style={{ animationDelay: "400ms" }}>
            <CardHeader className='pb-3'>
                <div className='flex items-center justify-between'>
                    <h3 className='text-sm font-medium'>Cluster Activity</h3>
                    <Button variant='ghost' size='sm' className='h-auto gap-1 px-2 py-1 text-xs text-muted-foreground'>
                        View all <ArrowRight className='size-3' />
                    </Button>
                </div>
            </CardHeader>
            <CardContent className='p-0'>
                <div className='divide-y divide-border'>
                    {placeholderEvents.map((event) => {
                        const config = eventConfig[event.type];
                        const Icon = config.icon;

                        return (
                            <div key={event.id} className='flex items-start gap-3 px-4 py-2.5'>
                                <div className={`mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-full ${config.color}`}>
                                    <Icon className='size-3' />
                                </div>
                                <div className='flex-1 overflow-hidden'>
                                    <div className='flex items-center gap-2'>
                                        <span className='truncate font-mono text-sm font-medium'>{event.target}</span>
                                        <span className='shrink-0 text-[11px] text-muted-foreground'>{event.time}</span>
                                    </div>
                                    <p className='truncate text-xs text-muted-foreground'>{event.message}</p>
                                </div>
                            </div>
                        );
                    })}
                </div>
            </CardContent>
        </Card>
    );
}

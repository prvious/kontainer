import { createFileRoute, Link } from "@tanstack/react-router";
import { useServices } from "@/hooks/use-services";
import { useNodes } from "@/hooks/use-nodes";
import { useTaskStats } from "@/hooks/use-task-stats";
import { PageHeader } from "@/components/page-header";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { Progress } from "@/components/ui/progress";
import { Separator } from "@/components/ui/separator";
import { AlertTriangle, ArrowRight, CheckCircle, Container, Layers, RefreshCw, Server, XCircle } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { ServiceItem } from "@/types/services";
import type { NodeItem } from "@/types/nodes";
import type { TaskStats } from "@/types/task-stats";

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

function computeServiceStats(services: ServiceItem[]) {
    const total = services.length;
    const running = services.filter((s) => s.replicasRunning > 0).length;
    const stacks = new Set(services.map((s) => s.stack).filter(Boolean)).size;
    const healthPct = total > 0 ? Math.round((running / total) * 100) : 0;
    return { total, running, stacks, healthPct };
}

function computeNodeStats(nodes: NodeItem[]) {
    const total = nodes.length;
    const managers = nodes.filter((n) => n.role === "manager").length;
    const workers = nodes.filter((n) => n.role === "worker").length;
    const down = nodes.filter((n) => n.status === "down" || n.status === "disconnected").length;
    const ready = nodes.filter((n) => n.status === "ready").length;
    const healthPct = total > 0 ? Math.round((ready / total) * 100) : 0;
    return { total, managers, workers, down, ready, healthPct };
}

function StatCard({
    label,
    value,
    icon: Icon,
    accent,
    delay,
}: {
    label: string;
    value: string | number;
    icon: React.ComponentType<{ className?: string }>;
    accent?: string;
    delay: number;
}) {
    return (
        <Card className='animate-fade-in-up' style={{ animationDelay: `${delay}ms` }}>
            <CardContent className='flex items-center gap-4 p-4'>
                <div className={`flex size-10 items-center justify-center rounded-lg ${accent ?? "bg-muted"}`}>
                    <Icon className='size-5' />
                </div>
                <div>
                    <p className='text-2xl font-semibold tracking-tight'>{value}</p>
                    <p className='text-xs text-muted-foreground'>{label}</p>
                </div>
            </CardContent>
        </Card>
    );
}

function ClusterHealthCard({ services, nodes, taskStats }: { services: ServiceItem[]; nodes: NodeItem[]; taskStats: TaskStats }) {
    const svcStats = computeServiceStats(services);
    const nodeStats = computeNodeStats(nodes);
    const taskHealthPct = taskStats.desiredRunning > 0 ? Math.round((taskStats.running / taskStats.desiredRunning) * 100) : 0;

    const composite = Math.round((svcStats.healthPct + nodeStats.healthPct + taskHealthPct) / 3);

    return (
        <Card className='animate-fade-in-up' style={{ animationDelay: "200ms" }}>
            <CardHeader className='pb-3'>
                <CardTitle className='flex items-center justify-between text-sm'>
                    Cluster Health
                    <span className='font-mono text-xs text-muted-foreground'>{composite}%</span>
                </CardTitle>
            </CardHeader>
            <CardContent className='space-y-3'>
                <Progress value={composite} className='h-2' />
                <p className='text-xs text-muted-foreground'>
                    {svcStats.running}/{svcStats.total} services &middot; {nodeStats.ready}/{nodeStats.total} nodes &middot; {taskStats.running}/
                    {taskStats.desiredRunning} tasks
                </p>
            </CardContent>
        </Card>
    );
}

function NodesOverviewCard({ nodes }: { nodes: NodeItem[] }) {
    const stats = computeNodeStats(nodes);

    return (
        <Card className='animate-fade-in-up' style={{ animationDelay: "250ms" }}>
            <CardHeader className='pb-3'>
                <CardTitle className='flex items-center justify-between text-sm'>
                    Nodes Overview
                    <Link to='/nodes'>
                        <Button variant='ghost' size='sm' className='h-auto gap-1 px-2 py-1 text-xs text-muted-foreground'>
                            View all <ArrowRight className='size-3' />
                        </Button>
                    </Link>
                </CardTitle>
            </CardHeader>
            <CardContent className='space-y-3'>
                <div className='grid grid-cols-3 gap-4 text-center'>
                    <div>
                        <p className='text-lg font-semibold'>{stats.managers}</p>
                        <p className='text-xs text-muted-foreground'>Managers</p>
                    </div>
                    <div>
                        <p className='text-lg font-semibold'>{stats.workers}</p>
                        <p className='text-xs text-muted-foreground'>Workers</p>
                    </div>
                    <div>
                        <p className={`text-lg font-semibold ${stats.down > 0 ? "text-destructive" : ""}`}>{stats.down}</p>
                        <p className='text-xs text-muted-foreground'>Down</p>
                    </div>
                </div>
                <Progress value={stats.healthPct} className='h-2' />
            </CardContent>
        </Card>
    );
}

function StackBreakdown({ services }: { services: ServiceItem[] }) {
    const groups: Record<string, ServiceItem[]> = {};
    for (const svc of services) {
        const key = svc.stack || "Standalone";
        if (!groups[key]) groups[key] = [];
        groups[key].push(svc);
    }

    const sortedKeys = Object.keys(groups).sort((a, b) => {
        if (a === "Standalone") return 1;
        if (b === "Standalone") return -1;
        return a.localeCompare(b);
    });

    return (
        <Card className='animate-fade-in-up' style={{ animationDelay: "300ms" }}>
            <CardHeader className='pb-3'>
                <CardTitle className='flex items-center justify-between text-sm'>
                    Stacks
                    <Link to='/services'>
                        <Button variant='ghost' size='sm' className='h-auto gap-1 px-2 py-1 text-xs text-muted-foreground'>
                            View all <ArrowRight className='size-3' />
                        </Button>
                    </Link>
                </CardTitle>
            </CardHeader>
            <CardContent>
                <div className='space-y-1'>
                    {sortedKeys.map((stack) => {
                        const items = groups[stack];
                        const healthy = items.filter((s) => s.replicasDesired === -1 || s.replicasRunning >= s.replicasDesired).length;
                        const allHealthy = healthy === items.length;

                        return (
                            <div key={stack} className='flex items-center justify-between rounded-md px-2 py-2 text-sm transition-colors hover:bg-muted/50'>
                                <div className='flex items-center gap-2.5'>
                                    <Layers className='size-3.5 text-muted-foreground' />
                                    <span className='font-medium'>{stack}</span>
                                </div>
                                <div className='flex items-center gap-2'>
                                    <span className='font-mono text-xs text-muted-foreground'>
                                        {healthy}/{items.length}
                                    </span>
                                    <span className={`size-1.5 rounded-full ${allHealthy ? "bg-success" : 'animate-pulse-dot bg-destructive'}`} />
                                </div>
                            </div>
                        );
                    })}
                </div>
            </CardContent>
        </Card>
    );
}

function RecentServices({ services }: { services: ServiceItem[] }) {
    const recent = [...services].sort((a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime()).slice(0, 6);

    return (
        <Card className='animate-fade-in-up' style={{ animationDelay: "400ms" }}>
            <CardHeader className='pb-3'>
                <CardTitle className='flex items-center justify-between text-sm'>
                    Recent Activity
                    <Link to='/services'>
                        <Button variant='ghost' size='sm' className='h-auto gap-1 px-2 py-1 text-xs text-muted-foreground'>
                            All services <ArrowRight className='size-3' />
                        </Button>
                    </Link>
                </CardTitle>
            </CardHeader>
            <CardContent>
                <div className='space-y-0.5'>
                    {recent.map((svc) => {
                        const healthy = svc.replicasDesired === -1 || svc.replicasRunning >= svc.replicasDesired;
                        return (
                            <Link
                                key={svc.id}
                                to='/services/$serviceId'
                                params={{ serviceId: svc.id }}
                                className='flex items-center justify-between rounded-md px-2 py-2 transition-colors hover:bg-muted/50'
                            >
                                <div className='flex items-center gap-2.5 overflow-hidden'>
                                    <span className={`size-1.5 shrink-0 rounded-full ${healthy ? "bg-success" : 'animate-pulse-dot bg-destructive'}`} />
                                    <span className='truncate text-sm font-medium'>{svc.name}</span>
                                </div>
                                <div className='flex shrink-0 items-center gap-3'>
                                    <span className='font-mono text-xs text-muted-foreground'>{formatRelativeTime(svc.updatedAt)}</span>
                                    {svc.replicasDesired === -1 ? (
                                        <Badge variant='secondary' className='text-[10px]'>
                                            global
                                        </Badge>
                                    ) : (
                                        <Badge variant={healthy ? "default" : "destructive"} className='font-mono text-[10px]'>
                                            {svc.replicasRunning}/{svc.replicasDesired}
                                        </Badge>
                                    )}
                                </div>
                            </Link>
                        );
                    })}
                </div>
            </CardContent>
        </Card>
    );
}

function LoadingSkeleton() {
    return (
        <div className='space-y-6 p-6'>
            <div className='grid gap-4 sm:grid-cols-2 lg:grid-cols-4'>
                {[1, 2, 3, 4].map((i) => (
                    <Card key={i}>
                        <CardContent className='flex items-center gap-4 p-4'>
                            <Skeleton className='size-10 rounded-lg' />
                            <div className='space-y-1.5'>
                                <Skeleton className='h-6 w-12' />
                                <Skeleton className='h-3 w-20' />
                            </div>
                        </CardContent>
                    </Card>
                ))}
            </div>
            <div className='grid gap-4 lg:grid-cols-2'>
                <Card>
                    <CardHeader>
                        <Skeleton className='h-4 w-32' />
                    </CardHeader>
                    <CardContent>
                        <Skeleton className='h-2 w-full' />
                    </CardContent>
                </Card>
                <Card>
                    <CardHeader>
                        <Skeleton className='h-4 w-24' />
                    </CardHeader>
                    <CardContent className='space-y-3'>
                        {[1, 2, 3].map((i) => (
                            <Skeleton key={i} className='h-8 w-full' />
                        ))}
                    </CardContent>
                </Card>
            </div>
        </div>
    );
}

function DashboardPage() {
    const { data: services, isLoading: servicesLoading, error: servicesError, refetch: refetchServices, dataUpdatedAt } = useServices();
    const { data: nodes, isLoading: nodesLoading, error: nodesError } = useNodes();
    const { data: taskStats, isLoading: taskStatsLoading, error: taskStatsError } = useTaskStats();

    const isLoading = servicesLoading || nodesLoading || taskStatsLoading;
    const error = servicesError || nodesError || taskStatsError;

    if (isLoading) return <LoadingSkeleton />;

    if (error) {
        return (
            <>
                <PageHeader title='Overview' />
                <div className='flex flex-1 flex-col items-center justify-center gap-3 text-sm text-muted-foreground'>
                    <AlertTriangle className='size-6 text-destructive' />
                    <p>Failed to connect to agent</p>
                    <Button variant='outline' size='sm' onClick={() => refetchServices()}>
                        <RefreshCw className='size-3.5' /> Retry
                    </Button>
                </div>
            </>
        );
    }

    const serviceItems = services ?? [];
    const nodeItems = nodes ?? [];
    const stats = taskStats ?? { total: 0, running: 0, failed: 0, preparing: 0, complete: 0, desiredRunning: 0 };
    const svcStats = computeServiceStats(serviceItems);
    const nStats = computeNodeStats(nodeItems);
    const lastUpdate = dataUpdatedAt ? new Date(dataUpdatedAt).toLocaleTimeString() : "";

    return (
        <>
            <PageHeader
                title='Overview'
                description={`${svcStats.total} services \u00b7 ${nStats.total} nodes \u00b7 ${stats.running} tasks running`}
                actions={
                    <div className='flex items-center gap-3'>
                        {lastUpdate && (
                            <span className='flex items-center gap-1.5 text-xs text-muted-foreground'>
                                <span className='animate-pulse-dot size-1.5 rounded-full bg-success' />
                                Live
                            </span>
                        )}
                    </div>
                }
            />

            <div className='space-y-6 p-6'>
                {/* Row 1 — Stat Cards */}
                <div className='grid gap-4 sm:grid-cols-2 lg:grid-cols-4'>
                    <StatCard label='Total Services' value={svcStats.total} icon={Container} accent='bg-muted' delay={0} />
                    <StatCard label='Total Nodes' value={nStats.total} icon={Server} accent='bg-muted' delay={50} />
                    <StatCard label='Running Tasks' value={stats.running} icon={CheckCircle} accent='bg-success/10 text-success' delay={100} />
                    <StatCard
                        label='Failed Tasks'
                        value={stats.failed}
                        icon={XCircle}
                        accent={stats.failed > 0 ? "bg-destructive/10 text-destructive" : "bg-muted"}
                        delay={150}
                    />
                </div>

                <Separator />

                {/* Row 2 — Cluster Health + Nodes Overview */}
                <div className='grid gap-4 lg:grid-cols-2'>
                    <ClusterHealthCard services={serviceItems} nodes={nodeItems} taskStats={stats} />
                    <NodesOverviewCard nodes={nodeItems} />
                </div>

                {/* Row 3 — Stacks + Recent Activity */}
                <div className='grid gap-4 lg:grid-cols-2'>
                    <StackBreakdown services={serviceItems} />
                    <RecentServices services={serviceItems} />
                </div>
            </div>
        </>
    );
}

export const Route = createFileRoute("/_authenticated/")({
    component: DashboardPage,
});

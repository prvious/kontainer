import { createFileRoute } from "@tanstack/react-router";
import { useServices } from "@/hooks/use-services";
import { useNodes } from "@/hooks/use-nodes";
import { useTaskStats } from "@/hooks/use-task-stats";
import { useClusterMetrics } from "@/hooks/use-cluster-metrics";
import { StatsCards } from "@/components/dashboard/stats-cards";
import { ResourceCharts } from "@/components/dashboard/resource-charts";
import { ContainerTable } from "@/components/dashboard/container-table";
import { ServicesList } from "@/components/dashboard/services-list";
import { ActivityFeed } from "@/components/dashboard/activity-feed";
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { AlertTriangle, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";

function LoadingSkeleton() {
    return (
        <div className='space-y-6 p-6'>
            <div className='space-y-1'>
                <Skeleton className='h-7 w-56' />
                <Skeleton className='h-4 w-96' />
            </div>
            <div className='grid gap-4 sm:grid-cols-2 lg:grid-cols-4'>
                {[1, 2, 3, 4].map((i) => (
                    <Card key={i}>
                        <CardContent className='p-4'>
                            <Skeleton className='mb-2 h-3 w-20' />
                            <Skeleton className='h-8 w-16' />
                            <Skeleton className='mt-2 h-3 w-32' />
                        </CardContent>
                    </Card>
                ))}
            </div>
            <div className='grid gap-4 lg:grid-cols-2'>
                <Card>
                    <CardContent className='p-6'>
                        <Skeleton className='h-[200px] w-full' />
                    </CardContent>
                </Card>
                <Card>
                    <CardContent className='p-6'>
                        <Skeleton className='h-[200px] w-full' />
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
    const { data: metricsData } = useClusterMetrics();

    const isLoading = servicesLoading || nodesLoading || taskStatsLoading;
    const error = servicesError || nodesError || taskStatsError;

    if (isLoading) return <LoadingSkeleton />;

    if (error) {
        return (
            <div className='flex flex-1 flex-col items-center justify-center gap-3 text-sm text-muted-foreground'>
                <AlertTriangle className='size-6 text-destructive' />
                <p>Failed to connect to agent</p>
                <Button variant='outline' size='sm' onClick={() => refetchServices()}>
                    <RefreshCw className='size-3.5' /> Retry
                </Button>
            </div>
        );
    }

    const serviceItems = services ?? [];
    const nodeItems = nodes ?? [];
    const stats = taskStats ?? { total: 0, running: 0, failed: 0, preparing: 0, complete: 0, desiredRunning: 0 };
    const agents = metricsData?.agents ?? [];
    const history = metricsData?.history ?? [];
    const lastUpdate = dataUpdatedAt ? new Date(dataUpdatedAt).toLocaleTimeString() : "";

    return (
        <div className='space-y-6 p-6'>
            {/* Page Title */}
            <div>
                <div className='flex items-center gap-3'>
                    <h1 className='text-xl font-semibold tracking-tight'>Infrastructure Overview</h1>
                    {lastUpdate && (
                        <span className='flex items-center gap-1.5 text-xs text-muted-foreground'>
                            <span className='animate-pulse-dot size-1.5 rounded-full bg-primary' />
                            Live
                        </span>
                    )}
                </div>
                <p className='mt-1 text-sm text-muted-foreground'>
                    {nodeItems.length} node{nodeItems.length !== 1 ? "s" : ""} &middot; {serviceItems.length} service{serviceItems.length !== 1 ? "s" : ""}{" "}
                    &middot; {stats.running} task{stats.running !== 1 ? "s" : ""} running
                </p>
            </div>

            {/* Stats Cards */}
            <StatsCards nodes={nodeItems} agents={agents} />

            {/* Resource Charts */}
            <ResourceCharts agents={agents} history={history} />

            {/* Container Table */}
            <ContainerTable agents={agents} taskStats={taskStats ?? null} />

            {/* Services + Activity Feed */}
            <div className='grid gap-4 lg:grid-cols-5'>
                <div className='lg:col-span-3'>
                    <ServicesList services={serviceItems} />
                </div>
                <div className='lg:col-span-2'>
                    <ActivityFeed />
                </div>
            </div>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/")({
    component: DashboardPage,
});

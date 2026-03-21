import { useState } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { useQueryClient } from "@tanstack/react-query";
import { useTasks } from "@/hooks/use-tasks";
import { useTaskStats } from "@/hooks/use-task-stats";
import { useClusterMetrics } from "@/hooks/use-cluster-metrics";
import { useNodes } from "@/hooks/use-nodes";
import { useAuth } from "@/hooks/use-auth";
import { MetricCard } from "@/components/dashboard/metric-card";
import { TraefikBanner } from "@/components/dashboard/traefik-banner";
import { ContainersTable } from "@/components/dashboard/containers-table";
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { AlertTriangle, ArrowUpDown, Box, Cpu, MemoryStick, Plus, RefreshCw, Search, SlidersHorizontal } from "lucide-react";
import type { ContainerMetrics } from "@/types/metrics";

function LoadingSkeleton() {
    return (
        <div className='space-y-6 px-8 py-6'>
            <div className='space-y-1'>
                <Skeleton className='h-7 w-56' />
                <Skeleton className='h-4 w-96' />
            </div>
            <div className='flex gap-4'>
                {[1, 2, 3, 4].map((i) => (
                    <Card key={i} className='flex-1'>
                        <CardContent className='p-4'>
                            <Skeleton className='mb-2 h-3 w-20' />
                            <Skeleton className='h-8 w-16' />
                            <Skeleton className='mt-2 h-3 w-32' />
                        </CardContent>
                    </Card>
                ))}
            </div>
            <Skeleton className='h-10 w-full' />
            <Card>
                <CardContent className='p-6'>
                    <Skeleton className='h-[300px] w-full' />
                </CardContent>
            </Card>
        </div>
    );
}

function DashboardPage() {
    const [search, setSearch] = useState("");
    const queryClient = useQueryClient();
    const { profiles, activeProfileId } = useAuth();
    const clusterName = profiles.find((p) => p.id === activeProfileId)?.label ?? "Cluster";

    const { data: tasks, isLoading: tasksLoading, error: tasksError } = useTasks();
    const { data: taskStats, isLoading: taskStatsLoading, error: taskStatsError } = useTaskStats();
    const { data: metricsData, isLoading: metricsLoading, error: metricsError } = useClusterMetrics();
    const { data: nodes, isLoading: nodesLoading, error: nodesError } = useNodes();

    const isLoading = tasksLoading || taskStatsLoading || nodesLoading || metricsLoading;
    const error = tasksError || taskStatsError || nodesError || metricsError;

    if (isLoading) return <LoadingSkeleton />;

    if (error) {
        return (
            <div className='flex flex-1 flex-col items-center justify-center gap-3 text-sm text-muted-foreground'>
                <AlertTriangle className='size-6 text-destructive' />
                <p>Failed to connect to agent</p>
                <Button
                    variant='outline'
                    size='sm'
                    onClick={() => {
                        queryClient.invalidateQueries({ queryKey: ["tasks"] });
                        queryClient.invalidateQueries({ queryKey: ["taskStats"] });
                        queryClient.invalidateQueries({ queryKey: ["nodes"] });
                        queryClient.invalidateQueries({ queryKey: ["cluster-metrics"] });
                    }}
                >
                    <RefreshCw className='size-3.5' /> Retry
                </Button>
            </div>
        );
    }

    const taskItems = tasks ?? [];
    const nodeItems = nodes ?? [];
    const stats = taskStats ?? { total: 0, running: 0, failed: 0, preparing: 0, complete: 0, desiredRunning: 0 };
    const agents = metricsData?.agents ?? [];

    // Build container metrics map
    const metricsMap = new Map<string, ContainerMetrics>();
    for (const agent of agents) {
        for (const container of agent.containers) {
            metricsMap.set(container.containerId, container);
        }
    }

    // Compute cluster-level CPU
    const avgCpu = agents.length > 0 ? agents.reduce((sum, a) => sum + a.node.cpuPercent, 0) / agents.length : null;

    // Compute cluster-level memory
    const totalMemUsed = agents.reduce((sum, a) => sum + a.node.memoryUsed, 0);
    const totalMemTotal = agents.reduce((sum, a) => sum + a.node.memoryTotal, 0);
    const memPercent = totalMemTotal > 0 ? (totalMemUsed / totalMemTotal) * 100 : 0;
    const usedGB = (totalMemUsed / 1e9).toFixed(1);
    const totalGB = (totalMemTotal / 1e9).toFixed(1);

    return (
        <div className='space-y-6 px-8 py-6'>
            {/* Page Header */}
            <div className='flex items-start justify-between'>
                <div>
                    <h1 className='font-display text-[24px] font-bold tracking-tight'>CLUSTER OVERVIEW</h1>
                    <p className='mt-1 font-mono text-sm text-muted-foreground'>
                        {clusterName} &middot; {nodeItems.length} node{nodeItems.length !== 1 ? "s" : ""} &middot; {stats.running} container
                        {stats.running !== 1 ? "s" : ""}
                    </p>
                </div>
                <div className='flex items-center gap-2'>
                    <Button
                        variant='outline'
                        size='sm'
                        onClick={() => {
                            queryClient.invalidateQueries({ queryKey: ["tasks"] });
                            queryClient.invalidateQueries({ queryKey: ["taskStats"] });
                            queryClient.invalidateQueries({ queryKey: ["nodes"] });
                            queryClient.invalidateQueries({ queryKey: ["cluster-metrics"] });
                        }}
                    >
                        <RefreshCw className='size-3.5' /> REFRESH
                    </Button>
                    <Button variant='secondary' size='sm' disabled className='opacity-50'>
                        <Plus className='size-3.5' /> DEPLOY
                    </Button>
                </div>
            </div>

            {/* Metrics Row */}
            <div className='flex gap-4'>
                <MetricCard
                    icon={Box}
                    iconColor='text-accent-green'
                    label='CONTAINERS'
                    value={String(stats.running)}
                    subtitle={`/ ${stats.total} slots`}
                    badges={[
                        ...(stats.running > 0 ? [{ label: `${stats.running} running`, color: "bg-accent-green/10", textColor: "text-accent-green" }] : []),
                        ...(stats.failed > 0 ? [{ label: `${stats.failed} failed`, color: "bg-accent-red/10", textColor: "text-accent-red" }] : []),
                        ...(stats.preparing > 0
                            ? [{ label: `${stats.preparing} preparing`, color: "bg-accent-orange/10", textColor: "text-accent-orange" }]
                            : []),
                    ]}
                />
                <MetricCard
                    icon={Cpu}
                    iconColor='text-accent-cyan'
                    label='CPU USAGE'
                    value={avgCpu !== null ? `${avgCpu.toFixed(1)}%` : "--"}
                    subtitle={`avg across ${nodeItems.length} node${nodeItems.length !== 1 ? "s" : ""}`}
                    bar={{ percent: avgCpu ?? 0, color: "bg-accent-cyan" }}
                />
                <MetricCard
                    icon={MemoryStick}
                    iconColor='text-[#A78BFA]'
                    label='MEMORY'
                    value={totalMemTotal > 0 ? `${usedGB} GB` : "--"}
                    subtitle={`/ ${totalGB} GB`}
                    bar={{ percent: memPercent, color: "bg-[#A78BFA]" }}
                />
                <MetricCard
                    icon={ArrowUpDown}
                    iconColor='text-accent-orange'
                    label='NETWORK I/O'
                    value='--'
                    subtitle='placeholder'
                    bar={{ percent: 0, color: "bg-accent-orange" }}
                />
            </div>

            {/* Traefik Banner */}
            <TraefikBanner />

            {/* Container Table Section */}
            <div className='border border-border bg-card'>
                {/* Table Header */}
                <div className='flex items-center justify-between border-b border-border px-4 py-3'>
                    <div className='flex items-center gap-3'>
                        <span className='font-mono text-[11px] font-bold tracking-[0.5px] text-foreground'>ALL CONTAINERS</span>
                        <span className='rounded bg-bg-elevated px-1.5 py-0.5 font-mono text-[10px] font-medium text-muted-foreground'>{taskItems.length}</span>
                    </div>
                    <div className='flex items-center gap-2'>
                        <div className='relative'>
                            <Search className='absolute top-1/2 left-2 size-3 -translate-y-1/2 text-muted-foreground' />
                            <Input
                                placeholder='Search containers...'
                                value={search}
                                onChange={(e) => setSearch(e.target.value)}
                                className='h-7 w-48 pl-7 font-mono text-xs'
                            />
                        </div>
                        <Button variant='outline' size='sm' disabled className='opacity-50'>
                            <SlidersHorizontal className='size-3' /> FILTER
                        </Button>
                    </div>
                </div>

                {/* Table */}
                <ContainersTable tasks={taskItems} containerMetrics={metricsMap} search={search} />
            </div>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/")({
    component: DashboardPage,
});

import { createFileRoute } from '@tanstack/react-router';
import { useNodes } from '@/hooks/use-nodes';
import { useClusterMetrics } from '@/hooks/use-cluster-metrics';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Progress } from '@/components/ui/progress';
import { Skeleton } from '@/components/ui/skeleton';
import { PageHeader } from '@/components/page-header';
import { NodeActions } from '@/components/node-actions';
import { AlertCircle, AlertTriangle, Search, Server } from 'lucide-react';
import { useMemo, useState } from 'react';
import type { NodeItem } from '@/types/nodes';
import type { AgentMetricsSnapshot } from '@/types/metrics';

function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function metricsColorClass(pct: number): string {
    if (pct >= 85) return 'text-destructive';
    if (pct >= 60) return 'text-amber-500';
    return 'text-success';
}

function RoleBadges({ node }: { node: NodeItem }) {
    if (node.role === 'manager' && node.leader) {
        return (
            <Badge className='h-4 border-0 bg-emerald-500/15 px-1.5 font-mono text-[10px] text-emerald-500 hover:bg-emerald-500/15'>
                LEADER
            </Badge>
        );
    }
    if (node.role === 'manager') {
        return (
            <Badge className='h-4 border-0 bg-cyan-500/15 px-1.5 font-mono text-[10px] text-cyan-500 hover:bg-cyan-500/15'>
                MANAGER
            </Badge>
        );
    }
    return (
        <Badge className='h-4 border-0 bg-muted px-1.5 font-mono text-[10px] text-muted-foreground hover:bg-muted'>
            WORKER
        </Badge>
    );
}

function AvailabilityBadge({ availability }: { availability: string }) {
    if (availability === 'drain') {
        return <span className='font-mono text-[10px] font-medium text-orange-500'>DRAIN</span>;
    }
    if (availability === 'pause') {
        return <span className='font-mono text-[10px] font-medium text-amber-500'>PAUSE</span>;
    }
    return <span className='font-mono text-[10px] font-medium text-emerald-500'>ACTIVE</span>;
}

function MetricsBar({ label, percent, detail }: { label: string; percent: number; detail: string }) {
    return (
        <div className='flex flex-col gap-1'>
            <div className='flex items-baseline justify-between'>
                <span className='font-mono text-[9px] font-bold tracking-[0.5px] text-muted-foreground uppercase'>{label}</span>
                <span className={`font-mono text-[11px] font-medium ${metricsColorClass(percent)}`}>{percent.toFixed(1)}%</span>
            </div>
            <Progress value={percent} className='h-1' />
            <span className='font-mono text-[10px] text-muted-foreground'>{detail}</span>
        </div>
    );
}

function NodeCard({ node, metrics }: { node: NodeItem; metrics?: AgentMetricsSnapshot }) {
    const isUnhealthy = node.status !== 'ready';
    const containerCount = metrics?.containers.length ?? 0;

    return (
        <div className='flex min-w-[300px] flex-1 flex-col gap-3 border border-border bg-card p-4' data-testid='node-card'>
            {/* Header */}
            <div className='flex items-start justify-between'>
                <div className='flex flex-col gap-1.5'>
                    <span className='font-mono text-sm font-bold text-foreground'>{node.hostname}</span>
                    <div className='flex items-center gap-1.5'>
                        <RoleBadges node={node} />
                        <AvailabilityBadge availability={node.availability} />
                    </div>
                </div>
                <NodeActions node={node} />
            </div>

            {/* Warning banner for unhealthy nodes */}
            {isUnhealthy && (
                <div className='flex items-center gap-2 rounded bg-amber-500/10 px-3 py-2'>
                    <AlertTriangle className='size-3.5 text-amber-500' />
                    <span className='font-mono text-[11px] font-medium text-amber-500'>
                        Node status: {node.status}
                    </span>
                </div>
            )}

            {/* Info grid */}
            <div className='grid grid-cols-2 gap-x-4 gap-y-1.5'>
                <div className='flex flex-col'>
                    <span className='font-mono text-[9px] font-bold tracking-[0.5px] text-muted-foreground uppercase'>IP</span>
                    <span className='font-mono text-[11px] text-foreground'>{node.ip}</span>
                </div>
                <div className='flex flex-col'>
                    <span className='font-mono text-[9px] font-bold tracking-[0.5px] text-muted-foreground uppercase'>Docker</span>
                    <span className='font-mono text-[11px] text-foreground'>{node.engineVersion}</span>
                </div>
                <div className='flex flex-col'>
                    <span className='font-mono text-[9px] font-bold tracking-[0.5px] text-muted-foreground uppercase'>OS</span>
                    <span className='font-mono text-[11px] text-foreground'>{node.os}</span>
                </div>
                <div className='flex flex-col'>
                    <span className='font-mono text-[9px] font-bold tracking-[0.5px] text-muted-foreground uppercase'>Arch</span>
                    <span className='font-mono text-[11px] text-foreground'>{node.architecture}</span>
                </div>
            </div>

            {/* Metrics */}
            {metrics ? (
                <div className='flex flex-col gap-2 border-t border-border pt-3'>
                    <MetricsBar
                        label='CPU'
                        percent={metrics.node.cpuPercent}
                        detail={`${metrics.node.cpus} cores`}
                    />
                    <MetricsBar
                        label='MEM'
                        percent={metrics.node.memoryPercent}
                        detail={`${formatBytes(metrics.node.memoryUsed)} / ${formatBytes(metrics.node.memoryTotal)}`}
                    />
                </div>
            ) : (
                <div className='border-t border-border pt-3'>
                    <span className='font-mono text-[10px] text-muted-foreground'>No metrics available</span>
                </div>
            )}

            {/* Container count */}
            <div className='border-t border-border pt-2'>
                <span className='font-mono text-[11px] text-muted-foreground'>
                    {containerCount} {containerCount === 1 ? 'container' : 'containers'}
                </span>
            </div>
        </div>
    );
}

function LoadingSkeleton() {
    return (
        <div className='flex flex-col gap-6 p-6'>
            <div className='flex items-center justify-between'>
                <div className='flex flex-col gap-1'>
                    <Skeleton className='h-7 w-24' />
                    <Skeleton className='h-3 w-56' />
                </div>
                <Skeleton className='h-7 w-28' />
            </div>
            <div className='flex items-center gap-3'>
                <Skeleton className='h-7 w-[200px]' />
                <Skeleton className='h-3 w-40' />
            </div>
            <div className='flex flex-wrap gap-4'>
                {[1, 2, 3].map((i) => (
                    <div key={i} className='flex min-w-[300px] flex-1 flex-col gap-3 border border-border bg-card p-4'>
                        <Skeleton className='h-4 w-32' />
                        <div className='flex gap-1.5'>
                            <Skeleton className='h-4 w-16 rounded-full' />
                            <Skeleton className='h-4 w-12' />
                        </div>
                        <div className='grid grid-cols-2 gap-2'>
                            <Skeleton className='h-6 w-full' />
                            <Skeleton className='h-6 w-full' />
                            <Skeleton className='h-6 w-full' />
                            <Skeleton className='h-6 w-full' />
                        </div>
                        <Skeleton className='h-8 w-full' />
                        <Skeleton className='h-8 w-full' />
                    </div>
                ))}
            </div>
        </div>
    );
}

function NodesPage() {
    const { data: nodes, isLoading, error } = useNodes();
    const { data: metricsData } = useClusterMetrics();
    const [search, setSearch] = useState('');

    const metricsMap = useMemo(() => {
        const map: Record<string, AgentMetricsSnapshot> = {};
        for (const agent of metricsData?.agents ?? []) {
            map[agent.nodeId] = agent;
        }
        return map;
    }, [metricsData]);

    const filtered = (nodes ?? []).filter(
        (node) =>
            node.hostname.toLowerCase().includes(search.toLowerCase()) ||
            node.role.toLowerCase().includes(search.toLowerCase()) ||
            node.status.toLowerCase().includes(search.toLowerCase()) ||
            node.ip.toLowerCase().includes(search.toLowerCase()),
    );

    if (isLoading) {
        return <LoadingSkeleton />;
    }

    if (error) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-xs text-destructive'>
                <AlertCircle className='size-4' />
                <p>Failed to load nodes</p>
            </div>
        );
    }

    const nodeCount = nodes?.length ?? 0;
    const subtitle = `${nodeCount} ${nodeCount === 1 ? 'node' : 'nodes'} · Docker Swarm cluster`;

    return (
        <div className='flex h-full flex-col gap-6 p-6'>
            <PageHeader title='NODES' subtitle={subtitle}>
                <Button variant='outline' size='sm' disabled className='h-7 font-mono text-[11px] text-muted-foreground'>
                    ADD NODE
                </Button>
            </PageHeader>

            {/* Filter bar */}
            <div className='flex items-center gap-3'>
                <div className='relative'>
                    <Search className='absolute top-1/2 left-2.5 size-3 -translate-y-1/2 text-muted-foreground' />
                    <Input
                        placeholder='Search nodes...'
                        value={search}
                        onChange={(e) => setSearch(e.target.value)}
                        className='h-7 w-[200px] bg-transparent pl-8 font-mono text-[11px]'
                    />
                </div>
                <span className='font-mono text-[11px] text-muted-foreground'>Docker Swarm cluster</span>
            </div>

            {/* Cards */}
            {filtered.length === 0 ? (
                <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                    <Server className='size-4' />
                    <p>{search ? 'No nodes match your filter' : 'No nodes found'}</p>
                </div>
            ) : (
                <div className='flex flex-wrap gap-4'>
                    {filtered.map((node) => (
                        <NodeCard key={node.id} node={node} metrics={metricsMap[node.id]} />
                    ))}
                </div>
            )}
        </div>
    );
}

export const Route = createFileRoute('/_authenticated/nodes/')({
    component: NodesPage,
});

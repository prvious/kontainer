import { createFileRoute } from "@tanstack/react-router";
import { useNodes } from "@/hooks/use-nodes";
import { useClusterMetrics } from "@/hooks/use-cluster-metrics";
import { PageHeader } from "@/components/page-header";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { NodeActions } from "@/components/node-actions";
import { AlertCircle, ChevronRight, Search, Server } from "lucide-react";
import { Fragment, useMemo, useState } from "react";
import type { NodeItem } from "@/types/nodes";
import type { AgentMetricsSnapshot, ContainerMetrics } from "@/types/metrics";

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

function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function metricsColorClass(pct: number): string {
    if (pct >= 85) return "text-destructive";
    if (pct >= 60) return "text-amber-500";
    return "text-success";
}

function StatusDot({ node }: { node: NodeItem }) {
    if (node.status === "down" || node.status === "disconnected") {
        return <span className='animate-pulse-dot size-2 rounded-full bg-destructive' title={node.status} />;
    }
    if (node.availability === "drain" || node.availability === "pause") {
        return <span className='size-2 rounded-full bg-amber-500' title={node.availability} />;
    }
    return <span className='size-2 rounded-full bg-success' title='ready' />;
}

function RoleBadge({ node }: { node: NodeItem }) {
    if (node.role === "manager") {
        return <Badge variant='default'>{node.leader ? "manager (leader)" : "manager"}</Badge>;
    }
    return <Badge variant='secondary'>worker</Badge>;
}

function AvailabilityBadge({ availability }: { availability: string }) {
    if (availability === "drain") {
        return <Badge variant='destructive'>drain</Badge>;
    }
    if (availability === "pause") {
        return <Badge variant='secondary'>pause</Badge>;
    }
    return <Badge variant='default'>active</Badge>;
}

function ContainersTable({ containers }: { containers: ContainerMetrics[] }) {
    if (containers.length === 0) {
        return <p className='text-xs text-muted-foreground'>No running containers</p>;
    }

    return (
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead className='text-xs'>Name</TableHead>
                    <TableHead className='text-xs'>Service</TableHead>
                    <TableHead className='text-right text-xs'>CPU %</TableHead>
                    <TableHead className='text-right text-xs'>Memory</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {containers.map((c) => (
                    <TableRow key={c.containerId}>
                        <TableCell className='py-1'>
                            <span className='font-mono text-xs'>{c.name}</span>
                        </TableCell>
                        <TableCell className='py-1'>
                            <span className='text-xs text-muted-foreground'>{c.serviceName || "—"}</span>
                        </TableCell>
                        <TableCell className='py-1 text-right'>
                            <span className={`text-xs font-medium ${metricsColorClass(c.cpuPercent)}`}>{c.cpuPercent.toFixed(1)}%</span>
                        </TableCell>
                        <TableCell className='py-1 text-right'>
                            <span className='text-xs text-muted-foreground'>
                                {formatBytes(c.memoryUsed)} / {formatBytes(c.memoryLimit)}
                            </span>
                        </TableCell>
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}

function ExpandedNodeDetail({ metrics }: { metrics: AgentMetricsSnapshot }) {
    const { node, containers } = metrics;

    return (
        <div className='flex flex-col gap-4 p-4'>
            <div className='grid grid-cols-3 gap-4'>
                <div className='flex flex-col gap-1'>
                    <span className='text-xs font-medium text-muted-foreground'>CPU</span>
                    <span className='text-sm font-medium'>
                        {node.cpuPercent.toFixed(1)}% of {node.cpus} cores
                    </span>
                    <Progress value={node.cpuPercent} className='h-1.5' />
                </div>
                <div className='flex flex-col gap-1'>
                    <span className='text-xs font-medium text-muted-foreground'>Memory</span>
                    <span className='text-sm font-medium'>
                        {formatBytes(node.memoryUsed)} / {formatBytes(node.memoryTotal)} ({node.memoryPercent.toFixed(1)}%)
                    </span>
                    <Progress value={node.memoryPercent} className='h-1.5' />
                </div>
                <div className='flex flex-col gap-1'>
                    <span className='text-xs font-medium text-muted-foreground'>Disk</span>
                    <span className='text-sm font-medium'>
                        {formatBytes(node.diskUsed)} / {formatBytes(node.diskTotal)} ({node.diskPercent.toFixed(1)}%)
                    </span>
                    <Progress value={node.diskPercent} className='h-1.5' />
                </div>
            </div>

            <div>
                <h4 className='mb-2 text-xs font-medium text-muted-foreground'>Containers ({containers.length})</h4>
                <ContainersTable containers={containers} />
            </div>
        </div>
    );
}

interface NodesTableProps {
    nodes: NodeItem[];
    metricsMap: Record<string, AgentMetricsSnapshot>;
    expandedNodeId: string | null;
    onToggleExpand: (nodeId: string) => void;
}

function NodesTable({ nodes, metricsMap, expandedNodeId, onToggleExpand }: NodesTableProps) {
    return (
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead className='w-8' />
                    <TableHead>Hostname</TableHead>
                    <TableHead>Role</TableHead>
                    <TableHead>Availability</TableHead>
                    <TableHead>IP</TableHead>
                    <TableHead>Engine</TableHead>
                    <TableHead>OS / Arch</TableHead>
                    <TableHead>CPU</TableHead>
                    <TableHead>Memory</TableHead>
                    <TableHead>Disk</TableHead>
                    <TableHead className='text-right'>Updated</TableHead>
                    <TableHead className='w-10' />
                </TableRow>
            </TableHeader>
            <TableBody>
                {nodes.map((node) => {
                    const metrics = metricsMap[node.id];
                    const isExpanded = expandedNodeId === node.id;

                    return (
                        <Fragment key={node.id}>
                            <TableRow className={metrics ? "cursor-pointer" : ""} onClick={() => metrics && onToggleExpand(node.id)}>
                                <TableCell>
                                    <StatusDot node={node} />
                                </TableCell>
                                <TableCell className='font-medium'>
                                    <span className='flex items-center gap-1.5'>
                                        {metrics && (
                                            <ChevronRight className={`size-3.5 text-muted-foreground transition-transform ${isExpanded ? "rotate-90" : ""}`} />
                                        )}
                                        {node.hostname}
                                    </span>
                                </TableCell>
                                <TableCell>
                                    <RoleBadge node={node} />
                                </TableCell>
                                <TableCell>
                                    <AvailabilityBadge availability={node.availability} />
                                </TableCell>
                                <TableCell>
                                    <span className='font-mono text-xs text-muted-foreground'>{node.ip}</span>
                                </TableCell>
                                <TableCell>
                                    <span className='font-mono text-xs text-muted-foreground'>{node.engineVersion}</span>
                                </TableCell>
                                <TableCell>
                                    <span className='text-xs text-muted-foreground'>
                                        {node.os}/{node.architecture}
                                    </span>
                                </TableCell>
                                <TableCell>
                                    {metrics ? (
                                        <div className='flex items-center gap-2'>
                                            <span className={`text-xs font-medium ${metricsColorClass(metrics.node.cpuPercent)}`}>
                                                {metrics.node.cpuPercent.toFixed(1)}%
                                            </span>
                                            <Progress value={metrics.node.cpuPercent} className='h-1.5 w-16' />
                                        </div>
                                    ) : (
                                        <span className='text-xs text-muted-foreground'>—</span>
                                    )}
                                </TableCell>
                                <TableCell>
                                    {metrics ? (
                                        <span className='text-xs text-muted-foreground'>
                                            {formatBytes(metrics.node.memoryUsed)} / {formatBytes(metrics.node.memoryTotal)}
                                        </span>
                                    ) : (
                                        <span className='text-xs text-muted-foreground'>—</span>
                                    )}
                                </TableCell>
                                <TableCell>
                                    {metrics ? (
                                        <span className={`text-xs font-medium ${metricsColorClass(metrics.node.diskPercent)}`}>
                                            {metrics.node.diskPercent.toFixed(1)}%
                                        </span>
                                    ) : (
                                        <span className='text-xs text-muted-foreground'>—</span>
                                    )}
                                </TableCell>
                                <TableCell className='text-right'>
                                    <span className='text-xs text-muted-foreground'>{formatRelativeTime(node.updatedAt)}</span>
                                </TableCell>
                                <TableCell onClick={(e) => e.stopPropagation()}>
                                    <NodeActions node={node} />
                                </TableCell>
                            </TableRow>
                            {isExpanded && metrics && (
                                <TableRow key={`${node.id}-expanded`}>
                                    <TableCell colSpan={12} className='bg-muted/30 p-0'>
                                        <ExpandedNodeDetail metrics={metrics} />
                                    </TableCell>
                                </TableRow>
                            )}
                        </Fragment>
                    );
                })}
            </TableBody>
        </Table>
    );
}

function LoadingSkeleton() {
    return (
        <div className='p-6'>
            <Card>
                <CardContent className='p-0'>
                    <div className='space-y-0'>
                        {[1, 2, 3, 4, 5].map((i) => (
                            <div key={i} className='flex items-center gap-4 border-b px-4 py-3 last:border-0'>
                                <Skeleton className='size-2 rounded-full' />
                                <Skeleton className='h-4 w-32' />
                                <Skeleton className='h-5 w-20' />
                                <Skeleton className='h-5 w-16' />
                                <Skeleton className='h-4 w-24' />
                                <Skeleton className='ml-auto h-4 w-16' />
                            </div>
                        ))}
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}

function NodesPage() {
    const { data: nodes, isLoading, error } = useNodes();
    const { data: metricsData } = useClusterMetrics();
    const [search, setSearch] = useState("");
    const [expandedNodeId, setExpandedNodeId] = useState<string | null>(null);

    const metricsMap = useMemo(() => {
        const map: Record<string, AgentMetricsSnapshot> = {};
        for (const agent of metricsData?.agents ?? []) {
            map[agent.nodeId] = agent;
        }
        return map;
    }, [metricsData]);

    function handleToggleExpand(nodeId: string): void {
        setExpandedNodeId((prev) => (prev === nodeId ? null : nodeId));
    }

    const filtered = (nodes ?? []).filter(
        (node) =>
            node.hostname.toLowerCase().includes(search.toLowerCase()) ||
            node.role.toLowerCase().includes(search.toLowerCase()) ||
            node.status.toLowerCase().includes(search.toLowerCase()) ||
            node.ip.toLowerCase().includes(search.toLowerCase()),
    );

    if (isLoading) {
        return (
            <>
                <PageHeader title='Nodes' />
                <LoadingSkeleton />
            </>
        );
    }

    if (error) {
        return (
            <>
                <PageHeader title='Nodes' />
                <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
                    <AlertCircle className='size-5' />
                    <p>Failed to load nodes</p>
                </div>
            </>
        );
    }

    return (
        <>
            <PageHeader
                title='Nodes'
                description={`${nodes?.length ?? 0} nodes`}
                actions={
                    <div className='relative'>
                        <Search className='absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground' />
                        <Input placeholder='Search nodes...' value={search} onChange={(e) => setSearch(e.target.value)} className='h-8 w-56 pl-8 text-sm' />
                    </div>
                }
            />

            <div className='p-6'>
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-muted-foreground'>
                        <Server className='size-5' />
                        <p>{search ? "No nodes match your search" : "No nodes found"}</p>
                    </div>
                ) : (
                    <Card>
                        <CardContent className='p-0'>
                            <NodesTable nodes={filtered} metricsMap={metricsMap} expandedNodeId={expandedNodeId} onToggleExpand={handleToggleExpand} />
                        </CardContent>
                    </Card>
                )}
            </div>
        </>
    );
}

export const Route = createFileRoute("/_authenticated/nodes/")({
    component: NodesPage,
});

import { createFileRoute } from "@tanstack/react-router";
import { useNodes } from "@/hooks/use-nodes";
import { useClusterMetrics } from "@/hooks/use-cluster-metrics";
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
        return <span className='animate-pulse-dot size-1.5 rounded-full bg-destructive' title={node.status} />;
    }
    if (node.availability === "drain" || node.availability === "pause") {
        return <span className='size-1.5 rounded-full bg-amber-500' title={node.availability} />;
    }
    return <span className='size-1.5 rounded-full bg-success' title='ready' />;
}

function RoleBadge({ node }: { node: NodeItem }) {
    if (node.role === "manager") {
        return (
            <Badge variant='default' className='h-4 px-1.5 text-[10px]'>
                {node.leader ? "leader" : "manager"}
            </Badge>
        );
    }
    return (
        <Badge variant='secondary' className='h-4 px-1.5 text-[10px]'>
            worker
        </Badge>
    );
}

function AvailabilityBadge({ availability }: { availability: string }) {
    if (availability === "drain") {
        return (
            <Badge variant='destructive' className='h-4 px-1.5 text-[10px]'>
                drain
            </Badge>
        );
    }
    if (availability === "pause") {
        return (
            <Badge variant='secondary' className='h-4 px-1.5 text-[10px]'>
                pause
            </Badge>
        );
    }
    return (
        <Badge variant='default' className='h-4 px-1.5 text-[10px]'>
            active
        </Badge>
    );
}

function ContainersTable({ containers }: { containers: ContainerMetrics[] }) {
    if (containers.length === 0) {
        return <p className='px-3 py-2 text-[11px] text-muted-foreground'>No running containers</p>;
    }

    return (
        <Table>
            <TableHeader>
                <TableRow className='hover:bg-transparent'>
                    <TableHead className='h-6 px-3 text-[11px]'>Name</TableHead>
                    <TableHead className='h-6 px-3 text-[11px]'>Service</TableHead>
                    <TableHead className='h-6 px-3 text-right text-[11px]'>CPU %</TableHead>
                    <TableHead className='h-6 px-3 text-right text-[11px]'>Memory</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {containers.map((c) => (
                    <TableRow key={c.containerId} className='hover:bg-transparent'>
                        <TableCell className='px-3 py-0.5'>
                            <span className='font-mono text-[11px]'>{c.name}</span>
                        </TableCell>
                        <TableCell className='px-3 py-0.5'>
                            <span className='text-[11px] text-muted-foreground'>{c.serviceName || "\u2014"}</span>
                        </TableCell>
                        <TableCell className='px-3 py-0.5 text-right'>
                            <span className={`font-mono text-[11px] font-medium ${metricsColorClass(c.cpuPercent)}`}>{c.cpuPercent.toFixed(1)}%</span>
                        </TableCell>
                        <TableCell className='px-3 py-0.5 text-right'>
                            <span className='font-mono text-[11px] text-muted-foreground'>
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
        <div className='flex flex-col gap-3 px-3 py-3'>
            <div className='grid grid-cols-3 gap-3'>
                <div className='flex flex-col gap-0.5'>
                    <span className='text-[10px] font-medium tracking-wider text-muted-foreground uppercase'>CPU</span>
                    <div className='flex items-baseline gap-1.5'>
                        <span className={`font-mono text-xs font-medium ${metricsColorClass(node.cpuPercent)}`}>{node.cpuPercent.toFixed(1)}%</span>
                        <span className='text-[10px] text-muted-foreground'>of {node.cpus} cores</span>
                    </div>
                    <Progress value={node.cpuPercent} className='h-1' />
                </div>
                <div className='flex flex-col gap-0.5'>
                    <span className='text-[10px] font-medium tracking-wider text-muted-foreground uppercase'>Memory</span>
                    <div className='flex items-baseline gap-1.5'>
                        <span className={`font-mono text-xs font-medium ${metricsColorClass(node.memoryPercent)}`}>{node.memoryPercent.toFixed(1)}%</span>
                        <span className='text-[10px] text-muted-foreground'>
                            {formatBytes(node.memoryUsed)} / {formatBytes(node.memoryTotal)}
                        </span>
                    </div>
                    <Progress value={node.memoryPercent} className='h-1' />
                </div>
                <div className='flex flex-col gap-0.5'>
                    <span className='text-[10px] font-medium tracking-wider text-muted-foreground uppercase'>Disk</span>
                    <div className='flex items-baseline gap-1.5'>
                        <span className={`font-mono text-xs font-medium ${metricsColorClass(node.diskPercent)}`}>{node.diskPercent.toFixed(1)}%</span>
                        <span className='text-[10px] text-muted-foreground'>
                            {formatBytes(node.diskUsed)} / {formatBytes(node.diskTotal)}
                        </span>
                    </div>
                    <Progress value={node.diskPercent} className='h-1' />
                </div>
            </div>

            <div>
                <h4 className='mb-1 text-[10px] font-medium tracking-wider text-muted-foreground uppercase'>Containers ({containers.length})</h4>
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
                <TableRow className='bg-muted/40 hover:bg-muted/40'>
                    <TableHead className='h-7 w-6 pr-0 pl-3' />
                    <TableHead className='h-7 text-[11px]'>Hostname</TableHead>
                    <TableHead className='h-7 text-[11px]'>Role</TableHead>
                    <TableHead className='h-7 text-[11px]'>Availability</TableHead>
                    <TableHead className='h-7 text-[11px]'>IP</TableHead>
                    <TableHead className='h-7 text-[11px]'>Engine</TableHead>
                    <TableHead className='h-7 text-[11px]'>OS / Arch</TableHead>
                    <TableHead className='h-7 text-[11px]'>CPU</TableHead>
                    <TableHead className='h-7 text-[11px]'>Memory</TableHead>
                    <TableHead className='h-7 text-[11px]'>Disk</TableHead>
                    <TableHead className='h-7 text-right text-[11px]'>Updated</TableHead>
                    <TableHead className='h-7 w-8' />
                </TableRow>
            </TableHeader>
            <TableBody>
                {nodes.map((node) => {
                    const metrics = metricsMap[node.id];
                    const isExpanded = expandedNodeId === node.id;

                    return (
                        <Fragment key={node.id}>
                            <TableRow className={metrics ? "cursor-pointer" : ""} onClick={() => metrics && onToggleExpand(node.id)}>
                                <TableCell className='py-1.5 pr-0 pl-3'>
                                    <StatusDot node={node} />
                                </TableCell>
                                <TableCell className='py-1.5'>
                                    <span className='flex items-center gap-1'>
                                        {metrics && (
                                            <ChevronRight className={`size-3 text-muted-foreground transition-transform ${isExpanded ? "rotate-90" : ""}`} />
                                        )}
                                        <span className='text-xs font-medium'>{node.hostname}</span>
                                    </span>
                                </TableCell>
                                <TableCell className='py-1.5'>
                                    <RoleBadge node={node} />
                                </TableCell>
                                <TableCell className='py-1.5'>
                                    <AvailabilityBadge availability={node.availability} />
                                </TableCell>
                                <TableCell className='py-1.5'>
                                    <span className='font-mono text-[11px] text-muted-foreground'>{node.ip}</span>
                                </TableCell>
                                <TableCell className='py-1.5'>
                                    <span className='font-mono text-[11px] text-muted-foreground'>{node.engineVersion}</span>
                                </TableCell>
                                <TableCell className='py-1.5'>
                                    <span className='font-mono text-[11px] text-muted-foreground'>
                                        {node.os}/{node.architecture}
                                    </span>
                                </TableCell>
                                <TableCell className='py-1.5'>
                                    {metrics ? (
                                        <div className='flex items-center gap-1.5'>
                                            <span className={`font-mono text-[11px] font-medium ${metricsColorClass(metrics.node.cpuPercent)}`}>
                                                {metrics.node.cpuPercent.toFixed(1)}%
                                            </span>
                                            <Progress value={metrics.node.cpuPercent} className='h-1 w-12' />
                                        </div>
                                    ) : (
                                        <span className='text-[11px] text-muted-foreground'>\u2014</span>
                                    )}
                                </TableCell>
                                <TableCell className='py-1.5'>
                                    {metrics ? (
                                        <span className='font-mono text-[11px] text-muted-foreground'>
                                            {formatBytes(metrics.node.memoryUsed)} / {formatBytes(metrics.node.memoryTotal)}
                                        </span>
                                    ) : (
                                        <span className='text-[11px] text-muted-foreground'>\u2014</span>
                                    )}
                                </TableCell>
                                <TableCell className='py-1.5'>
                                    {metrics ? (
                                        <span className={`font-mono text-[11px] font-medium ${metricsColorClass(metrics.node.diskPercent)}`}>
                                            {metrics.node.diskPercent.toFixed(1)}%
                                        </span>
                                    ) : (
                                        <span className='text-[11px] text-muted-foreground'>\u2014</span>
                                    )}
                                </TableCell>
                                <TableCell className='py-1.5 text-right'>
                                    <span className='font-mono text-[11px] text-muted-foreground'>{formatRelativeTime(node.updatedAt)}</span>
                                </TableCell>
                                <TableCell className='py-1.5' onClick={(e) => e.stopPropagation()}>
                                    <NodeActions node={node} />
                                </TableCell>
                            </TableRow>
                            {isExpanded && metrics && (
                                <TableRow key={`${node.id}-expanded`} className='hover:bg-transparent'>
                                    <TableCell colSpan={12} className='bg-muted/20 p-0'>
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
        <div className='flex flex-col'>
            <div className='flex h-10 items-center gap-3 border-b border-border px-3'>
                <Skeleton className='h-4 w-14' />
                <Skeleton className='h-4 w-8 rounded-full' />
                <div className='flex-1' />
                <Skeleton className='h-6 w-40' />
            </div>
            <div className='space-y-0'>
                {[1, 2, 3, 4, 5].map((i) => (
                    <div key={i} className='flex items-center gap-3 border-b border-border px-3 py-2 last:border-0'>
                        <Skeleton className='size-1.5 rounded-full' />
                        <Skeleton className='h-3 w-28' />
                        <Skeleton className='h-4 w-14 rounded-full' />
                        <Skeleton className='h-4 w-12 rounded-full' />
                        <Skeleton className='h-3 w-24' />
                        <Skeleton className='ml-auto h-3 w-14' />
                    </div>
                ))}
            </div>
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

    return (
        <div className='flex h-full flex-col'>
            {/* Compact toolbar */}
            <div className='flex h-10 shrink-0 items-center gap-3 border-b border-border px-3'>
                <h1 className='text-xs font-semibold'>Nodes</h1>
                <span className='rounded-md bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground'>{nodes?.length ?? 0}</span>
                <div className='flex-1' />
                <div className='relative'>
                    <Search className='absolute top-1/2 left-2 size-3 -translate-y-1/2 text-muted-foreground' />
                    <Input
                        placeholder='Filter nodes...'
                        value={search}
                        onChange={(e) => setSearch(e.target.value)}
                        className='h-6 w-48 rounded-sm border-border bg-transparent pl-7 text-[11px] focus-visible:ring-1'
                    />
                </div>
            </div>

            {/* Table content — edge-to-edge */}
            <div className='flex-1 overflow-y-auto'>
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                        <Server className='size-4' />
                        <p>{search ? "No nodes match your filter" : "No nodes found"}</p>
                    </div>
                ) : (
                    <NodesTable nodes={filtered} metricsMap={metricsMap} expandedNodeId={expandedNodeId} onToggleExpand={handleToggleExpand} />
                )}
            </div>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/nodes/")({
    component: NodesPage,
});

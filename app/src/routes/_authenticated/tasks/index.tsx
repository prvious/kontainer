import { createFileRoute } from "@tanstack/react-router";
import { useTasks } from "@/hooks/use-tasks";
import { useNodes } from "@/hooks/use-nodes";
import { useClusterMetrics } from "@/hooks/use-cluster-metrics";
import { Skeleton } from "@/components/ui/skeleton";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { PageHeader } from "@/components/page-header";
import { TableShell } from "@/components/table-shell";
import { ColumnHeaders } from "@/components/column-headers";
import { AlertCircle, Container, ScrollText, Terminal } from "lucide-react";
import { useState, useMemo } from "react";
import type { TaskItem } from "@/types/tasks";
import type { ContainerMetrics } from "@/types/metrics";

function formatMemory(bytes: number): string {
    if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
    if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
    if (bytes >= 1e6) return `${Math.round(bytes / 1e6)} MB`;
    return `${Math.round(bytes / 1e3)} KB`;
}

function formatUptime(timestamp: string): string {
    const ms = Date.now() - new Date(timestamp).getTime();
    if (!Number.isFinite(ms) || ms < 0) return "--";
    const seconds = Math.floor(ms / 1000);
    if (seconds < 60) return "just now";
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ${minutes % 60}m`;
    const days = Math.floor(hours / 24);
    return `${days}d ${hours % 24}h`;
}

function statusInfo(state: string): { label: string; dotColor: string; textColor: string } {
    const s = state.toLowerCase();
    if (s === "running") return { label: "UP", dotColor: "bg-accent-green", textColor: "text-accent-green" };
    if (["paused", "preparing", "pending", "assigned", "accepted"].includes(s))
        return { label: "PAUSE", dotColor: "bg-accent-orange", textColor: "text-accent-orange" };
    return { label: "EXIT", dotColor: "bg-accent-red", textColor: "text-accent-red" };
}

const CONTAINER_COLUMNS = [
    { label: "STATUS", width: 70 },
    { label: "NAME" },
    { label: "IMAGE", width: 200 },
    { label: "CPU", width: 60, align: "right" as const },
    { label: "MEM", width: 70, align: "right" as const },
    { label: "PORTS", width: 100 },
    { label: "UPTIME", width: 80, align: "right" as const },
    { label: "", width: 60 },
];

function TaskRow({ task, metrics }: { task: TaskItem; metrics: ContainerMetrics | undefined }) {
    const status = statusInfo(task.state);
    const isError = task.state.toLowerCase().includes("failed") || task.state.toLowerCase().includes("rejected");

    return (
        <div className={`flex items-center border-b border-border px-4 py-2.5 hover:bg-bg-elevated/50 ${isError ? "bg-accent-red/5" : ""}`}>
            {/* STATUS */}
            <div style={{ width: 70 }} className='flex items-center gap-1.5'>
                <span className={`size-1.5 shrink-0 rounded-full ${status.dotColor}`} />
                <span className={`font-mono text-[10px] font-semibold ${status.textColor}`}>{status.label}</span>
            </div>

            {/* NAME */}
            <div style={{ flex: 1 }} className='min-w-0'>
                <span className='truncate font-mono text-xs text-foreground'>{task.serviceName}</span>
            </div>

            {/* IMAGE */}
            <div style={{ width: 200 }}>
                <span className='truncate font-mono text-xs text-muted-foreground'>{task.image}</span>
            </div>

            {/* CPU */}
            <div style={{ width: 60 }} className='text-right'>
                {metrics ? (
                    <span className={`font-mono text-xs ${metrics.cpuPercent > 8 ? "text-accent-orange" : "text-foreground"}`}>
                        {metrics.cpuPercent.toFixed(1)}%
                    </span>
                ) : (
                    <span className='font-mono text-xs text-muted-foreground'>&mdash;</span>
                )}
            </div>

            {/* MEM */}
            <div style={{ width: 70 }} className='text-right'>
                {metrics ? (
                    <span className={`font-mono text-xs ${metrics.memoryUsed >= 1e9 ? "text-accent-orange" : "text-foreground"}`}>
                        {formatMemory(metrics.memoryUsed)}
                    </span>
                ) : (
                    <span className='font-mono text-xs text-muted-foreground'>&mdash;</span>
                )}
            </div>

            {/* PORTS */}
            <div style={{ width: 100 }}>
                <span className='font-mono text-xs text-muted-foreground'>&mdash;</span>
            </div>

            {/* UPTIME */}
            <div style={{ width: 80 }} className='text-right'>
                <span className='font-mono text-xs text-muted-foreground'>{formatUptime(task.timestamp)}</span>
            </div>

            {/* Actions */}
            <div style={{ width: 60 }} className='flex items-center justify-end gap-1'>
                <TooltipProvider>
                    <Tooltip>
                        <TooltipTrigger className='inline-flex size-6 items-center justify-center rounded text-muted-foreground hover:text-foreground'>
                            <Terminal className='size-3' />
                        </TooltipTrigger>
                        <TooltipContent>Terminal</TooltipContent>
                    </Tooltip>
                </TooltipProvider>
                <TooltipProvider>
                    <Tooltip>
                        <TooltipTrigger className='inline-flex size-6 items-center justify-center rounded text-muted-foreground hover:text-foreground'>
                            <ScrollText className='size-3' />
                        </TooltipTrigger>
                        <TooltipContent>Logs</TooltipContent>
                    </Tooltip>
                </TooltipProvider>
            </div>
        </div>
    );
}

function LoadingSkeleton() {
    return (
        <div className='flex flex-col gap-6 p-6'>
            <div className='flex items-center justify-between'>
                <div className='flex flex-col gap-1'>
                    <Skeleton className='h-7 w-32' />
                    <Skeleton className='h-3 w-56' />
                </div>
            </div>
            <div className='flex flex-col overflow-hidden border border-border'>
                <div className='flex items-center justify-between border-b border-border px-4 py-3'>
                    <Skeleton className='h-4 w-36' />
                    <Skeleton className='h-7 w-48' />
                </div>
                <div className='space-y-0'>
                    {[1, 2, 3, 4, 5].map((i) => (
                        <div key={i} className='flex items-center gap-3 border-b border-border px-4 py-2.5 last:border-0'>
                            <Skeleton className='size-1.5 rounded-full' />
                            <Skeleton className='h-3 w-28' />
                            <Skeleton className='h-3 w-40' />
                            <Skeleton className='ml-auto h-3 w-12' />
                            <Skeleton className='h-3 w-14' />
                        </div>
                    ))}
                </div>
            </div>
        </div>
    );
}

function TasksPage() {
    const { data: tasks, isLoading, error } = useTasks();
    const { data: nodes } = useNodes();
    const { data: metricsData } = useClusterMetrics();
    const [search, setSearch] = useState("");

    const containerMetrics = useMemo(() => {
        const map = new Map<string, ContainerMetrics>();
        if (!metricsData?.agents) return map;
        for (const agent of metricsData.agents) {
            for (const c of agent.containers) {
                map.set(c.containerId, c);
            }
        }
        return map;
    }, [metricsData]);

    const filtered = useMemo(() => {
        const all = tasks ?? [];
        if (!search) return all;
        const q = search.toLowerCase();
        return all.filter((task) => task.serviceName.toLowerCase().includes(q) || task.image.toLowerCase().includes(q) || task.state.toLowerCase().includes(q));
    }, [tasks, search]);

    if (isLoading) {
        return <LoadingSkeleton />;
    }

    if (error) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-xs text-destructive'>
                <AlertCircle className='size-4' />
                <p>Failed to load tasks</p>
            </div>
        );
    }

    const runningCount = (tasks ?? []).filter((t) => t.state.toLowerCase() === "running").length;
    const nodeCount = nodes?.length ?? 0;
    const subtitle = `Running ${runningCount} containers across ${nodeCount} ${nodeCount === 1 ? "node" : "nodes"}`;

    return (
        <div className='flex h-full flex-col gap-6 p-6'>
            <PageHeader title='CONTAINERS' subtitle={subtitle} />

            <TableShell title='ALL CONTAINERS' count={filtered.length} searchValue={search} onSearchChange={setSearch} searchPlaceholder='Search containers...'>
                <ColumnHeaders columns={CONTAINER_COLUMNS} />
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                        <Container className='size-4' />
                        <p>{search ? "No containers match your filter" : "No containers found"}</p>
                    </div>
                ) : (
                    <div>
                        {filtered.map((task) => (
                            <TaskRow key={task.id} task={task} metrics={containerMetrics.get(task.containerId)} />
                        ))}
                    </div>
                )}
            </TableShell>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/tasks/")({
    component: TasksPage,
});

import { useMemo } from "react";
import { ScrollText, Terminal } from "lucide-react";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import type { TaskItem } from "@/types/tasks";
import type { ContainerMetrics } from "@/types/metrics";

interface ContainersTableProps {
    tasks: TaskItem[];
    containerMetrics: Map<string, ContainerMetrics>;
    search: string;
}

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
    if (s === "paused" || s === "preparing" || s === "pending" || s === "assigned" || s === "accepted")
        return { label: "PAUSE", dotColor: "bg-accent-orange", textColor: "text-accent-orange" };
    return { label: "EXIT", dotColor: "bg-accent-red", textColor: "text-accent-red" };
}

const columns = [
    { label: "STATUS", width: "w-[70px]" },
    { label: "NAME", width: "flex-1 min-w-0" },
    { label: "IMAGE", width: "w-[200px]" },
    { label: "CPU", width: "w-[60px]", align: "text-right" },
    { label: "MEM", width: "w-[70px]", align: "text-right" },
    { label: "PORTS", width: "w-[100px]" },
    { label: "UPTIME", width: "w-[80px]", align: "text-right" },
    { label: "", width: "w-[60px]" },
] as const;

export function ContainersTable({ tasks, containerMetrics, search }: ContainersTableProps) {
    const filteredTasks = useMemo(() => {
        if (!search) return tasks;
        const q = search.toLowerCase();
        return tasks.filter((t) => t.serviceName.toLowerCase().includes(q) || t.image.toLowerCase().includes(q));
    }, [tasks, search]);

    return (
        <div className='overflow-hidden rounded-lg border border-border'>
            {/* Header */}
            <div className='flex items-center bg-bg-elevated px-4 py-2'>
                {columns.map((col) => (
                    <div
                        key={col.label || "actions"}
                        className={`font-mono text-[9px] font-bold tracking-[0.5px] text-muted-foreground uppercase ${col.width} ${"align" in col ? col.align : ""}`}
                    >
                        {col.label}
                    </div>
                ))}
            </div>

            {/* Rows */}
            {filteredTasks.length === 0 ? (
                <div className='py-8 text-center font-mono text-xs text-muted-foreground'>No containers found</div>
            ) : (
                filteredTasks.map((task) => {
                    const status = statusInfo(task.state);
                    const metrics = containerMetrics.get(task.containerId);
                    const isError = task.state.toLowerCase().includes("failed") || task.state.toLowerCase().includes("rejected");

                    return (
                        <div key={task.id} className={`flex items-center border-b border-border px-4 py-2.5 ${isError ? "bg-accent-red/5" : ""}`}>
                            {/* STATUS */}
                            <div className='flex w-[70px] items-center gap-1.5'>
                                <span className={`size-1.5 shrink-0 rounded-full ${status.dotColor}`} />
                                <span className={`font-mono text-[10px] font-semibold ${status.textColor}`}>{status.label}</span>
                            </div>

                            {/* NAME */}
                            <div className='min-w-0 flex-1'>
                                <span className='truncate font-mono text-xs text-foreground'>{task.serviceName}</span>
                            </div>

                            {/* IMAGE */}
                            <div className='w-[200px]'>
                                <span className='truncate font-mono text-xs text-muted-foreground'>{task.image}</span>
                            </div>

                            {/* CPU */}
                            <div className='w-[60px] text-right'>
                                {metrics ? (
                                    <span className={`font-mono text-xs ${metrics.cpuPercent > 8 ? "text-accent-orange" : "text-foreground"}`}>
                                        {metrics.cpuPercent.toFixed(1)}%
                                    </span>
                                ) : (
                                    <span className='font-mono text-xs text-muted-foreground'>&mdash;</span>
                                )}
                            </div>

                            {/* MEM */}
                            <div className='w-[70px] text-right'>
                                {metrics ? (
                                    <span className={`font-mono text-xs ${metrics.memoryUsed >= 1e9 ? "text-accent-orange" : "text-foreground"}`}>
                                        {formatMemory(metrics.memoryUsed)}
                                    </span>
                                ) : (
                                    <span className='font-mono text-xs text-muted-foreground'>&mdash;</span>
                                )}
                            </div>

                            {/* PORTS */}
                            <div className='w-[100px]'>
                                <span className='font-mono text-xs text-muted-foreground'>&mdash;</span>
                            </div>

                            {/* UPTIME */}
                            <div className='w-[80px] text-right'>
                                <span className='font-mono text-xs text-muted-foreground'>{formatUptime(task.timestamp)}</span>
                            </div>

                            {/* Actions */}
                            <div className='flex w-[60px] items-center justify-end gap-1'>
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
                })
            )}
        </div>
    );
}

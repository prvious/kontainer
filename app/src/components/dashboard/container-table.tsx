import { useState, useMemo } from "react";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { MoreHorizontal, Pause, Play, RotateCw, Server, Terminal } from "lucide-react";
import { useTerminal } from "@/hooks/use-terminal";
import { Button } from "@/components/ui/button";
import type { AgentMetricsSnapshot } from "@/types/metrics";
import type { TaskStats } from "@/types/task-stats";

interface ContainerTableProps {
    agents: AgentMetricsSnapshot[];
    taskStats: TaskStats | null;
}

interface ContainerRow {
    id: string;
    name: string;
    image: string;
    status: "running" | "pending" | "failed";
    node: string;
    cpu: string;
    memory: string;
    uptime: string;
    fullContainerId: string;
    fullNodeId: string;
}

function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function formatUptime(createdAt: number): string {
    if (createdAt <= 0) return "—";
    const seconds = Math.floor(Date.now() / 1000 - createdAt);
    if (seconds < 60) return `${seconds}s`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    const remainingMinutes = minutes % 60;
    if (hours < 24) return `${hours}h ${remainingMinutes}m`;
    const days = Math.floor(hours / 24);
    const remainingHours = hours % 24;
    return `${days}d ${remainingHours}h`;
}

function buildContainerRows(agents: AgentMetricsSnapshot[]): ContainerRow[] {
    const rows: ContainerRow[] = [];
    for (const agent of agents) {
        for (const c of agent.containers) {
            rows.push({
                id: c.containerId.slice(0, 12),
                name: c.name || c.serviceName,
                image: c.serviceName,
                status: "running",
                node: agent.nodeId.slice(0, 12),
                cpu: `${c.cpuPercent.toFixed(1)}%`,
                memory: formatBytes(c.memoryUsed),
                uptime: formatUptime(c.createdAt),
                fullContainerId: c.containerId,
                fullNodeId: agent.nodeId,
            });
        }
    }
    return rows;
}

// Placeholder rows when we have stats but no container-level data
function buildPlaceholderRows(taskStats: TaskStats): ContainerRow[] {
    const rows: ContainerRow[] = [];
    for (let i = 0; i < Math.min(taskStats.running, 8); i++) {
        rows.push({
            id: `task-${String(i + 1).padStart(3, "0")}`,
            name: `service-task.${i + 1}`,
            image: "application:latest",
            status: "running",
            node: `node-${(i % 3) + 1}`,
            cpu: `${(Math.random() * 15).toFixed(1)}%`,
            memory: formatBytes(Math.random() * 512 * 1024 * 1024),
            uptime: `${Math.floor(Math.random() * 48)}h ${Math.floor(Math.random() * 60)}m`,
            fullContainerId: "",
            fullNodeId: "",
        });
    }
    for (let i = 0; i < Math.min(taskStats.failed, 3); i++) {
        rows.push({
            id: `fail-${String(i + 1).padStart(3, "0")}`,
            name: `failed-task.${i + 1}`,
            image: "application:latest",
            status: "failed",
            node: `node-${(i % 3) + 1}`,
            cpu: "0.0%",
            memory: "0 B",
            uptime: "—",
            fullContainerId: "",
            fullNodeId: "",
        });
    }
    return rows;
}

type FilterTab = "all" | "running" | "pending" | "failed";

const statusStyles: Record<string, string> = {
    running: "bg-primary/10 text-primary border-primary/20",
    pending: "bg-warning/10 text-warning border-warning/20",
    failed: "bg-destructive/10 text-destructive border-destructive/20",
};

const statusDotStyles: Record<string, string> = {
    running: "bg-primary",
    pending: "bg-warning animate-pulse-dot",
    failed: "bg-destructive",
};

export function ContainerTable({ agents, taskStats }: ContainerTableProps) {
    const [filter, setFilter] = useState<FilterTab>("all");
    const { openExec } = useTerminal();

    const rows = useMemo(() => {
        const agentRows = buildContainerRows(agents);
        if (agentRows.length > 0) return agentRows;
        if (taskStats) return buildPlaceholderRows(taskStats);
        return [];
    }, [agents, taskStats]);

    const filtered = filter === "all" ? rows : rows.filter((r) => r.status === filter);
    const runningCount = rows.filter((r) => r.status === "running").length;
    const tabs: { key: FilterTab; label: string }[] = [
        { key: "all", label: "All" },
        { key: "running", label: "Running" },
        { key: "pending", label: "Pending" },
        { key: "failed", label: "Failed" },
    ];

    return (
        <Card className='animate-fade-in-up' style={{ animationDelay: "300ms" }}>
            <CardHeader className='pb-3'>
                <div className='flex items-center justify-between'>
                    <div className='flex items-center gap-3'>
                        <h3 className='text-sm font-medium'>Tasks</h3>
                        <span className='text-xs text-muted-foreground'>
                            {runningCount} of {rows.length} running
                        </span>
                    </div>
                    <div className='flex gap-1'>
                        {tabs.map((tab) => (
                            <button
                                key={tab.key}
                                onClick={() => setFilter(tab.key)}
                                className={`rounded-md px-2.5 py-1 text-[11px] font-medium transition-colors ${
                                    filter === tab.key ? 'bg-primary/10 text-primary' : 'text-muted-foreground hover:bg-accent'
                                }`}
                            >
                                {tab.label}
                            </button>
                        ))}
                    </div>
                </div>
            </CardHeader>
            <CardContent className='p-0'>
                {filtered.length === 0 ? (
                    <div className='py-8 text-center text-sm text-muted-foreground'>No tasks match this filter</div>
                ) : (
                    <Table>
                        <TableHeader>
                            <TableRow>
                                <TableHead>Task</TableHead>
                                <TableHead>Image</TableHead>
                                <TableHead>Status</TableHead>
                                <TableHead>Node</TableHead>
                                <TableHead className='text-right'>CPU</TableHead>
                                <TableHead className='text-right'>Memory</TableHead>
                                <TableHead className='text-right'>Uptime</TableHead>
                                <TableHead className='w-16' />
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {filtered.map((row) => (
                                <TableRow key={row.id} className='group'>
                                    <TableCell>
                                        <div>
                                            <span className='text-sm font-medium'>{row.name}</span>
                                            <p className='font-mono text-[11px] text-muted-foreground'>{row.id}</p>
                                        </div>
                                    </TableCell>
                                    <TableCell>
                                        <span className='font-mono text-xs text-muted-foreground'>{row.image}</span>
                                    </TableCell>
                                    <TableCell>
                                        <Badge variant='outline' className={statusStyles[row.status]}>
                                            <span className={`mr-1.5 inline-block size-1.5 rounded-full ${statusDotStyles[row.status]}`} />
                                            {row.status}
                                        </Badge>
                                    </TableCell>
                                    <TableCell>
                                        <span className='inline-flex items-center gap-1.5 font-mono text-xs'>
                                            <Server className='size-3 text-muted-foreground' />
                                            {row.node}
                                        </span>
                                    </TableCell>
                                    <TableCell className='text-right font-mono text-xs'>{row.cpu}</TableCell>
                                    <TableCell className='text-right font-mono text-xs'>{row.memory}</TableCell>
                                    <TableCell className='text-right font-mono text-xs'>{row.uptime}</TableCell>
                                    <TableCell>
                                        <div className='flex justify-end gap-1 opacity-0 transition-opacity group-hover:opacity-100'>
                                            {row.status === "running" && row.fullContainerId && (
                                                <Button
                                                    variant='ghost'
                                                    size='sm'
                                                    className='size-7 p-0'
                                                    onClick={() =>
                                                        openExec({
                                                            containerId: row.fullContainerId,
                                                            nodeId: row.fullNodeId,
                                                            containerName: row.name,
                                                        })
                                                    }
                                                >
                                                    <Terminal className='size-3' />
                                                </Button>
                                            )}
                                            {row.status === "running" ? (
                                                <Button variant='ghost' size='sm' className='size-7 p-0'>
                                                    <Pause className='size-3' />
                                                </Button>
                                            ) : (
                                                <Button variant='ghost' size='sm' className='size-7 p-0'>
                                                    <Play className='size-3' />
                                                </Button>
                                            )}
                                            <Button variant='ghost' size='sm' className='size-7 p-0'>
                                                <RotateCw className='size-3' />
                                            </Button>
                                            <Button variant='ghost' size='sm' className='size-7 p-0'>
                                                <MoreHorizontal className='size-3' />
                                            </Button>
                                        </div>
                                    </TableCell>
                                </TableRow>
                            ))}
                        </TableBody>
                    </Table>
                )}
            </CardContent>
        </Card>
    );
}

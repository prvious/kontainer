import { useMemo, useState } from "react";
import { useServiceTasks } from "@/hooks/use-service-tasks";
import { useClusterMetrics } from "@/hooks/use-cluster-metrics";
import { TaskDetailSheet } from "@/components/task-detail-sheet";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { AlertCircle, ListTodo } from "lucide-react";
import type { ServiceTaskItem } from "@/types/service-tasks";
import type { ContainerMetrics } from "@/types/metrics";

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

function StateBadge({ state }: { state: string }) {
    const variants: Record<string, "default" | "secondary" | "destructive"> = {
        running: "default",
        complete: "secondary",
        failed: "destructive",
        rejected: "destructive",
        shutdown: "secondary",
        preparing: "secondary",
        ready: "secondary",
        starting: "secondary",
        assigned: "secondary",
        accepted: "secondary",
        pending: "secondary",
    };

    return <Badge variant={variants[state] ?? "secondary"}>{state}</Badge>;
}

function TasksTable({
    tasks,
    onTaskClick,
    containerMetricsMap,
}: {
    tasks: ServiceTaskItem[];
    onTaskClick: (taskId: string) => void;
    containerMetricsMap: Record<string, ContainerMetrics>;
}) {
    return (
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead className='w-16'>Slot</TableHead>
                    <TableHead>State</TableHead>
                    <TableHead>Desired</TableHead>
                    <TableHead>Node</TableHead>
                    <TableHead>Exit Code</TableHead>
                    <TableHead>Container</TableHead>
                    <TableHead>CPU</TableHead>
                    <TableHead>Memory</TableHead>
                    <TableHead className='text-right'>Updated</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {tasks.map((task) => {
                    const cm = task.containerId ? containerMetricsMap[task.containerId] : undefined;
                    return (
                        <TableRow key={task.id} className='cursor-pointer' onClick={() => onTaskClick(task.id)} data-testid={`task-row-${task.id}`}>
                            <TableCell className='font-mono text-xs'>{task.slot || "-"}</TableCell>
                            <TableCell>
                                <div className='flex items-center gap-2'>
                                    <StateBadge state={task.state} />
                                    {task.error && (
                                        <TooltipProvider>
                                            <Tooltip>
                                                <TooltipTrigger>
                                                    <AlertCircle className='size-3.5 text-destructive' />
                                                </TooltipTrigger>
                                                <TooltipContent>
                                                    <p className='max-w-xs text-xs'>{task.error}</p>
                                                </TooltipContent>
                                            </Tooltip>
                                        </TooltipProvider>
                                    )}
                                </div>
                            </TableCell>
                            <TableCell>
                                <StateBadge state={task.desiredState} />
                            </TableCell>
                            <TableCell>
                                <span className='text-sm'>{task.nodeName || "-"}</span>
                            </TableCell>
                            <TableCell>
                                {task.exitCode !== null ? (
                                    <span className={`font-mono text-xs ${task.exitCode !== 0 ? "text-destructive" : ""}`}>{task.exitCode}</span>
                                ) : (
                                    <span className='text-xs text-muted-foreground'>-</span>
                                )}
                            </TableCell>
                            <TableCell>
                                {task.containerId ? (
                                    <span className='font-mono text-xs text-muted-foreground'>{task.containerId}</span>
                                ) : (
                                    <span className='text-xs text-muted-foreground'>-</span>
                                )}
                            </TableCell>
                            <TableCell>
                                {cm ? (
                                    <span className='font-mono text-xs'>{cm.cpuPercent.toFixed(1)}%</span>
                                ) : (
                                    <span className='text-xs text-muted-foreground'>-</span>
                                )}
                            </TableCell>
                            <TableCell>
                                {cm ? (
                                    <span className='font-mono text-xs'>
                                        {formatBytes(cm.memoryUsed)} / {formatBytes(cm.memoryLimit)}
                                    </span>
                                ) : (
                                    <span className='text-xs text-muted-foreground'>-</span>
                                )}
                            </TableCell>
                            <TableCell className='text-right'>
                                <span className='text-xs text-muted-foreground'>{formatRelativeTime(task.timestamp)}</span>
                            </TableCell>
                        </TableRow>
                    );
                })}
            </TableBody>
        </Table>
    );
}

export function ServiceTasks({ serviceId }: { serviceId: string }) {
    const { data: tasks, isLoading, error } = useServiceTasks(serviceId);
    const { data: metricsData } = useClusterMetrics();
    const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null);

    const containerMetricsMap = useMemo(() => {
        const map: Record<string, ContainerMetrics> = {};
        for (const agent of metricsData?.agents ?? []) {
            for (const c of agent.containers) {
                map[c.containerId] = c;
            }
        }
        return map;
    }, [metricsData]);

    if (isLoading) {
        return (
            <Card>
                <CardHeader>
                    <CardTitle className='text-sm'>Tasks</CardTitle>
                </CardHeader>
                <CardContent>
                    <div className='space-y-3'>
                        {[1, 2, 3].map((i) => (
                            <Skeleton key={i} className='h-8 w-full' />
                        ))}
                    </div>
                </CardContent>
            </Card>
        );
    }

    if (error) {
        return (
            <Card>
                <CardHeader>
                    <CardTitle className='text-sm'>Tasks</CardTitle>
                </CardHeader>
                <CardContent>
                    <div className='flex flex-col items-center justify-center gap-2 py-8 text-sm text-destructive'>
                        <AlertCircle className='size-5' />
                        <p>Failed to load tasks</p>
                    </div>
                </CardContent>
            </Card>
        );
    }

    if (!tasks || tasks.length === 0) {
        return (
            <Card>
                <CardHeader>
                    <CardTitle className='text-sm'>Tasks</CardTitle>
                </CardHeader>
                <CardContent>
                    <div className='flex flex-col items-center justify-center gap-2 py-8 text-sm text-muted-foreground'>
                        <ListTodo className='size-5' />
                        <p>No tasks found</p>
                    </div>
                </CardContent>
            </Card>
        );
    }

    return (
        <>
            <Card>
                <CardHeader>
                    <CardTitle className='text-sm'>Tasks ({tasks.length})</CardTitle>
                </CardHeader>
                <CardContent className='p-0'>
                    <TasksTable tasks={tasks} onTaskClick={(id) => setSelectedTaskId(id)} containerMetricsMap={containerMetricsMap} />
                </CardContent>
            </Card>
            <TaskDetailSheet taskId={selectedTaskId} open={selectedTaskId !== null} onOpenChange={(open) => !open && setSelectedTaskId(null)} />
        </>
    );
}

import { useState } from "react";
import { useServiceTasks } from "@/hooks/use-service-tasks";
import { TaskDetailSheet } from "@/components/task-detail-sheet";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { AlertCircle, ListTodo } from "lucide-react";
import type { ServiceTaskItem } from "@/types/service-tasks";

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

function TasksTable({ tasks, onTaskClick }: { tasks: ServiceTaskItem[]; onTaskClick: (taskId: string) => void }) {
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
                    <TableHead className='text-right'>Updated</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {tasks.map((task) => (
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
                        <TableCell className='text-right'>
                            <span className='text-xs text-muted-foreground'>{formatRelativeTime(task.timestamp)}</span>
                        </TableCell>
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}

export function ServiceTasks({ serviceId }: { serviceId: string }) {
    const { data: tasks, isLoading, error } = useServiceTasks(serviceId);
    const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null);

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
                    <TasksTable tasks={tasks} onTaskClick={(id) => setSelectedTaskId(id)} />
                </CardContent>
            </Card>
            <TaskDetailSheet taskId={selectedTaskId} open={selectedTaskId !== null} onOpenChange={(open) => !open && setSelectedTaskId(null)} />
        </>
    );
}

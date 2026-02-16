import { createFileRoute, Link } from "@tanstack/react-router";
import { useTasks } from "@/hooks/use-tasks";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Activity, AlertCircle, ArrowUpRight, Search } from "lucide-react";
import { useState } from "react";
import { formatRelativeTime, getStateColor } from "@/lib/task-utils";
import type { TaskItem } from "@/types/tasks";

function TasksTable({ tasks }: { tasks: TaskItem[] }) {
    return (
        <Table>
            <TableHeader>
                <TableRow className='bg-muted/40 hover:bg-muted/40'>
                    <TableHead className='h-7 w-6 pr-0 pl-3' />
                    <TableHead className='h-7 text-[11px]'>Service</TableHead>
                    <TableHead className='h-7 text-[11px]'>Node</TableHead>
                    <TableHead className='h-7 text-[11px]'>Image</TableHead>
                    <TableHead className='h-7 text-[11px]'>State</TableHead>
                    <TableHead className='h-7 text-right text-[11px]'>Updated</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {tasks.map((task) => (
                    <TableRow key={task.id} className='group'>
                        <TableCell className='py-1.5 pr-0 pl-3'>
                            <span className={`size-1.5 rounded-full ${getStateColor(task.state)}`} title={task.state} />
                        </TableCell>
                        <TableCell className='py-1.5'>
                            <Link
                                to='/services/$serviceId'
                                params={{ serviceId: task.serviceId }}
                                className='inline-flex items-center gap-1 text-xs font-medium hover:underline'
                            >
                                {task.serviceName}
                                <ArrowUpRight className='size-3 opacity-0 transition-opacity group-hover:opacity-100' />
                            </Link>
                        </TableCell>
                        <TableCell className='py-1.5'>
                            <span className='text-[11px] text-muted-foreground'>{task.nodeName}</span>
                        </TableCell>
                        <TableCell className='py-1.5'>
                            <span className='font-mono text-[11px] text-muted-foreground'>{task.image}</span>
                        </TableCell>
                        <TableCell className='py-1.5'>
                            <Badge variant={task.state.toLowerCase() === "running" ? "default" : "secondary"} className='h-4 px-1.5 text-[10px]'>
                                {task.state}
                            </Badge>
                        </TableCell>
                        <TableCell className='py-1.5 text-right'>
                            <span className='font-mono text-[11px] text-muted-foreground'>{formatRelativeTime(task.timestamp)}</span>
                        </TableCell>
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}

function LoadingSkeleton() {
    return (
        <div className='flex flex-col'>
            <div className='flex h-10 items-center gap-3 border-b border-border px-3'>
                <Skeleton className='h-4 w-12' />
                <Skeleton className='h-4 w-8 rounded-full' />
                <div className='flex-1' />
                <Skeleton className='h-6 w-40' />
            </div>
            <div className='space-y-0'>
                {[1, 2, 3, 4, 5].map((i) => (
                    <div key={i} className='flex items-center gap-3 border-b border-border px-3 py-2 last:border-0'>
                        <Skeleton className='size-1.5 rounded-full' />
                        <Skeleton className='h-3 w-28' />
                        <Skeleton className='h-3 w-20' />
                        <Skeleton className='h-3 w-40' />
                        <Skeleton className='ml-auto h-4 w-14 rounded-full' />
                    </div>
                ))}
            </div>
        </div>
    );
}

function TasksPage() {
    const { data: tasks, isLoading, error } = useTasks();
    const [search, setSearch] = useState("");

    const filtered = (tasks ?? []).filter(
        (task) =>
            task.serviceName.toLowerCase().includes(search.toLowerCase()) ||
            task.nodeName.toLowerCase().includes(search.toLowerCase()) ||
            task.image.toLowerCase().includes(search.toLowerCase()) ||
            task.state.toLowerCase().includes(search.toLowerCase()),
    );

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

    return (
        <div className='flex h-full flex-col'>
            <div className='flex h-10 shrink-0 items-center gap-3 border-b border-border px-3'>
                <h1 className='text-xs font-semibold'>Tasks</h1>
                <span className='rounded-md bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground'>{tasks?.length ?? 0}</span>
                <div className='flex-1' />
                <div className='relative'>
                    <Search className='absolute top-1/2 left-2 size-3 -translate-y-1/2 text-muted-foreground' />
                    <Input
                        placeholder='Filter tasks...'
                        value={search}
                        onChange={(e) => setSearch(e.target.value)}
                        className='h-6 w-48 rounded-sm border-border bg-transparent pl-7 text-[11px] focus-visible:ring-1'
                    />
                </div>
            </div>
            <div className='flex-1 overflow-y-auto'>
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                        <Activity className='size-4' />
                        <p>{search ? "No tasks match your filter" : "No tasks found"}</p>
                    </div>
                ) : (
                    <TasksTable tasks={filtered} />
                )}
            </div>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/tasks/")({
    component: TasksPage,
});

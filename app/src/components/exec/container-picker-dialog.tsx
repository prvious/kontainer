import { useMemo, useState } from "react";
import { Search } from "lucide-react";
import { useTasks } from "@/hooks/use-tasks";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import type { TaskItem } from "@/types/tasks";

interface ContainerPickerDialogProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    onSelect: (target: { containerId: string; nodeId: string; containerName: string }) => void;
    excludeContainerIds?: string[];
}

function statusDotClass(state: string): string {
    const s = state.toLowerCase();
    if (s === "running") return "bg-accent-green";
    if (s === "preparing" || s === "paused") return "bg-accent-orange";
    return "bg-accent-red";
}

function ContainerPickerRow({ task, excluded, onSelect }: { task: TaskItem; excluded: boolean; onSelect: () => void }) {
    const isRunning = task.state.toLowerCase() === "running";
    const clickable = isRunning && !excluded;

    return (
        <button
            onClick={clickable ? onSelect : undefined}
            disabled={!clickable}
            className={`flex w-full items-center gap-3 rounded-md px-3 py-2.5 text-left transition-colors ${
                clickable ? "hover:bg-accent" : 'cursor-not-allowed opacity-40'
            }`}
        >
            <span className={`mt-0.5 size-2 shrink-0 rounded-full ${statusDotClass(task.state)}`} />
            <div className='min-w-0 flex-1'>
                <p className='truncate font-mono text-sm font-semibold'>{task.serviceName}</p>
                <p className='truncate font-mono text-xs text-muted-foreground'>{task.image}</p>
            </div>
            <div className='flex shrink-0 flex-col items-end gap-0.5'>
                {excluded ? (
                    <span className='font-mono text-[10px] text-muted-foreground'>Already open</span>
                ) : (
                    <span className='font-mono text-[10px] text-muted-foreground'>{task.nodeName}</span>
                )}
            </div>
        </button>
    );
}

function SkeletonRows() {
    return (
        <div className='space-y-1 p-1'>
            {[1, 2, 3, 4].map((i) => (
                <div key={i} className='flex items-center gap-3 rounded-md px-3 py-2.5'>
                    <Skeleton className='size-2 rounded-full' />
                    <div className='flex-1 space-y-1.5'>
                        <Skeleton className='h-3.5 w-36' />
                        <Skeleton className='h-3 w-52' />
                    </div>
                    <Skeleton className='h-3 w-14' />
                </div>
            ))}
        </div>
    );
}

export function ContainerPickerDialog({ open, onOpenChange, onSelect, excludeContainerIds = [] }: ContainerPickerDialogProps) {
    const [search, setSearch] = useState("");

    const { data: tasks, isLoading, error, refetch } = useTasks();

    const filtered = useMemo(() => {
        if (!tasks) return [];
        const query = search.trim().toLowerCase();
        if (!query) return tasks;
        return tasks.filter(
            (t) => t.serviceName.toLowerCase().includes(query) || t.image.toLowerCase().includes(query) || t.containerId.toLowerCase().includes(query),
        );
    }, [tasks, search]);

    const excludedSet = useMemo(() => new Set(excludeContainerIds), [excludeContainerIds]);

    function handleSelect(task: TaskItem) {
        onSelect({ containerId: task.containerId, nodeId: task.nodeId, containerName: task.serviceName });
        onOpenChange(false);
    }

    const hasTasks = tasks && tasks.length > 0;
    const hasResults = filtered.length > 0;

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent showCloseButton={false} className='gap-0 p-0 sm:max-w-md'>
                <DialogHeader className='border-b border-border px-4 pt-4 pb-3'>
                    <DialogTitle>Select Container</DialogTitle>
                    <DialogDescription>Choose a container to exec into</DialogDescription>
                </DialogHeader>

                {/* Search input */}
                <div className='flex items-center gap-2 border-b border-border px-3'>
                    <Search className='size-4 shrink-0 text-muted-foreground' />
                    <input
                        value={search}
                        onChange={(e) => setSearch(e.target.value)}
                        placeholder='Search by name, image, or container ID...'
                        className='flex-1 bg-transparent py-3 text-sm outline-none placeholder:text-muted-foreground'
                        autoFocus
                    />
                </div>

                {/* Container list */}
                <ScrollArea className='max-h-80'>
                    {isLoading ? (
                        <SkeletonRows />
                    ) : error ? (
                        <div className='flex flex-col items-center gap-2 px-4 py-8 text-center'>
                            <p className='text-sm text-destructive'>Failed to load containers</p>
                            <p className='text-xs text-muted-foreground'>
                                {typeof error === "string" ? error : error instanceof Error ? error.message : "Check your connection"}
                            </p>
                            <button onClick={() => refetch()} className='mt-1 font-mono text-xs font-semibold text-accent-cyan hover:underline'>
                                Retry
                            </button>
                        </div>
                    ) : !hasTasks ? (
                        <div className='px-4 py-8 text-center'>
                            <p className='text-sm text-muted-foreground'>No containers found</p>
                        </div>
                    ) : !hasResults ? (
                        <div className='px-4 py-8 text-center'>
                            <p className='text-sm text-muted-foreground'>No containers match</p>
                        </div>
                    ) : (
                        <div className='p-1'>
                            {filtered.map((task) => (
                                <ContainerPickerRow
                                    key={task.id}
                                    task={task}
                                    excluded={excludedSet.has(task.containerId)}
                                    onSelect={() => handleSelect(task)}
                                />
                            ))}
                        </div>
                    )}
                </ScrollArea>
            </DialogContent>
        </Dialog>
    );
}

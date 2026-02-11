import { useCallback, useMemo, useState } from "react";
import { useNodeTasks } from "@/hooks/use-node-tasks";
import { useUpdateNodeAvailability } from "@/hooks/use-update-node-availability";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { AlertCircle, ArrowRight, CheckCircle2, Loader2, Power, XCircle } from "lucide-react";
import type { NodeItem } from "@/types/nodes";
import type { NodeTaskItem } from "@/types/node-tasks";

type DrainPhase = "confirm" | "draining" | "completed" | "error";

interface TaskMigrationInfo {
    task: NodeTaskItem;
    status: "migrating" | "migrated";
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

function MigrationStatusIcon({ status }: { status: TaskMigrationInfo["status"] }) {
    if (status === "migrating") {
        return <Loader2 className='size-3.5 animate-spin text-amber-500' />;
    }
    return <CheckCircle2 className='size-3.5 text-success' />;
}

interface NodeDrainDialogProps {
    node: NodeItem;
    open: boolean;
    onOpenChange: (open: boolean) => void;
}

export function NodeDrainDialog({ node, open, onOpenChange }: NodeDrainDialogProps) {
    const [phase, setPhase] = useState<DrainPhase>("confirm");
    const [errorMessage, setErrorMessage] = useState<string | null>(null);
    const [initialTasks, setInitialTasks] = useState<NodeTaskItem[] | null>(null);

    const { mutate: updateAvailability, isPending: isUpdating } = useUpdateNodeAvailability(node.id);

    // Only fetch tasks when draining to track migration progress
    const { data: currentTasks } = useNodeTasks(phase === "draining" ? node.id : null, {
        refetchInterval: phase === "draining" ? 1000 : false,
    });

    // Compute migrations from initial tasks and current state
    const { migrations, progress, allMigrated } = useMemo(() => {
        if (phase !== "draining" || initialTasks === null) {
            return { migrations: [], progress: 0, allMigrated: false };
        }

        if (initialTasks.length === 0) {
            return { migrations: [], progress: 100, allMigrated: true };
        }

        const currentTaskIds = new Set(
            (currentTasks ?? []).filter((t) => t.state === "running" || t.state === "starting" || t.state === "preparing").map((t) => t.id),
        );

        const migrationsList: TaskMigrationInfo[] = initialTasks.map((task) => {
            const isMigrated = !currentTaskIds.has(task.id);
            return {
                task,
                status: isMigrated ? "migrated" : "migrating",
            };
        });

        const migratedCount = migrationsList.filter((m) => m.status === "migrated").length;
        const progressPct = migrationsList.length > 0 ? Math.round((migratedCount / migrationsList.length) * 100) : 100;
        const allDone = migratedCount === migrationsList.length;

        return { migrations: migrationsList, progress: progressPct, allMigrated: allDone };
    }, [phase, initialTasks, currentTasks]);

    // Handle first data load to capture initial tasks
    const handleDataLoaded = useCallback(
        (tasks: NodeTaskItem[]) => {
            if (initialTasks === null) {
                const running = tasks.filter((t) => t.state === "running" || t.state === "starting" || t.state === "preparing");
                setInitialTasks(running);
            }
        },
        [initialTasks],
    );

    // Check when we should capture initial tasks and when drain is complete
    if (phase === "draining" && currentTasks && initialTasks === null) {
        handleDataLoaded(currentTasks);
    }

    if (phase === "draining" && initialTasks !== null && allMigrated) {
        // Transition to completed via queueMicrotask to avoid state update during render
        queueMicrotask(() => setPhase("completed"));
    }

    const handleStartDrain = useCallback(() => {
        setPhase("draining");

        updateAvailability("drain", {
            onError: (error) => {
                setPhase("error");
                setErrorMessage(String(error));
            },
        });
    }, [updateAvailability]);

    const handleClose = useCallback(() => {
        // Reset state when closing
        setPhase("confirm");
        setErrorMessage(null);
        setInitialTasks(null);
        onOpenChange(false);
    }, [onOpenChange]);

    return (
        <Dialog open={open} onOpenChange={handleClose}>
            <DialogContent className='sm:max-w-lg' showCloseButton={phase !== "draining"}>
                <DialogHeader>
                    <DialogTitle className='flex items-center gap-2'>
                        <Power className='size-4' />
                        {phase === "confirm" && `Drain Node: ${node.hostname}`}
                        {phase === "draining" && "Draining Node..."}
                        {phase === "completed" && "Node Drained"}
                        {phase === "error" && "Drain Failed"}
                    </DialogTitle>
                    <DialogDescription>
                        {phase === "confirm" && "Draining will reschedule all running tasks to other nodes in the cluster."}
                        {phase === "draining" && "Tasks are being migrated to other nodes. This may take a moment."}
                        {phase === "completed" && "All tasks have been successfully migrated to other nodes."}
                        {phase === "error" && "An error occurred while draining the node."}
                    </DialogDescription>
                </DialogHeader>

                {phase === "confirm" && (
                    <div className='space-y-4'>
                        <div className='rounded-md border border-amber-500/30 bg-amber-500/5 p-3'>
                            <div className='flex items-start gap-2'>
                                <AlertCircle className='mt-0.5 size-4 text-amber-500' />
                                <div className='text-sm'>
                                    <p className='font-medium text-amber-500'>Warning</p>
                                    <p className='mt-1 text-muted-foreground'>
                                        Running tasks on <span className='font-medium'>{node.hostname}</span> will be stopped and rescheduled to other available
                                        nodes.
                                    </p>
                                </div>
                            </div>
                        </div>
                    </div>
                )}

                {phase === "draining" && (
                    <div className='space-y-4'>
                        <div className='space-y-2'>
                            <div className='flex items-center justify-between text-sm'>
                                <span className='text-muted-foreground'>Migration Progress</span>
                                <span className='font-medium'>{progress}%</span>
                            </div>
                            <Progress value={progress} />
                        </div>

                        {migrations.length > 0 && (
                            <div className='max-h-64 overflow-auto rounded-md border'>
                                <Table>
                                    <TableHeader>
                                        <TableRow>
                                            <TableHead className='w-8'>Status</TableHead>
                                            <TableHead>Service</TableHead>
                                            <TableHead>State</TableHead>
                                            <TableHead className='text-right'>Slot</TableHead>
                                        </TableRow>
                                    </TableHeader>
                                    <TableBody>
                                        {migrations.map(({ task, status }) => (
                                            <TableRow key={task.id}>
                                                <TableCell>
                                                    <MigrationStatusIcon status={status} />
                                                </TableCell>
                                                <TableCell>
                                                    <span className='text-sm font-medium'>{task.serviceName}</span>
                                                </TableCell>
                                                <TableCell>
                                                    <div className='flex items-center gap-1.5'>
                                                        <StateBadge state={task.state} />
                                                        {status === "migrated" && (
                                                            <>
                                                                <ArrowRight className='size-3 text-muted-foreground' />
                                                                <Badge variant='secondary'>migrated</Badge>
                                                            </>
                                                        )}
                                                    </div>
                                                </TableCell>
                                                <TableCell className='text-right'>
                                                    <span className='font-mono text-xs text-muted-foreground'>{task.slot}</span>
                                                </TableCell>
                                            </TableRow>
                                        ))}
                                    </TableBody>
                                </Table>
                            </div>
                        )}

                        {initialTasks === null && (
                            <div className='flex flex-col items-center justify-center gap-2 py-6 text-sm text-muted-foreground'>
                                <Loader2 className='size-5 animate-spin' />
                                <p>Checking for running tasks...</p>
                            </div>
                        )}

                        {initialTasks !== null && initialTasks.length === 0 && (
                            <div className='flex flex-col items-center justify-center gap-2 py-6 text-sm text-muted-foreground'>
                                <CheckCircle2 className='size-5 text-success' />
                                <p>No running tasks to migrate</p>
                            </div>
                        )}
                    </div>
                )}

                {phase === "completed" && (
                    <div className='flex flex-col items-center justify-center gap-3 py-6'>
                        <div className='flex size-12 items-center justify-center rounded-full bg-success/10'>
                            <CheckCircle2 className='size-6 text-success' />
                        </div>
                        <div className='text-center'>
                            <p className='font-medium'>Drain Complete</p>
                            <p className='mt-1 text-sm text-muted-foreground'>
                                {initialTasks?.length ?? 0} {(initialTasks?.length ?? 0) === 1 ? "task was" : "tasks were"} migrated successfully.
                            </p>
                        </div>
                    </div>
                )}

                {phase === "error" && (
                    <div className='space-y-4'>
                        <div className='rounded-md border border-destructive/30 bg-destructive/5 p-3'>
                            <div className='flex items-start gap-2'>
                                <XCircle className='mt-0.5 size-4 text-destructive' />
                                <div className='text-sm'>
                                    <p className='font-medium text-destructive'>Error</p>
                                    <p className='mt-1 text-muted-foreground'>{errorMessage || "An unknown error occurred"}</p>
                                </div>
                            </div>
                        </div>
                    </div>
                )}

                <DialogFooter>
                    {phase === "confirm" && (
                        <>
                            <Button variant='outline' onClick={handleClose}>
                                Cancel
                            </Button>
                            <Button variant='destructive' onClick={handleStartDrain} disabled={isUpdating}>
                                {isUpdating ? (
                                    <>
                                        <Loader2 className='size-3.5 animate-spin' />
                                        Starting...
                                    </>
                                ) : (
                                    <>
                                        <Power className='size-3.5' />
                                        Drain Node
                                    </>
                                )}
                            </Button>
                        </>
                    )}
                    {phase === "draining" && <p className='text-xs text-muted-foreground'>Please wait while tasks are being migrated...</p>}
                    {(phase === "completed" || phase === "error") && (
                        <Button variant='outline' onClick={handleClose}>
                            Close
                        </Button>
                    )}
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}

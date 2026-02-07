import { useState } from "react";
import { useStacks } from "@/hooks/use-stacks";
import { useRemoveStack } from "@/hooks/use-remove-stack";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import {
    AlertDialog,
    AlertDialogAction,
    AlertDialogCancel,
    AlertDialogContent,
    AlertDialogDescription,
    AlertDialogFooter,
    AlertDialogHeader,
    AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { AlertCircle, Layers, Loader2, Trash2 } from "lucide-react";
import type { StackItem } from "@/types/stacks";

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

function HealthDot({ stack }: { stack: StackItem }) {
    const healthy = stack.runningReplicas >= stack.desiredReplicas && stack.desiredReplicas > 0;
    return <span className={`size-2 rounded-full ${healthy ? "bg-success" : 'animate-pulse-dot bg-destructive'}`} />;
}

function ReplicasBadge({ stack }: { stack: StackItem }) {
    const healthy = stack.runningReplicas === stack.desiredReplicas && stack.desiredReplicas > 0;
    return (
        <Badge variant={healthy ? "default" : "destructive"} className='font-mono'>
            {stack.runningReplicas}/{stack.desiredReplicas}
        </Badge>
    );
}

function StacksTable({ stacks }: { stacks: StackItem[] }) {
    const { mutate: removeStack, isPending } = useRemoveStack();
    const [confirmStack, setConfirmStack] = useState<string | null>(null);

    return (
        <>
            <Table>
                <TableHeader>
                    <TableRow>
                        <TableHead className='w-8' />
                        <TableHead>Stack</TableHead>
                        <TableHead>Services</TableHead>
                        <TableHead>Replicas</TableHead>
                        <TableHead className='text-right'>Updated</TableHead>
                        <TableHead className='w-12' />
                    </TableRow>
                </TableHeader>
                <TableBody>
                    {stacks.map((stack) => (
                        <TableRow key={stack.name} className='group'>
                            <TableCell>
                                <HealthDot stack={stack} />
                            </TableCell>
                            <TableCell>
                                <span className='inline-flex items-center gap-1.5 font-medium'>
                                    <Layers className='size-3.5 text-muted-foreground' />
                                    {stack.name}
                                </span>
                            </TableCell>
                            <TableCell>
                                <span className='font-mono text-sm'>{stack.services}</span>
                            </TableCell>
                            <TableCell>
                                <ReplicasBadge stack={stack} />
                            </TableCell>
                            <TableCell className='text-right'>
                                <span className='text-xs text-muted-foreground'>{formatRelativeTime(stack.updatedAt)}</span>
                            </TableCell>
                            <TableCell>
                                <Button
                                    variant='ghost'
                                    size='sm'
                                    className='size-7 p-0 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 hover:text-destructive'
                                    disabled={isPending}
                                    onClick={() => setConfirmStack(stack.name)}
                                    aria-label={`Remove stack ${stack.name}`}
                                >
                                    {isPending && confirmStack === stack.name ? <Loader2 className='size-3.5 animate-spin' /> : <Trash2 className='size-3.5' />}
                                </Button>
                            </TableCell>
                        </TableRow>
                    ))}
                </TableBody>
            </Table>

            <AlertDialog open={confirmStack !== null} onOpenChange={(open) => !open && setConfirmStack(null)}>
                <AlertDialogContent>
                    <AlertDialogHeader>
                        <AlertDialogTitle>Remove stack "{confirmStack}"?</AlertDialogTitle>
                        <AlertDialogDescription>This will remove all services in the stack. This action cannot be undone.</AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                        <AlertDialogCancel>Cancel</AlertDialogCancel>
                        <AlertDialogAction
                            variant='destructive'
                            onClick={() => {
                                if (confirmStack) {
                                    removeStack(confirmStack, { onSettled: () => setConfirmStack(null) });
                                }
                            }}
                        >
                            Remove
                        </AlertDialogAction>
                    </AlertDialogFooter>
                </AlertDialogContent>
            </AlertDialog>
        </>
    );
}

function LoadingSkeleton() {
    return (
        <Card>
            <CardContent className='p-0'>
                <div className='space-y-0'>
                    {[1, 2, 3].map((i) => (
                        <div key={i} className='flex items-center gap-4 border-b px-4 py-3 last:border-0'>
                            <Skeleton className='size-2 rounded-full' />
                            <Skeleton className='h-4 w-32' />
                            <Skeleton className='h-4 w-12' />
                            <Skeleton className='ml-auto h-5 w-16' />
                        </div>
                    ))}
                </div>
            </CardContent>
        </Card>
    );
}

export function StacksList() {
    const { data: stacks, isLoading, error } = useStacks();

    if (isLoading) {
        return <LoadingSkeleton />;
    }

    if (error) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
                <AlertCircle className='size-5' />
                <p>Failed to load stacks</p>
            </div>
        );
    }

    if (!stacks || stacks.length === 0) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-muted-foreground'>
                <Layers className='size-5' />
                <p>No stacks found</p>
            </div>
        );
    }

    return (
        <Card>
            <CardContent className='p-0'>
                <StacksTable stacks={stacks} />
            </CardContent>
        </Card>
    );
}

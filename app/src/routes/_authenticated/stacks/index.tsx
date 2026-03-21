import { createFileRoute, Link } from "@tanstack/react-router";
import { useStacks } from "@/hooks/use-stacks";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableRow } from "@/components/ui/table";
import { PageHeader } from "@/components/page-header";
import { TableShell } from "@/components/table-shell";
import { ColumnHeaders } from "@/components/column-headers";
import { AlertCircle, ArrowUpRight, Layers, Plus } from "lucide-react";
import { useState } from "react";
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
    return <span className={`size-1.5 rounded-full ${healthy ? "bg-success" : 'animate-pulse-dot bg-destructive'}`} />;
}

function ReplicasBadge({ stack }: { stack: StackItem }) {
    const healthy = stack.runningReplicas === stack.desiredReplicas && stack.desiredReplicas > 0;
    return (
        <Badge variant={healthy ? "default" : "destructive"} className='h-4 px-1.5 font-mono text-[10px]'>
            {stack.runningReplicas}/{stack.desiredReplicas}
        </Badge>
    );
}

const COLUMNS = [
    { label: "STATUS", width: 70 },
    { label: "NAME" },
    { label: "SERVICES", width: 80, align: "right" as const },
    { label: "REPLICAS", width: 80 },
    { label: "CPU", width: 60, align: "right" as const },
    { label: "MEM", width: 70, align: "right" as const },
    { label: "LAST DEPLOY", width: 100, align: "right" as const },
    { label: "Actions", width: 60 },
];

function StacksTable({ stacks }: { stacks: StackItem[] }) {
    return (
        <Table>
            <TableBody>
                {stacks.map((stack) => (
                    <TableRow key={stack.name} className='group'>
                        <TableCell className='py-1.5' style={{ width: 70 }}>
                            <HealthDot stack={stack} />
                        </TableCell>
                        <TableCell className='py-1.5'>
                            <Link
                                to='/stacks/$stackName'
                                params={{ stackName: stack.name }}
                                className='inline-flex items-center gap-1 text-xs font-medium hover:underline'
                            >
                                <Layers className='size-3 text-muted-foreground' />
                                {stack.name}
                                <ArrowUpRight className='size-3 opacity-0 transition-opacity group-hover:opacity-100' />
                            </Link>
                        </TableCell>
                        <TableCell className='py-1.5 text-right' style={{ width: 80 }}>
                            <span className='font-mono text-[11px]'>{stack.services}</span>
                        </TableCell>
                        <TableCell className='py-1.5' style={{ width: 80 }}>
                            <ReplicasBadge stack={stack} />
                        </TableCell>
                        <TableCell className='py-1.5 text-right' style={{ width: 60 }}>
                            <span className='font-mono text-[11px] text-muted-foreground'>—</span>
                        </TableCell>
                        <TableCell className='py-1.5 text-right' style={{ width: 70 }}>
                            <span className='font-mono text-[11px] text-muted-foreground'>—</span>
                        </TableCell>
                        <TableCell className='py-1.5 text-right' style={{ width: 100 }}>
                            <span className='font-mono text-[11px] text-muted-foreground'>{formatRelativeTime(stack.updatedAt)}</span>
                        </TableCell>
                        <TableCell className='py-1.5' style={{ width: 60 }} />
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}

function LoadingSkeleton() {
    return (
        <div className='flex flex-col gap-6 p-6'>
            <div className='flex items-center justify-between'>
                <div className='flex flex-col gap-1'>
                    <Skeleton className='h-7 w-24' />
                    <Skeleton className='h-3 w-40' />
                </div>
                <Skeleton className='h-8 w-32' />
            </div>
            <div className='flex flex-col overflow-hidden border border-border bg-card'>
                <div className='flex items-center justify-between border-b border-border px-4 py-3'>
                    <Skeleton className='h-4 w-32' />
                    <Skeleton className='h-7 w-48' />
                </div>
                <div className='space-y-0'>
                    {[1, 2, 3].map((i) => (
                        <div key={i} className='flex items-center gap-3 border-b border-border px-4 py-2 last:border-0'>
                            <Skeleton className='size-1.5 rounded-full' />
                            <Skeleton className='h-3 w-28' />
                            <Skeleton className='h-3 w-8' />
                            <Skeleton className='ml-auto h-4 w-12 rounded-full' />
                        </div>
                    ))}
                </div>
            </div>
        </div>
    );
}

export function StacksPage() {
    const { data: stacks, isLoading, error } = useStacks();
    const [search, setSearch] = useState("");

    const total = stacks?.length ?? 0;
    const healthy = (stacks ?? []).filter((s) => s.runningReplicas >= s.desiredReplicas && s.desiredReplicas > 0).length;
    const filtered = (stacks ?? []).filter((s) => s.name.toLowerCase().includes(search.toLowerCase()));

    if (isLoading) {
        return <LoadingSkeleton />;
    }

    if (error) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-xs text-destructive'>
                <AlertCircle className='size-4' />
                <p>Failed to load stacks</p>
            </div>
        );
    }

    return (
        <div className='flex h-full flex-col gap-6 overflow-y-auto p-6'>
            <PageHeader title='STACKS' subtitle={`${total} stacks · ${healthy} healthy`}>
                <Button variant='ghost' size='sm' className='h-7 px-3 text-[11px]' disabled>
                    <Plus className='size-3' />
                    DEPLOY STACK
                </Button>
            </PageHeader>

            <TableShell title='DEPLOYED STACKS' count={filtered.length} searchValue={search} onSearchChange={setSearch} searchPlaceholder='Search stacks...'>
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                        <Layers className='size-4' />
                        <p>{search ? "No stacks match your filter" : "No stacks found"}</p>
                    </div>
                ) : (
                    <>
                        <ColumnHeaders columns={COLUMNS} />
                        <StacksTable stacks={filtered} />
                    </>
                )}
            </TableShell>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/stacks/")({
    component: StacksPage,
});

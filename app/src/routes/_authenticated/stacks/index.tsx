import { createFileRoute, Link } from "@tanstack/react-router";
import { useStacks } from "@/hooks/use-stacks";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { PageHeader } from "@/components/page-header";
import { TableShell } from "@/components/table-shell";
import { TableToolbar } from "@/components/table-toolbar";
import { ColumnHeaders } from "@/components/column-headers";
import { RefreshButton } from "@/components/refresh-button";
import { AlertCircle, ArrowUpRight, Layers, Plus } from "lucide-react";
import { useState } from "react";
import type { StackItem } from "@/types/stacks";
import { formatRelativeTime } from "@/lib/format-utils";

const QUERY_KEYS: string[][] = [["stacks"]];

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
    { label: "", width: 60 },
];

function StackRow({ stack }: { stack: StackItem }) {
    return (
        <div className='group flex items-center border-b border-border px-4 py-2.5 hover:bg-bg-elevated/50'>
            <div style={{ width: 70 }} className='flex items-center'>
                <HealthDot stack={stack} />
            </div>
            <div style={{ flex: 1 }} className='flex items-center'>
                <Link
                    to='/stacks/$stackName'
                    params={{ stackName: stack.name }}
                    className='inline-flex items-center gap-1 font-mono text-xs font-medium hover:underline'
                >
                    <Layers className='size-3 text-muted-foreground' />
                    {stack.name}
                    <ArrowUpRight className='size-3 opacity-0 transition-opacity group-hover:opacity-100' />
                </Link>
            </div>
            <div style={{ width: 80 }} className='text-right'>
                <span className='font-mono text-[11px]'>{stack.services}</span>
            </div>
            <div style={{ width: 80 }}>
                <ReplicasBadge stack={stack} />
            </div>
            <div style={{ width: 60 }} className='text-right'>
                <span className='font-mono text-[11px] text-muted-foreground'>—</span>
            </div>
            <div style={{ width: 70 }} className='text-right'>
                <span className='font-mono text-[11px] text-muted-foreground'>—</span>
            </div>
            <div style={{ width: 100 }} className='text-right'>
                <span className='font-mono text-[11px] text-muted-foreground'>{formatRelativeTime(stack.updatedAt)}</span>
            </div>
            <div style={{ width: 60 }} />
        </div>
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
            <div className='flex flex-col items-center justify-center gap-3 py-12'>
                <AlertCircle className='size-5 text-destructive' />
                <p className='text-sm text-destructive'>Failed to load stacks</p>
                <RefreshButton queryKeys={QUERY_KEYS} />
            </div>
        );
    }

    return (
        <div className='flex h-full flex-col gap-6 overflow-y-auto p-6'>
            <PageHeader title='STACKS' subtitle={`${total} stacks · ${healthy} healthy`}>
                <RefreshButton queryKeys={QUERY_KEYS} />
                <Button variant='ghost' size='sm' className='h-7 px-3 text-[11px]' disabled>
                    <Plus className='size-3' />
                    DEPLOY STACK
                </Button>
            </PageHeader>

            <TableToolbar searchValue={search} onSearchChange={setSearch} searchPlaceholder='Search stacks...' />

            <TableShell title='DEPLOYED STACKS' count={filtered.length}>
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                        <Layers className='size-4' />
                        <p>{search ? "No stacks match your filter" : "No stacks found"}</p>
                    </div>
                ) : (
                    <>
                        <ColumnHeaders columns={COLUMNS} />
                        <div>
                            {filtered.map((stack) => (
                                <StackRow key={stack.name} stack={stack} />
                            ))}
                        </div>
                    </>
                )}
            </TableShell>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/stacks/")({
    component: StacksPage,
});

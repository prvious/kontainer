import { createFileRoute, Link } from "@tanstack/react-router";
import { useStacks } from "@/hooks/use-stacks";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { AlertCircle, ArrowUpRight, Layers, Search } from "lucide-react";
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

function StacksTable({ stacks }: { stacks: StackItem[] }) {
    return (
        <Table>
            <TableHeader>
                <TableRow className='bg-muted/40 hover:bg-muted/40'>
                    <TableHead className='h-7 w-6 pr-0 pl-3' />
                    <TableHead className='h-7 text-[11px]'>Stack</TableHead>
                    <TableHead className='h-7 text-[11px]'>Services</TableHead>
                    <TableHead className='h-7 text-[11px]'>Replicas</TableHead>
                    <TableHead className='h-7 text-right text-[11px]'>Updated</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {stacks.map((stack) => (
                    <TableRow key={stack.name} className='group'>
                        <TableCell className='py-1.5 pr-0 pl-3'>
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
                        <TableCell className='py-1.5'>
                            <span className='font-mono text-[11px]'>{stack.services}</span>
                        </TableCell>
                        <TableCell className='py-1.5'>
                            <ReplicasBadge stack={stack} />
                        </TableCell>
                        <TableCell className='py-1.5 text-right'>
                            <span className='font-mono text-[11px] text-muted-foreground'>{formatRelativeTime(stack.updatedAt)}</span>
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
                <Skeleton className='h-4 w-14' />
                <Skeleton className='h-4 w-8 rounded-full' />
                <div className='flex-1' />
                <Skeleton className='h-6 w-40' />
            </div>
            <div className='space-y-0'>
                {[1, 2, 3].map((i) => (
                    <div key={i} className='flex items-center gap-3 border-b border-border px-3 py-2 last:border-0'>
                        <Skeleton className='size-1.5 rounded-full' />
                        <Skeleton className='h-3 w-28' />
                        <Skeleton className='h-3 w-8' />
                        <Skeleton className='ml-auto h-4 w-12 rounded-full' />
                    </div>
                ))}
            </div>
        </div>
    );
}

function StacksPage() {
    const { data: stacks, isLoading, error } = useStacks();
    const [search, setSearch] = useState("");

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
        <div className='flex h-full flex-col'>
            <div className='flex h-10 shrink-0 items-center gap-3 border-b border-border px-3'>
                <h1 className='text-xs font-semibold'>Stacks</h1>
                <span className='rounded-md bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground'>{stacks?.length ?? 0}</span>
                <div className='flex-1' />
                <div className='relative'>
                    <Search className='absolute top-1/2 left-2 size-3 -translate-y-1/2 text-muted-foreground' />
                    <Input
                        placeholder='Filter stacks...'
                        value={search}
                        onChange={(e) => setSearch(e.target.value)}
                        className='h-6 w-48 rounded-sm border-border bg-transparent pl-7 text-[11px] focus-visible:ring-1'
                    />
                </div>
            </div>
            <div className='flex-1 overflow-y-auto'>
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                        <Layers className='size-4' />
                        <p>{search ? "No stacks match your filter" : "No stacks found"}</p>
                    </div>
                ) : (
                    <StacksTable stacks={filtered} />
                )}
            </div>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/stacks/")({
    component: StacksPage,
});

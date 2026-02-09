import { createFileRoute, Link } from "@tanstack/react-router";
import { useStacks } from "@/hooks/use-stacks";
import { Card, CardContent } from "@/components/ui/card";
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
    return (
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead className='w-8' />
                    <TableHead>Stack</TableHead>
                    <TableHead>Services</TableHead>
                    <TableHead>Replicas</TableHead>
                    <TableHead className='text-right'>Updated</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {stacks.map((stack) => (
                    <TableRow key={stack.name} className='group'>
                        <TableCell>
                            <HealthDot stack={stack} />
                        </TableCell>
                        <TableCell>
                            <Link
                                to='/stacks/$stackName'
                                params={{ stackName: stack.name }}
                                className='inline-flex items-center gap-1.5 font-medium hover:underline'
                            >
                                <Layers className='size-3.5 text-muted-foreground' />
                                {stack.name}
                                <ArrowUpRight className='size-3 opacity-0 transition-opacity group-hover:opacity-100' />
                            </Link>
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
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}

function LoadingSkeleton() {
    return (
        <div className='p-6'>
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
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
                <AlertCircle className='size-5' />
                <p>Failed to load stacks</p>
            </div>
        );
    }

    return (
        <div className='p-6'>
            <div className='mb-6 flex items-center justify-between'>
                <div>
                    <h1 className='text-xl font-semibold tracking-tight'>Stacks</h1>
                    <p className='mt-1 text-sm text-muted-foreground'>{stacks?.length ?? 0} stacks</p>
                </div>
                <div className='relative'>
                    <Search className='absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground' />
                    <Input placeholder='Search stacks...' value={search} onChange={(e) => setSearch(e.target.value)} className='h-8 w-56 pl-8 text-sm' />
                </div>
            </div>
            {filtered.length === 0 ? (
                <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-muted-foreground'>
                    <Layers className='size-5' />
                    <p>{search ? "No stacks match your search" : "No stacks found"}</p>
                </div>
            ) : (
                <Card>
                    <CardContent className='p-0'>
                        <StacksTable stacks={filtered} />
                    </CardContent>
                </Card>
            )}
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/stacks/")({
    component: StacksPage,
});

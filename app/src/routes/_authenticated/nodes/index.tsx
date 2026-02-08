import { createFileRoute } from "@tanstack/react-router";
import { useNodes } from "@/hooks/use-nodes";
import { PageHeader } from "@/components/page-header";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { NodeActions } from "@/components/node-actions";
import { AlertCircle, Search, Server } from "lucide-react";
import { useState } from "react";
import type { NodeItem } from "@/types/nodes";

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

function StatusDot({ node }: { node: NodeItem }) {
    if (node.status === "down" || node.status === "disconnected") {
        return <span className='animate-pulse-dot size-2 rounded-full bg-destructive' title={node.status} />;
    }
    if (node.availability === "drain" || node.availability === "pause") {
        return <span className='size-2 rounded-full bg-amber-500' title={node.availability} />;
    }
    return <span className='size-2 rounded-full bg-success' title='ready' />;
}

function RoleBadge({ node }: { node: NodeItem }) {
    if (node.role === "manager") {
        return <Badge variant='default'>{node.leader ? "manager (leader)" : "manager"}</Badge>;
    }
    return <Badge variant='secondary'>worker</Badge>;
}

function AvailabilityBadge({ availability }: { availability: string }) {
    if (availability === "drain") {
        return <Badge variant='destructive'>drain</Badge>;
    }
    if (availability === "pause") {
        return <Badge variant='secondary'>pause</Badge>;
    }
    return <Badge variant='default'>active</Badge>;
}

function NodesTable({ nodes }: { nodes: NodeItem[] }) {
    return (
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead className='w-8' />
                    <TableHead>Hostname</TableHead>
                    <TableHead>Role</TableHead>
                    <TableHead>Availability</TableHead>
                    <TableHead>IP</TableHead>
                    <TableHead>Engine</TableHead>
                    <TableHead>OS / Arch</TableHead>
                    <TableHead className='text-right'>Updated</TableHead>
                    <TableHead className='w-10' />
                </TableRow>
            </TableHeader>
            <TableBody>
                {nodes.map((node) => (
                    <TableRow key={node.id}>
                        <TableCell>
                            <StatusDot node={node} />
                        </TableCell>
                        <TableCell className='font-medium'>{node.hostname}</TableCell>
                        <TableCell>
                            <RoleBadge node={node} />
                        </TableCell>
                        <TableCell>
                            <AvailabilityBadge availability={node.availability} />
                        </TableCell>
                        <TableCell>
                            <span className='font-mono text-xs text-muted-foreground'>{node.ip}</span>
                        </TableCell>
                        <TableCell>
                            <span className='font-mono text-xs text-muted-foreground'>{node.engineVersion}</span>
                        </TableCell>
                        <TableCell>
                            <span className='text-xs text-muted-foreground'>
                                {node.os}/{node.architecture}
                            </span>
                        </TableCell>
                        <TableCell className='text-right'>
                            <span className='text-xs text-muted-foreground'>{formatRelativeTime(node.updatedAt)}</span>
                        </TableCell>
                        <TableCell>
                            <NodeActions node={node} />
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
                        {[1, 2, 3, 4, 5].map((i) => (
                            <div key={i} className='flex items-center gap-4 border-b px-4 py-3 last:border-0'>
                                <Skeleton className='size-2 rounded-full' />
                                <Skeleton className='h-4 w-32' />
                                <Skeleton className='h-5 w-20' />
                                <Skeleton className='h-5 w-16' />
                                <Skeleton className='h-4 w-24' />
                                <Skeleton className='ml-auto h-4 w-16' />
                            </div>
                        ))}
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}

function NodesPage() {
    const { data: nodes, isLoading, error } = useNodes();
    const [search, setSearch] = useState("");

    const filtered = (nodes ?? []).filter(
        (node) =>
            node.hostname.toLowerCase().includes(search.toLowerCase()) ||
            node.role.toLowerCase().includes(search.toLowerCase()) ||
            node.status.toLowerCase().includes(search.toLowerCase()) ||
            node.ip.toLowerCase().includes(search.toLowerCase()),
    );

    if (isLoading) {
        return (
            <>
                <PageHeader title='Nodes' />
                <LoadingSkeleton />
            </>
        );
    }

    if (error) {
        return (
            <>
                <PageHeader title='Nodes' />
                <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-destructive'>
                    <AlertCircle className='size-5' />
                    <p>Failed to load nodes</p>
                </div>
            </>
        );
    }

    return (
        <>
            <PageHeader
                title='Nodes'
                description={`${nodes?.length ?? 0} nodes`}
                actions={
                    <div className='relative'>
                        <Search className='absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground' />
                        <Input placeholder='Search nodes...' value={search} onChange={(e) => setSearch(e.target.value)} className='h-8 w-56 pl-8 text-sm' />
                    </div>
                }
            />

            <div className='p-6'>
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-2 py-12 text-sm text-muted-foreground'>
                        <Server className='size-5' />
                        <p>{search ? "No nodes match your search" : "No nodes found"}</p>
                    </div>
                ) : (
                    <Card>
                        <CardContent className='p-0'>
                            <NodesTable nodes={filtered} />
                        </CardContent>
                    </Card>
                )}
            </div>
        </>
    );
}

export const Route = createFileRoute("/_authenticated/nodes/")({
    component: NodesPage,
});

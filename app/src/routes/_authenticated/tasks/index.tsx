import { useState } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { AlertCircle, ChevronLeft, ChevronRight, RefreshCw } from "lucide-react";
import { useAuth } from "@/hooks/use-auth";
import { useContainerInventory } from "@/hooks/use-container-inventory";
import { useNodes } from "@/hooks/use-nodes";
import { inventoryErrorCode, type ContainerState, type InventoryErrorCode } from "@/types/inventory";
import { PageHeader } from "@/components/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";

const PAGE_SIZE = 50;

const errorMessages: Record<InventoryErrorCode, string> = {
    unauthenticated: "Your session is no longer available. Sign in again.",
    relogin_required: "Your session expired or was revoked. Sign in again.",
    vault_unavailable: "The credential vault is unavailable. Unlock it and retry restore.",
    vault_write_failed: "The refreshed session could not be saved. Retry saving it.",
    local_state_failed: "Local session state could not be read or saved.",
    cleanup_uncertain: "Session cleanup needs attention before inventory can continue.",
    invalid_node_id: "The node ID is invalid.",
    invalid_pagination: "This page is no longer available. Start again from page one.",
    invalid_request: "The controller rejected the request.",
    node_not_found: "The node left the cluster or is no longer available.",
    not_found: "The controller no longer has this item.",
    conflict: "The controller refused because the item changed. Refresh and try again.",
    forbidden: "Your session cannot read this inventory.",
    controller_unavailable: "The controller is unavailable. Try again.",
    agent_unavailable: "The node agent is unavailable. Try again.",
    agent_timeout: "The node agent did not respond in time. Try again.",
    invalid_response: "The controller returned an invalid inventory response.",
};

function ErrorPanel({ error, retry, reset }: { error: unknown; retry: () => void; reset?: () => void }) {
    const code = inventoryErrorCode(error);
    return (
        <div role='alert' className='flex flex-col items-center gap-3 rounded-lg border border-destructive/30 bg-destructive/5 px-6 py-10 text-center'>
            <AlertCircle className='size-5 text-destructive' />
            <p className='text-sm text-foreground'>{errorMessages[code]}</p>
            {code === "invalid_pagination" && reset ? (
                <Button variant='outline' size='sm' onClick={reset}>
                    First page
                </Button>
            ) : !["unauthenticated", "relogin_required", "vault_write_failed", "cleanup_uncertain"].includes(code) ? (
                <Button variant='outline' size='sm' onClick={retry}>
                    Retry
                </Button>
            ) : null}
        </div>
    );
}

function LoadingRows() {
    return (
        <div aria-label='Loading containers' className='space-y-2 rounded-lg border border-border p-4'>
            {[1, 2, 3, 4].map((row) => (
                <Skeleton key={row} className='h-9 w-full' />
            ))}
        </div>
    );
}

function stateVariant(state: ContainerState): "secondary" | "outline" | "destructive" {
    if (state === "running") return "secondary";
    if (state === "exited" || state === "dead") return "destructive";
    return "outline";
}

function ContainerInventory({ profileId, nodeId }: { profileId: string; nodeId: string }) {
    const [cursors, setCursors] = useState<(string | null)[]>([null]);
    const cursor = cursors[cursors.length - 1];
    const inventory = useContainerInventory(profileId, nodeId, cursor, PAGE_SIZE);
    const page = inventory.data;

    return (
        <div className='flex w-full flex-col gap-5'>
            <PageHeader title='CONTAINERS' subtitle={`Node ${nodeId} · read-only inventory`}>
                <Button variant='outline' size='sm' onClick={() => void inventory.refetch()} disabled={inventory.isFetching}>
                    <RefreshCw className='size-3.5' /> Refresh
                </Button>
            </PageHeader>
            {inventory.isPending ? (
                <LoadingRows />
            ) : inventory.error ? (
                <ErrorPanel error={inventory.error} retry={() => void inventory.refetch()} reset={() => setCursors([null])} />
            ) : page?.items.length === 0 ? (
                <div className='rounded-lg border border-border px-6 py-12 text-center text-sm text-muted-foreground'>No containers on this page.</div>
            ) : (
                <div className='rounded-lg border border-border'>
                    <Table>
                        <TableHeader>
                            <TableRow>
                                <TableHead>Name</TableHead>
                                <TableHead>Image</TableHead>
                                <TableHead>State</TableHead>
                                <TableHead>Status</TableHead>
                                <TableHead>Created</TableHead>
                                <TableHead>Node</TableHead>
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {page?.items.map((item) => (
                                <TableRow key={`${item.nodeId}:${item.containerId}`}>
                                    <TableCell className='font-medium'>{item.displayName}</TableCell>
                                    <TableCell className='font-mono text-xs'>{item.imageReference}</TableCell>
                                    <TableCell>
                                        <Badge variant={stateVariant(item.state)}>{item.state}</Badge>
                                    </TableCell>
                                    <TableCell className='text-muted-foreground'>{item.status}</TableCell>
                                    <TableCell className='font-mono text-xs text-muted-foreground'>{item.createdAt}</TableCell>
                                    <TableCell className='font-mono text-xs text-muted-foreground'>{item.nodeId}</TableCell>
                                </TableRow>
                            ))}
                        </TableBody>
                    </Table>
                </div>
            )}
            <div className='flex items-center justify-end gap-2'>
                <span className='mr-2 text-xs text-muted-foreground'>Page {cursors.length}</span>
                <Button
                    variant='outline'
                    size='sm'
                    disabled={cursors.length === 1 || inventory.isFetching}
                    onClick={() => setCursors((current) => current.slice(0, -1))}
                >
                    <ChevronLeft className='size-3.5' /> Previous
                </Button>
                <Button
                    variant='outline'
                    size='sm'
                    disabled={!page?.nextCursor || inventory.isFetching}
                    onClick={() => {
                        if (page?.nextCursor) setCursors((current) => [...current, page.nextCursor]);
                    }}
                >
                    Next <ChevronRight className='size-3.5' />
                </Button>
            </div>
        </div>
    );
}

/** Until spec 07 fans out across nodes, the screen reads the first online member. */
function ContainersPage() {
    const { session } = useAuth();
    const profileId = session?.profileId ?? null;
    const nodes = useNodes();
    if (!profileId || nodes.isPending) return <LoadingRows />;
    if (nodes.error) return <ErrorPanel error={nodes.error} retry={() => void nodes.refetch()} />;
    const node = nodes.data.items.find((item) => item.status === "online");
    if (!node)
        return (
            <div className='flex flex-col gap-5'>
                <PageHeader title='CONTAINERS' subtitle='Read-only inventory' />
                <div className='rounded-md border border-border bg-card px-6 py-12 text-center font-mono text-2xs text-muted-foreground'>
                    No node is online. Start an agent and it appears here within seconds.
                </div>
            </div>
        );
    return <ContainerInventory key={`${profileId}:${node.id}`} profileId={profileId} nodeId={node.id} />;
}

export const Route = createFileRoute("/_authenticated/tasks/")({ component: ContainersPage });

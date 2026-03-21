import { createFileRoute } from "@tanstack/react-router";
import { useNetworks } from "@/hooks/use-networks";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { PageHeader } from "@/components/page-header";
import { TableShell } from "@/components/table-shell";
import { TableToolbar } from "@/components/table-toolbar";
import { ColumnHeaders } from "@/components/column-headers";
import { RefreshButton } from "@/components/refresh-button";
import { AlertCircle, MoreHorizontal, Network } from "lucide-react";
import { useState } from "react";
import type { NetworkItem } from "@/types/networks";

const NETWORK_COLUMNS = [
    { label: "STATUS", width: 70 },
    { label: "NAME" },
    { label: "ID", width: 150 },
    { label: "DRIVER", width: 100 },
    { label: "SCOPE", width: 80 },
    { label: "SUBNET", width: 120 },
    { label: "", width: 60 },
];

function NetworkRow({ network }: { network: NetworkItem }) {
    const scopeClass = network.scope === "swarm" ? "text-cyan-400" : "text-muted-foreground";

    return (
        <div className='flex items-center border-b border-border px-4 py-2.5 hover:bg-bg-elevated/50'>
            <div style={{ width: 70 }} className='flex items-center'>
                <span className='size-1.5 rounded-full bg-success' title='active' />
            </div>
            <div style={{ flex: 1 }} className='flex items-center'>
                <span className='font-mono text-xs font-medium'>{network.name}</span>
            </div>
            <div style={{ width: 150 }}>
                <span className='truncate font-mono text-[11px] text-muted-foreground' title={network.id}>
                    {network.id.slice(0, 12)}
                </span>
            </div>
            <div style={{ width: 100 }}>
                <span className='font-mono text-[11px] text-muted-foreground'>{network.driver}</span>
            </div>
            <div style={{ width: 80 }}>
                <span className={`font-mono text-[11px] ${scopeClass}`}>{network.scope}</span>
            </div>
            <div style={{ width: 120 }}>
                <span className='font-mono text-[11px] text-muted-foreground'>{network.subnet ?? <span className='opacity-30'>—</span>}</span>
            </div>
            <div style={{ width: 60 }} className='flex items-center justify-end'>
                <button className='flex size-6 items-center justify-center rounded text-muted-foreground opacity-0 transition-opacity hover:bg-bg-elevated hover:opacity-100 focus:opacity-100'>
                    <MoreHorizontal className='size-3.5' />
                </button>
            </div>
        </div>
    );
}

function LoadingSkeleton() {
    return (
        <div className='flex flex-col gap-6 p-6'>
            <div className='flex items-center justify-between'>
                <div className='flex flex-col gap-1'>
                    <Skeleton className='h-7 w-28' />
                    <Skeleton className='h-3 w-48' />
                </div>
                <Skeleton className='h-7 w-36' />
            </div>
            <div className='flex flex-col overflow-hidden border border-border'>
                <div className='flex items-center justify-between border-b border-border px-4 py-3'>
                    <Skeleton className='h-4 w-32' />
                    <Skeleton className='h-7 w-48' />
                </div>
                <div className='space-y-0'>
                    {[1, 2, 3, 4, 5].map((i) => (
                        <div key={i} className='flex items-center gap-3 border-b border-border px-4 py-2.5 last:border-0'>
                            <Skeleton className='size-1.5 rounded-full' />
                            <Skeleton className='h-3 w-28' />
                            <Skeleton className='h-3 w-28' />
                            <Skeleton className='h-3 w-16' />
                            <Skeleton className='ml-auto h-3 w-14' />
                        </div>
                    ))}
                </div>
            </div>
        </div>
    );
}

function NetworksPage() {
    const { data: networks, isLoading, error } = useNetworks();
    const [search, setSearch] = useState("");

    const filtered = (networks ?? []).filter(
        (net) =>
            net.name.toLowerCase().includes(search.toLowerCase()) ||
            net.driver.toLowerCase().includes(search.toLowerCase()) ||
            net.scope.toLowerCase().includes(search.toLowerCase()),
    );

    if (isLoading) {
        return <LoadingSkeleton />;
    }

    if (error) {
        return (
            <div className='flex flex-col items-center justify-center gap-2 py-12 text-xs text-destructive'>
                <AlertCircle className='size-4' />
                <p>Failed to load networks</p>
            </div>
        );
    }

    const networkCount = networks?.length ?? 0;
    const overlayCount = (networks ?? []).filter((n) => n.driver === "overlay").length;
    const bridgeCount = (networks ?? []).filter((n) => n.driver === "bridge").length;
    const subtitle = `${networkCount} ${networkCount === 1 ? "network" : "networks"} · ${overlayCount} overlay · ${bridgeCount} bridge`;

    return (
        <div className='flex h-full flex-col gap-6 p-6'>
            <PageHeader title='NETWORKS' subtitle={subtitle}>
                <RefreshButton queryKeys={[["networks"]]} />
                <Button variant='outline' size='sm' disabled className='h-7 font-mono text-[11px] text-muted-foreground'>
                    CREATE NETWORK
                </Button>
            </PageHeader>

            <TableToolbar searchValue={search} onSearchChange={setSearch} searchPlaceholder='Search networks...' />

            <TableShell title='ALL NETWORKS' count={filtered.length}>
                <ColumnHeaders columns={NETWORK_COLUMNS} />
                {filtered.length === 0 ? (
                    <div className='flex flex-col items-center justify-center gap-1.5 py-12 text-xs text-muted-foreground'>
                        <Network className='size-4' />
                        <p>{search ? "No networks match your filter" : "No networks found"}</p>
                    </div>
                ) : (
                    <div>
                        {filtered.map((net) => (
                            <NetworkRow key={net.id} network={net} />
                        ))}
                    </div>
                )}
            </TableShell>
        </div>
    );
}

export const Route = createFileRoute("/_authenticated/networks/")({
    component: NetworksPage,
});

import type { ReactNode } from "react";
import { Trash2 } from "lucide-react";
import { DataTable, type DataTableColumn } from "@/components/data-table/data-table";
import { RowActions } from "@/components/data-table/row-actions";
import { LabelTooltip } from "@/components/kit/tooltips";
import { NETWORK_SORTING } from "@/components/networks/network-search";
import { RemoveBlockedPopover } from "@/components/networks/remove-blocked-popover";
import { useNow } from "@/hooks/use-now";
import { relativeTime } from "@/lib/time";
import { networkRowId, shortNetworkId, type NetworkView } from "@/types/networks";
import { cn } from "@/lib/utils";

type Row = NetworkView & Record<string, unknown>;

const BUILTIN = {
    title: "Built-in networks can't be removed",
    reason: "Docker creates and relies on this network, so Kontainer never offers to delete it.",
};

export interface NetworksTableProps {
    rows: NetworkView[];
    search: string;
    /** Rows an in-flight removal acts on. */
    pending: Set<string>;
    /** Row id of a just-created network. */
    highlighted?: string;
    onRemove: (network: NetworkView) => void;
    countLabel: string;
    isFirstLoad: boolean;
    isRefetching: boolean;
    emptyState: ReactNode;
    errorState?: ReactNode;
}

function Actions({ row, onRemove }: { row: NetworkView; onRemove: (network: NetworkView) => void }) {
    if (row.builtin)
        return <RowActions label={row.name} actions={[{ label: "Remove", icon: Trash2, onClick: () => {}, disabled: true, disabledReason: BUILTIN }]} />;
    if (row.containers > 0) return <RemoveBlockedPopover network={row} />;
    return <RowActions label={row.name} actions={[{ label: "Remove", icon: Trash2, onClick: () => onRemove(row), mutates: true }]} />;
}

/** ALL NETWORKS: NAME, ID, DRIVER, SCOPE, SUBNET, CONTAINERS, CREATED, and the trash. */
export function NetworksTable(props: NetworksTableProps) {
    const now = useNow();
    const columns: DataTableColumn<Row>[] = [
        {
            id: "name",
            header: "NAME",
            sortValue: (row) => row.name,
            searchValue: (row) => `${row.name} ${row.id}`,
            cell: (row) => (
                <span className='flex min-w-0 flex-col'>
                    <span className={cn("truncate text-xs font-semibold", row.builtin ? "text-secondary-foreground" : "text-foreground")}>{row.name}</span>
                    {row.scope === "local" && <span className='truncate text-3xs text-muted-foreground'>{row.nodeIds[0]}</span>}
                </span>
            ),
        },
        {
            id: "id",
            header: "ID",
            className: "w-32",
            cell: (row) => (
                <LabelTooltip label={row.id}>
                    <span tabIndex={0} className='text-muted-foreground'>
                        {shortNetworkId(row.id)}
                    </span>
                </LabelTooltip>
            ),
        },
        { id: "driver", header: "DRIVER", className: "w-24", cell: (row) => row.driver },
        { id: "scope", header: "SCOPE", className: "w-20", cell: (row) => row.scope },
        { id: "subnet", header: "SUBNET", className: "w-36", cell: (row) => (row.subnets.length > 0 ? row.subnets.join(", ") : "—") },
        { id: "containers", header: "CONTAINERS", className: "w-24", sortValue: (row) => row.containers, cell: (row) => row.containers },
        {
            id: "created",
            header: "CREATED",
            className: "w-28",
            sortValue: (row) => (row.createdAt ? Date.parse(row.createdAt) : null),
            cell: (row) => <span className='text-muted-foreground'>{row.createdAt ? relativeTime(Date.parse(row.createdAt), now) : "—"}</span>,
        },
    ];

    return (
        <DataTable<Row>
            title='ALL NETWORKS'
            data={props.rows as Row[]}
            columns={columns}
            getRowId={(row) => networkRowId(row)}
            search={props.search}
            sorting={NETWORK_SORTING}
            rowActions={(row) => <Actions row={row} onRemove={props.onRemove} />}
            rowClassName={(row) =>
                cn(
                    props.pending.has(networkRowId(row)) && "opacity-50 transition-opacity",
                    props.highlighted === networkRowId(row) && "bg-primary-faint transition-colors",
                )
            }
            countLabel={props.countLabel}
            isFirstLoad={props.isFirstLoad}
            isRefetching={props.isRefetching}
            emptyState={props.emptyState}
            errorState={props.errorState}
        />
    );
}

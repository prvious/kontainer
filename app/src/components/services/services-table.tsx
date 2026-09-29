import type { ReactNode } from "react";
import type { SortingState } from "@tanstack/react-table";
import { DataTable, type DataTableColumn } from "@/components/data-table/data-table";
import { RowActions, type RowMenuItem } from "@/components/data-table/row-actions";
import { StatusDot, type Tone } from "@/components/kit/badges";
import { portsText } from "@/components/services/service-search";
import { ReplicaCell } from "@/components/services/replica-cell";
import { useNow } from "@/hooks/use-now";
import { relativeTime } from "@/lib/time";
import type { ServiceStatus, ServiceView } from "@/types/services";
import { cn } from "@/lib/utils";

type Row = ServiceView & Record<string, unknown>;

const STATUS_TONE: Record<ServiceStatus, Tone> = {
    running: "success",
    updating: "warning",
    scaling: "warning",
    partial: "warning",
    stopped: "destructive",
};

export interface ServicesTableProps {
    rows: ServiceView[];
    search: string;
    sorting: SortingState;
    onSortingChange: (sorting: SortingState) => void;
    statusOf: (service: ServiceView) => ServiceStatus;
    /** The in-flight scale of a service, from its operation's detail. */
    scalingOf: (service: ServiceView) => { desired: number; running: number } | null;
    /** The service whose replica count is a stepper right now. */
    editing: string | null;
    onEdit: (serviceId: string | null) => void;
    onScale: (service: ServiceView, replicas: number) => void;
    onRestart: (service: ServiceView) => void;
    onRollback: (service: ServiceView) => void;
    /** Opens the service's log stream. */
    onLogs: (service: ServiceView) => void;
    /** False while the manager can't take writes. */
    writable: boolean;
    countLabel: string;
    isFirstLoad: boolean;
    isRefetching: boolean;
    emptyState: ReactNode;
    errorState?: ReactNode;
}

/** SWARM SERVICES: STATUS, SERVICE NAME, IMAGE, MODE, REPLICAS, PORTS, UPDATED, and ⋮. */
export function ServicesTable(props: ServicesTableProps) {
    const now = useNow();
    const columns: DataTableColumn<Row>[] = [
        {
            id: "status",
            header: "STATUS",
            className: "w-24",
            cell: (row) => {
                const status = props.statusOf(row);
                return (
                    <span
                        className={cn(
                            "flex items-center gap-1.5 text-2xs",
                            status === "running" ? "text-success" : status === "stopped" ? "text-destructive" : "text-warning",
                        )}
                    >
                        <StatusDot tone={STATUS_TONE[status]} />
                        {status}
                    </span>
                );
            },
        },
        {
            id: "name",
            header: "SERVICE NAME",
            sortValue: (row) => row.name,
            searchValue: (row) => `${row.name} ${row.imageReference}`,
            cell: (row) => (
                <span className='flex min-w-0 flex-col'>
                    <span className='truncate text-xs font-semibold text-foreground'>{row.name}</span>
                    {row.stack && <span className='truncate text-3xs text-muted-foreground'>{row.stack}</span>}
                </span>
            ),
        },
        { id: "image", header: "IMAGE", className: "w-52", cell: (row) => <span className='block truncate'>{row.image}</span> },
        {
            id: "mode",
            header: "MODE",
            className: "w-24",
            cell: (row) => <span className='rounded-sm border border-border bg-secondary px-1.5 py-0.5 text-3xs text-secondary-foreground'>{row.mode}</span>,
        },
        {
            id: "replicas",
            header: "REPLICAS",
            className: "w-32 text-right",
            cell: (row) => (
                <ReplicaCell
                    service={row}
                    scaling={props.scalingOf(row)}
                    editing={props.editing === row.id}
                    writable={props.writable}
                    onEdit={() => props.onEdit(row.id)}
                    onCancel={() => props.onEdit(null)}
                    onSubmit={(replicas) => props.onScale(row, replicas)}
                />
            ),
        },
        { id: "ports", header: "PORTS", className: "w-32", cell: (row) => <span className='block truncate'>{portsText(row.ports)}</span> },
        {
            id: "updated",
            header: "UPDATED",
            className: "w-24",
            sortValue: (row) => (row.updatedAt ? Date.parse(row.updatedAt) : null),
            cell: (row) => <span className='text-muted-foreground'>{row.updatedAt ? relativeTime(Date.parse(row.updatedAt), now) : "—"}</span>,
        },
    ];

    function menu(row: ServiceView): RowMenuItem[] {
        const blocked = !props.writable || props.statusOf(row) === "scaling";
        return [
            { label: "Scale", onClick: () => props.onEdit(row.id), disabled: blocked || row.mode !== "replicated", mutates: true },
            { label: "Restart", onClick: () => props.onRestart(row), disabled: blocked, mutates: true },
            { label: "Rollback", onClick: () => props.onRollback(row), disabled: blocked, mutates: true },
            { label: "Logs", onClick: () => props.onLogs(row), separatorBefore: true },
        ];
    }

    return (
        <DataTable<Row>
            title='SWARM SERVICES'
            data={props.rows as Row[]}
            columns={columns}
            getRowId={(row) => row.id}
            search={props.search}
            sorting={props.sorting}
            onSortingChange={props.onSortingChange}
            rowActions={(row) => <RowActions label={row.name} menu={menu(row)} />}
            rowClassName={(row) => cn(props.statusOf(row) === "stopped" && "bg-destructive-soft/40")}
            countLabel={props.countLabel}
            isFirstLoad={props.isFirstLoad}
            isRefetching={props.isRefetching}
            emptyState={props.emptyState}
            errorState={props.errorState}
        />
    );
}

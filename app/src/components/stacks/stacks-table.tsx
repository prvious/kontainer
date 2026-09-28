import type { ReactNode } from "react";
import type { SortingState } from "@tanstack/react-table";
import { DataTable, type DataTableColumn } from "@/components/data-table/data-table";
import { RowActions } from "@/components/data-table/row-actions";
import { StatusDot, type Tone } from "@/components/kit/badges";
import { bytes } from "@/components/containers/container-format";
import { useNow } from "@/hooks/use-now";
import { relativeTime } from "@/lib/time";
import type { StackStatus, StackView } from "@/types/stacks";
import { cn } from "@/lib/utils";

type Row = StackView & Record<string, unknown>;

const STATUS_TONE: Record<StackStatus, Tone> = { running: "success", partial: "warning", stopped: "destructive" };

export interface StacksTableProps {
    rows: StackView[];
    search: string;
    sorting: SortingState;
    onSortingChange: (sorting: SortingState) => void;
    /** Stacks an in-flight removal acts on. */
    pending: Set<string>;
    onShowServices: (stack: StackView) => void;
    onRemove: (stack: StackView) => void;
    writable: boolean;
    countLabel: string;
    isFirstLoad: boolean;
    isRefetching: boolean;
    emptyState: ReactNode;
    errorState?: ReactNode;
}

/** DEPLOYED STACKS: STATUS, STACK NAME, SERVICES, CONTAINERS, CPU, MEMORY, LAST DEPLOYED, and ⋮. */
export function StacksTable(props: StacksTableProps) {
    const now = useNow();
    const columns: DataTableColumn<Row>[] = [
        {
            id: "status",
            header: "STATUS",
            className: "w-24",
            cell: (row) => (
                <span
                    className={cn(
                        "flex items-center gap-1.5 text-2xs",
                        row.status === "running" ? "text-success" : row.status === "partial" ? "text-warning" : "text-destructive",
                    )}
                >
                    <StatusDot tone={STATUS_TONE[row.status]} />
                    {row.status}
                </span>
            ),
        },
        {
            id: "name",
            header: "STACK NAME",
            sortValue: (row) => row.name,
            searchValue: (row) => row.name,
            cell: (row) => <span className='block truncate text-xs font-semibold text-foreground'>{row.name}</span>,
        },
        { id: "services", header: "SERVICES", className: "w-22", cell: (row) => row.services },
        {
            id: "containers",
            header: "CONTAINERS",
            className: "w-26",
            cell: (row) => (
                <span className={cn(row.runningContainers < row.desiredContainers && "text-warning")}>
                    {row.runningContainers}/{row.desiredContainers}
                </span>
            ),
        },
        { id: "cpu", header: "CPU", className: "w-18", cell: (row) => (row.cpuPercent === null ? "—" : `${row.cpuPercent.toFixed(1)}%`) },
        { id: "memory", header: "MEMORY", className: "w-22", cell: (row) => bytes(row.memoryBytes) },
        {
            id: "lastDeployed",
            header: "LAST DEPLOYED",
            className: "w-28",
            sortValue: (row) => (row.lastDeployed ? Date.parse(row.lastDeployed) : null),
            cell: (row) => <span className='text-muted-foreground'>{row.lastDeployed ? relativeTime(Date.parse(row.lastDeployed), now) : "—"}</span>,
        },
    ];

    return (
        <DataTable<Row>
            title='DEPLOYED STACKS'
            data={props.rows as Row[]}
            columns={columns}
            getRowId={(row) => row.name}
            search={props.search}
            sorting={props.sorting}
            onSortingChange={props.onSortingChange}
            rowActions={(row) => (
                <RowActions
                    label={row.name}
                    menu={[
                        { label: "Show services", onClick: () => props.onShowServices(row) },
                        {
                            label: "Remove",
                            onClick: () => props.onRemove(row),
                            destructive: true,
                            separatorBefore: true,
                            disabled: !props.writable || props.pending.has(row.name),
                        },
                    ]}
                />
            )}
            rowClassName={(row) => cn(props.pending.has(row.name) && "opacity-50 transition-opacity", row.status === "stopped" && "bg-destructive-soft/40")}
            countLabel={props.countLabel}
            isFirstLoad={props.isFirstLoad}
            isRefetching={props.isRefetching}
            emptyState={props.emptyState}
            errorState={props.errorState}
        />
    );
}

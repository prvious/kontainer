import type { ReactNode } from "react";
import type { RowSelectionState, SortingState } from "@tanstack/react-table";
import { Pause, Play, RotateCw, Terminal } from "lucide-react";
import { DataTable, type DataTableColumn } from "@/components/data-table/data-table";
import { RowActions, type RowIconAction, type RowMenuItem } from "@/components/data-table/row-actions";
import { StatusChip } from "@/components/kit/badges";
import { bytes, portsText, statusChip, uptimeText } from "@/components/containers/container-format";
import { useNow } from "@/hooks/use-now";
import type { ContainerRow } from "@/hooks/use-containers";
import { containerRowId, type ContainerAction } from "@/types/containers";
import { cn } from "@/lib/utils";

type Row = ContainerRow & Record<string, unknown>;

const EXEC_LATER = { title: "Exec isn't available yet", reason: "Opening a shell in a container arrives with the Exec screen." };
const STALE = { title: "Node didn't answer", reason: "This row is the last thing the node reported. Actions return when it checks in again." };

export interface ContainersTableProps {
    /** Section title; the Containers page's ALL CONTAINERS by default. */
    title?: string;
    rows: ContainerRow[];
    snapshotAt: string;
    search: string;
    sorting: SortingState;
    onSortingChange: (sorting: SortingState) => void;
    /** Omit both for a table without bulk selection. */
    selection?: RowSelectionState;
    onSelectionChange?: (selection: RowSelectionState) => void;
    /** Right side of the section header (search box, filter button). */
    headerRight?: ReactNode;
    /** Cached rows shown while the controller is unreachable. */
    dimmed?: boolean;
    /** Rows a bulk operation failed on, with the failure code. */
    failures: Map<string, string>;
    /** Rows an in-flight operation acts on. */
    pending: Set<string>;
    onAction: (row: ContainerRow, action: ContainerAction) => void;
    onRemove: (row: ContainerRow) => void;
    /** Opens the log stream of the row's service; rows without one have no logs to follow. */
    onLogs?: (row: ContainerRow) => void;
    countLabel: string;
    isFirstLoad: boolean;
    isRefetching: boolean;
    emptyState: ReactNode;
    errorState?: ReactNode;
}

function rowActions(
    row: ContainerRow,
    onAction: ContainersTableProps["onAction"],
    onRemove: ContainersTableProps["onRemove"],
    onLogs: ContainersTableProps["onLogs"],
) {
    const stale = row.stale;
    const disabled = stale ? { disabled: true, disabledReason: STALE } : {};
    const actions: RowIconAction[] = [{ label: "Exec", icon: Terminal, onClick: () => {}, disabled: true, disabledReason: stale ? STALE : EXEC_LATER }];
    if (row.state === "running") actions.push({ label: "Pause", icon: Pause, onClick: () => onAction(row, "pause"), mutates: true, ...disabled });
    else if (row.state === "paused") actions.push({ label: "Unpause", icon: Play, onClick: () => onAction(row, "unpause"), mutates: true, ...disabled });
    else actions.push({ label: "Restart", icon: RotateCw, onClick: () => onAction(row, "restart"), mutates: true, ...disabled });

    const live = row.state === "running" || row.state === "paused" || row.state === "restarting";
    const menu: RowMenuItem[] = [
        { label: row.serviceName ? "Logs" : "Logs (no service)", onClick: () => onLogs?.(row), disabled: !row.serviceName || !onLogs },
        { label: "Exec", onClick: () => {}, disabled: true },
        { label: "Restart", onClick: () => onAction(row, "restart"), disabled: stale, separatorBefore: true, mutates: true },
        live
            ? { label: "Stop", onClick: () => onAction(row, "stop"), disabled: stale, mutates: true }
            : { label: "Start", onClick: () => onAction(row, "start"), disabled: stale, mutates: true },
        row.state === "paused"
            ? { label: "Unpause", onClick: () => onAction(row, "unpause"), disabled: stale, mutates: true }
            : { label: "Pause", onClick: () => onAction(row, "pause"), disabled: stale || row.state !== "running", mutates: true },
        { label: "Remove", onClick: () => onRemove(row), disabled: stale, destructive: true, separatorBefore: true, mutates: true },
    ];
    return <RowActions label={row.name} actions={actions} menu={menu} />;
}

/** The ALL CONTAINERS table (`UFRzW`): STATUS, NAME, IMAGE, CPU, MEM, PORTS, UPTIME, actions. */
export function ContainersTable(props: ContainersTableProps) {
    const now = useNow();
    const { snapshotAt, failures } = props;
    const columns: DataTableColumn<Row>[] = [
        {
            id: "status",
            header: "STATUS",
            className: "w-24",
            cell: (row) => {
                const chip = statusChip(row);
                return <StatusChip tone={chip.tone}>{chip.label}</StatusChip>;
            },
        },
        {
            id: "name",
            header: "NAME",
            sortValue: (row) => row.name,
            searchValue: (row) => `${row.name} ${row.containerId}`,
            cell: (row) => (
                <span className='flex min-w-0 flex-col'>
                    <span className={cn("truncate text-xs font-semibold", row.stale ? "text-secondary-foreground" : "text-foreground")}>{row.name}</span>
                    <span className='truncate text-3xs text-muted-foreground'>
                        {row.serviceName ? [row.serviceName, row.stackNamespace].filter(Boolean).join(" · ") : row.nodeName}
                    </span>
                </span>
            ),
        },
        {
            id: "image",
            header: "IMAGE",
            className: "w-48",
            searchValue: (row) => row.image,
            cell: (row) => <span className='block truncate'>{row.image}</span>,
        },
        {
            id: "cpu",
            header: "CPU",
            className: "w-16",
            sortValue: (row) => (row.state === "running" && !row.stale ? row.cpuPercent : null),
            cell: (row) => (row.state === "running" && !row.stale && row.cpuPercent !== null ? `${row.cpuPercent.toFixed(1)}%` : "—"),
        },
        {
            id: "memory",
            header: "MEM",
            className: "w-20",
            sortValue: (row) => (row.stale ? null : row.memoryUsedBytes),
            cell: (row) => (row.stale ? "—" : bytes(row.memoryUsedBytes)),
        },
        { id: "ports", header: "PORTS", className: "w-28", cell: (row) => (row.stale ? "—" : portsText(row.ports)) },
        {
            id: "uptime",
            header: "UPTIME",
            className: "w-28",
            sortValue: (row) => (row.startedAt ? Date.parse(row.startedAt) : null),
            cell: (row) => {
                const failure = failures.get(containerRowId(row));
                if (failure) return <span className='text-destructive'>failed · {failure}</span>;
                return <span className='text-muted-foreground'>{uptimeText(row, snapshotAt, now)}</span>;
            },
        },
    ];

    return (
        <DataTable<Row>
            title={props.title ?? "ALL CONTAINERS"}
            data={props.rows as Row[]}
            columns={columns}
            getRowId={(row) => containerRowId(row)}
            search={props.search}
            sorting={props.sorting}
            onSortingChange={props.onSortingChange}
            rowSelection={props.selection}
            onRowSelectionChange={props.onSelectionChange}
            headerRight={props.headerRight}
            rowActions={(row) => rowActions(row, props.onAction, props.onRemove, props.onLogs)}
            rowClassName={(row) =>
                cn((props.pending.has(containerRowId(row)) || props.dimmed) && "opacity-50 transition-opacity", row.stale && "text-muted-foreground")
            }
            countLabel={props.countLabel}
            isFirstLoad={props.isFirstLoad}
            isRefetching={props.isRefetching}
            emptyState={props.emptyState}
            errorState={props.errorState}
        />
    );
}

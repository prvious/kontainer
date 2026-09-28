import type { ReactNode } from "react";
import type { SortingState } from "@tanstack/react-table";
import { Copy, FolderOpen, Trash2 } from "lucide-react";
import { DataTable, type DataTableColumn } from "@/components/data-table/data-table";
import { RowActions } from "@/components/data-table/row-actions";
import { LabelTooltip } from "@/components/kit/tooltips";
import { bytes } from "@/components/containers/container-format";
import { middleTruncate } from "@/components/volumes/volume-search";
import { useNow } from "@/hooks/use-now";
import { relativeTime } from "@/lib/time";
import { volumeRowId, type VolumeView } from "@/types/volumes";
import { cn } from "@/lib/utils";

type Row = VolumeView & Record<string, unknown>;

const BROWSE_LATER = { title: "Not built yet", reason: "Browsing a volume's files arrives with the Volume Explorer." };

export interface VolumesTableProps {
    rows: VolumeView[];
    search: string;
    sorting: SortingState;
    onSortingChange: (sorting: SortingState) => void;
    /** Rows an in-flight removal acts on. */
    pending: Set<string>;
    /** `nodeId/name` of a just-created volume. */
    highlighted?: string;
    onRemove: (volume: VolumeView) => void;
    countLabel: string;
    headerRight?: ReactNode;
    isFirstLoad: boolean;
    isRefetching: boolean;
    emptyState: ReactNode;
    errorState?: ReactNode;
}

function usersReason(volume: VolumeView): string {
    const listed = volume.users.join(", ");
    const more = volume.usedBy > volume.users.length ? ` and ${volume.usedBy - volume.users.length} more` : "";
    return `${listed}${more}. Docker refuses to remove a mounted volume; remove ${volume.usedBy === 1 ? "that container" : "those containers"} first.`;
}

/** ALL VOLUMES: NAME, NODE, DRIVER, MOUNT POINT, SIZE, USED BY, CREATED, and row actions. */
export function VolumesTable(props: VolumesTableProps) {
    const now = useNow();
    const columns: DataTableColumn<Row>[] = [
        {
            id: "name",
            header: "NAME",
            sortValue: (row) => row.name,
            searchValue: (row) => row.name,
            cell: (row) => (
                <span className={cn("block truncate text-xs font-semibold", row.usedBy > 0 ? "text-foreground" : "text-secondary-foreground")}>{row.name}</span>
            ),
        },
        { id: "node", header: "NODE", className: "w-28", cell: (row) => <span className='block truncate'>{row.nodeId}</span> },
        { id: "driver", header: "DRIVER", className: "w-20", cell: (row) => row.driver },
        {
            id: "mountPoint",
            header: "MOUNT POINT",
            className: "w-80",
            searchValue: (row) => row.mountPoint,
            cell: (row) => (
                <span className='group/mount flex min-w-0 items-center gap-1.5'>
                    <LabelTooltip label={row.mountPoint}>
                        <span tabIndex={0} className='truncate text-muted-foreground'>
                            {middleTruncate(row.mountPoint)}
                        </span>
                    </LabelTooltip>
                    <button
                        type='button'
                        aria-label={`Copy mount point of ${row.name}`}
                        onClick={() => void navigator.clipboard.writeText(row.mountPoint).catch(() => {})}
                        className='shrink-0 rounded-sm p-0.5 text-muted-foreground opacity-0 transition-opacity group-hover/mount:opacity-100 hover:text-foreground focus-visible:opacity-100'
                    >
                        <Copy aria-hidden className='size-3' />
                    </button>
                </span>
            ),
        },
        { id: "size", header: "SIZE", className: "w-20", sortValue: (row) => row.sizeBytes, cell: (row) => bytes(row.sizeBytes) },
        { id: "usedBy", header: "USED BY", className: "w-18", cell: (row) => row.usedBy },
        {
            id: "created",
            header: "CREATED",
            className: "w-24",
            sortValue: (row) => (row.createdAt ? Date.parse(row.createdAt) : null),
            cell: (row) => <span className='text-muted-foreground'>{row.createdAt ? relativeTime(Date.parse(row.createdAt), now) : "—"}</span>,
        },
    ];

    return (
        <DataTable<Row>
            title='ALL VOLUMES'
            data={props.rows as Row[]}
            columns={columns}
            getRowId={(row) => volumeRowId(row)}
            search={props.search}
            sorting={props.sorting}
            onSortingChange={props.onSortingChange}
            rowActions={(row) => (
                <RowActions
                    label={row.name}
                    actions={[
                        { label: "Browse files", icon: FolderOpen, onClick: () => {}, disabled: true, disabledReason: BROWSE_LATER },
                        row.usedBy > 0
                            ? {
                                  label: "Remove",
                                  icon: Trash2,
                                  onClick: () => {},
                                  disabled: true,
                                  disabledReason: {
                                      title: `In use by ${row.usedBy} ${row.usedBy === 1 ? "container" : "containers"}`,
                                      reason: usersReason(row),
                                  },
                              }
                            : { label: "Remove", icon: Trash2, onClick: () => props.onRemove(row) },
                    ]}
                />
            )}
            rowClassName={(row) =>
                cn(
                    props.pending.has(volumeRowId(row)) && "opacity-50 transition-opacity",
                    props.highlighted === volumeRowId(row) && "bg-primary-faint transition-colors",
                )
            }
            countLabel={props.countLabel}
            headerRight={props.headerRight}
            isFirstLoad={props.isFirstLoad}
            isRefetching={props.isRefetching}
            emptyState={props.emptyState}
            errorState={props.errorState}
        />
    );
}

import type { ReactNode } from "react";
import type { SortingState } from "@tanstack/react-table";
import { Trash2 } from "lucide-react";
import { DataTable, type DataTableColumn } from "@/components/data-table/data-table";
import { RowActions } from "@/components/data-table/row-actions";
import { LabelBadge } from "@/components/kit/badges";
import { LabelTooltip } from "@/components/kit/tooltips";
import { bytes } from "@/components/containers/container-format";
import { useNow } from "@/hooks/use-now";
import { relativeTime } from "@/lib/time";
import { imageLabel, imageName, shortImageId, type ImageView } from "@/types/images";
import { cn } from "@/lib/utils";

type Row = ImageView & Record<string, unknown>;

export interface ImagesTableProps {
    rows: ImageView[];
    search: string;
    sorting: SortingState;
    onSortingChange: (sorting: SortingState) => void;
    /** Images an in-flight removal acts on. */
    pending: Set<string>;
    onDelete: (image: ImageView) => void;
    countLabel: string;
    headerRight?: ReactNode;
    isFirstLoad: boolean;
    isRefetching: boolean;
    emptyState: ReactNode;
    errorState?: ReactNode;
}

/** LOCAL IMAGES: NAME / TAG, IMAGE ID, SIZE, CREATED, USED BY, NODES, and a trash action. */
export function ImagesTable(props: ImagesTableProps) {
    const now = useNow();
    const columns: DataTableColumn<Row>[] = [
        {
            id: "name",
            header: "NAME / TAG",
            sortValue: (row) => imageLabel(row),
            searchValue: (row) => [...row.repoTags, ...row.repoDigests, row.imageId].join(" "),
            cell: (row) => {
                const { repository, tag } = imageName(row);
                const untagged = row.repoTags.length === 0;
                return (
                    <span className='flex min-w-0 items-center gap-2'>
                        <span className={cn("truncate text-xs font-semibold", untagged ? "text-muted-foreground" : "text-foreground")}>
                            {untagged ? "<none>" : `${repository}:${tag}`}
                        </span>
                        {row.repoTags.length > 1 && <span className='shrink-0 text-3xs text-muted-foreground'>+{row.repoTags.length - 1}</span>}
                    </span>
                );
            },
        },
        {
            id: "imageId",
            header: "IMAGE ID",
            className: "w-36",
            cell: (row) => (
                <LabelTooltip label={row.imageId}>
                    <span tabIndex={0} className='text-muted-foreground'>
                        {shortImageId(row.imageId)}
                    </span>
                </LabelTooltip>
            ),
        },
        { id: "size", header: "SIZE", className: "w-24", sortValue: (row) => row.sizeBytes, cell: (row) => bytes(row.sizeBytes) },
        {
            id: "created",
            header: "CREATED",
            className: "w-28",
            sortValue: (row) => Date.parse(row.createdAt),
            cell: (row) => <span className='text-muted-foreground'>{relativeTime(Date.parse(row.createdAt), now)}</span>,
        },
        { id: "usedBy", header: "USED BY", className: "w-20", cell: (row) => row.usedBy },
        {
            id: "nodes",
            header: "NODES",
            className: "w-56",
            cell: (row) => (
                <span className='flex flex-wrap gap-1'>
                    {row.nodeIds.map((nodeId) => (
                        <LabelBadge key={nodeId}>{nodeId}</LabelBadge>
                    ))}
                </span>
            ),
        },
    ];

    return (
        <DataTable<Row>
            title='LOCAL IMAGES'
            data={props.rows as Row[]}
            columns={columns}
            getRowId={(row) => row.imageId}
            search={props.search}
            sorting={props.sorting}
            onSortingChange={props.onSortingChange}
            rowActions={(row) => (
                <RowActions label={imageLabel(row)} actions={[{ label: "Delete", icon: Trash2, onClick: () => props.onDelete(row), mutates: true }]} />
            )}
            rowClassName={(row) => cn(props.pending.has(row.imageId) && "opacity-50 transition-opacity")}
            countLabel={props.countLabel}
            headerRight={props.headerRight}
            isFirstLoad={props.isFirstLoad}
            isRefetching={props.isRefetching}
            emptyState={props.emptyState}
            errorState={props.errorState}
        />
    );
}

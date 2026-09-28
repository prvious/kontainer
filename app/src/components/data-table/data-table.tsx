import { useMemo, type ReactNode } from "react";
import {
    columnFilteringFeature,
    createColumnHelper,
    createFilteredRowModel,
    createSortedRowModel,
    filterFn_includesString,
    globalFilteringFeature,
    rowSelectionFeature,
    rowSortingFeature,
    tableFeatures,
    useTable,
    type RowSelectionState,
    type SortingState,
    type Updater,
} from "@tanstack/react-table";
import { ArrowDown, ArrowUp, RefreshCw } from "lucide-react";
import { Checkbox } from "@/components/ui/checkbox";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { LabelBadge } from "@/components/kit/badges";
import { SkeletonRows } from "@/components/kit/skeletons";
import { cn } from "@/lib/utils";

/** How a page describes one column; the table maps it to TanStack column definitions. */
export interface DataTableColumn<TData> {
    id: string;
    header: string;
    cell: (row: TData) => ReactNode;
    /** Enables sorting by this column. `null` sorts last. */
    sortValue?: (row: TData) => string | number | null;
    /** Includes this column in the global search. */
    searchValue?: (row: TData) => string;
    /** Width and alignment classes shared by the header and the cells. */
    className?: string;
}

const features = tableFeatures({
    columnFilteringFeature,
    globalFilteringFeature,
    filteredRowModel: createFilteredRowModel(),
    filterFns: { includesString: filterFn_includesString },
    rowSortingFeature,
    sortedRowModel: createSortedRowModel(),
    rowSelectionFeature,
});

type Features = typeof features;
const EMPTY_SORTING: SortingState = [];
const EMPTY_SELECTION: RowSelectionState = {};

function resolve<T>(updater: Updater<T>, current: T): T {
    return typeof updater === "function" ? (updater as (old: T) => T)(current) : updater;
}

function compare(left: string | number | null, right: string | number | null): number {
    if (left === right) return 0;
    if (left === null) return 1;
    if (right === null) return -1;
    if (typeof left === "number" && typeof right === "number") return left - right;
    return String(left).localeCompare(String(right), undefined, { numeric: true, sensitivity: "base" });
}

export interface DataTableProps<TData extends Record<string, unknown>> {
    /** Section header title, e.g. "ALL CONTAINERS". */
    title: string;
    data: TData[];
    columns: DataTableColumn<TData>[];
    getRowId: (row: TData) => string;
    /** Global search text; rows match when any searchable column contains it. */
    search?: string;
    sorting?: SortingState;
    onSortingChange?: (sorting: SortingState) => void;
    /** Enables the checkbox column. Selection is owned by the page so the bulk bar can act on it. */
    rowSelection?: RowSelectionState;
    onRowSelectionChange?: (selection: RowSelectionState) => void;
    /** Actions revealed on row hover; rendered in the last column. */
    rowActions?: (row: TData) => ReactNode;
    rowClassName?: (row: TData) => string | undefined;
    /** Right side of the section header (search box, filter button). */
    headerRight?: ReactNode;
    /** Count badge text; defaults to the number of rows shown. */
    countLabel?: string;
    /** First load only: six skeleton rows. */
    isFirstLoad?: boolean;
    /** A background refetch: rows stay, the header icon spins. */
    isRefetching?: boolean;
    /** Rendered inside the body when there are zero rows after filtering. */
    emptyState?: ReactNode;
    /** Replaces the body when the region failed to load. */
    errorState?: ReactNode;
    className?: string;
}

export function DataTable<TData extends Record<string, unknown>>({
    title,
    data,
    columns,
    getRowId,
    search = "",
    sorting = EMPTY_SORTING,
    onSortingChange,
    rowSelection,
    onRowSelectionChange,
    rowActions,
    rowClassName,
    headerRight,
    countLabel,
    isFirstLoad = false,
    isRefetching = false,
    emptyState,
    errorState,
    className,
}: DataTableProps<TData>) {
    const selectable = rowSelection !== undefined;
    const tableColumns = useMemo(() => {
        const helper = createColumnHelper<Features, TData>();
        return helper.columns(
            columns.map((column) =>
                helper.accessor((row) => column.searchValue?.(row) ?? "", {
                    id: column.id,
                    header: column.header,
                    enableSorting: column.sortValue !== undefined,
                    enableGlobalFilter: column.searchValue !== undefined,
                    sortFn: (left, right) => compare(column.sortValue?.(left.original) ?? null, column.sortValue?.(right.original) ?? null),
                }),
            ),
        );
    }, [columns]);
    const columnsById = useMemo(() => new Map(columns.map((column) => [column.id, column])), [columns]);

    const table = useTable({
        features,
        columns: tableColumns,
        data,
        getRowId: (row) => getRowId(row),
        state: { globalFilter: search, sorting, rowSelection: rowSelection ?? EMPTY_SELECTION },
        globalFilterFn: "includesString",
        getColumnCanGlobalFilter: (column) => columnsById.get(column.id)?.searchValue !== undefined,
        onSortingChange: (updater) => onSortingChange?.(resolve(updater, sorting)),
        onRowSelectionChange: (updater) => onRowSelectionChange?.(resolve(updater, rowSelection ?? EMPTY_SELECTION)),
        enableRowSelection: selectable,
    });

    const rows = table.getRowModel().rows;
    const allSelected = selectable && rows.length > 0 && table.getIsAllRowsSelected();
    const someSelected = selectable && table.getIsSomeRowsSelected() && !allSelected;
    const columnCount = columns.length + (selectable ? 1 : 0) + (rowActions ? 1 : 0);

    let body: ReactNode;
    if (errorState) {
        body = (
            <TableRow className='hover:bg-transparent'>
                <TableCell colSpan={columnCount} className='p-0'>
                    {errorState}
                </TableCell>
            </TableRow>
        );
    } else if (isFirstLoad) {
        body = (
            <TableRow className='hover:bg-transparent'>
                <TableCell colSpan={columnCount} className='p-0'>
                    <SkeletonRows count={6} />
                </TableCell>
            </TableRow>
        );
    } else if (rows.length === 0) {
        body = (
            <TableRow className='hover:bg-transparent'>
                <TableCell colSpan={columnCount} className='p-0 whitespace-normal'>
                    {emptyState}
                </TableCell>
            </TableRow>
        );
    } else {
        body = rows.map((row) => (
            <TableRow
                key={row.id}
                data-state={row.getIsSelected() ? "selected" : undefined}
                className={cn(
                    "group/row h-11 border-b border-border transition-colors hover:bg-secondary/60 data-[state=selected]:bg-primary-faint",
                    rowClassName?.(row.original),
                )}
            >
                {selectable && (
                    <TableCell className='w-10 px-4'>
                        <Checkbox
                            aria-label={`Select ${row.id}`}
                            checked={row.getIsSelected()}
                            onCheckedChange={(checked) => row.toggleSelected(checked === true)}
                            className='size-3.5 rounded-sm'
                        />
                    </TableCell>
                )}
                {row.getAllCells().map((cell) => {
                    const column = columnsById.get(cell.column.id);
                    return (
                        <TableCell key={cell.id} className={cn("px-2 font-mono text-2xs text-secondary-foreground first:pl-4", column?.className)}>
                            {column?.cell(row.original)}
                        </TableCell>
                    );
                })}
                {rowActions && (
                    <TableCell className='w-24 pr-4 text-right'>
                        <div className='flex justify-end opacity-60 transition-opacity group-hover/row:opacity-100 focus-within:opacity-100'>
                            {rowActions(row.original)}
                        </div>
                    </TableCell>
                )}
            </TableRow>
        ));
    }

    return (
        <section className={cn("flex flex-col overflow-hidden rounded-md border border-border bg-card", className)}>
            <header className='flex h-11 items-center justify-between gap-3 border-b border-border px-4'>
                <div className='flex items-center gap-3'>
                    <h2 className='font-mono text-2xs font-bold tracking-[1px] text-foreground uppercase'>{title}</h2>
                    <LabelBadge>{countLabel ?? rows.length}</LabelBadge>
                    {isRefetching && <RefreshCw aria-label='Refreshing' className='size-3 animate-spin text-muted-foreground' />}
                </div>
                {headerRight && <div className='flex items-center gap-2'>{headerRight}</div>}
            </header>
            {/* Tables scroll horizontally inside their card below the design width (design-gaps G20). */}
            <div className='overflow-x-auto'>
                <Table className='min-w-[1100px] table-fixed'>
                    <TableHeader className='bg-secondary'>
                        <TableRow className='h-8 border-b border-border hover:bg-secondary'>
                            {selectable && (
                                <TableHead className='w-10 px-4'>
                                    <Checkbox
                                        aria-label='Select all rows'
                                        checked={allSelected}
                                        indeterminate={someSelected}
                                        onCheckedChange={(checked) => table.toggleAllRowsSelected(checked === true)}
                                        className='size-3.5 rounded-sm'
                                    />
                                </TableHead>
                            )}
                            {table.getFlatHeaders().map((header) => {
                                const column = columnsById.get(header.column.id);
                                const sorted = header.column.getIsSorted();
                                return (
                                    <TableHead
                                        key={header.id}
                                        aria-sort={sorted === "asc" ? "ascending" : sorted === "desc" ? "descending" : undefined}
                                        className={cn("px-2 first:pl-4", column?.className)}
                                    >
                                        {header.column.getCanSort() ? (
                                            <button
                                                type='button'
                                                onClick={header.column.getToggleSortingHandler()}
                                                className='inline-flex items-center gap-1 font-mono text-3xs font-bold tracking-[1.1px] text-muted-foreground uppercase hover:text-foreground'
                                            >
                                                {column?.header}
                                                {sorted === "asc" && <ArrowUp aria-hidden className='size-2.5' />}
                                                {sorted === "desc" && <ArrowDown aria-hidden className='size-2.5' />}
                                            </button>
                                        ) : (
                                            <span className='font-mono text-3xs font-bold tracking-[1.1px] text-muted-foreground uppercase'>
                                                {column?.header}
                                            </span>
                                        )}
                                    </TableHead>
                                );
                            })}
                            {rowActions && <TableHead className='w-24 pr-4' aria-label='Actions' />}
                        </TableRow>
                    </TableHeader>
                    <TableBody>{body}</TableBody>
                </Table>
            </div>
        </section>
    );
}

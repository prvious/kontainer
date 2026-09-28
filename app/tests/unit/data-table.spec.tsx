import { useState } from "react";
import { describe, expect, it } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { RowSelectionState, SortingState } from "@tanstack/react-table";
import { DataTable, type DataTableColumn } from "@/components/data-table/data-table";
import { BulkBar } from "@/components/data-table/bulk-bar";
import { TableToolbar } from "@/components/data-table/table-toolbar";
import { EmptyState } from "@/components/kit/region-states";

interface Row extends Record<string, unknown> {
    id: string;
    name: string;
    image: string;
    cpu: number | null;
    state: "running" | "exited";
}

const ROWS: Row[] = [
    { id: "a", name: "nginx-proxy-01", image: "nginx:1.25-alpine", cpu: 2.3, state: "running" },
    { id: "b", name: "api-gateway", image: "kontainer/gateway:2.1", cpu: 8.7, state: "running" },
    { id: "c", name: "worker-processor-03", image: "kontainer/worker:1.8", cpu: null, state: "exited" },
];

const COLUMNS: DataTableColumn<Row>[] = [
    { id: "name", header: "Name", cell: (row) => row.name, sortValue: (row) => row.name, searchValue: (row) => row.name },
    { id: "image", header: "Image", cell: (row) => row.image, searchValue: (row) => row.image },
    { id: "cpu", header: "CPU", cell: (row) => (row.cpu === null ? "—" : `${row.cpu}%`), sortValue: (row) => row.cpu },
];

function Harness({ initialState }: { initialState?: Row["state"] }) {
    const [search, setSearch] = useState("");
    const [state, setState] = useState<Row["state"] | undefined>(initialState);
    const [sorting, setSorting] = useState<SortingState>([]);
    const [selection, setSelection] = useState<RowSelectionState>({});
    const rows = state ? ROWS.filter((row) => row.state === state) : ROWS;
    const selected = Object.keys(selection).filter((id) => selection[id]);
    const chips = state ? [{ key: "state", label: `status: ${state}`, onRemove: () => setState(undefined) }] : [];

    return (
        <div>
            {selected.length > 0 ? (
                <BulkBar count={selected.length} total={rows.length} onClear={() => setSelection({})}>
                    <button>RESTART</button>
                </BulkBar>
            ) : (
                <TableToolbar
                    search={search}
                    onSearchChange={setSearch}
                    searchPlaceholder='Search containers'
                    chips={chips}
                    onClearFilters={() => setState(undefined)}
                />
            )}
            <DataTable
                title='ALL CONTAINERS'
                data={rows}
                columns={COLUMNS}
                getRowId={(row) => row.id}
                search={search}
                sorting={sorting}
                onSortingChange={setSorting}
                rowSelection={selection}
                onRowSelectionChange={setSelection}
                emptyState={
                    <EmptyState
                        title='No containers match these filters'
                        primaryAction={{ label: "CLEAR FILTERS", onClick: () => (setSearch(""), setState(undefined)) }}
                        secondaryAction={{ label: "SHOW ALL", onClick: () => setState(undefined) }}
                    >
                        {`Nothing matches "${search}"${chips.map((chip) => ` with ${chip.label}`).join("")}.`}
                    </EmptyState>
                }
            />
        </div>
    );
}

const bodyRows = () => within(screen.getAllByRole("rowgroup")[1]).getAllByRole("row");
const names = () => bodyRows().map((row) => within(row).getAllByRole("cell")[1].textContent);

describe("DataTable", () => {
    it("sorts by a sortable column, toggling direction", async () => {
        const user = userEvent.setup();
        render(<Harness />);
        await user.click(screen.getByRole("button", { name: "Name" }));
        expect(names()).toEqual(["api-gateway", "nginx-proxy-01", "worker-processor-03"]);
        await user.click(screen.getByRole("button", { name: /Name/ }));
        expect(names()).toEqual(["worker-processor-03", "nginx-proxy-01", "api-gateway"]);
        // CPU sorts numerically with missing values last.
        await user.click(screen.getByRole("button", { name: "CPU" }));
        expect(names()).toEqual(["nginx-proxy-01", "api-gateway", "worker-processor-03"]);
    });

    it("searches across searchable columns and counts the rows shown", async () => {
        const user = userEvent.setup();
        render(<Harness />);
        await user.type(screen.getByRole("searchbox", { name: "Search containers" }), "kontainer/");
        expect(names()).toEqual(["api-gateway", "worker-processor-03"]);
        expect(screen.getByText("ALL CONTAINERS").nextSibling).toHaveTextContent("2");
    });

    it("shows filter chips and clears them", async () => {
        const user = userEvent.setup();
        render(<Harness initialState='running' />);
        expect(names()).toHaveLength(2);
        await user.click(screen.getByRole("button", { name: "Clear filters" }));
        expect(names()).toHaveLength(3);
    });

    it("swaps the toolbar for the bulk bar on selection, with an indeterminate header, and Esc clears", async () => {
        const user = userEvent.setup();
        render(<Harness />);
        await user.click(screen.getByRole("checkbox", { name: "Select a" }));
        expect(screen.getByRole("toolbar", { name: "Bulk actions" })).toHaveTextContent("1 selected");
        expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
        expect(screen.getByRole("checkbox", { name: "Select all rows" })).toHaveAttribute("aria-checked", "mixed");

        await user.click(screen.getByRole("checkbox", { name: "Select all rows" }));
        expect(screen.getByRole("toolbar", { name: "Bulk actions" })).toHaveTextContent("3 selected");

        await user.keyboard("{Escape}");
        expect(screen.queryByRole("toolbar", { name: "Bulk actions" })).not.toBeInTheDocument();
        expect(screen.getByRole("searchbox")).toBeInTheDocument();
    });

    it("renders the empty slot naming the active search and filters, with CLEAR FILTERS and SHOW ALL", async () => {
        const user = userEvent.setup();
        render(<Harness initialState='exited' />);
        await user.type(screen.getByRole("searchbox"), "payments");
        expect(screen.getByText('Nothing matches "payments" with status: exited.')).toBeInTheDocument();
        await user.click(screen.getByRole("button", { name: "SHOW ALL" }));
        // SHOW ALL keeps the search text.
        expect(screen.getByRole("searchbox")).toHaveValue("payments");
        await user.click(screen.getByRole("button", { name: "CLEAR FILTERS" }));
        expect(names()).toHaveLength(3);
    });

    it("renders six skeleton rows on first load and keeps rows while refetching", () => {
        const { rerender } = render(<DataTable title='ALL' data={ROWS} columns={COLUMNS} getRowId={(row) => row.id} isFirstLoad />);
        expect(screen.getAllByTestId("skeleton-row")).toHaveLength(6);
        rerender(<DataTable title='ALL' data={ROWS} columns={COLUMNS} getRowId={(row) => row.id} isRefetching />);
        expect(screen.queryByTestId("skeleton-row")).not.toBeInTheDocument();
        expect(screen.getByLabelText("Refreshing")).toBeInTheDocument();
        expect(names()).toHaveLength(3);
    });
});

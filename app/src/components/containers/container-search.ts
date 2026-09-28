import type { SortingState } from "@tanstack/react-table";
import type { ContainerRow } from "@/hooks/use-containers";

export const STATE_FILTERS = ["running", "paused", "exited", "restarting", "created", "dead"] as const;
export type StateFilter = (typeof STATE_FILTERS)[number];

export const CONTAINER_SORTS = ["recent", "name", "cpu", "memory"] as const;
export type ContainerSort = (typeof CONTAINER_SORTS)[number];

export const SORT_LABELS: Record<ContainerSort, string> = {
    recent: "Recent activity",
    name: "Name",
    cpu: "CPU",
    memory: "Memory",
};

/** The page's URL state. Defaults are absent so a clean URL means "everything, recent first". */
export interface ContainerSearch {
    q?: string;
    state?: StateFilter[];
    node?: string;
    ports?: "published";
    sort?: ContainerSort;
}

/** Plain validation with defaults: unknown values drop out rather than failing the route. */
export function parseContainerSearch(raw: Record<string, unknown>): ContainerSearch {
    const search: ContainerSearch = {};
    if (typeof raw.q === "string" && raw.q.trim() !== "") search.q = raw.q.slice(0, 200);
    const states = (Array.isArray(raw.state) ? raw.state : typeof raw.state === "string" ? [raw.state] : []).filter((value): value is StateFilter =>
        STATE_FILTERS.some((state) => state === value),
    );
    if (states.length > 0) search.state = [...new Set(states)];
    if (typeof raw.node === "string" && /^(?!\.{1,2}$)[A-Za-z0-9._-]{1,128}$/.test(raw.node)) search.node = raw.node;
    if (raw.ports === "published") search.ports = "published";
    if (CONTAINER_SORTS.some((sort) => sort === raw.sort) && raw.sort !== "recent") search.sort = raw.sort as ContainerSort;
    return search;
}

/** Chip filters (state, node, ports) apply here; the free-text search runs in the table. */
export function applyFilters(rows: ContainerRow[], search: ContainerSearch): ContainerRow[] {
    return rows.filter(
        (row) =>
            (!search.state?.length || search.state.some((state) => state === row.state)) &&
            (!search.node || row.nodeId === search.node) &&
            (!search.ports || row.ports.length > 0),
    );
}

export function activeFilterCount(search: ContainerSearch): number {
    return (search.state?.length ?? 0) + (search.node ? 1 : 0) + (search.ports ? 1 : 0);
}

/** The table sorting the sort param stands for. */
export function sortingFor(sort: ContainerSort | undefined): SortingState {
    switch (sort ?? "recent") {
        case "name":
            return [{ id: "name", desc: false }];
        case "cpu":
            return [{ id: "cpu", desc: true }];
        case "memory":
            return [{ id: "memory", desc: true }];
        default:
            return [{ id: "uptime", desc: true }];
    }
}

/** The sort param a header click stands for, when it is one of the page's sorts. */
export function sortFor(sorting: SortingState): ContainerSort | undefined {
    const [first] = sorting;
    if (!first) return undefined;
    if (first.id === "name") return "name";
    if (first.id === "cpu") return "cpu";
    if (first.id === "memory") return "memory";
    if (first.id === "uptime") return "recent";
    return undefined;
}

import type { SortingState } from "@tanstack/react-table";
import type { ServiceMode, ServiceStatus, ServiceView } from "@/types/services";

export const SERVICE_SORTS = ["name", "updated"] as const;
export type ServiceSort = (typeof SERVICE_SORTS)[number];

export const SERVICE_SORT_LABELS: Record<ServiceSort, string> = {
    name: "Name",
    updated: "Updated",
};

export const STATUS_FILTERS = ["running", "updating", "partial", "stopped"] as const;

/** The page's URL state. A clean URL means every service, by name. */
export interface ServiceSearch {
    q?: string;
    stack?: string;
    mode?: ServiceMode;
    status?: (typeof STATUS_FILTERS)[number];
    sort?: ServiceSort;
}

const NAME = /^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$/;

/** Plain validation with defaults: unknown values drop out rather than failing the route. */
export function parseServiceSearch(raw: Record<string, unknown>): ServiceSearch {
    const search: ServiceSearch = {};
    if (typeof raw.q === "string" && raw.q.trim() !== "") search.q = raw.q.slice(0, 200);
    if (typeof raw.stack === "string" && NAME.test(raw.stack)) search.stack = raw.stack;
    if (raw.mode === "replicated" || raw.mode === "global") search.mode = raw.mode;
    if (STATUS_FILTERS.some((status) => status === raw.status)) search.status = raw.status as ServiceSearch["status"];
    if (raw.sort === "updated") search.sort = "updated";
    return search;
}

/** Status counts `scaling` as `updating` for the filter. */
export function applyServiceFilters(rows: ServiceView[], search: ServiceSearch, statusOf: (row: ServiceView) => ServiceStatus): ServiceView[] {
    return rows.filter((row) => {
        const status = statusOf(row);
        return (
            (!search.stack || row.stack === search.stack) &&
            (!search.mode || row.mode === search.mode) &&
            (!search.status || (search.status === "updating" ? status === "updating" || status === "scaling" : status === search.status))
        );
    });
}

export function activeServiceFilterCount(search: ServiceSearch): number {
    return (search.stack ? 1 : 0) + (search.mode ? 1 : 0) + (search.status ? 1 : 0);
}

export function serviceSortingFor(sort: ServiceSort | undefined): SortingState {
    return sort === "updated" ? [{ id: "updated", desc: true }] : [{ id: "name", desc: false }];
}

export function serviceSortFor(sorting: SortingState): ServiceSort | undefined {
    const [first] = sorting;
    if (first?.id === "name" || first?.id === "updated") return first.id;
    return undefined;
}

/** `18 services · 15 replicated · 3 global · all tasks running` (or `· k services degraded`). */
export function serviceSummary(rows: ServiceView[]): string {
    const replicated = rows.filter((row) => row.mode === "replicated").length;
    const degraded = rows.filter((row) => row.runningReplicas < row.desiredReplicas).length;
    const health = degraded === 0 ? "all tasks running" : `${degraded} ${degraded === 1 ? "service" : "services"} degraded`;
    return [`${rows.length} ${rows.length === 1 ? "service" : "services"}`, `${replicated} replicated`, `${rows.length - replicated} global`, health].join(
        " · ",
    );
}

/** `80:80, 443:443`; `—` when nothing is published. */
export function portsText(ports: string[]): string {
    if (ports.length === 0) return "—";
    return ports.map((port) => port.replace(/\/tcp$/, "")).join(", ");
}

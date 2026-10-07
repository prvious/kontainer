import type { SortingState } from "@tanstack/react-table";
import type { VolumeView } from "@/types/volumes";

export const VOLUME_SORTS = ["size", "name", "created"] as const;
export type VolumeSort = (typeof VOLUME_SORTS)[number];

export const VOLUME_SORT_LABELS: Record<VolumeSort, string> = {
    size: "Size",
    name: "Name",
    created: "Created",
};

/** The page's URL state. Defaults are absent so a clean URL means "everything, largest first". */
export interface VolumeSearch {
    q?: string;
    node?: string;
    driver?: string;
    attached?: "yes" | "no";
    sort?: VolumeSort;
    /** `nodeId/name` of a just-created volume, highlighted until the next navigation. */
    highlightedRow?: string;
}

const NODE_ID = /^(?!\.{1,2}$)[A-Za-z0-9._-]{1,128}$/;

/** Plain validation with defaults: unknown values drop out rather than failing the route. */
export function parseVolumeSearch(raw: Record<string, unknown>): VolumeSearch {
    const search: VolumeSearch = {};
    if (typeof raw.q === "string" && raw.q.trim() !== "") search.q = raw.q.slice(0, 200);
    if (typeof raw.node === "string" && NODE_ID.test(raw.node)) search.node = raw.node;
    if (typeof raw.driver === "string" && /^[A-Za-z0-9._/-]{1,64}$/.test(raw.driver)) search.driver = raw.driver;
    if (raw.attached === "yes" || raw.attached === "no") search.attached = raw.attached;
    if (VOLUME_SORTS.some((sort) => sort === raw.sort) && raw.sort !== "size") search.sort = raw.sort as VolumeSort;
    if (typeof raw.highlightedRow === "string" && raw.highlightedRow.length <= 400) search.highlightedRow = raw.highlightedRow;
    return search;
}

/** Chip filters apply here; the free-text search runs in the table. */
export function applyVolumeFilters(rows: VolumeView[], search: VolumeSearch): VolumeView[] {
    return rows.filter(
        (row) =>
            (!search.node || row.nodeId === search.node) &&
            (!search.driver || row.driver === search.driver) &&
            (!search.attached || (search.attached === "yes") === row.usedBy > 0),
    );
}

export function activeVolumeFilterCount(search: VolumeSearch): number {
    return (search.node ? 1 : 0) + (search.driver ? 1 : 0) + (search.attached ? 1 : 0);
}

export function volumeSortingFor(sort: VolumeSort | undefined): SortingState {
    switch (sort ?? "size") {
        case "name":
            return [{ id: "name", desc: false }];
        case "created":
            return [{ id: "created", desc: true }];
        default:
            return [{ id: "size", desc: true }];
    }
}

export function volumeSortFor(sorting: SortingState): VolumeSort | undefined {
    const [first] = sorting;
    if (first?.id === "size" || first?.id === "name" || first?.id === "created") return first.id;
    return undefined;
}

/** Known sizes summed; `pending` while any node hasn't reported one yet. */
export function allocated(rows: VolumeView[]): { bytes: number; pending: boolean } {
    return {
        bytes: rows.reduce((total, row) => total + (row.sizeBytes ?? 0), 0),
        pending: rows.some((row) => row.sizeBytes === null),
    };
}

/** `/var/lib/…/_data`: keep both ends of a long path. */
export function middleTruncate(value: string, max = 44): string {
    if (value.length <= max) return value;
    const keep = max - 1;
    const head = Math.ceil(keep / 2);
    return `${value.slice(0, head)}…${value.slice(value.length - (keep - head))}`;
}

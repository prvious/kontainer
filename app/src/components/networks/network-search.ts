import type { SortingState } from "@tanstack/react-table";
import { BUILTIN_NETWORKS, NETWORK_NAME, type NetworkScope, type NetworkView } from "@/types/networks";

/** The page's URL state. A clean URL means everything, by name. */
export interface NetworkSearch {
    q?: string;
    driver?: string;
    scope?: NetworkScope;
    node?: string;
    /** Row id of a just-created network, highlighted until the next navigation. */
    highlightedRow?: string;
}

const NODE_ID = /^(?!\.{1,2}$)[A-Za-z0-9._-]{1,128}$/;

/** Plain validation with defaults: unknown values drop out rather than failing the route. */
export function parseNetworkSearch(raw: Record<string, unknown>): NetworkSearch {
    const search: NetworkSearch = {};
    if (typeof raw.q === "string" && raw.q.trim() !== "") search.q = raw.q.slice(0, 200);
    if (typeof raw.driver === "string" && /^[A-Za-z0-9._/-]{1,64}$/.test(raw.driver)) search.driver = raw.driver;
    if (raw.scope === "swarm" || raw.scope === "local") search.scope = raw.scope;
    if (typeof raw.node === "string" && NODE_ID.test(raw.node)) search.node = raw.node;
    if (typeof raw.highlightedRow === "string" && raw.highlightedRow.length <= 400) search.highlightedRow = raw.highlightedRow;
    return search;
}

export function applyNetworkFilters(rows: NetworkView[], search: NetworkSearch): NetworkView[] {
    return rows.filter(
        (row) =>
            (!search.driver || row.driver === search.driver) &&
            (!search.scope || row.scope === search.scope) &&
            (!search.node || row.nodeIds.includes(search.node)),
    );
}

export function activeNetworkFilterCount(search: NetworkSearch): number {
    return (search.driver ? 1 : 0) + (search.scope ? 1 : 0) + (search.node ? 1 : 0);
}

export const NETWORK_SORTING: SortingState = [{ id: "name", desc: false }];

/** `6 networks · 2 overlay · 3 bridge · 1 host`: counts per driver, largest first. */
export function driverSummary(rows: NetworkView[]): string {
    const counts = new Map<string, number>();
    for (const row of rows) counts.set(row.driver, (counts.get(row.driver) ?? 0) + 1);
    const drivers = [...counts.entries()].sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]));
    return [`${rows.length} ${rows.length === 1 ? "network" : "networks"}`, ...drivers.map(([driver, count]) => `${count} ${driver}`)].join(" · ");
}

/** Why a name can't be used, or `null`. */
export function networkNameProblem(name: string, existing: NetworkView[]): string | null {
    if (!NETWORK_NAME.test(name)) return "1–128 letters, digits, and . _ -, starting with a letter or digit.";
    if (BUILTIN_NETWORKS.includes(name)) return `${name} is one of Docker's built-in networks.`;
    if (existing.some((network) => network.name === name)) return `A network named ${name} already exists. Pick another name.`;
    return null;
}

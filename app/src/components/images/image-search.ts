import type { SortingState } from "@tanstack/react-table";

export const IMAGE_SORTS = ["created", "size", "name"] as const;
export type ImageSort = (typeof IMAGE_SORTS)[number];

export const IMAGE_SORT_LABELS: Record<ImageSort, string> = {
    created: "Created",
    size: "Size",
    name: "Name",
};

/** The page's URL state. Defaults are absent so a clean URL means "everything, newest first". */
export interface ImageSearch {
    q?: string;
    sort?: ImageSort;
}

/** Plain validation with defaults: unknown values drop out rather than failing the route. */
export function parseImageSearch(raw: Record<string, unknown>): ImageSearch {
    const search: ImageSearch = {};
    if (typeof raw.q === "string" && raw.q.trim() !== "") search.q = raw.q.slice(0, 200);
    if (IMAGE_SORTS.some((sort) => sort === raw.sort) && raw.sort !== "created") search.sort = raw.sort as ImageSort;
    return search;
}

/** The table sorting the sort param stands for. */
export function imageSortingFor(sort: ImageSort | undefined): SortingState {
    switch (sort ?? "created") {
        case "size":
            return [{ id: "size", desc: true }];
        case "name":
            return [{ id: "name", desc: false }];
        default:
            return [{ id: "created", desc: true }];
    }
}

/** The sort param a header click stands for, when it is one of the page's sorts. */
export function imageSortFor(sorting: SortingState): ImageSort | undefined {
    const [first] = sorting;
    if (first?.id === "size" || first?.id === "name" || first?.id === "created") return first.id;
    return undefined;
}

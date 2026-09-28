/** Credential-free container inventory crossing the Tauri boundary. */
export const CONTAINER_STATES = ["unknown", "created", "running", "paused", "restarting", "removing", "exited", "dead", "stopping"] as const;
export type ContainerState = (typeof CONTAINER_STATES)[number];

export interface ContainerItem {
    containerId: string;
    displayName: string;
    imageReference: string;
    state: ContainerState;
    status: string;
    createdAt: string;
    nodeId: string;
}

export interface ContainerPage {
    items: ContainerItem[];
    nextCursor: string | null;
}

export const INVENTORY_ERROR_CODES = [
    "unauthenticated",
    "relogin_required",
    "vault_unavailable",
    "vault_write_failed",
    "local_state_failed",
    "cleanup_uncertain",
    "invalid_node_id",
    "invalid_pagination",
    "invalid_request",
    "node_not_found",
    "not_found",
    "conflict",
    "forbidden",
    "controller_unavailable",
    "agent_unavailable",
    "agent_timeout",
    "invalid_response",
] as const;
export type InventoryErrorCode = (typeof INVENTORY_ERROR_CODES)[number];

export function inventoryErrorCode(error: unknown): InventoryErrorCode {
    const value = typeof error === "string" ? error : error instanceof Error ? error.message : "";
    return INVENTORY_ERROR_CODES.find((code) => code === value) ?? "invalid_response";
}

/** An object with exactly `fields` as its keys, or `invalid_response`. */
export function exactRecord(value: unknown, fields: string[]): Record<string, unknown> {
    if (typeof value !== "object" || value === null || Array.isArray(value)) throw "invalid_response";
    const data = value as Record<string, unknown>;
    if (Object.keys(data).sort().join("|") !== fields.slice().sort().join("|")) throw "invalid_response";
    return data;
}

export function containerPageFromIpc(value: unknown): ContainerPage {
    const page = exactRecord(value, ["items", "nextCursor"]);
    if (!Array.isArray(page.items) || page.items.length > 100 || (page.nextCursor !== null && typeof page.nextCursor !== "string")) {
        throw "invalid_response";
    }
    const items = page.items.map((value): ContainerItem => {
        const item = exactRecord(value, ["containerId", "displayName", "imageReference", "state", "status", "createdAt", "nodeId"]);
        if (
            typeof item.containerId !== "string" ||
            typeof item.displayName !== "string" ||
            typeof item.imageReference !== "string" ||
            !CONTAINER_STATES.some((state) => state === item.state) ||
            typeof item.status !== "string" ||
            typeof item.createdAt !== "string" ||
            typeof item.nodeId !== "string"
        )
            throw "invalid_response";
        return {
            containerId: item.containerId,
            displayName: item.displayName,
            imageReference: item.imageReference,
            state: item.state as ContainerState,
            status: item.status,
            createdAt: item.createdAt,
            nodeId: item.nodeId,
        };
    });
    return { items, nextCursor: page.nextCursor as string | null };
}

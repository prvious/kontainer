/** Closed error codes for Rust-authenticated controller calls; mirrors `InventoryError` in Rust. */
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
    "too_many_containers",
    "forbidden",
    "controller_unavailable",
    "agent_unavailable",
    "agent_timeout",
    "quorum_unavailable",
    "not_a_manager",
    "version_conflict",
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

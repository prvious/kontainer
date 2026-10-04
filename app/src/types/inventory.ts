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
    "limit_reached",
    "invalid_response",
] as const;
export type InventoryErrorCode = (typeof INVENTORY_ERROR_CODES)[number];

export function inventoryErrorCode(error: unknown): InventoryErrorCode {
    const value = typeof error === "string" ? error : error instanceof Error ? error.message : "";
    return INVENTORY_ERROR_CODES.find((code) => code === value) ?? "invalid_response";
}

/** True when the text holds a control character (C0 or DEL), which no label or path may carry. */
export function hasControlCharacter(value: string): boolean {
    for (const character of value) {
        const code = character.charCodeAt(0);
        if (code < 0x20 || code === 0x7f) return true;
    }
    return false;
}

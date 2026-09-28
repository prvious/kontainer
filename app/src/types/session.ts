/**
 * Credential-free view of the Rust-owned desktop session.
 *
 * Mirrors `SessionView` in `app/src-tauri/src/session/models.rs`; the shared
 * fixture at `app/tests/fixtures/session-ipc.json` keeps both sides in sync.
 * Access and refresh credentials never appear here or in any IPC payload.
 */
export interface SessionView {
    profileId: string;
    origin: string;
    clusterId: string;
    principalId: string;
    githubLogin: string;
    label: string | null;
    accessExpiresAtUnixMs: number;
}

/** Non-secret origin/label hint for prefilling the login screen. */
export interface LoginHint {
    origin: string;
    label: string | null;
}

/**
 * Closed error codes returned by the custody commands, matching
 * `SessionError` in `app/src-tauri/src/session/errors.rs`.
 */
export const SESSION_ERROR_CODES = [
    "invalid_origin",
    "browser_open_failed",
    "login_cancelled",
    "login_expired",
    "login_denied",
    "provider_unavailable",
    "controller_unavailable",
    "invalid_response",
    "profile_conflict",
    "relogin_required",
    "vault_unavailable",
    "vault_write_failed",
    "local_state_failed",
    "revoke_failed",
    "local_cleanup_failed",
    "cleanup_uncertain",
] as const;

export type SessionErrorCode = (typeof SESSION_ERROR_CODES)[number];

export function sessionErrorCode(error: unknown): SessionErrorCode {
    const value = typeof error === "string" ? error : error instanceof Error ? error.message : "";
    return SESSION_ERROR_CODES.find((code) => code === value) ?? "invalid_response";
}

function record(value: unknown): Record<string, unknown> {
    if (typeof value !== "object" || value === null || Array.isArray(value)) throw "invalid_response";
    return value as Record<string, unknown>;
}

/** Project native output onto the public view before it enters the query cache. */
export function sessionViewFromIpc(value: unknown): SessionView {
    const data = record(value);
    if (
        typeof data.profileId !== "string" ||
        typeof data.origin !== "string" ||
        typeof data.clusterId !== "string" ||
        typeof data.principalId !== "string" ||
        typeof data.githubLogin !== "string" ||
        (data.label !== null && typeof data.label !== "string") ||
        typeof data.accessExpiresAtUnixMs !== "number" ||
        !Number.isFinite(data.accessExpiresAtUnixMs)
    )
        throw "invalid_response";
    return {
        profileId: data.profileId,
        origin: data.origin,
        clusterId: data.clusterId,
        principalId: data.principalId,
        githubLogin: data.githubLogin,
        label: data.label,
        accessExpiresAtUnixMs: data.accessExpiresAtUnixMs,
    };
}

export function loginHintFromIpc(value: unknown): LoginHint {
    const data = record(value);
    if (typeof data.origin !== "string" || (data.label !== null && typeof data.label !== "string")) throw "invalid_response";
    return { origin: data.origin, label: data.label };
}

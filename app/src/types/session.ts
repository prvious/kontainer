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

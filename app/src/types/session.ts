/**
 * Credential-free view of the Rust-owned desktop session.
 *
 * Mirrors `SessionView` in `app/src-tauri/src/session/models.rs`; the shared
 * fixture at `app/tests/fixtures/session-ipc.json`, typed against these views,
 * keeps both sides in sync (ADR 0032). Access and refresh credentials never
 * appear here or in any IPC payload.
 */
export interface SessionView {
    profileId: string;
    origin: string;
    clusterId: string;
    principalId: string;
    email: string;
    displayName: string;
    label: string | null;
    accessExpiresAtUnixMs: number;
}

export interface ConnectionSummary {
    nodes: number;
    services: number;
}

/** A controller signed in to before. Mirrors `SavedConnection` in Rust. */
export interface SavedConnection {
    id: string;
    origin: string;
    label: string | null;
    lastUsedAtUnixMs: number;
    lastSummary: ConnectionSummary | null;
}

export type ProbeDiagnostic = "dns" | "tcp" | "tls" | "http" | "timeout";

/** A two-second `/health/ready` probe. Mirrors `ProbeView` in Rust. */
export interface ProbeView {
    status: "ready" | "unreachable";
    diagnostic?: ProbeDiagnostic;
}

/**
 * Closed error codes returned by the custody commands, matching
 * `SessionError` in `app/src-tauri/src/session/errors.rs`.
 */
export const SESSION_ERROR_CODES = [
    "invalid_origin",
    "invalid_credentials",
    "login_locked",
    "owner_exists",
    "validation_failed",
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

export type FieldProblem = "invalid" | "empty" | "too_short" | "too_long";

/** Mirrors `FieldErrorsView` in Rust. `setupCode` is only ever `empty` or `invalid`. */
export interface AccountFieldErrors {
    email?: FieldProblem;
    password?: FieldProblem;
    displayName?: FieldProblem;
    setupCode?: FieldProblem;
}

export const ACCOUNT_FIELDS = ["email", "password", "displayName", "setupCode"] as const satisfies readonly (keyof AccountFieldErrors)[];

/** A failed sign-in, registration, or bootstrap check. Mirrors `LoginFailure` in Rust. */
export interface LoginFailure {
    code: SessionErrorCode;
    attemptsRemaining?: number;
    retryAfterSeconds?: number;
    fields?: AccountFieldErrors;
    requestId?: string;
}

export function sessionErrorCode(error: unknown): SessionErrorCode {
    const value = typeof error === "string" ? error : error instanceof Error ? error.message : "";
    return SESSION_ERROR_CODES.find((code) => code === value) ?? "invalid_response";
}

export function isLoginFailure(value: unknown): value is LoginFailure {
    return typeof value === "object" && value !== null && "code" in value && SESSION_ERROR_CODES.includes((value as LoginFailure).code);
}

/**
 * Read a failure from a rejected sign-in, registration, or bootstrap invoke: Rust's
 * `LoginFailure`, or anything else (a plain `SessionError` string, a thrown error) closed over a code.
 */
export function loginFailureFromIpc(error: unknown): LoginFailure {
    return isLoginFailure(error) ? error : { code: sessionErrorCode(error) };
}

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

function record(value: unknown): Record<string, unknown> {
    if (typeof value !== "object" || value === null || Array.isArray(value)) throw "invalid_response";
    return value as Record<string, unknown>;
}

const optionalString = (value: unknown) => (typeof value === "string" ? value : undefined);
const optionalCount = (value: unknown) => (typeof value === "number" && Number.isInteger(value) && value >= 0 ? value : undefined);
const FIELD_PROBLEMS: readonly FieldProblem[] = ["invalid", "empty", "too_short", "too_long"];
const fieldProblem = (value: unknown) => FIELD_PROBLEMS.find((problem) => problem === value);

/** Project native output onto the public view before it enters the query cache. */
export function sessionViewFromIpc(value: unknown): SessionView {
    const data = record(value);
    if (
        typeof data.profileId !== "string" ||
        typeof data.origin !== "string" ||
        typeof data.clusterId !== "string" ||
        typeof data.principalId !== "string" ||
        typeof data.email !== "string" ||
        typeof data.displayName !== "string" ||
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
        email: data.email,
        displayName: data.displayName,
        label: data.label,
        accessExpiresAtUnixMs: data.accessExpiresAtUnixMs,
    };
}

export function savedConnectionsFromIpc(value: unknown): SavedConnection[] {
    if (!Array.isArray(value)) throw "invalid_response";
    return value.map((entry) => {
        const data = record(entry);
        if (
            typeof data.id !== "string" ||
            typeof data.origin !== "string" ||
            (data.label !== null && typeof data.label !== "string") ||
            typeof data.lastUsedAtUnixMs !== "number"
        )
            throw "invalid_response";
        let lastSummary: ConnectionSummary | null = null;
        if (data.lastSummary !== null && data.lastSummary !== undefined) {
            const summary = record(data.lastSummary);
            const nodes = optionalCount(summary.nodes);
            const services = optionalCount(summary.services);
            if (nodes === undefined || services === undefined) throw "invalid_response";
            lastSummary = { nodes, services };
        }
        return { id: data.id, origin: data.origin, label: data.label, lastUsedAtUnixMs: data.lastUsedAtUnixMs, lastSummary };
    });
}

const PROBE_DIAGNOSTICS: readonly ProbeDiagnostic[] = ["dns", "tcp", "tls", "http", "timeout"];

export function probeViewFromIpc(value: unknown): ProbeView {
    const data = record(value);
    if (data.status !== "ready" && data.status !== "unreachable") throw "invalid_response";
    const diagnostic = PROBE_DIAGNOSTICS.find((candidate) => candidate === data.diagnostic);
    return diagnostic ? { status: data.status, diagnostic } : { status: data.status };
}

/** Read a failure from a rejected sign-in, registration, or bootstrap invoke. */
export function loginFailureFromIpc(error: unknown): LoginFailure {
    if (typeof error === "object" && error !== null && !Array.isArray(error) && "code" in error) {
        const data = error as Record<string, unknown>;
        const failure: LoginFailure = { code: sessionErrorCode(data.code) };
        const attemptsRemaining = optionalCount(data.attemptsRemaining);
        const retryAfterSeconds = optionalCount(data.retryAfterSeconds);
        const requestId = optionalString(data.requestId);
        if (attemptsRemaining !== undefined) failure.attemptsRemaining = attemptsRemaining;
        if (retryAfterSeconds !== undefined) failure.retryAfterSeconds = retryAfterSeconds;
        if (requestId !== undefined) failure.requestId = requestId;
        if (typeof data.fields === "object" && data.fields !== null) {
            const fields = data.fields as Record<string, unknown>;
            failure.fields = {};
            for (const key of ACCOUNT_FIELDS) {
                const problem = fieldProblem(fields[key]);
                if (problem) failure.fields[key] = problem;
            }
        }
        return failure;
    }
    return { code: sessionErrorCode(error) };
}

export function isLoginFailure(value: unknown): value is LoginFailure {
    return typeof value === "object" && value !== null && "code" in value && SESSION_ERROR_CODES.includes((value as LoginFailure).code);
}

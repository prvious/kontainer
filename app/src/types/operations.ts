/**
 * Durable operations crossing the Tauri boundary.
 *
 * Mirrors `app/src-tauri/src/session/manager_operations.rs`; the shared fixture at
 * `app/tests/fixtures/operations-ipc.json` keeps both sides in sync.
 */

export const OPERATION_STATES = ["recoverable", "running", "uncertain", "succeeded", "failed", "cancelled"] as const;
export type OperationState = (typeof OPERATION_STATES)[number];

export interface OperationView {
    id: string;
    kind: string;
    state: OperationState;
    errorCode: string | null;
    errorMessage: string | null;
    /** The kind's latest progress or result document, e.g. `{ total, done }`. */
    progress: Record<string, unknown> | null;
    createdAt: string;
    updatedAt: string;
    startedAt: string | null;
    finishedAt: string | null;
    requestId: string | null;
}

export interface AdmittedOperation {
    operationId: string;
    replayed: boolean;
}

export const isTerminal = (state: OperationState) => state === "succeeded" || state === "failed" || state === "cancelled";

/**
 * `{ total, done, failed }` from a bulk operation, or `{ desired, running }` from a service one
 * (read as tasks running out of desired).
 */
export function progressCounts(progress: Record<string, unknown> | null): { total: number; done: number; failed: number; unit?: string } | null {
    if (!progress) return null;
    const { total, done, failed, desired, running } = progress;
    if (typeof desired === "number" && typeof running === "number" && desired > 0) {
        return { total: desired, done: Math.min(running, desired), failed: 0, unit: "tasks running" };
    }
    if (typeof total !== "number" || typeof done !== "number" || total <= 0) return null;
    return { total, done, failed: typeof failed === "number" ? failed : 0 };
}

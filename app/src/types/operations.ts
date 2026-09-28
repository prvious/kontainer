/**
 * Durable operations crossing the Tauri boundary.
 *
 * Mirrors `app/src-tauri/src/session/manager_operations.rs`; the shared fixture at
 * `app/tests/fixtures/operations-ipc.json` keeps both sides in sync.
 */
import { exactRecord } from "@/types/inventory";

export const OPERATION_STATES = ["recoverable", "running", "uncertain", "succeeded", "failed", "cancelled"] as const;
export type OperationState = (typeof OPERATION_STATES)[number];

export interface OperationView {
    id: string;
    kind: string;
    state: OperationState;
    lastEventSequence: number;
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

export interface OperationEventView {
    sequence: number;
    state: OperationState;
    errorCode: string | null;
    message: string | null;
    detail: Record<string, unknown> | null;
    createdAt: string;
}

export interface AdmittedOperation {
    operationId: string;
    replayed: boolean;
}

export const isTerminal = (state: OperationState) => state === "succeeded" || state === "failed" || state === "cancelled";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const TIMESTAMP = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;
const isTimestamp = (value: unknown): value is string => typeof value === "string" && TIMESTAMP.test(value);
const isOptionalTimestamp = (value: unknown) => value === null || isTimestamp(value);
const isOptionalText = (value: unknown): value is string | null => value === null || typeof value === "string";
const isState = (value: unknown): value is OperationState => OPERATION_STATES.some((state) => state === value);
const isDocument = (value: unknown): value is Record<string, unknown> | null => value === null || (typeof value === "object" && !Array.isArray(value));

export function operationFromIpc(value: unknown): OperationView {
    const data = exactRecord(value, [
        "id",
        "kind",
        "state",
        "lastEventSequence",
        "errorCode",
        "errorMessage",
        "progress",
        "createdAt",
        "updatedAt",
        "startedAt",
        "finishedAt",
        "requestId",
    ]);
    if (
        typeof data.id !== "string" ||
        !UUID.test(data.id) ||
        typeof data.kind !== "string" ||
        !isState(data.state) ||
        !Number.isInteger(data.lastEventSequence) ||
        !isOptionalText(data.errorCode) ||
        !isOptionalText(data.errorMessage) ||
        !isDocument(data.progress) ||
        !isTimestamp(data.createdAt) ||
        !isTimestamp(data.updatedAt) ||
        !isOptionalTimestamp(data.startedAt) ||
        !isOptionalTimestamp(data.finishedAt) ||
        !isOptionalText(data.requestId)
    )
        throw "invalid_response";
    return data as unknown as OperationView;
}

export function operationEventsFromIpc(value: unknown): OperationEventView[] {
    if (!Array.isArray(value) || value.length > 100) throw "invalid_response";
    return value.map((entry) => {
        const data = exactRecord(entry, ["sequence", "state", "errorCode", "message", "detail", "createdAt"]);
        if (
            !Number.isInteger(data.sequence) ||
            !isState(data.state) ||
            !isOptionalText(data.errorCode) ||
            !isOptionalText(data.message) ||
            !isDocument(data.detail) ||
            !isTimestamp(data.createdAt)
        )
            throw "invalid_response";
        return data as unknown as OperationEventView;
    });
}

export function admittedFromIpc(value: unknown): AdmittedOperation {
    const data = exactRecord(value, ["operationId", "replayed"]);
    if (typeof data.operationId !== "string" || !UUID.test(data.operationId) || typeof data.replayed !== "boolean") throw "invalid_response";
    return { operationId: data.operationId, replayed: data.replayed };
}

/** `{ total, done, failed? }` when the progress document carries counts. */
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

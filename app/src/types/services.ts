/**
 * Swarm services crossing the Tauri boundary.
 *
 * Mirrors `app/src-tauri/src/session/manager_services.rs`; the shared fixture at
 * `app/tests/fixtures/services-ipc.json` keeps both sides in sync.
 */
import { exactRecord } from "@/types/inventory";

export const SERVICE_MODES = ["replicated", "global"] as const;
export type ServiceMode = (typeof SERVICE_MODES)[number];

export interface ServiceView {
    id: string;
    name: string;
    /** Without the digest. */
    image: string;
    imageReference: string;
    mode: ServiceMode;
    /** Replicated: the replicas asked for. Global: one per eligible node. */
    desiredReplicas: number;
    runningReplicas: number;
    /** `published:target/proto`. */
    ports: string[];
    createdAt: string | null;
    updatedAt: string | null;
    /** Swarm's object version; writes send it back. */
    version: number;
    stack: string | null;
    updateState: string | null;
    updateMessage: string | null;
}

export interface ServiceListView {
    items: ServiceView[];
    snapshotAt: string;
}

export interface TaskView {
    id: string;
    slot: number | null;
    swarmNodeId: string | null;
    state: string;
    desiredState: string;
    error: string | null;
    message: string | null;
    updatedAt: string | null;
    containerId: string | null;
}

export interface ServiceDetailView {
    service: ServiceView;
    tasks: TaskView[];
    latestError: string | null;
}

const TIMESTAMP = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;
const SWARM_ID = /^[0-9a-z]{25}$/;
const isCount = (value: unknown): value is number => typeof value === "number" && Number.isInteger(value) && value >= 0;
const isOptionalText = (value: unknown) => value === null || typeof value === "string";
const isOptionalTime = (value: unknown) => value === null || (typeof value === "string" && TIMESTAMP.test(value));

function serviceFromIpc(value: unknown): ServiceView {
    const data = exactRecord(value, [
        "id",
        "name",
        "image",
        "imageReference",
        "mode",
        "desiredReplicas",
        "runningReplicas",
        "ports",
        "createdAt",
        "updatedAt",
        "version",
        "stack",
        "updateState",
        "updateMessage",
    ]);
    if (
        typeof data.id !== "string" ||
        !SWARM_ID.test(data.id) ||
        typeof data.name !== "string" ||
        typeof data.image !== "string" ||
        typeof data.imageReference !== "string" ||
        !SERVICE_MODES.some((mode) => mode === data.mode) ||
        !isCount(data.desiredReplicas) ||
        !isCount(data.runningReplicas) ||
        !Array.isArray(data.ports) ||
        !data.ports.every((port) => typeof port === "string") ||
        !isOptionalTime(data.createdAt) ||
        !isOptionalTime(data.updatedAt) ||
        !isCount(data.version) ||
        !isOptionalText(data.stack) ||
        !isOptionalText(data.updateState) ||
        !isOptionalText(data.updateMessage)
    )
        throw "invalid_response";
    return data as unknown as ServiceView;
}

export function serviceListFromIpc(value: unknown): ServiceListView {
    const data = exactRecord(value, ["items", "snapshotAt"]);
    if (!Array.isArray(data.items) || data.items.length > 2000 || typeof data.snapshotAt !== "string" || !TIMESTAMP.test(data.snapshotAt))
        throw "invalid_response";
    return { items: data.items.map(serviceFromIpc), snapshotAt: data.snapshotAt };
}

export function serviceDetailFromIpc(value: unknown): ServiceDetailView {
    const data = exactRecord(value, ["service", "tasks", "latestError"]);
    if (!Array.isArray(data.tasks) || data.tasks.length > 50 || !isOptionalText(data.latestError)) throw "invalid_response";
    const tasks = data.tasks.map((entry): TaskView => {
        const task = exactRecord(entry, ["id", "slot", "swarmNodeId", "state", "desiredState", "error", "message", "updatedAt", "containerId"]);
        if (
            typeof task.id !== "string" ||
            !(task.slot === null || isCount(task.slot)) ||
            !isOptionalText(task.swarmNodeId) ||
            typeof task.state !== "string" ||
            typeof task.desiredState !== "string" ||
            !isOptionalText(task.error) ||
            !isOptionalText(task.message) ||
            !isOptionalTime(task.updatedAt) ||
            !isOptionalText(task.containerId)
        )
            throw "invalid_response";
        return task as unknown as TaskView;
    });
    return { service: serviceFromIpc(data.service), tasks, latestError: data.latestError as string | null };
}

export type ServiceStatus = "running" | "updating" | "scaling" | "partial" | "stopped";

/**
 * The STATUS column: `scaling` while a scale operation of ours runs, `updating` while Swarm
 * updates it or another operation runs, then `stopped` (nothing running), `partial`
 * (0 < running < desired), or `running` (converged).
 */
export function serviceStatus(
    service: Pick<ServiceView, "runningReplicas" | "desiredReplicas" | "updateState">,
    inFlight: "scale" | "other" | null,
): ServiceStatus {
    if (inFlight === "scale") return "scaling";
    if (inFlight === "other" || service.updateState === "updating" || service.updateState === "rollback_started") return "updating";
    if (service.runningReplicas === 0 && service.desiredReplicas > 0) return "stopped";
    if (service.runningReplicas < service.desiredReplicas) return "partial";
    return "running";
}

/**
 * Stacks crossing the Tauri boundary.
 *
 * Mirrors `app/src-tauri/src/session/manager_stacks.rs`; the shared fixture at
 * `app/tests/fixtures/stacks-ipc.json` keeps both sides in sync.
 */
import { exactRecord } from "@/types/inventory";

export const STACK_STATUSES = ["running", "partial", "stopped"] as const;
export type StackStatus = (typeof STACK_STATUSES)[number];

export interface StackView {
    name: string;
    status: StackStatus;
    services: number;
    /** Tasks running and desired, summed over the stack's services. */
    runningContainers: number;
    desiredContainers: number;
    cpuPercent: number | null;
    memoryBytes: number | null;
    lastDeployed: string | null;
}

export interface StackListView {
    items: StackView[];
    snapshotAt: string;
}

export interface StackPlanView {
    services: string[];
    networks: string[];
    /** Named volumes the services mount; removal leaves them. */
    volumesRetained: string[];
}

const TIMESTAMP = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;
const isCount = (value: unknown): value is number => typeof value === "number" && Number.isInteger(value) && value >= 0;
const isTextList = (value: unknown): value is string[] => Array.isArray(value) && value.every((entry) => typeof entry === "string");

function stackFromIpc(value: unknown): StackView {
    const data = exactRecord(value, ["name", "status", "services", "runningContainers", "desiredContainers", "cpuPercent", "memoryBytes", "lastDeployed"]);
    if (
        typeof data.name !== "string" ||
        !STACK_STATUSES.some((status) => status === data.status) ||
        !isCount(data.services) ||
        !isCount(data.runningContainers) ||
        !isCount(data.desiredContainers) ||
        !(data.cpuPercent === null || (typeof data.cpuPercent === "number" && Number.isFinite(data.cpuPercent) && data.cpuPercent >= 0)) ||
        !(data.memoryBytes === null || isCount(data.memoryBytes)) ||
        !(data.lastDeployed === null || (typeof data.lastDeployed === "string" && TIMESTAMP.test(data.lastDeployed)))
    )
        throw "invalid_response";
    return data as unknown as StackView;
}

export function stackListFromIpc(value: unknown): StackListView {
    const data = exactRecord(value, ["items", "snapshotAt"]);
    if (!Array.isArray(data.items) || data.items.length > 1000 || typeof data.snapshotAt !== "string" || !TIMESTAMP.test(data.snapshotAt))
        throw "invalid_response";
    return { items: data.items.map(stackFromIpc), snapshotAt: data.snapshotAt };
}

export function stackPlanFromIpc(value: unknown): StackPlanView {
    const data = exactRecord(value, ["services", "networks", "volumesRetained"]);
    if (!isTextList(data.services) || !isTextList(data.networks) || !isTextList(data.volumesRetained)) throw "invalid_response";
    return data as unknown as StackPlanView;
}

/** The `stack.remove` phases, as the dialog names them. */
export const STACK_REMOVAL_PHASES: Record<string, string> = {
    removing_services: "Removing services",
    waiting_for_tasks: "Waiting for tasks to stop",
    removing_networks: "Removing networks",
    removed: "Removed",
};

/**
 * Containers across the cluster, crossing the Tauri boundary.
 *
 * Mirrors `app/src-tauri/src/session/manager_containers.rs`; the shared fixture at
 * `app/tests/fixtures/containers-ipc.json` keeps both sides in sync.
 */
import { exactRecord } from "@/types/inventory";

export const CONTAINER_STATES = ["unknown", "created", "running", "paused", "restarting", "removing", "exited", "dead", "stopping"] as const;
export type ContainerState = (typeof CONTAINER_STATES)[number];

export const NODE_OUTCOME_STATUSES = ["ok", "timeout", "unavailable", "error"] as const;
export type NodeOutcomeStatus = (typeof NODE_OUTCOME_STATUSES)[number];

export const CONTAINER_ACTIONS = ["start", "stop", "restart", "pause", "unpause", "remove"] as const;
export type ContainerAction = (typeof CONTAINER_ACTIONS)[number];

export interface ContainerView {
    nodeId: string;
    nodeName: string;
    containerId: string;
    name: string;
    image: string;
    state: ContainerState;
    status: string;
    createdAt: string;
    startedAt: string | null;
    finishedAt: string | null;
    /** Published ports, `host:container/proto`. */
    ports: string[];
    serviceName: string | null;
    stackNamespace: string | null;
    taskId: string | null;
    cpuPercent: number | null;
    memoryUsedBytes: number | null;
    memoryLimitBytes: number | null;
    /** Networks it has endpoints on, by name. */
    networks: string[];
}

export interface NodeOutcomeView {
    nodeId: string;
    status: NodeOutcomeStatus;
    message: string | null;
}

export interface ContainerListView {
    items: ContainerView[];
    nodes: NodeOutcomeView[];
    snapshotAt: string;
}

const TIMESTAMP = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;
const isTimestamp = (value: unknown): value is string => typeof value === "string" && TIMESTAMP.test(value);
const isOptionalTimestamp = (value: unknown) => value === null || isTimestamp(value);
const isOptionalText = (value: unknown) => value === null || typeof value === "string";
const isOptionalFigure = (value: unknown) => value === null || (typeof value === "number" && Number.isFinite(value) && value >= 0);
const CONTAINER_ID = /^[0-9a-f]{64}$/;
const NODE_ID = /^(?!\.{1,2}$)[A-Za-z0-9._-]{1,128}$/;

function containerFromIpc(value: unknown): ContainerView {
    const data = exactRecord(value, [
        "nodeId",
        "nodeName",
        "containerId",
        "name",
        "image",
        "state",
        "status",
        "createdAt",
        "startedAt",
        "finishedAt",
        "ports",
        "serviceName",
        "stackNamespace",
        "taskId",
        "cpuPercent",
        "memoryUsedBytes",
        "memoryLimitBytes",
        "networks",
    ]);
    if (
        typeof data.nodeId !== "string" ||
        !NODE_ID.test(data.nodeId) ||
        typeof data.nodeName !== "string" ||
        typeof data.containerId !== "string" ||
        !CONTAINER_ID.test(data.containerId) ||
        typeof data.name !== "string" ||
        typeof data.image !== "string" ||
        !CONTAINER_STATES.some((state) => state === data.state) ||
        typeof data.status !== "string" ||
        !isTimestamp(data.createdAt) ||
        !isOptionalTimestamp(data.startedAt) ||
        !isOptionalTimestamp(data.finishedAt) ||
        !Array.isArray(data.ports) ||
        !data.ports.every((port) => typeof port === "string") ||
        !isOptionalText(data.serviceName) ||
        !isOptionalText(data.stackNamespace) ||
        !isOptionalText(data.taskId) ||
        !isOptionalFigure(data.cpuPercent) ||
        !isOptionalFigure(data.memoryUsedBytes) ||
        !isOptionalFigure(data.memoryLimitBytes) ||
        !Array.isArray(data.networks) ||
        data.networks.length > 16 ||
        !data.networks.every((network) => typeof network === "string")
    )
        throw "invalid_response";
    return data as unknown as ContainerView;
}

export function containerListFromIpc(value: unknown): ContainerListView {
    const data = exactRecord(value, ["items", "nodes", "snapshotAt"]);
    if (!Array.isArray(data.items) || data.items.length > 2000 || !Array.isArray(data.nodes) || !isTimestamp(data.snapshotAt)) throw "invalid_response";
    return { items: data.items.map(containerFromIpc), nodes: data.nodes.map(nodeOutcomeFromIpc), snapshotAt: data.snapshotAt };
}

/** One member's answer to a fan-out read. */
export function nodeOutcomeFromIpc(value: unknown): NodeOutcomeView {
    const node = exactRecord(value, ["nodeId", "status", "message"]);
    if (
        typeof node.nodeId !== "string" ||
        !NODE_ID.test(node.nodeId) ||
        !NODE_OUTCOME_STATUSES.some((status) => status === node.status) ||
        !isOptionalText(node.message)
    )
        throw "invalid_response";
    return node as unknown as NodeOutcomeView;
}

/** Stable row id: a container id is unique per node, not across the cluster. */
export const containerRowId = (container: Pick<ContainerView, "nodeId" | "containerId">) => `${container.nodeId}/${container.containerId}`;

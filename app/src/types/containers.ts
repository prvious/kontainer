/**
 * Containers across the cluster, crossing the Tauri boundary.
 *
 * Mirrors `app/src-tauri/src/session/manager_containers.rs`; the shared fixture at
 * `app/tests/fixtures/containers-ipc.json` keeps both sides in sync.
 */

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

/** Stable row id: a container id is unique per node, not across the cluster. */
export const containerRowId = (container: Pick<ContainerView, "nodeId" | "containerId">) => `${container.nodeId}/${container.containerId}`;

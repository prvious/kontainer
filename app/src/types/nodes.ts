/**
 * Node membership, samples, and controller settings crossing the Tauri boundary.
 *
 * Mirrors `app/src-tauri/src/session/manager_nodes.rs`; the shared fixture at
 * `app/tests/fixtures/nodes-ipc.json` keeps both sides in sync.
 */

/** The agent side; `no-agent` for a Swarm member that never checked in. */
export const AGENT_STATUSES = ["online", "degraded", "offline", "no-agent"] as const;
export type AgentStatus = (typeof AGENT_STATUSES)[number];
/** Agent and Swarm together: `active` only when both are fine, `down` when either says so. */
export const NODE_HEALTHS = ["active", "degraded", "down"] as const;
export type NodeHealth = (typeof NODE_HEALTHS)[number];
export const NODE_ROLES = ["manager", "worker", "unknown"] as const;
export type NodeRole = (typeof NODE_ROLES)[number];
export const NODE_AVAILABILITIES = ["active", "pause", "drain"] as const;
export type NodeAvailability = (typeof NODE_AVAILABILITIES)[number];
export const QUORUM_STATES = ["healthy", "degraded"] as const;
export type QuorumState = (typeof QUORUM_STATES)[number];

export interface NodeSample {
    /** 0–100 of the node. */
    cpuPercent: number;
    memoryUsedBytes: number;
    containersRunning: number;
    containersPaused: number;
    containersExited: number;
    netRxBytesPerSec: number | null;
    netTxBytesPerSec: number | null;
}

/** Membership joined to the manager's Swarm member. Agent fields are null without an agent. */
export interface NodeSummary {
    /** The agent's identity; the Swarm node id for a member without an agent. */
    id: string;
    hostname: string;
    agentStatus: AgentStatus;
    health: NodeHealth;
    role: NodeRole;
    isLeader: boolean;
    availability: NodeAvailability | null;
    /** The address the manager sees the node at. */
    address: string | null;
    agentVersion: string | null;
    dockerVersion: string;
    os: string;
    arch: string;
    cpuCores: number;
    memoryTotalBytes: number;
    lastCheckInAt: string | null;
    latestSample: NodeSample | null;
}

/** Counts by health, manager reachability, quorum, and load over online nodes. */
export interface ClusterSummary {
    nodesTotal: number;
    nodesActive: number;
    nodesDegraded: number;
    nodesDown: number;
    /** Null when the manager didn't answer: unknown, not zero. */
    managersTotal: number | null;
    managersReachable: number | null;
    quorum: QuorumState;
    /** Mean over online nodes with a sample; null when there are none. */
    cpuPercent: number | null;
    memoryUsedBytes: number;
    memoryTotalBytes: number;
    containersRunning: number;
}

export interface NodeList {
    items: NodeSummary[];
    snapshotAt: string;
}

/** One bucket; figures are null when no sample landed in it, never 0. */
export interface SamplePoint {
    at: string;
    cpuPercent: number | null;
    memoryUsedBytes: number | null;
    containersRunning: number | null;
    netRxBytesPerSec: number | null;
    netTxBytesPerSec: number | null;
    /** Nodes that contributed a sample. */
    nodes: number;
}

/** The cluster series: per bucket, mean CPU over contributing nodes and summed figures. */
export interface SampleSeries {
    points: SamplePoint[];
}

export const SAMPLE_WINDOWS = ["5m", "15m", "1h", "24h", "7d"] as const;
export type SampleWindow = (typeof SAMPLE_WINDOWS)[number];

export interface ControllerSettings {
    metricsRetentionDays: number;
}

/** `/api/v1/status` for the sidebar Agent Info box and Settings. Mirrors `ControllerStatusView`. */
export interface ControllerStatusView {
    version: string;
    /** The controller's tracing filter: a level such as `info`, or a `RUST_LOG` value. */
    logLevel: string;
    startedAt: string;
}

/** Status bar figures from the cluster summary; the Docker version comes from the first online node. */
export function clusterUsage(
    summary: ClusterSummary | undefined,
    nodes: NodeSummary[] | undefined,
): {
    cpuPercent?: number;
    memoryUsedBytes?: number;
    memoryTotalBytes?: number;
    dockerVersion?: string;
} {
    const online = nodes?.find((node) => node.agentStatus === "online" && node.dockerVersion !== "");
    const usage = online ? { dockerVersion: online.dockerVersion } : {};
    if (!summary || summary.cpuPercent === null) return usage;
    return { ...usage, cpuPercent: summary.cpuPercent, memoryUsedBytes: summary.memoryUsedBytes, memoryTotalBytes: summary.memoryTotalBytes };
}

/** Nodes an agent can act on: pull, create, and target pickers offer only these. */
export function agentNodes(nodes: NodeSummary[]): NodeSummary[] {
    return nodes.filter((node) => node.agentStatus !== "no-agent");
}

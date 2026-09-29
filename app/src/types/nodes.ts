/**
 * Node membership, samples, and controller settings crossing the Tauri boundary.
 *
 * Mirrors `app/src-tauri/src/session/manager_nodes.rs`; the shared fixture at
 * `app/tests/fixtures/nodes-ipc.json` keeps both sides in sync.
 */
import { exactRecord, hasControlCharacter } from "@/types/inventory";

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
export const SWARM_NODE_STATUSES = ["ready", "down", "unknown"] as const;
export type SwarmNodeStatus = (typeof SWARM_NODE_STATUSES)[number];
export const QUORUM_STATES = ["healthy", "degraded"] as const;
export type QuorumState = (typeof QUORUM_STATES)[number];

export interface NodeSample {
    sampledAt: string;
    /** 0–100 of the node. */
    cpuPercent: number;
    memoryUsedBytes: number;
    diskUsedBytes: number | null;
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
    swarmStatus: SwarmNodeStatus;
    swarmNodeId: string | null;
    /** The address the manager sees the node at. */
    address: string | null;
    agentVersion: string | null;
    dockerVersion: string;
    os: string;
    arch: string;
    cpuCores: number;
    memoryTotalBytes: number;
    agentStartedAt: string | null;
    firstSeenAt: string | null;
    lastCheckInAt: string | null;
    latestSample: NodeSample | null;
}

/** Counts by health, manager reachability, quorum, and load over online nodes. */
export interface ClusterSummary {
    nodesTotal: number;
    nodesActive: number;
    nodesDegraded: number;
    nodesDown: number;
    managersTotal: number;
    managersReachable: number;
    quorum: QuorumState;
    /** Mean over online nodes with a sample; null when there are none. */
    cpuPercent: number | null;
    memoryUsedBytes: number;
    memoryTotalBytes: number;
    containersRunning: number;
    snapshotAt: string;
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
    diskUsedBytes: number | null;
    containersRunning: number | null;
    netRxBytesPerSec: number | null;
    netTxBytesPerSec: number | null;
    /** Nodes that contributed a sample. */
    nodes: number;
}

export interface SampleSeries {
    window: SampleWindow;
    bucket: string;
    points: SamplePoint[];
    snapshotAt: string;
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

const isCount = (value: unknown): value is number => typeof value === "number" && Number.isInteger(value) && value >= 0;
const isFigure = (value: unknown): value is number => typeof value === "number" && Number.isFinite(value) && value >= 0;
const isOptionalFigure = (value: unknown): value is number | null => value === null || isFigure(value);
const isOptionalText = (value: unknown): value is string | null => value === null || typeof value === "string";
const TIMESTAMP = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;
const isTimestamp = (value: unknown): value is string => typeof value === "string" && TIMESTAMP.test(value);
const isOptionalTimestamp = (value: unknown): value is string | null => value === null || isTimestamp(value);
const oneOf = <T extends string>(values: readonly T[], value: unknown): value is T => values.some((entry) => entry === value);
const NODE_ID = /^(?!\.{1,2}$)[A-Za-z0-9._-]{1,128}$/;

function sampleFromIpc(value: unknown): NodeSample {
    const data = exactRecord(value, [
        "sampledAt",
        "cpuPercent",
        "memoryUsedBytes",
        "diskUsedBytes",
        "containersRunning",
        "containersPaused",
        "containersExited",
        "netRxBytesPerSec",
        "netTxBytesPerSec",
    ]);
    if (
        !isTimestamp(data.sampledAt) ||
        !isFigure(data.cpuPercent) ||
        !isCount(data.memoryUsedBytes) ||
        !(data.diskUsedBytes === null || isCount(data.diskUsedBytes)) ||
        !isCount(data.containersRunning) ||
        !isCount(data.containersPaused) ||
        !isCount(data.containersExited) ||
        !isOptionalFigure(data.netRxBytesPerSec) ||
        !isOptionalFigure(data.netTxBytesPerSec)
    )
        throw "invalid_response";
    return data as unknown as NodeSample;
}

function nodeFromIpc(value: unknown): NodeSummary {
    const data = exactRecord(value, [
        "id",
        "hostname",
        "agentStatus",
        "health",
        "role",
        "isLeader",
        "availability",
        "swarmStatus",
        "swarmNodeId",
        "address",
        "agentVersion",
        "dockerVersion",
        "os",
        "arch",
        "cpuCores",
        "memoryTotalBytes",
        "agentStartedAt",
        "firstSeenAt",
        "lastCheckInAt",
        "latestSample",
    ]);
    if (
        typeof data.id !== "string" ||
        !NODE_ID.test(data.id) ||
        typeof data.hostname !== "string" ||
        !oneOf(AGENT_STATUSES, data.agentStatus) ||
        !oneOf(NODE_HEALTHS, data.health) ||
        !oneOf(NODE_ROLES, data.role) ||
        typeof data.isLeader !== "boolean" ||
        !(data.availability === null || oneOf(NODE_AVAILABILITIES, data.availability)) ||
        !oneOf(SWARM_NODE_STATUSES, data.swarmStatus) ||
        !isOptionalText(data.swarmNodeId) ||
        !isOptionalText(data.address) ||
        !isOptionalText(data.agentVersion) ||
        typeof data.dockerVersion !== "string" ||
        typeof data.os !== "string" ||
        typeof data.arch !== "string" ||
        !isCount(data.cpuCores) ||
        !isCount(data.memoryTotalBytes) ||
        !isOptionalTimestamp(data.agentStartedAt) ||
        !isOptionalTimestamp(data.firstSeenAt) ||
        !isOptionalTimestamp(data.lastCheckInAt)
    )
        throw "invalid_response";
    // An agent row carries every agent field; a Swarm-only row carries none.
    const hasAgent = data.agentStatus !== "no-agent";
    const agentFields = [data.agentVersion, data.agentStartedAt, data.firstSeenAt, data.lastCheckInAt];
    if (agentFields.some((field) => (field !== null) !== hasAgent) || (!hasAgent && data.latestSample !== null)) throw "invalid_response";
    return { ...(data as unknown as NodeSummary), latestSample: data.latestSample === null ? null : sampleFromIpc(data.latestSample) };
}

export function nodeListFromIpc(value: unknown): NodeList {
    const data = exactRecord(value, ["items", "snapshotAt"]);
    if (!Array.isArray(data.items) || !isTimestamp(data.snapshotAt)) throw "invalid_response";
    return { items: data.items.map(nodeFromIpc), snapshotAt: data.snapshotAt };
}

export function clusterSummaryFromIpc(value: unknown): ClusterSummary {
    const data = exactRecord(value, [
        "nodesTotal",
        "nodesActive",
        "nodesDegraded",
        "nodesDown",
        "managersTotal",
        "managersReachable",
        "quorum",
        "cpuPercent",
        "memoryUsedBytes",
        "memoryTotalBytes",
        "containersRunning",
        "snapshotAt",
    ]);
    const counts = [
        data.nodesTotal,
        data.nodesActive,
        data.nodesDegraded,
        data.nodesDown,
        data.managersTotal,
        data.managersReachable,
        data.memoryUsedBytes,
        data.memoryTotalBytes,
        data.containersRunning,
    ];
    if (!counts.every(isCount) || !oneOf(QUORUM_STATES, data.quorum) || !isOptionalFigure(data.cpuPercent) || !isTimestamp(data.snapshotAt))
        throw "invalid_response";
    const summary = data as unknown as ClusterSummary;
    if (summary.nodesActive + summary.nodesDegraded + summary.nodesDown !== summary.nodesTotal || summary.managersReachable > summary.managersTotal)
        throw "invalid_response";
    return summary;
}

export function sampleSeriesFromIpc(value: unknown): SampleSeries {
    const data = exactRecord(value, ["window", "bucket", "points", "snapshotAt"]);
    if (
        !SAMPLE_WINDOWS.some((window) => window === data.window) ||
        typeof data.bucket !== "string" ||
        !Array.isArray(data.points) ||
        data.points.length > 200 ||
        !isTimestamp(data.snapshotAt)
    )
        throw "invalid_response";
    const points = data.points.map((entry): SamplePoint => {
        const point = exactRecord(entry, [
            "at",
            "cpuPercent",
            "memoryUsedBytes",
            "diskUsedBytes",
            "containersRunning",
            "netRxBytesPerSec",
            "netTxBytesPerSec",
            "nodes",
        ]);
        if (
            !isTimestamp(point.at) ||
            !isOptionalFigure(point.cpuPercent) ||
            !isOptionalFigure(point.memoryUsedBytes) ||
            !isOptionalFigure(point.diskUsedBytes) ||
            !isOptionalFigure(point.containersRunning) ||
            !isOptionalFigure(point.netRxBytesPerSec) ||
            !isOptionalFigure(point.netTxBytesPerSec) ||
            !isCount(point.nodes)
        )
            throw "invalid_response";
        return point as unknown as SamplePoint;
    });
    return { window: data.window as SampleWindow, bucket: data.bucket, points, snapshotAt: data.snapshotAt };
}

export function controllerSettingsFromIpc(value: unknown): ControllerSettings {
    const data = exactRecord(value, ["metricsRetentionDays"]);
    if (!isCount(data.metricsRetentionDays) || data.metricsRetentionDays < 1 || data.metricsRetentionDays > 365) throw "invalid_response";
    return { metricsRetentionDays: data.metricsRetentionDays };
}

const isPrintable = (value: unknown, limit: number): value is string =>
    typeof value === "string" && value.length > 0 && value.length <= limit && !hasControlCharacter(value);

export function controllerStatusFromIpc(value: unknown): ControllerStatusView {
    const data = exactRecord(value, ["version", "logLevel", "startedAt"]);
    if (!isPrintable(data.version, 64) || !isPrintable(data.logLevel, 512) || !isTimestamp(data.startedAt)) throw "invalid_response";
    return { version: data.version, logLevel: data.logLevel, startedAt: data.startedAt };
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

/** Nodes degraded or down: the sidebar's `N DOWN` badge. */
export function nodesDown(summary: ClusterSummary | undefined): number | undefined {
    return summary ? summary.nodesDegraded + summary.nodesDown : undefined;
}

/** Nodes an agent can act on: pull, create, and target pickers offer only these. */
export function agentNodes(nodes: NodeSummary[]): NodeSummary[] {
    return nodes.filter((node) => node.agentStatus !== "no-agent");
}

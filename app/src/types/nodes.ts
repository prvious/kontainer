/**
 * Node membership, samples, and controller settings crossing the Tauri boundary.
 *
 * Mirrors `app/src-tauri/src/session/manager_nodes.rs`; the shared fixture at
 * `app/tests/fixtures/nodes-ipc.json` keeps both sides in sync.
 */
import { exactRecord } from "@/types/inventory";

export const NODE_STATUSES = ["online", "degraded", "offline"] as const;
export type NodeStatus = (typeof NODE_STATUSES)[number];

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

export interface NodeSummary {
    id: string;
    hostname: string;
    status: NodeStatus;
    swarmNodeId: string | null;
    role: string | null;
    availability: string | null;
    agentVersion: string;
    dockerVersion: string;
    os: string;
    arch: string;
    cpuCores: number;
    memoryTotalBytes: number;
    agentStartedAt: string;
    firstSeenAt: string;
    lastCheckInAt: string;
    latestSample: NodeSample | null;
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

const isCount = (value: unknown): value is number => typeof value === "number" && Number.isInteger(value) && value >= 0;
const isFigure = (value: unknown): value is number => typeof value === "number" && Number.isFinite(value) && value >= 0;
const isOptionalFigure = (value: unknown): value is number | null => value === null || isFigure(value);
const isOptionalText = (value: unknown): value is string | null => value === null || typeof value === "string";
const TIMESTAMP = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;
const isTimestamp = (value: unknown): value is string => typeof value === "string" && TIMESTAMP.test(value);
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
        "status",
        "swarmNodeId",
        "role",
        "availability",
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
        !NODE_STATUSES.some((status) => status === data.status) ||
        !isOptionalText(data.swarmNodeId) ||
        !isOptionalText(data.role) ||
        !isOptionalText(data.availability) ||
        typeof data.agentVersion !== "string" ||
        typeof data.dockerVersion !== "string" ||
        typeof data.os !== "string" ||
        typeof data.arch !== "string" ||
        !isCount(data.cpuCores) ||
        !isCount(data.memoryTotalBytes) ||
        !isTimestamp(data.agentStartedAt) ||
        !isTimestamp(data.firstSeenAt) ||
        !isTimestamp(data.lastCheckInAt)
    )
        throw "invalid_response";
    return { ...(data as unknown as NodeSummary), latestSample: data.latestSample === null ? null : sampleFromIpc(data.latestSample) };
}

export function nodeListFromIpc(value: unknown): NodeList {
    const data = exactRecord(value, ["items", "snapshotAt"]);
    if (!Array.isArray(data.items) || !isTimestamp(data.snapshotAt)) throw "invalid_response";
    return { items: data.items.map(nodeFromIpc), snapshotAt: data.snapshotAt };
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

/** Status bar figures across online nodes: mean CPU, summed memory; undefined without samples. */
export function clusterUsage(nodes: NodeSummary[] | undefined): {
    cpuPercent?: number;
    memoryUsedBytes?: number;
    memoryTotalBytes?: number;
    dockerVersion?: string;
} {
    const sampled = (nodes ?? []).filter((node) => node.status === "online" && node.latestSample);
    if (sampled.length === 0) return {};
    return {
        cpuPercent: sampled.reduce((sum, node) => sum + (node.latestSample?.cpuPercent ?? 0), 0) / sampled.length,
        memoryUsedBytes: sampled.reduce((sum, node) => sum + (node.latestSample?.memoryUsedBytes ?? 0), 0),
        memoryTotalBytes: sampled.reduce((sum, node) => sum + node.memoryTotalBytes, 0),
        dockerVersion: sampled[0].dockerVersion,
    };
}

/** Nodes not online: the sidebar's `N DOWN` badge. */
export function nodesDown(nodes: NodeSummary[] | undefined): number | undefined {
    return nodes?.filter((node) => node.status !== "online").length;
}

import type { ContainerRow } from "@/hooks/use-containers";
import type { ClusterSummary, NodeSummary, SamplePoint } from "@/types/nodes";

export interface ContainerCounts {
    running: number;
    paused: number;
    /** Exited and dead. */
    exited: number;
    total: number;
}

/** Live containers by state for the CONTAINERS card. */
export function containerCounts(rows: ContainerRow[]): ContainerCounts {
    const live = rows.filter((row) => !row.stale);
    return {
        running: live.filter((row) => row.state === "running").length,
        paused: live.filter((row) => row.state === "paused").length,
        exited: live.filter((row) => row.state === "exited" || row.state === "dead").length,
        total: live.length,
    };
}

/** The busiest bucket of the cluster CPU series; null without a sampled bucket. */
export function cpuPeak(points: SamplePoint[]): number | null {
    const values = points.flatMap((point) => (point.cpuPercent === null ? [] : [point.cpuPercent]));
    return values.length === 0 ? null : Math.max(...values);
}

/** Used against total over the sampled nodes; null when no online node has a sample. */
export function memoryFigures(summary: ClusterSummary): { usedBytes: number; totalBytes: number; percent: number; freeBytes: number } | null {
    if (summary.cpuPercent === null || summary.memoryTotalBytes === 0) return null;
    return {
        usedBytes: summary.memoryUsedBytes,
        totalBytes: summary.memoryTotalBytes,
        percent: (summary.memoryUsedBytes / summary.memoryTotalBytes) * 100,
        freeBytes: Math.max(0, summary.memoryTotalBytes - summary.memoryUsedBytes),
    };
}

/** Receive and transmit rates summed over online nodes that reported them; null when none did. */
export function networkRates(nodes: NodeSummary[]): { rx: number; tx: number; total: number } | null {
    const online = nodes.filter((node) => node.agentStatus === "online" && node.latestSample);
    const rx = online.flatMap((node) => (node.latestSample?.netRxBytesPerSec == null ? [] : [node.latestSample.netRxBytesPerSec]));
    const tx = online.flatMap((node) => (node.latestSample?.netTxBytesPerSec == null ? [] : [node.latestSample.netTxBytesPerSec]));
    if (rx.length === 0 && tx.length === 0) return null;
    const sum = (values: number[]) => values.reduce((total, value) => total + value, 0);
    return { rx: sum(rx), tx: sum(tx), total: sum(rx) + sum(tx) };
}

/** One row per node for the CPU ring's breakdown; a node without a live sample is unreachable, never 0%. */
export function cpuByNode(nodes: NodeSummary[]): { id: string; name: string; cores: number; percent: number | null }[] {
    return nodes.map((node) => ({
        id: node.id,
        name: node.hostname || node.id,
        cores: node.cpuCores,
        percent: node.agentStatus === "online" && node.latestSample ? node.latestSample.cpuPercent : null,
    }));
}

/** Online nodes with a sample: the ones the aggregate CPU averages over. */
export function sampledNodeCount(nodes: NodeSummary[]): number {
    return nodes.filter((node) => node.agentStatus === "online" && node.latestSample).length;
}

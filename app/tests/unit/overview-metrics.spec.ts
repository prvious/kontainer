import { describe, expect, it } from "vitest";
import nodesFixture from "../fixtures/nodes-ipc.json";
import containersFixture from "../fixtures/containers-ipc.json";
import { containerCounts, cpuByNode, cpuPeak, memoryFigures, networkRates, sampledNodeCount } from "@/components/overview/overview-metrics";
import { clusterSummaryFromIpc, nodeListFromIpc, sampleSeriesFromIpc } from "@/types/nodes";
import { containerListFromIpc } from "@/types/containers";

const nodes = nodeListFromIpc(nodesFixture.nodeList).items;
const summary = clusterSummaryFromIpc(nodesFixture.clusterSummary);

describe("Overview card math", () => {
    it("takes the CPU peak from the busiest bucket and ignores gaps", () => {
        const series = sampleSeriesFromIpc(nodesFixture.clusterSamples);
        expect(cpuPeak(series.points)).toBe(30);
        expect(cpuPeak(series.points.slice(0, 1))).toBeNull();
        expect(cpuPeak([])).toBeNull();
    });

    it("derives allocation and free memory from the summary, and nothing without a sampled node", () => {
        expect(memoryFigures(summary)).toEqual({ usedBytes: 1 << 30, totalBytes: 8 * 1024 ** 3, percent: 12.5, freeBytes: 7 * 1024 ** 3 });
        expect(memoryFigures({ ...summary, cpuPercent: null })).toBeNull();
        expect(memoryFigures({ ...summary, memoryUsedBytes: 9 * 1024 ** 3 })?.freeBytes).toBe(0);
    });

    it("sums network rates over online nodes and skips directions nobody reported", () => {
        expect(networkRates(nodes)).toEqual({ rx: 2048, tx: 0, total: 2048 });
        const [manager] = nodes;
        const second = { ...manager, id: "worker-1", latestSample: { ...manager.latestSample!, netRxBytesPerSec: 1000, netTxBytesPerSec: 500 } };
        expect(networkRates([manager, second])).toEqual({ rx: 3048, tx: 500, total: 3548 });
        expect(networkRates([{ ...manager, agentStatus: "degraded" }])).toBeNull();
    });

    it("counts live containers by state and breaks CPU down per node with unreachable ones as null", () => {
        const rows = containerListFromIpc(containersFixture.containerList).items.map((item) => ({ ...item, stale: false }));
        expect(containerCounts([...rows, { ...rows[0], stale: true }])).toEqual({ running: 1, paused: 0, exited: 1, total: 2 });
        expect(cpuByNode(nodes)).toEqual([
            { id: "manager", name: "manager", cores: 4, percent: 12.5 },
            { id: "worker-2", name: "worker-2", cores: 2, percent: null },
            { id: "w3kq2m9p0r1s3t5u7w9y0z2b4", name: "worker-3", cores: 2, percent: null },
        ]);
        expect(sampledNodeCount(nodes)).toBe(1);
    });
});

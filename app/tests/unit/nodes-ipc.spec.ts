import { describe, expect, it } from "vitest";
import fixture from "../fixtures/nodes-ipc.json";
import { agentNodes, clusterSummaryFromIpc, clusterUsage, controllerSettingsFromIpc, nodeListFromIpc, nodesDown, sampleSeriesFromIpc } from "@/types/nodes";

describe("nodes IPC contract", () => {
    it("parses the shared fixture exactly as Rust serializes it", () => {
        expect(nodeListFromIpc(fixture.nodeList)).toEqual(fixture.nodeList);
        expect(clusterSummaryFromIpc(fixture.clusterSummary)).toEqual(fixture.clusterSummary);
        expect(sampleSeriesFromIpc(fixture.clusterSamples)).toEqual(fixture.clusterSamples);
        expect(controllerSettingsFromIpc(fixture.settings)).toEqual({ metricsRetentionDays: 7 });
        const points = sampleSeriesFromIpc(fixture.clusterSamples).points;
        expect(points[0].cpuPercent).toBeNull();
        expect(points[0].nodes).toBe(0);
    });

    it("rejects extra fields, bad ids, unknown statuses, and out-of-range values", () => {
        const [manager] = fixture.nodeList.items;
        const list = (item: unknown) => ({ ...fixture.nodeList, items: [item] });
        expect(() => nodeListFromIpc({ ...fixture.nodeList, secret: "canary" })).toThrow();
        expect(() => nodeListFromIpc(list({ ...manager, id: "../bad" }))).toThrow();
        expect(() => nodeListFromIpc(list({ ...manager, agentStatus: "unknown" }))).toThrow();
        expect(() => nodeListFromIpc(list({ ...manager, role: "leader" }))).toThrow();
        expect(() => nodeListFromIpc(list({ ...manager, availability: "drained" }))).toThrow();
        expect(() => nodeListFromIpc(list({ ...manager, lastCheckInAt: null }))).toThrow();
        const swarmOnly = fixture.nodeList.items[2];
        expect(() => nodeListFromIpc(list({ ...swarmOnly, agentVersion: "0.1.0" }))).toThrow();
        expect(() => nodeListFromIpc(list({ ...swarmOnly, latestSample: manager.latestSample }))).toThrow();
        expect(() => clusterSummaryFromIpc({ ...fixture.clusterSummary, nodesTotal: 4 })).toThrow();
        expect(() => clusterSummaryFromIpc({ ...fixture.clusterSummary, managersReachable: 2 })).toThrow();
        expect(() => clusterSummaryFromIpc({ ...fixture.clusterSummary, quorum: "lost" })).toThrow();
        expect(() => clusterSummaryFromIpc({ ...fixture.clusterSummary, secret: "canary" })).toThrow();
        expect(() => nodeListFromIpc(list({ ...manager, lastCheckInAt: "2026-09-28T12:00:00Z" }))).toThrow();
        expect(() => nodeListFromIpc(list({ ...manager, latestSample: { ...manager.latestSample, cpuPercent: -1 } }))).toThrow();
        expect(() => sampleSeriesFromIpc({ ...fixture.clusterSamples, window: "2h" })).toThrow();
        expect(() => controllerSettingsFromIpc({ metricsRetentionDays: 0 })).toThrow();
        expect(() => controllerSettingsFromIpc({ metricsRetentionDays: 366 })).toThrow();
    });

    it("derives the status bar figures and the DOWN badge from the cluster summary", () => {
        const nodes = nodeListFromIpc(fixture.nodeList).items;
        const summary = clusterSummaryFromIpc(fixture.clusterSummary);
        expect(clusterUsage(summary, nodes)).toEqual({ cpuPercent: 12.5, memoryUsedBytes: 1073741824, memoryTotalBytes: 8589934592, dockerVersion: "29.8.0" });
        expect(nodesDown(summary)).toBe(2);
        expect(clusterUsage({ ...summary, cpuPercent: null }, [])).toEqual({});
        expect(clusterUsage(undefined, undefined)).toEqual({});
        expect(nodesDown(undefined)).toBeUndefined();
        expect(agentNodes(nodes).map((node) => node.id)).toEqual(["manager", "worker-2"]);
    });
});

import { describe, expect, it } from "vitest";
import fixture from "../fixtures/nodes-ipc.json";
import { clusterUsage, controllerSettingsFromIpc, nodeListFromIpc, nodesDown, sampleSeriesFromIpc } from "@/types/nodes";

describe("nodes IPC contract", () => {
    it("parses the shared fixture exactly as Rust serializes it", () => {
        expect(nodeListFromIpc(fixture.nodeList)).toEqual(fixture.nodeList);
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
        expect(() => nodeListFromIpc(list({ ...manager, status: "unknown" }))).toThrow();
        expect(() => nodeListFromIpc(list({ ...manager, lastCheckInAt: "2026-09-28T12:00:00Z" }))).toThrow();
        expect(() => nodeListFromIpc(list({ ...manager, latestSample: { ...manager.latestSample, cpuPercent: -1 } }))).toThrow();
        expect(() => sampleSeriesFromIpc({ ...fixture.clusterSamples, window: "2h" })).toThrow();
        expect(() => controllerSettingsFromIpc({ metricsRetentionDays: 0 })).toThrow();
        expect(() => controllerSettingsFromIpc({ metricsRetentionDays: 366 })).toThrow();
    });

    it("derives the status bar figures and the DOWN badge from online members", () => {
        const nodes = nodeListFromIpc(fixture.nodeList).items;
        expect(clusterUsage(nodes)).toEqual({ cpuPercent: 12.5, memoryUsedBytes: 1073741824, memoryTotalBytes: 8589934592, dockerVersion: "29.8.0" });
        expect(nodesDown(nodes)).toBe(1);
        expect(clusterUsage([])).toEqual({});
        expect(clusterUsage(undefined)).toEqual({});
        expect(nodesDown(undefined)).toBeUndefined();
    });
});

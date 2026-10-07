import { describe, expect, it } from "vitest";
import fixture from "../fixtures/nodes-ipc.json";
import { ipcFixture } from "../ipc-fixture";
import {
    agentNodes,
    clusterUsage,
    type ClusterSummary,
    type ControllerSettings,
    type ControllerStatusView,
    type NodeList,
    type SampleSeries,
} from "@/types/nodes";

// The shared fixture must fit the views Rust's own tests serialize to it.
const nodes = ipcFixture<NodeList>(fixture.nodeList).items;
const summary = ipcFixture<ClusterSummary>(fixture.clusterSummary);
ipcFixture<SampleSeries>(fixture.clusterSamples);
ipcFixture<ControllerSettings>(fixture.settings);
ipcFixture<ControllerStatusView>(fixture.status);

describe("nodes IPC contract", () => {
    it("derives the status bar figures from the cluster summary", () => {
        expect(clusterUsage(summary, nodes)).toEqual({ cpuPercent: 12.5, memoryUsedBytes: 1073741824, memoryTotalBytes: 8589934592, dockerVersion: "29.8.0" });
        expect(clusterUsage({ ...summary, cpuPercent: null }, [])).toEqual({});
        expect(clusterUsage(undefined, undefined)).toEqual({});
        expect(agentNodes(nodes).map((node) => node.id)).toEqual(["manager", "worker-2"]);
    });
});

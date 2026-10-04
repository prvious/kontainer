import { describe, expect, it } from "vitest";
import { healthNote } from "@/components/nodes/node-format";
import type { NodeSample, NodeSummary } from "@/types/nodes";

const now = Date.parse("2026-10-04T12:00:00.000Z");
const secondsAgo = (seconds: number) => new Date(now - seconds * 1000).toISOString();

const sample: NodeSample = {
    sampledAt: secondsAgo(2),
    cpuPercent: 12,
    memoryUsedBytes: 1_000_000_000,
    diskUsedBytes: null,
    containersRunning: 5,
    containersPaused: 0,
    containersExited: 0,
    netRxBytesPerSec: null,
    netTxBytesPerSec: null,
};

const degraded: NodeSummary = {
    id: "worker-1",
    hostname: "worker-1",
    agentStatus: "degraded",
    health: "degraded",
    role: "worker",
    isLeader: false,
    availability: "active",
    swarmStatus: "ready",
    swarmNodeId: "swarm-worker-1",
    address: "10.0.1.11",
    agentVersion: "0.1.0",
    dockerVersion: "28.5.1",
    os: "Alpine Linux v3.22",
    arch: "aarch64",
    cpuCores: 4,
    memoryTotalBytes: 8_000_000_000,
    agentStartedAt: secondsAgo(3600),
    firstSeenAt: secondsAgo(3600),
    lastCheckInAt: secondsAgo(3),
    latestSample: null,
};

describe("a degraded node's note", () => {
    it("blames Docker when the agent checks in but sends no fresh sample", () => {
        expect(healthNote(degraded, now)).toBe("The agent can't read Docker on this node");
    });

    it("says when the agent last checked in once it has gone quiet", () => {
        expect(healthNote({ ...degraded, lastCheckInAt: secondsAgo(42) }, now)).toBe("Agent last checked in 42s ago");
        expect(healthNote({ ...degraded, latestSample: sample, lastCheckInAt: secondsAgo(42) }, now)).toBe("Agent last checked in 42s ago");
    });
});

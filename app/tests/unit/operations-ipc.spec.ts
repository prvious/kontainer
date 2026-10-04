import { describe, expect, it } from "vitest";
import fixture from "../fixtures/operations-ipc.json";
import inventory from "../fixtures/inventory-ipc.json";
import { ipcFixture } from "../ipc-fixture";
import { INVENTORY_ERROR_CODES } from "@/types/inventory";
import { isTerminal, progressCounts, type AdmittedOperation, type OperationEventView, type OperationView } from "@/types/operations";
import { operationMeta, operationPhase, operationSummary, operationTrack } from "@/components/operations/operation-meta";

// The shared fixture must fit the views Rust's own tests serialize to it.
const running = ipcFixture<OperationView>(fixture.operation);
const failed = ipcFixture<OperationView>(fixture.failedOperation);
ipcFixture<OperationEventView[]>(fixture.events);
ipcFixture<AdmittedOperation>(fixture.admitted);

describe("operations IPC contract", () => {
    it("shares the error codes with the Rust fixture", () => {
        expect(INVENTORY_ERROR_CODES).toEqual(inventory.errorCodes);
    });
});

describe("operation status lines", () => {
    const now = Date.parse("2026-09-28T12:00:12.500Z");

    it("reads like the design for every phase", () => {
        expect(operationMeta(undefined, now)).toBe("QUEUED · waiting for a worker");
        expect(operationMeta(running, now)).toBe("RUNNING · 1 of 3 · 12s");
        expect(operationMeta(failed, now)).toBe("FAILED · no_eligible_nodes — No node satisfies the placement constraints. · req 01J9ZM2R");
        const succeeded = { ...running, state: "succeeded" as const, progress: { total: 3, done: 3 }, finishedAt: "2026-09-28T12:00:18.500Z" };
        expect(operationMeta(succeeded, now)).toBe("SUCCEEDED · 3 of 3 · finished in 18s");
        expect(operationSummary(succeeded)).toBe("3 of 3 · finished in 18s");
        const uncertain = { ...running, state: "uncertain" as const, errorCode: "recovered_after_restart" };
        expect(operationMeta(uncertain, now)).toContain("the controller restarted while this ran");
        expect(operationMeta({ ...running, progress: { total: 3, done: 2, failed: 1 } }, now)).toBe("RUNNING · 2 of 3 · 1 failed · 12s");
    });

    it("maps phases and the track", () => {
        expect(operationPhase(undefined)).toBe("queued");
        expect(operationPhase({ ...running, state: "recoverable" })).toBe("queued");
        expect(operationPhase(failed)).toBe("failed");
        expect(operationTrack(running)).toBeCloseTo(1 / 3);
        expect(operationTrack({ ...running, state: "succeeded", progress: null })).toBe(1);
        expect(operationTrack(undefined)).toBeNull();
        expect(progressCounts({ total: 0, done: 0 })).toBeNull();
        expect(isTerminal("uncertain")).toBe(false);
    });
});

import { describe, expect, it } from "vitest";
import fixture from "../fixtures/stacks-ipc.json";
import { ipcFixture } from "../ipc-fixture";
import type { StackListView, StackPlanView } from "@/types/stacks";
import { operationSummary } from "@/components/operations/operation-meta";
import type { OperationView } from "@/types/operations";

// The shared fixtures must fit the views Rust's own tests serialize to them.
ipcFixture<StackListView>(fixture.stackList);
ipcFixture<StackPlanView>(fixture.plan);

describe("stacks IPC contract", () => {
    it("summarize a removal with its counts", () => {
        const view = {
            state: "succeeded",
            progress: { phase: "removed", servicesRemoved: 3, networksRemoved: 1, volumesRetained: ["toolbox_redis-data"] },
            startedAt: "2026-09-28T12:00:00.000Z",
            finishedAt: "2026-09-28T12:00:09.000Z",
        } as unknown as OperationView;
        expect(operationSummary(view)).toBe("3 services and 1 network removed · 1 volume kept · finished in 9s");
    });
});

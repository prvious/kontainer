import { describe, expect, it } from "vitest";
import fixture from "../fixtures/stacks-ipc.json";
import { stackListFromIpc, stackPlanFromIpc } from "@/types/stacks";
import { operationSummary } from "@/components/operations/operation-meta";
import type { OperationView } from "@/types/operations";

describe("stacks IPC contract", () => {
    it("parses the shared fixtures exactly as Rust serializes them", () => {
        expect(stackListFromIpc(fixture.stackList)).toEqual(fixture.stackList);
        expect(stackPlanFromIpc(fixture.plan)).toEqual(fixture.plan);
    });

    it("rejects extra fields, unknown statuses, and negative figures", () => {
        const [first] = fixture.stackList.items;
        const withItem = (item: unknown) => ({ ...fixture.stackList, items: [item] });
        expect(() => stackListFromIpc(withItem({ ...first, composeFile: "/secret.yml" }))).toThrow();
        expect(() => stackListFromIpc(withItem({ ...first, status: "exploded" }))).toThrow();
        expect(() => stackListFromIpc(withItem({ ...first, cpuPercent: -1 }))).toThrow();
        expect(() => stackPlanFromIpc({ ...fixture.plan, volumesRetained: [1] })).toThrow();
    });

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

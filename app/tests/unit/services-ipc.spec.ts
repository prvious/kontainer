import { describe, expect, it } from "vitest";
import fixture from "../fixtures/services-ipc.json";
import { serviceDetailFromIpc, serviceListFromIpc, serviceStatus } from "@/types/services";
import { applyServiceFilters, parseServiceSearch, portsText, serviceSummary } from "@/components/services/service-search";
import { progressCounts } from "@/types/operations";
import { operationMeta } from "@/components/operations/operation-meta";

const list = () => serviceListFromIpc(structuredClone(fixture.serviceList));

describe("services IPC contract", () => {
    it("parses the shared fixtures exactly as Rust serializes them", () => {
        expect(serviceListFromIpc(fixture.serviceList)).toEqual(fixture.serviceList);
        expect(serviceDetailFromIpc(fixture.serviceDetail)).toEqual(fixture.serviceDetail);
    });

    it("rejects extra fields, unknown modes, and bad ids", () => {
        const [first] = fixture.serviceList.items;
        const withItem = (item: unknown) => ({ ...fixture.serviceList, items: [item] });
        expect(() => serviceListFromIpc(withItem({ ...first, env: ["SECRET=canary"] }))).toThrow();
        expect(() => serviceListFromIpc(withItem({ ...first, mode: "job" }))).toThrow();
        expect(() => serviceListFromIpc(withItem({ ...first, id: "short" }))).toThrow();
        expect(() => serviceListFromIpc(withItem({ ...first, runningReplicas: -1 }))).toThrow();
    });
});

describe("service status and summary", () => {
    it("derive the STATUS column", () => {
        const [collector, web, alpine] = list().items;
        expect(serviceStatus(collector!, null)).toBe("running");
        expect(serviceStatus(web!, null)).toBe("partial");
        expect(serviceStatus(alpine!, null)).toBe("updating");
        expect(serviceStatus({ ...alpine!, updateState: null }, null)).toBe("stopped");
        expect(serviceStatus(collector!, "scale")).toBe("scaling");
        expect(serviceStatus(collector!, "other")).toBe("updating");
    });

    it("summarize modes and health, filter, and show ports", () => {
        const rows = list().items;
        expect(serviceSummary(rows)).toBe("3 services · 2 replicated · 1 global · 2 services degraded");
        expect(serviceSummary(rows.slice(0, 1))).toBe("1 service · 0 replicated · 1 global · all tasks running");
        const status = (row: (typeof rows)[number]) => serviceStatus(row, null);
        expect(applyServiceFilters(rows, { mode: "global" }, status).map((row) => row.name)).toEqual(["monitoring_collector"]);
        expect(applyServiceFilters(rows, { status: "partial" }, status).map((row) => row.name)).toEqual(["webapp_web"]);
        expect(applyServiceFilters(rows, { stack: "toolbox" }, status).map((row) => row.name)).toEqual(["toolbox_alpine"]);
        expect(parseServiceSearch({ mode: "job", status: "running", sort: "size", stack: "../x" })).toEqual({ status: "running" });
        expect(portsText(["80:80/tcp", "53:53/udp"])).toBe("80:80, 53:53/udp");
        expect(portsText([])).toBe("—");
    });

    it("read a service operation's progress as tasks running", () => {
        expect(progressCounts({ desired: 5, running: 4, phase: "converging" })).toEqual({ total: 5, done: 4, failed: 0, unit: "tasks running" });
        const view = {
            id: "op",
            kind: "service.scale",
            state: "running",
            lastEventSequence: 2,
            errorCode: null,
            errorMessage: null,
            progress: { desired: 5, running: 4, phase: "converging" },
            createdAt: "2026-09-28T12:00:00.000Z",
            updatedAt: "2026-09-28T12:00:05.000Z",
            startedAt: "2026-09-28T12:00:00.000Z",
            finishedAt: null,
            requestId: null,
        } as const;
        expect(operationMeta(view, Date.parse("2026-09-28T12:00:12.000Z"))).toBe("RUNNING · 4 of 5 tasks running · 12s");
    });
});

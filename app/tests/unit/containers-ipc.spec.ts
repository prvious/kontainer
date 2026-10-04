import { describe, expect, it } from "vitest";
import fixture from "../fixtures/containers-ipc.json";
import { ipcFixture } from "../ipc-fixture";
import { containerRowId, type ContainerListView } from "@/types/containers";
import { mergeSnapshots } from "@/hooks/use-containers";
import { applyFilters, parseContainerSearch, sortFor, sortingFor } from "@/components/containers/container-search";
import { bytes, portsText, statusChip, uptime, uptimeText } from "@/components/containers/container-format";

// The shared fixture must fit the view Rust's own tests serialize to it.
const list = () => ipcFixture<ContainerListView>(fixture.containerList);

describe("partial snapshots", () => {
    it("carries a silent node's last rows forward as stale and drops them once it answers", () => {
        const first = mergeSnapshots(undefined, list());
        expect(first.items.every((row) => !row.stale)).toBe(true);
        const silent = list();
        silent.items = silent.items.filter((item) => item.nodeId !== "worker-1");
        silent.nodes = silent.nodes.map((node) => (node.nodeId === "worker-1" ? { ...node, status: "timeout" } : node));
        const second = mergeSnapshots(first, silent);
        expect(second.items.map((row) => [row.name, row.stale])).toEqual([
            ["shop_web.1.x7kq", false],
            ["scratch", true],
        ]);
        const third = mergeSnapshots(second, list());
        expect(third.items.every((row) => !row.stale)).toBe(true);
        expect(third.items).toHaveLength(2);
    });
});

describe("container search params", () => {
    it("validate with defaults and drop unknown values", () => {
        expect(parseContainerSearch({})).toEqual({});
        expect(parseContainerSearch({ q: "web", state: ["running", "bogus", "running"], node: "manager", ports: "published", sort: "cpu" })).toEqual({
            q: "web",
            state: ["running"],
            node: "manager",
            ports: "published",
            sort: "cpu",
        });
        expect(parseContainerSearch({ state: "exited", sort: "recent", node: "../x", ports: "all" })).toEqual({ state: ["exited"] });
    });

    it("filter by state, node, and published ports", () => {
        const rows = mergeSnapshots(undefined, list()).items;
        expect(applyFilters(rows, { state: ["exited"] }).map((row) => row.name)).toEqual(["scratch"]);
        expect(applyFilters(rows, { node: "manager" }).map((row) => row.name)).toEqual(["shop_web.1.x7kq"]);
        expect(applyFilters(rows, { ports: "published" }).map((row) => row.name)).toEqual(["shop_web.1.x7kq"]);
    });

    it("map sort params to table sorting and back", () => {
        expect(sortingFor(undefined)).toEqual([{ id: "uptime", desc: true }]);
        expect(sortingFor("name")).toEqual([{ id: "name", desc: false }]);
        expect(sortFor([{ id: "cpu", desc: true }])).toBe("cpu");
        expect(sortFor([{ id: "image", desc: false }])).toBeUndefined();
    });
});

describe("container formatting", () => {
    const [web, scratch] = mergeSnapshots(undefined, list()).items;
    const now = Date.parse("2026-09-28T12:03:02.000Z");

    it("labels states like the design", () => {
        expect(statusChip(web)).toEqual({ label: "UP", tone: "success" });
        expect(statusChip(scratch)).toEqual({ label: "EXITED", tone: "destructive" });
        expect(statusChip({ ...web, stale: true })).toEqual({ label: "UNKNOWN", tone: "neutral" });
        expect(statusChip({ ...web, state: "paused" })).toEqual({ label: "PAUSED", tone: "warning" });
    });

    it("formats uptime, exits, bytes, and ports", () => {
        expect(uptime(14 * 86_400 + 7 * 3600 + 5)).toBe("14d 7h");
        expect(uptime(125)).toBe("2m");
        expect(uptimeText(web, "2026-09-28T12:05:00.000Z", now)).toBe("3m");
        expect(uptimeText(scratch, "2026-09-28T12:05:00.000Z", now)).toBe("exited 2m ago");
        expect(bytes(52_428_800)).toBe("50 MB");
        expect(bytes(2.1 * 1024 ** 3)).toBe("2.1 GB");
        expect(bytes(null)).toBe("—");
        expect(portsText(["8080:80/tcp", "8080:80/udp", "443:443/tcp"])).toBe("8080, 443");
        expect(portsText([])).toBe("—");
        expect(containerRowId(web)).toBe(`manager/${"a".repeat(64)}`);
    });
});

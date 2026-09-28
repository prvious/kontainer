import { describe, expect, it } from "vitest";
import fixture from "../fixtures/networks-ipc.json";
import containersFixture from "../fixtures/containers-ipc.json";
import { attachedFromIpc, isCidr, networkListFromIpc, networkNode, networkRowId, shortNetworkId } from "@/types/networks";
import { containerListFromIpc } from "@/types/containers";
import { applyFilters, parseContainerSearch } from "@/components/containers/container-search";
import { mergeSnapshots } from "@/hooks/use-containers";
import { applyNetworkFilters, driverSummary, networkNameProblem, parseNetworkSearch } from "@/components/networks/network-search";

const list = () => networkListFromIpc(structuredClone(fixture.networkList));

describe("networks IPC contract", () => {
    it("parses the shared fixtures exactly as Rust serializes them", () => {
        expect(networkListFromIpc(fixture.networkList)).toEqual(fixture.networkList);
        expect(attachedFromIpc(fixture.attached)).toEqual(fixture.attached);
    });

    it("rejects extra fields, unknown scopes, and local networks on several nodes", () => {
        const [bridge] = fixture.networkList.items;
        const withItem = (item: unknown) => ({ ...fixture.networkList, items: [item] });
        expect(() => networkListFromIpc(withItem({ ...bridge, options: { secret: "canary" } }))).toThrow();
        expect(() => networkListFromIpc(withItem({ ...bridge, scope: "global" }))).toThrow();
        expect(() => networkListFromIpc(withItem({ ...bridge, nodeIds: ["manager", "worker-1"] }))).toThrow();
        expect(() => networkListFromIpc({ ...fixture.networkList, manager: "maybe" })).toThrow();
        expect(() => attachedFromIpc([{ nodeId: "manager", containerId: "short", name: "x" }])).toThrow();
    });

    it("keys swarm networks once and local ones per node", () => {
        const [bridge, overlay] = list().items;
        expect(networkRowId(bridge!)).toBe(`manager/${"b".repeat(64)}`);
        expect(networkRowId(overlay!)).toBe("x7kq2m9p0r1s3t5u7w9y0z2b4");
        expect(networkNode(bridge!)).toBe("manager");
        expect(networkNode(overlay!)).toBeNull();
        expect(shortNetworkId(overlay!.id)).toBe("x7kq2m9p0r1s");
    });

    it("carries container networks for the Containers page filter", () => {
        const rows = mergeSnapshots(undefined, containerListFromIpc(structuredClone(containersFixture.containerList))).items;
        expect(parseContainerSearch({ network: "shop_default" })).toEqual({ network: "shop_default" });
        expect(parseContainerSearch({ network: "-bad" })).toEqual({});
        expect(applyFilters(rows, { network: "shop_default" }).map((row) => row.name)).toEqual(["shop_web.1.x7kq"]);
    });
});

describe("network helpers", () => {
    it.each(["10.0.9.0/24", "172.28.0.0/16", "10.0.0.0/8", "192.168.1.128/25", "fd00:1::/64", "fd00:0:0:1::/64"])("accepts the CIDR %s", (cidr) =>
        expect(isCidr(cidr)).toBe(true),
    );
    it.each(["10.0.9.1/24", "10.0.9.0", "10.0.9.0/33", "10.0.9.0/4", "10.0.9/24", "999.0.0.0/8", "10.0.9.0/024", "010.0.9.0/24", "fd00::1/64", "banana"])(
        "rejects the CIDR %s",
        (cidr) => expect(isCidr(cidr)).toBe(false),
    );

    it("validate names against the pattern, the built-ins, and the current list", () => {
        const rows = list().items;
        expect(networkNameProblem("app-network", rows)).toBeNull();
        expect(networkNameProblem("-bad", rows)).toMatch(/1–128 letters/);
        expect(networkNameProblem("ingress", rows)).toMatch(/built-in/);
        expect(networkNameProblem("toolbox_kontainer", rows)).toBe("A network named toolbox_kontainer already exists. Pick another name.");
    });

    it("summarize drivers and filter by scope, driver, and node", () => {
        const rows = list().items;
        expect(driverSummary(rows)).toBe("3 networks · 2 overlay · 1 bridge");
        expect(applyNetworkFilters(rows, { scope: "swarm" }).map((row) => row.name)).toEqual(["toolbox_kontainer", "empty-overlay"]);
        expect(applyNetworkFilters(rows, { node: "worker-1" }).map((row) => row.name)).toEqual(["toolbox_kontainer"]);
        expect(parseNetworkSearch({ scope: "global", driver: "overlay", node: "../x" })).toEqual({ driver: "overlay" });
    });
});

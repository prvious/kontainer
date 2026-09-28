import { describe, expect, it } from "vitest";
import fixture from "../fixtures/volumes-ipc.json";
import { LABEL_PROBLEMS, parseLabel, VOLUME_NAME, volumeListFromIpc, volumeRowId } from "@/types/volumes";
import { allocated, applyVolumeFilters, middleTruncate, parseVolumeSearch, volumeSortFor, volumeSortingFor } from "@/components/volumes/volume-search";

const list = () => volumeListFromIpc(structuredClone(fixture.volumeList));

describe("volumes IPC contract", () => {
    it("parses the shared fixture exactly as Rust serializes it", () => {
        expect(volumeListFromIpc(fixture.volumeList)).toEqual(fixture.volumeList);
    });

    it("rejects extra fields, bad names, and negative sizes", () => {
        const [redis] = fixture.volumeList.items;
        const withItem = (item: unknown) => ({ ...fixture.volumeList, items: [item] });
        expect(() => volumeListFromIpc(withItem({ ...redis, options: { device: "secret-canary" } }))).toThrow();
        expect(() => volumeListFromIpc(withItem({ ...redis, name: "-bad" }))).toThrow();
        expect(() => volumeListFromIpc(withItem({ ...redis, sizeBytes: -1 }))).toThrow();
        expect(() => volumeListFromIpc(withItem({ ...redis, labels: { a: 1 } }))).toThrow();
    });

    it("keys rows by node and name", () => {
        expect(list().items.map(volumeRowId)).toEqual(["manager/toolbox_redis-data", "worker-1/scratch"]);
    });
});

describe("volume names and labels", () => {
    it.each(["pg", "postgres-data", "a.b_c-d", "0volume"])("accepts the name %s", (name) => expect(VOLUME_NAME.test(name)).toBe(true));
    it.each(["", "a", "-data", ".data", "has space", "sl/ash", "a".repeat(256)])("rejects the name %j", (name) => expect(VOLUME_NAME.test(name)).toBe(false));

    it("parse key=value labels and say why one can't be added", () => {
        expect(parseLabel("app=billing", {})).toEqual({ key: "app", value: "billing" });
        expect(parseLabel("tier", {})).toEqual({ key: "tier", value: "" });
        expect(parseLabel("=x", {})).toBe("format");
        expect(parseLabel("app=other", { app: "billing" })).toBe("duplicate");
        expect(parseLabel(`k=${"v".repeat(257)}`, {})).toBe("too_long");
        const many = Object.fromEntries(Array.from({ length: 64 }, (_, index) => [`k${index}`, ""]));
        expect(parseLabel("one=more", many)).toBe("too_many");
        expect(LABEL_PROBLEMS.duplicate).toBe("That key is already set.");
    });
});

describe("volume search and figures", () => {
    it("validate params with defaults and drop unknown values", () => {
        expect(parseVolumeSearch({})).toEqual({});
        expect(parseVolumeSearch({ q: "redis", node: "manager", driver: "local", attached: "no", sort: "name", highlightedRow: "manager/x" })).toEqual({
            q: "redis",
            node: "manager",
            driver: "local",
            attached: "no",
            sort: "name",
            highlightedRow: "manager/x",
        });
        expect(parseVolumeSearch({ node: "../x", attached: "maybe", sort: "size" })).toEqual({});
    });

    it("filter by node, driver, and attached", () => {
        const rows = list().items;
        expect(applyVolumeFilters(rows, { attached: "yes" }).map((row) => row.name)).toEqual(["toolbox_redis-data"]);
        expect(applyVolumeFilters(rows, { attached: "no", node: "worker-1" }).map((row) => row.name)).toEqual(["scratch"]);
        expect(applyVolumeFilters(rows, { driver: "nfs" })).toEqual([]);
        expect(volumeSortingFor(undefined)).toEqual([{ id: "size", desc: true }]);
        expect(volumeSortFor(volumeSortingFor("created"))).toBe("created");
    });

    it("sum known sizes and say when some are pending", () => {
        expect(allocated(list().items)).toEqual({ bytes: 536_870_912, pending: true });
        expect(allocated([])).toEqual({ bytes: 0, pending: false });
    });

    it("keep both ends of a long mount point", () => {
        const path = "/var/lib/docker/volumes/a-really-long-volume-name-for-tests/_data";
        const shown = middleTruncate(path, 30);
        expect(shown).toHaveLength(30);
        expect(shown.startsWith("/var/lib/dock")).toBe(true);
        expect(shown.endsWith("tests/_data")).toBe(true);
        expect(middleTruncate("/short")).toBe("/short");
    });
});

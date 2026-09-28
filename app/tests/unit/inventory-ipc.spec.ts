import { describe, expect, it } from "vitest";
import fixture from "../fixtures/inventory-ipc.json";
import { INVENTORY_ERROR_CODES, containerPageFromIpc, inventoryErrorCode } from "@/types/inventory";

describe("inventory IPC contract", () => {
    it("matches the Rust fixture and projects exactly the page fields", () => {
        expect(containerPageFromIpc(fixture.containerPage)).toEqual(fixture.containerPage);
        expect(Object.keys(containerPageFromIpc(fixture.containerPage))).toEqual(["items", "nextCursor"]);
        expect(Object.keys(containerPageFromIpc(fixture.containerPage).items[0])).toEqual([
            "containerId",
            "displayName",
            "imageReference",
            "state",
            "status",
            "createdAt",
            "nodeId",
        ]);
        expect(INVENTORY_ERROR_CODES).toEqual(fixture.errorCodes);
        expect(fixture.commands).toEqual(["list_container_inventory"]);
    });

    it("rejects extra fields and unknown states without caching canaries", () => {
        expect(() => containerPageFromIpc({ ...fixture.containerPage, credential: "secret-canary" })).toThrow();
        expect(() => containerPageFromIpc({ ...fixture.containerPage, items: [{ ...fixture.containerPage.items[0], state: "secret-canary" }] })).toThrow();
        expect(inventoryErrorCode(new Error("secret-canary"))).toBe("invalid_response");
    });
});

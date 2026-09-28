import { describe, expect, it } from "vitest";
import fixture from "../fixtures/inventory-ipc.json";
import { INVENTORY_ERROR_CODES, exactRecord, inventoryErrorCode } from "@/types/inventory";

describe("authenticated call error codes", () => {
    it("match the Rust fixture and close over unknown values", () => {
        expect(INVENTORY_ERROR_CODES).toEqual(fixture.errorCodes);
        expect(inventoryErrorCode("too_many_containers")).toBe("too_many_containers");
        expect(inventoryErrorCode(new Error("secret-canary"))).toBe("invalid_response");
        expect(inventoryErrorCode(undefined)).toBe("invalid_response");
    });

    it("exact records reject missing and extra keys", () => {
        expect(exactRecord({ a: 1, b: 2 }, ["b", "a"])).toEqual({ a: 1, b: 2 });
        expect(() => exactRecord({ a: 1 }, ["a", "b"])).toThrow();
        expect(() => exactRecord({ a: 1, secret: "canary" }, ["a"])).toThrow();
        expect(() => exactRecord([1], ["0"])).toThrow();
    });
});

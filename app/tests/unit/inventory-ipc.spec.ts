import { describe, expect, it } from "vitest";
import fixture from "../fixtures/inventory-ipc.json";
import { INVENTORY_ERROR_CODES, inventoryErrorCode } from "@/types/inventory";

describe("authenticated call error codes", () => {
    it("match the Rust fixture and close over unknown values", () => {
        expect(INVENTORY_ERROR_CODES).toEqual(fixture.errorCodes);
        expect(inventoryErrorCode("too_many_containers")).toBe("too_many_containers");
        expect(inventoryErrorCode(new Error("secret-canary"))).toBe("invalid_response");
        expect(inventoryErrorCode(undefined)).toBe("invalid_response");
    });
});

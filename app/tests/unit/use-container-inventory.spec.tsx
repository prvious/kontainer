import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AuthContext } from "@/contexts/auth-context-value";
import { useContainerInventory, containerInventoryKey } from "@/hooks/use-container-inventory";
import { nodesQueries } from "@/hooks/use-nodes";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import fixture from "../fixtures/inventory-ipc.json";

const profileId = "018f2e34-0000-7000-8000-0000000000bb";
const reportInventoryCustodyError = vi.fn();
const auth = { reportInventoryCustodyError, session: { profileId }, expired: false } as unknown as AuthContextValue;

function createWrapper(value: AuthContextValue = auth) {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const wrapper = ({ children }: { children: React.ReactNode }) => (
        <AuthContext value={value}>
            <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
        </AuthContext>
    );
    return { queryClient, wrapper };
}

describe("native container inventory queries", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("keys each page by profile, node, cursor, and limit without credentials", async () => {
        vi.mocked(invoke).mockResolvedValue(fixture.containerPage);
        const { queryClient, wrapper } = createWrapper();
        const cursor = "v1:" + "a".repeat(64);
        const { result } = renderHook(() => useContainerInventory(profileId, "node-1", cursor, 50), { wrapper });
        await waitFor(() => expect(result.current.data).toEqual(fixture.containerPage));
        expect(invoke).toHaveBeenCalledExactlyOnceWith("list_container_inventory", {
            profileId,
            nodeId: "node-1",
            cursor,
            limit: 50,
        });
        expect(queryClient.getQueryData(containerInventoryKey(profileId, "node-1", cursor, 50))).toEqual(fixture.containerPage);
    });

    it("reports a rejected session once without query retries or raw error text", async () => {
        vi.mocked(invoke).mockRejectedValue("relogin_required");
        const { wrapper } = createWrapper();
        const { result } = renderHook(() => useContainerInventory(profileId, "node-1", null, 50), { wrapper });
        await waitFor(() => expect(result.current.isError).toBe(true));
        expect(result.current.error).toBe("relogin_required");
        expect(reportInventoryCustodyError).toHaveBeenCalledExactlyOnceWith(profileId, "relogin_required");
        expect(invoke).toHaveBeenCalledTimes(1);
    });

    it("refreshes membership after a missing node", async () => {
        vi.mocked(invoke).mockRejectedValue("node_not_found");
        const { queryClient, wrapper } = createWrapper();
        const invalidate = vi.spyOn(queryClient, "invalidateQueries");
        const { result } = renderHook(() => useContainerInventory(profileId, "old-node", null, 50), { wrapper });
        await waitFor(() => expect(result.current.isError).toBe(true));
        expect(invalidate).toHaveBeenCalledWith({ queryKey: nodesQueries.all(profileId) });
    });
});

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAgentStatus } from "@/hooks/use-agent-status";

function createWrapper() {
    const qc = new QueryClient({
        defaultOptions: { queries: { retry: false } },
    });
    return ({ children }: { children: React.ReactNode }) => <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

describe("useAgentStatus", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("calls check_auth with url and token", async () => {
        vi.mocked(invoke).mockResolvedValue({
            user: { id: "u1", name: "Jane", email: "j@e.com", avatarUrl: "", provider: "github" },
            status: "ok",
        });

        const onAuthFailure = vi.fn();
        renderHook(() => useAgentStatus("http://localhost:8080", "my-jwt", onAuthFailure), { wrapper: createWrapper() });

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith("check_auth", {
                url: "http://localhost:8080",
                token: "my-jwt",
            });
        });

        expect(onAuthFailure).not.toHaveBeenCalled();
    });

    it("calls onAuthFailure on auth_failed error (401)", async () => {
        vi.mocked(invoke).mockRejectedValue("auth_failed");

        const onAuthFailure = vi.fn();
        renderHook(() => useAgentStatus("http://localhost:8080", "expired-jwt", onAuthFailure), { wrapper: createWrapper() });

        await waitFor(() => {
            expect(onAuthFailure).toHaveBeenCalledTimes(1);
        });
    });

    it("does NOT call onAuthFailure on non-auth errors", async () => {
        vi.mocked(invoke).mockRejectedValue("Could not reach agent at http://localhost:8080: timeout");

        const onAuthFailure = vi.fn();
        renderHook(() => useAgentStatus("http://localhost:8080", "my-jwt", onAuthFailure), { wrapper: createWrapper() });

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith("check_auth", expect.anything());
        });

        expect(onAuthFailure).not.toHaveBeenCalled();
    });

    it("is disabled when url is null", async () => {
        const onAuthFailure = vi.fn();
        renderHook(() => useAgentStatus(null, "my-jwt", onAuthFailure), { wrapper: createWrapper() });

        // Give it a tick to settle
        await new Promise((r) => setTimeout(r, 50));

        expect(invoke).not.toHaveBeenCalledWith("check_auth", expect.anything());
    });

    it("is disabled when token is null", async () => {
        const onAuthFailure = vi.fn();
        renderHook(() => useAgentStatus("http://localhost:8080", null, onAuthFailure), { wrapper: createWrapper() });

        await new Promise((r) => setTimeout(r, 50));

        expect(invoke).not.toHaveBeenCalledWith("check_auth", expect.anything());
    });
});

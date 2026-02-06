import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { AuthProvider } from "@/contexts/auth-context";
import { useAuth } from "@/hooks/use-auth";
import type { StoredSession } from "@/types/auth";

function TestConsumer() {
    const { user, isLoading } = useAuth();
    return (
        <div>
            <span data-testid='user'>{user ? user.name : "none"}</span>
            <span data-testid='isLoading'>{isLoading ? "true" : "false"}</span>
        </div>
    );
}

describe("Token Expiry", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.useFakeTimers();

        vi.mocked(onOpenUrl).mockImplementation(() => {
            return Promise.resolve(() => {});
        });
    });

    afterEach(() => {
        vi.useRealTimers();
    });

    function renderWithProvider() {
        return render(
            <AuthProvider>
                <TestConsumer />
            </AuthProvider>,
        );
    }

    it("auto-logs out when token expires", async () => {
        // Token expires at now + 15 seconds
        const nowSec = Math.floor(Date.now() / 1000);
        const session: StoredSession = {
            userId: "123",
            userName: "Jane Doe",
            userEmail: "jane@example.com",
            userAvatarUrl: "https://example.com/avatar.png",
            userProvider: "github",
            token: "valid.jwt.token",
            expiresAt: nowSec + 15,
            agentUrl: "http://localhost:8080",
        };

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_session") return session;
            if (cmd === "delete_session") return undefined;
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        // User should be restored
        expect(screen.getByTestId("user")).toHaveTextContent("Jane Doe");

        // Advance time past expiry (30s interval, token expires at +15s)
        await act(async () => {
            vi.advanceTimersByTime(31_000);
        });

        // User should be logged out
        expect(screen.getByTestId("user")).toHaveTextContent("none");
        expect(invoke).toHaveBeenCalledWith("delete_session");
    });

    it("does not log out when token is still valid", async () => {
        const nowSec = Math.floor(Date.now() / 1000);
        const session: StoredSession = {
            userId: "123",
            userName: "Jane Doe",
            userEmail: "jane@example.com",
            userAvatarUrl: "https://example.com/avatar.png",
            userProvider: "github",
            token: "valid.jwt.token",
            expiresAt: nowSec + 99999,
            agentUrl: "http://localhost:8080",
        };

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_session") return session;
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        expect(screen.getByTestId("user")).toHaveTextContent("Jane Doe");

        // Advance one interval
        await act(async () => {
            vi.advanceTimersByTime(31_000);
        });

        // User should still be logged in
        expect(screen.getByTestId("user")).toHaveTextContent("Jane Doe");
    });

    it("clears interval on unmount", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_session") return null;
            return undefined;
        });

        const clearIntervalSpy = vi.spyOn(globalThis, "clearInterval");

        const { unmount } = renderWithProvider();
        await act(async () => {});

        unmount();

        // Verify clearInterval was called (for the expiry timer)
        expect(clearIntervalSpy).toHaveBeenCalled();

        clearIntervalSpy.mockRestore();
    });
});

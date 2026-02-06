import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { AuthProvider } from "@/contexts/auth-context";
import { useAuth } from "@/hooks/use-auth";
import type { ParsedToken, StoredSession } from "@/types/auth";

const mockParsedToken: ParsedToken = {
    user: {
        id: "123",
        name: "Jane Doe",
        email: "jane@example.com",
        avatarUrl: "https://example.com/avatar.png",
        provider: "github",
    },
    expiresAt: 9999999999,
};

const mockStoredSession: StoredSession = {
    userId: "123",
    userName: "Jane Doe",
    userEmail: "jane@example.com",
    userAvatarUrl: "https://example.com/avatar.png",
    userProvider: "github",
    token: "stored.jwt.token",
    expiresAt: 9999999999,
    agentUrl: "http://localhost:8080",
};

function TestConsumer() {
    const { user, token, expiresAt, agentUrl, isLoading, login } = useAuth();
    return (
        <div>
            <span data-testid='user'>{user ? user.name : "none"}</span>
            <span data-testid='token'>{token ?? "none"}</span>
            <span data-testid='expiresAt'>{expiresAt?.toString() ?? "none"}</span>
            <span data-testid='agentUrl'>{agentUrl ?? "none"}</span>
            <span data-testid='isLoading'>{isLoading ? "true" : "false"}</span>
            <button onClick={() => login("http://localhost:8080")}>Login</button>
        </div>
    );
}

describe("Session Persistence", () => {
    let deepLinkCallback: ((urls: string[]) => void) | null = null;

    beforeEach(() => {
        vi.clearAllMocks();
        deepLinkCallback = null;

        vi.mocked(onOpenUrl).mockImplementation((cb) => {
            deepLinkCallback = cb;
            return Promise.resolve(() => {});
        });
    });

    function renderWithProvider() {
        return render(
            <AuthProvider>
                <TestConsumer />
            </AuthProvider>,
        );
    }

    it("calls save_session after successful deep-link auth", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_session") return null;
            if (cmd === "parse_auth_token") return mockParsedToken;
            if (cmd === "save_session") return undefined;
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        // Click login to set the pending agent URL
        await act(async () => {
            screen.getByText("Login").click();
        });

        // Simulate deep link callback
        await act(async () => {
            deepLinkCallback?.(["devoper://auth/callback?token=fake.jwt.token"]);
        });

        expect(invoke).toHaveBeenCalledWith("save_session", {
            input: {
                userId: "123",
                userName: "Jane Doe",
                userEmail: "jane@example.com",
                userAvatarUrl: "https://example.com/avatar.png",
                userProvider: "github",
                token: "fake.jwt.token",
                expiresAt: 9999999999,
                agentUrl: "http://localhost:8080",
            },
        });
    });

    it("restores session from storage on mount when token is valid", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_session") return mockStoredSession;
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        expect(screen.getByTestId("user")).toHaveTextContent("Jane Doe");
        expect(screen.getByTestId("token")).toHaveTextContent("stored.jwt.token");
        expect(screen.getByTestId("expiresAt")).toHaveTextContent("9999999999");
        expect(screen.getByTestId("agentUrl")).toHaveTextContent("http://localhost:8080");
        expect(screen.getByTestId("isLoading")).toHaveTextContent("false");
    });

    it("does not restore expired session and calls delete_session", async () => {
        const expiredSession: StoredSession = {
            ...mockStoredSession,
            expiresAt: 1000, // long expired
        };

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_session") return expiredSession;
            if (cmd === "delete_session") return undefined;
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        expect(invoke).toHaveBeenCalledWith("delete_session");
        expect(screen.getByTestId("user")).toHaveTextContent("none");
        expect(screen.getByTestId("isLoading")).toHaveTextContent("false");
    });

    it("handles load_session returning null gracefully", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_session") return null;
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        expect(screen.getByTestId("user")).toHaveTextContent("none");
        expect(screen.getByTestId("isLoading")).toHaveTextContent("false");
    });

    it("handles load_session rejection gracefully", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_session") throw new Error("store error");
            return undefined;
        });
        const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

        renderWithProvider();
        await act(async () => {});

        expect(screen.getByTestId("user")).toHaveTextContent("none");
        expect(screen.getByTestId("isLoading")).toHaveTextContent("false");
        expect(consoleSpy).toHaveBeenCalledWith("Failed to restore session:", expect.any(Error));

        consoleSpy.mockRestore();
    });

    it("saves agentUrl from login call with the session", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_session") return null;
            if (cmd === "parse_auth_token") return mockParsedToken;
            if (cmd === "save_session") return undefined;
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        // Login with a specific agent URL
        await act(async () => {
            screen.getByText("Login").click();
        });

        // Deep link callback
        await act(async () => {
            deepLinkCallback?.(["devoper://auth/callback?token=fake.jwt.token"]);
        });

        // Verify agentUrl is set in the context
        expect(screen.getByTestId("agentUrl")).toHaveTextContent("http://localhost:8080");

        // Verify save_session was called with the agentUrl
        expect(invoke).toHaveBeenCalledWith(
            "save_session",
            expect.objectContaining({
                input: expect.objectContaining({
                    agentUrl: "http://localhost:8080",
                }),
            }),
        );
    });
});

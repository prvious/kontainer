import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { AuthProvider } from "@/contexts/auth-context";
import { useAuth } from "@/hooks/use-auth";
import type { ParsedToken } from "@/types/auth";
import type { StoredProfiles } from "@/types/connection";

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

const mockStoredProfiles: StoredProfiles = {
    profiles: [
        {
            id: "profile-1",
            label: "Default",
            agentUrl: "http://localhost:8080",
            userId: "123",
            userName: "Jane Doe",
            userEmail: "jane@example.com",
            userAvatarUrl: "https://example.com/avatar.png",
            userProvider: "github",
            token: "stored.jwt.token",
            expiresAt: 9999999999,
        },
    ],
    activeProfileId: "profile-1",
};

const emptyProfiles: StoredProfiles = {
    profiles: [],
    activeProfileId: null,
};

class MockWebSocket {
    static instances: MockWebSocket[] = [];
    onmessage: ((ev: { data: string }) => void) | null = null;
    onerror: (() => void) | null = null;
    onclose: ((ev: { wasClean: boolean }) => void) | null = null;
    url: string;
    constructor(url: string) {
        this.url = url;
        MockWebSocket.instances.push(this);
    }
    close() {
        this.onclose?.({ wasClean: true });
    }
    serverSend(data: object) {
        this.onmessage?.({ data: JSON.stringify(data) });
    }
    static latest() {
        return this.instances.at(-1)!;
    }
    static clear() {
        this.instances = [];
    }
}

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
    beforeEach(() => {
        vi.clearAllMocks();
        MockWebSocket.clear();
        vi.stubGlobal("WebSocket", MockWebSocket);
    });

    afterEach(() => {
        vi.unstubAllGlobals();
    });

    function renderWithProvider() {
        return render(
            <AuthProvider>
                <TestConsumer />
            </AuthProvider>,
        );
    }

    it("calls save_profile after successful WebSocket auth", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return emptyProfiles;
            if (cmd === "parse_auth_token") return mockParsedToken;
            if (cmd === "save_profile") return undefined;
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        // Click login to open WebSocket
        await act(async () => {
            screen.getByText("Login").click();
        });

        const ws = MockWebSocket.latest();

        // Simulate session ID
        await act(async () => {
            ws.serverSend({ sessionId: "abc" });
        });

        // Simulate token delivery
        await act(async () => {
            ws.serverSend({ token: "fake.jwt.token" });
        });

        expect(invoke).toHaveBeenCalledWith(
            "save_profile",
            expect.objectContaining({
                input: expect.objectContaining({
                    agentUrl: "http://localhost:8080",
                    userId: "123",
                    userName: "Jane Doe",
                    token: "fake.jwt.token",
                    expiresAt: 9999999999,
                }),
            }),
        );
    });

    it("restores session from storage on mount when token is valid", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return mockStoredProfiles;
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

    it("does not restore expired session and calls delete_profile", async () => {
        const expiredProfiles: StoredProfiles = {
            profiles: [
                {
                    ...mockStoredProfiles.profiles[0],
                    expiresAt: 1000, // long expired
                },
            ],
            activeProfileId: "profile-1",
        };

        let deleted = false;
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return deleted ? emptyProfiles : expiredProfiles;
            if (cmd === "delete_profile") {
                deleted = true;
                return undefined;
            }
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        expect(invoke).toHaveBeenCalledWith("delete_profile", { profileId: "profile-1" });
        expect(screen.getByTestId("user")).toHaveTextContent("none");
        expect(screen.getByTestId("isLoading")).toHaveTextContent("false");
    });

    it("handles load_profiles returning empty gracefully", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return emptyProfiles;
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        expect(screen.getByTestId("user")).toHaveTextContent("none");
        expect(screen.getByTestId("isLoading")).toHaveTextContent("false");
    });

    it("handles load_profiles rejection gracefully", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") throw new Error("store error");
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

    it("saves agentUrl from login call with the profile", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return emptyProfiles;
            if (cmd === "parse_auth_token") return mockParsedToken;
            if (cmd === "save_profile") return undefined;
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        // Login with a specific agent URL
        await act(async () => {
            screen.getByText("Login").click();
        });

        const ws = MockWebSocket.latest();

        await act(async () => {
            ws.serverSend({ sessionId: "abc" });
        });

        await act(async () => {
            ws.serverSend({ token: "fake.jwt.token" });
        });

        // Verify agentUrl is set in the context
        expect(screen.getByTestId("agentUrl")).toHaveTextContent("http://localhost:8080");

        // Verify save_profile was called with the agentUrl
        expect(invoke).toHaveBeenCalledWith(
            "save_profile",
            expect.objectContaining({
                input: expect.objectContaining({
                    agentUrl: "http://localhost:8080",
                }),
            }),
        );
    });
});

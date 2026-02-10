import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
    let tokenCallback: ((event: { payload: { token: string } }) => void) | null = null;

    beforeEach(() => {
        vi.clearAllMocks();
        tokenCallback = null;

        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        vi.mocked(listen).mockImplementation(async (event: string, handler: any) => {
            if (event === "auth://token") tokenCallback = handler;
            return () => {};
        });

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return emptyProfiles;
            return undefined;
        });
    });

    function renderWithProvider() {
        return render(
            <AuthProvider>
                <TestConsumer />
            </AuthProvider>,
        );
    }

    it("calls save_profile after successful auth", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return emptyProfiles;
            if (cmd === "parse_auth_token") return mockParsedToken;
            if (cmd === "save_profile") return undefined;
            if (cmd === "auth_listen") {
                tokenCallback?.({ payload: { token: "fake.jwt.token" } });
                return undefined;
            }
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        await act(async () => {
            screen.getByText("Login").click();
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
            if (cmd === "auth_listen") {
                tokenCallback?.({ payload: { token: "fake.jwt.token" } });
                return undefined;
            }
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        await act(async () => {
            screen.getByText("Login").click();
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

import { useState } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
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

const emptyProfiles: StoredProfiles = {
    profiles: [],
    activeProfileId: null,
};

function TestConsumer() {
    const { user, token, expiresAt, agentUrl, isLoading, profiles, activeProfileId, login, logout, switchProfile, removeProfile } = useAuth();
    const [loginError, setLoginError] = useState<string | null>(null);
    return (
        <div>
            <span data-testid='user'>{user ? user.name : "none"}</span>
            <span data-testid='token'>{token ?? "none"}</span>
            <span data-testid='expiresAt'>{expiresAt?.toString() ?? "none"}</span>
            <span data-testid='agentUrl'>{agentUrl ?? "none"}</span>
            <span data-testid='isLoading'>{isLoading ? "true" : "false"}</span>
            <span data-testid='loginError'>{loginError ?? "none"}</span>
            <span data-testid='profileCount'>{profiles.length}</span>
            <span data-testid='activeProfileId'>{activeProfileId ?? "none"}</span>
            <button onClick={() => login("http://localhost:8080").catch((e) => setLoginError(typeof e === "string" ? e : e.message))}>Login</button>
            <button onClick={() => logout()}>Logout</button>
            <button data-testid='switch' onClick={() => switchProfile("profile-2")}>
                Switch
            </button>
            <button data-testid='remove' onClick={() => removeProfile("profile-2")}>
                Remove
            </button>
        </div>
    );
}

describe("AuthProvider", () => {
    let sessionIdCallback: ((event: { payload: { session_id: string } }) => void) | null = null;
    let tokenCallback: ((event: { payload: { token: string } }) => void) | null = null;
    let errorCallback: ((event: { payload: { error: string } }) => void) | null = null;

    beforeEach(() => {
        vi.clearAllMocks();
        sessionIdCallback = null;
        tokenCallback = null;
        errorCallback = null;

        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        vi.mocked(listen).mockImplementation(async (event: string, handler: any) => {
            if (event === "auth://session-id") sessionIdCallback = handler;
            if (event === "auth://token") tokenCallback = handler;
            if (event === "auth://error") errorCallback = handler;
            return () => {};
        });

        // Default: load_profiles returns empty
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

    it("provides null user initially and isLoading becomes false after mount", async () => {
        renderWithProvider();

        await act(async () => {});

        expect(screen.getByTestId("user")).toHaveTextContent("none");
        expect(screen.getByTestId("token")).toHaveTextContent("none");
        expect(screen.getByTestId("expiresAt")).toHaveTextContent("none");
        expect(screen.getByTestId("isLoading")).toHaveTextContent("false");
        expect(screen.getByTestId("profileCount")).toHaveTextContent("0");
    });

    it("login calls auth_listen and opens browser with session ID", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return emptyProfiles;
            if (cmd === "auth_listen") {
                // Simulate: session ID arrives from background WS
                sessionIdCallback?.({ payload: { session_id: "abc123" } });
                // Don't deliver token yet
                return undefined;
            }
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        await act(async () => {
            screen.getByText("Login").click();
        });

        expect(invoke).toHaveBeenCalledWith("auth_listen", { url: "http://localhost:8080" });
        expect(openUrl).toHaveBeenCalledWith("http://localhost:8080/auth/github?session=abc123");

        // Clean up: deliver token so login() resolves
        await act(async () => {
            tokenCallback?.({ payload: { token: "fake.jwt.token" } });
        });
    });

    it("handles token delivery and saves profile", async () => {
        const savedProfile: StoredProfiles = {
            profiles: [
                {
                    id: "test-uuid",
                    label: "Default",
                    agentUrl: "http://localhost:8080",
                    userId: "123",
                    userName: "Jane Doe",
                    userEmail: "jane@example.com",
                    userAvatarUrl: "https://example.com/avatar.png",
                    userProvider: "github",
                    token: "fake.jwt.token",
                    expiresAt: 9999999999,
                },
            ],
            activeProfileId: "test-uuid",
        };

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return savedProfile;
            if (cmd === "parse_auth_token") return mockParsedToken;
            if (cmd === "save_profile") return undefined;
            if (cmd === "auth_listen") {
                // Background task delivers token immediately
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

        expect(invoke).toHaveBeenCalledWith("parse_auth_token", {
            token: "fake.jwt.token",
        });
        expect(invoke).toHaveBeenCalledWith("save_profile", expect.objectContaining({ input: expect.objectContaining({ agentUrl: "http://localhost:8080" }) }));
        expect(screen.getByTestId("user")).toHaveTextContent("Jane Doe");
        expect(screen.getByTestId("token")).toHaveTextContent("fake.jwt.token");
        expect(screen.getByTestId("expiresAt")).toHaveTextContent("9999999999");
    });

    it("logout calls delete_session before clearing state", async () => {
        const callOrder: string[] = [];

        const savedProfile: StoredProfiles = {
            profiles: [
                {
                    id: "test-uuid",
                    label: "Default",
                    agentUrl: "http://localhost:8080",
                    userId: "123",
                    userName: "Jane Doe",
                    userEmail: "jane@example.com",
                    userAvatarUrl: "https://example.com/avatar.png",
                    userProvider: "github",
                    token: "fake.jwt.token",
                    expiresAt: 9999999999,
                },
            ],
            activeProfileId: "test-uuid",
        };

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return savedProfile;
            if (cmd === "parse_auth_token") return mockParsedToken;
            if (cmd === "auth_listen") {
                tokenCallback?.({ payload: { token: "fake.jwt.token" } });
                return undefined;
            }
            if (cmd === "delete_session") {
                callOrder.push("delete_session");
                savedProfile.profiles = [];
                savedProfile.activeProfileId = null;
                return undefined;
            }
            if (cmd === "save_profile") return undefined;
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        // Authenticate first
        await act(async () => {
            screen.getByText("Login").click();
        });

        expect(screen.getByTestId("user")).toHaveTextContent("Jane Doe");

        // Then logout
        await act(async () => {
            screen.getByText("Logout").click();
        });

        expect(callOrder).toContain("delete_session");
        expect(screen.getByTestId("user")).toHaveTextContent("none");
        expect(screen.getByTestId("token")).toHaveTextContent("none");
        expect(screen.getByTestId("expiresAt")).toHaveTextContent("none");
    });

    it("logout clears state even if delete_session fails", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return emptyProfiles;
            if (cmd === "parse_auth_token") return mockParsedToken;
            if (cmd === "auth_listen") {
                tokenCallback?.({ payload: { token: "fake.jwt.token" } });
                return undefined;
            }
            if (cmd === "delete_session") throw new Error("keychain error");
            if (cmd === "save_profile") return undefined;
            return undefined;
        });

        const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

        renderWithProvider();
        await act(async () => {});

        // Authenticate first
        await act(async () => {
            screen.getByText("Login").click();
        });

        expect(screen.getByTestId("user")).toHaveTextContent("Jane Doe");

        // Logout (delete_session will fail)
        await act(async () => {
            screen.getByText("Logout").click();
        });

        // State should still be cleared
        expect(screen.getByTestId("user")).toHaveTextContent("none");
        expect(screen.getByTestId("token")).toHaveTextContent("none");

        consoleSpy.mockRestore();
    });

    it("handles parse_auth_token failure gracefully", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return emptyProfiles;
            if (cmd === "auth_listen") {
                tokenCallback?.({ payload: { token: "bad.token.here" } });
                return undefined;
            }
            if (cmd === "parse_auth_token") throw new Error("parse failed");
            return undefined;
        });
        const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

        renderWithProvider();
        await act(async () => {});

        await act(async () => {
            screen.getByText("Login").click();
        });

        expect(screen.getByTestId("user")).toHaveTextContent("none");
        expect(consoleSpy).toHaveBeenCalledWith("Failed to parse auth token:", expect.any(Error));

        consoleSpy.mockRestore();
    });

    it("rejects login promise on auth_listen error", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return emptyProfiles;
            if (cmd === "auth_listen") {
                // Background task emits error event
                errorCallback?.({ payload: { error: "session expired" } });
                return undefined;
            }
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        await act(async () => {
            screen.getByText("Login").click();
        });

        expect(screen.getByTestId("user")).toHaveTextContent("none");
        expect(screen.getByTestId("loginError")).toHaveTextContent("session expired");
    });

    it("restores active profile on mount", async () => {
        const existingProfiles: StoredProfiles = {
            profiles: [
                {
                    id: "p1",
                    label: "Production",
                    agentUrl: "https://prod.example.com",
                    userId: "u1",
                    userName: "Alice",
                    userEmail: "alice@example.com",
                    userAvatarUrl: "https://example.com/alice.png",
                    userProvider: "github",
                    token: "prod-token",
                    expiresAt: 9999999999,
                },
            ],
            activeProfileId: "p1",
        };

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return existingProfiles;
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        expect(screen.getByTestId("user")).toHaveTextContent("Alice");
        expect(screen.getByTestId("agentUrl")).toHaveTextContent("https://prod.example.com");
        expect(screen.getByTestId("profileCount")).toHaveTextContent("1");
        expect(screen.getByTestId("activeProfileId")).toHaveTextContent("p1");
    });

    it("switchProfile changes the active profile", async () => {
        const twoProfiles: StoredProfiles = {
            profiles: [
                {
                    id: "p1",
                    label: "Production",
                    agentUrl: "https://prod.example.com",
                    userId: "u1",
                    userName: "Alice",
                    userEmail: "alice@example.com",
                    userAvatarUrl: "https://example.com/alice.png",
                    userProvider: "github",
                    token: "prod-token",
                    expiresAt: 9999999999,
                },
                {
                    id: "profile-2",
                    label: "Dev",
                    agentUrl: "https://dev.example.com",
                    userId: "u1",
                    userName: "Alice Dev",
                    userEmail: "alice@dev.com",
                    userAvatarUrl: "https://example.com/alice-dev.png",
                    userProvider: "github",
                    token: "dev-token",
                    expiresAt: 9999999999,
                },
            ],
            activeProfileId: "p1",
        };

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return twoProfiles;
            if (cmd === "set_active_profile") {
                twoProfiles.activeProfileId = "profile-2";
                return undefined;
            }
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        expect(screen.getByTestId("user")).toHaveTextContent("Alice");

        await act(async () => {
            screen.getByTestId("switch").click();
        });

        expect(invoke).toHaveBeenCalledWith("set_active_profile", { profileId: "profile-2" });
        expect(screen.getByTestId("user")).toHaveTextContent("Alice Dev");
        expect(screen.getByTestId("agentUrl")).toHaveTextContent("https://dev.example.com");
        expect(screen.getByTestId("activeProfileId")).toHaveTextContent("profile-2");
    });

    it("removeProfile deletes a non-active profile", async () => {
        const twoProfiles: StoredProfiles = {
            profiles: [
                {
                    id: "p1",
                    label: "Production",
                    agentUrl: "https://prod.example.com",
                    userId: "u1",
                    userName: "Alice",
                    userEmail: "alice@example.com",
                    userAvatarUrl: "https://example.com/alice.png",
                    userProvider: "github",
                    token: "prod-token",
                    expiresAt: 9999999999,
                },
                {
                    id: "profile-2",
                    label: "Dev",
                    agentUrl: "https://dev.example.com",
                    userId: "u1",
                    userName: "Alice Dev",
                    userEmail: "alice@dev.com",
                    userAvatarUrl: "https://example.com/alice-dev.png",
                    userProvider: "github",
                    token: "dev-token",
                    expiresAt: 9999999999,
                },
            ],
            activeProfileId: "p1",
        };

        const afterRemove: StoredProfiles = {
            profiles: [twoProfiles.profiles[0]],
            activeProfileId: "p1",
        };

        let removed = false;

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return removed ? afterRemove : twoProfiles;
            if (cmd === "delete_profile") {
                removed = true;
                return undefined;
            }
            return undefined;
        });

        renderWithProvider();
        await act(async () => {});

        expect(screen.getByTestId("profileCount")).toHaveTextContent("2");

        await act(async () => {
            screen.getByTestId("remove").click();
        });

        expect(invoke).toHaveBeenCalledWith("delete_profile", { profileId: "profile-2" });
        expect(screen.getByTestId("profileCount")).toHaveTextContent("1");
        // Active profile unchanged
        expect(screen.getByTestId("user")).toHaveTextContent("Alice");
    });
});

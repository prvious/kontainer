import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { AuthProvider } from "@/contexts/auth-context";
import { useAuth } from "@/hooks/use-auth";
import type { SessionView } from "@/types/session";

const session: SessionView = {
    profileId: "9e91e40b-09d0-44b8-9e1b-329455b25617",
    origin: "https://controller.example/",
    clusterId: "cluster-1",
    principalId: "0a749bf5-21a9-43ba-81b4-7065cddbb30f",
    githubLogin: "jane",
    label: "Production",
    accessExpiresAtUnixMs: 2_000_000_000_000,
};

let activeQueryClient: QueryClient;

function CaptureQueryClient() {
    const queryClient = useQueryClient();
    useEffect(() => {
        activeQueryClient = queryClient;
    }, [queryClient]);
    return null;
}

function TestConsumer() {
    const auth = useAuth();
    return (
        <div>
            <span data-testid='user'>{auth.session?.githubLogin ?? "none"}</span>
            <span data-testid='session'>{auth.session?.profileId ?? "none"}</span>
            <span data-testid='credential-keys'>
                {Object.keys(auth)
                    .filter((key) => /token|credential|secret/i.test(key))
                    .join(",") || "none"}
            </span>
            <span data-testid='origin'>{auth.session?.origin ?? "none"}</span>
            <span data-testid='issue'>{auth.issue ?? "none"}</span>
            <span data-testid='loading'>{String(auth.isLoading)}</span>
            <button
                onClick={() => {
                    void auth.login("https://controller.example", "Production").catch(() => {});
                }}
            >
                Login
            </button>
            <button
                onClick={() => {
                    void auth.logout().catch(() => {});
                }}
            >
                Logout
            </button>
            <button
                onClick={() => {
                    void auth.retryCommit().catch(() => {});
                }}
            >
                Retry commit
            </button>
            <button
                onClick={() => {
                    void auth.retryRestore().catch(() => {});
                }}
            >
                Retry restore
            </button>
            <button
                onClick={() => {
                    void auth.cancelLogin().catch(() => {});
                }}
            >
                Cancel login
            </button>
            <button onClick={() => auth.reportInventoryCustodyError?.(session.profileId, "relogin_required")}>Report inventory relogin</button>
            <button onClick={() => auth.reportInventoryCustodyError?.("old-profile", "relogin_required")}>Report stale inventory error</button>
        </div>
    );
}

function renderAuth() {
    return render(
        <AuthProvider>
            <CaptureQueryClient />
            <TestConsumer />
        </AuthProvider>,
    );
}

async function restored() {
    await waitFor(() => expect(screen.getByTestId("loading")).toHaveTextContent("false"));
}

describe("AuthProvider custody", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.mocked(invoke).mockImplementation(async (command: string) => (command === "session_restore" ? null : undefined));
    });

    it("starts without a profile and never holds a React credential", async () => {
        renderAuth();
        await restored();
        expect(screen.getByTestId("user")).toHaveTextContent("none");
        expect(screen.getByTestId("session")).toHaveTextContent("none");
        expect(screen.getByTestId("credential-keys")).toHaveTextContent("none");
        expect(invoke).toHaveBeenCalledWith("session_restore", undefined);
    });

    it("logs in only through the Rust custody command", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => (command === "session_login" ? session : null));
        renderAuth();
        await restored();
        await act(async () => {
            screen.getByText("Login").click();
        });
        await waitFor(() => expect(screen.getByTestId("session")).toHaveTextContent(session.profileId));
        expect(invoke).toHaveBeenCalledWith("session_login", { origin: "https://controller.example", label: "Production" });
        expect(vi.mocked(invoke).mock.calls.every(([command]) => !["auth_listen", "load_profiles", "save_profile"].includes(command))).toBe(true);
        expect(screen.getByTestId("credential-keys")).toHaveTextContent("none");
    });

    it("exposes the restored non-secret SessionView and nothing credential-shaped", async () => {
        vi.mocked(invoke).mockResolvedValue(session);
        renderAuth();
        await restored();
        expect(screen.getByTestId("user")).toHaveTextContent("jane");
        expect(screen.getByTestId("session")).toHaveTextContent(session.profileId);
        expect(screen.getByTestId("origin")).toHaveTextContent(session.origin);
        expect(screen.getByTestId("credential-keys")).toHaveTextContent("none");
    });

    it("shows vault_write_failed as a distinct login state", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_login") throw "vault_write_failed";
            return null;
        });
        renderAuth();
        await restored();
        await act(async () => {
            screen.getByText("Login").click();
        });
        await waitFor(() => expect(screen.getByTestId("issue")).toHaveTextContent("vault_write_failed"));
        expect(screen.getByTestId("session")).toHaveTextContent("none");
    });

    it("retries a held commit without another login", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_login") throw "vault_write_failed";
            if (command === "session_retry_commit") return session;
            return null;
        });
        renderAuth();
        await restored();
        await act(async () => {
            screen.getByText("Login").click();
        });
        await waitFor(() => expect(screen.getByTestId("issue")).toHaveTextContent("vault_write_failed"));
        await act(async () => {
            screen.getByText("Retry commit").click();
        });
        await waitFor(() => expect(screen.getByTestId("session")).toHaveTextContent(session.profileId));
        expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "session_login")).toHaveLength(1);
    });

    it("logs out through Rust before clearing the session", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => (command === "session_restore" ? session : undefined));
        renderAuth();
        await restored();
        await act(async () => {
            screen.getByText("Logout").click();
        });
        await waitFor(() => expect(screen.getByTestId("session")).toHaveTextContent("none"));
        expect(invoke).toHaveBeenCalledWith("session_logout", undefined);
    });

    it("keeps the held session visible when revocation fails", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_restore") return session;
            if (command === "session_logout") throw "revoke_failed";
            return undefined;
        });
        renderAuth();
        await restored();
        await act(async () => {
            screen.getByText("Logout").click();
        });
        await waitFor(() => expect(screen.getByTestId("issue")).toHaveTextContent("revoke_failed"));
        expect(screen.getByTestId("session")).toHaveTextContent(session.profileId);
    });

    it("clears authenticated state when local cleanup remains", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_restore") return session;
            if (command === "session_logout") throw "local_cleanup_failed";
            return undefined;
        });
        renderAuth();
        await restored();
        await act(async () => {
            screen.getByText("Logout").click();
        });
        await waitFor(() => expect(screen.getByTestId("issue")).toHaveTextContent("local_cleanup_failed"));
        expect(screen.getByTestId("session")).toHaveTextContent("none");
    });

    it("renders revoked restore as re-login-required", async () => {
        vi.mocked(invoke).mockRejectedValue("relogin_required");
        renderAuth();
        await restored();
        expect(screen.getByTestId("issue")).toHaveTextContent("relogin_required");
        expect(screen.getByTestId("session")).toHaveTextContent("none");
    });

    it("clears inventory cache and the visible session after a current-profile custody failure", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => (command === "session_restore" ? session : null));
        renderAuth();
        await restored();
        activeQueryClient.setQueryData(["container-inventory", session.profileId], "cached canary");
        await act(async () => screen.getByText("Report inventory relogin").click());
        expect(screen.getByTestId("session")).toHaveTextContent("none");
        expect(screen.getByTestId("issue")).toHaveTextContent("relogin_required");
        expect(activeQueryClient.getQueryData(["container-inventory", session.profileId])).toBeUndefined();
    });

    it("ignores a late custody error from a replaced profile", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => (command === "session_restore" ? session : null));
        renderAuth();
        await restored();
        activeQueryClient.setQueryData(["container-inventory", session.profileId], "cached canary");
        await act(async () => screen.getByText("Report stale inventory error").click());
        expect(screen.getByTestId("session")).toHaveTextContent(session.profileId);
        expect(screen.getByTestId("issue")).toHaveTextContent("none");
        expect(activeQueryClient.getQueryData(["container-inventory", session.profileId])).toBe("cached canary");
    });

    it("maps unknown native errors to a closed code without showing raw text", async () => {
        vi.mocked(invoke).mockRejectedValue("secret native error canary");
        renderAuth();
        await restored();
        expect(screen.getByTestId("issue")).toHaveTextContent("invalid_response");
        expect(document.body.textContent).not.toContain("secret native error canary");
    });

    it("calls the Rust cancellation command", async () => {
        renderAuth();
        await restored();
        await act(async () => {
            screen.getByText("Cancel login").click();
        });
        expect(invoke).toHaveBeenCalledWith("session_cancel_login", undefined);
    });

    it("logs in, restores after a React restart, and logs out through custody", async () => {
        let nativeSession: SessionView | null = null;
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_restore") return nativeSession;
            if (command === "session_login") return (nativeSession = session);
            if (command === "session_logout") nativeSession = null;
            return undefined;
        });

        const firstMount = renderAuth();
        await restored();
        await act(async () => {
            screen.getByText("Login").click();
        });
        await waitFor(() => expect(screen.getByTestId("session")).toHaveTextContent(session.profileId));
        firstMount.unmount();

        renderAuth();
        await restored();
        expect(screen.getByTestId("session")).toHaveTextContent(session.profileId);
        await act(async () => {
            screen.getByText("Logout").click();
        });
        await waitFor(() => expect(screen.getByTestId("session")).toHaveTextContent("none"));
        expect(
            vi
                .mocked(invoke)
                .mock.calls.map(([command]) => command)
                .filter((command) => command.startsWith("session_")),
        ).toEqual(["session_restore", "session_login", "session_restore", "session_logout"]);
    });
});

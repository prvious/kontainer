import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { onlineManager, useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { AuthProvider } from "@/contexts/auth-context";
import { useControllerHealth } from "@/hooks/session/use-controller-health";
import { useAuth } from "@/hooks/use-auth";
import type { SessionView } from "@/types/session";

/** The latest result each sign-in hook returned to the provider, to inspect what its observer still holds. */
const sessionCommands = vi.hoisted(() => ({ login: null as { variables: unknown } | null, register: null as { variables: unknown } | null }));

vi.mock("@/hooks/session/use-session-login", async (importOriginal) => {
    const actual = await importOriginal<typeof import("@/hooks/session/use-session-login")>();
    return { ...actual, useSessionLogin: () => (sessionCommands.login = actual.useSessionLogin()) };
});

vi.mock("@/hooks/session/use-session-register", async (importOriginal) => {
    const actual = await importOriginal<typeof import("@/hooks/session/use-session-register")>();
    return { ...actual, useSessionRegister: () => (sessionCommands.register = actual.useSessionRegister()) };
});

const session: SessionView = {
    profileId: "9e91e40b-09d0-44b8-9e1b-329455b25617",
    origin: "https://controller.example/",
    clusterId: "cluster-1",
    principalId: "0a749bf5-21a9-43ba-81b4-7065cddbb30f",
    email: "jane@example.com",
    displayName: "jane",
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
            <span data-testid='user'>{auth.session?.displayName ?? "none"}</span>
            <span data-testid='session'>{auth.session?.profileId ?? "none"}</span>
            <span data-testid='credential-keys'>
                {Object.keys(auth)
                    .filter((key) => /token|credential|secret/i.test(key))
                    .join(",") || "none"}
            </span>
            <span data-testid='origin'>{auth.session?.origin ?? "none"}</span>
            <span data-testid='issue'>{auth.issue ?? "none"}</span>
            <span data-testid='loading'>{String(auth.isLoading)}</span>
            <span data-testid='restore-error'>{auth.restoreError ?? "none"}</span>
            <span data-testid='expired'>{String(auth.expired)}</span>
            <span data-testid='next-origin'>{auth.nextOrigin ?? "none"}</span>
            <button
                onClick={() => {
                    void auth
                        .login({ origin: "https://controller.example", email: "jane@example.com", password: "correct horse battery", label: "Production" })
                        .catch(() => {});
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
                    void auth.logout(undefined, "https://agent-eu.example.com").catch(() => {});
                }}
            >
                Switch agent
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
            <button onClick={() => auth.reportInventoryCustodyError?.(session.profileId, "relogin_required")}>Report inventory relogin</button>
            <button onClick={() => auth.reportInventoryCustodyError?.(session.profileId, "vault_unavailable")}>Report vault failure</button>
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
        expect(invoke).toHaveBeenCalledWith("session_login", {
            origin: "https://controller.example",
            email: "jane@example.com",
            password: "correct horse battery",
            label: "Production",
        });
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
        expect(invoke).toHaveBeenCalledWith("session_logout", { summary: null });
    });

    it("remembers the agent a switching sign-out is headed for, and forgets it on a plain one", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => (command === "session_restore" ? session : undefined));
        renderAuth();
        await restored();
        await act(async () => {
            screen.getByText("Switch agent").click();
        });
        await waitFor(() => expect(screen.getByTestId("session")).toHaveTextContent("none"));
        expect(screen.getByTestId("next-origin")).toHaveTextContent("https://agent-eu.example.com");
        await act(async () => {
            screen.getByText("Logout").click();
        });
        await waitFor(() => expect(screen.getByTestId("next-origin")).toHaveTextContent("none"));
    });

    it("forgets a switch's agent once a session is published again", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => (command === "session_restore" || command === "session_login" ? session : undefined));
        renderAuth();
        await restored();
        await act(async () => screen.getByText("Switch agent").click());
        await waitFor(() => expect(screen.getByTestId("next-origin")).toHaveTextContent("https://agent-eu.example.com"));
        await act(async () => screen.getByText("Login").click());
        await waitFor(() => expect(screen.getByTestId("session")).toHaveTextContent(session.profileId));
        expect(screen.getByTestId("next-origin")).toHaveTextContent("none");
    });

    it("keeps probing and sends a change while the OS reports offline, since Rust does the networking", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => (command === "controller_health" ? { status: "ready" } : null));
        function Offline() {
            const health = useControllerHealth();
            const change = useMutation({ mutationFn: () => invoke("send_change") });
            return (
                <>
                    <span data-testid='probe'>{health.data?.status ?? "none"}</span>
                    <span data-testid='change'>{change.isPaused ? "parked" : change.status}</span>
                    <button onClick={() => change.mutate()}>Change</button>
                </>
            );
        }
        onlineManager.setOnline(false);
        try {
            render(
                <AuthProvider>
                    <Offline />
                </AuthProvider>,
            );
            await waitFor(() => expect(screen.getByTestId("probe")).toHaveTextContent("ready"));
            await act(async () => screen.getByText("Change").click());
            await waitFor(() => expect(screen.getByTestId("change")).toHaveTextContent("success"));
            expect(invoke).toHaveBeenCalledWith("send_change");
        } finally {
            onlineManager.setOnline(true);
        }
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

    it("keeps the signed-in app mounted while a restore is retried, even when the retry fails", async () => {
        let restores = 0;
        let fail: (error: unknown) => void = () => {};
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command !== "session_restore") return null;
            restores += 1;
            return restores === 1 ? session : new Promise((_, reject) => (fail = reject));
        });
        renderAuth();
        await restored();
        expect(screen.getByTestId("session")).toHaveTextContent(session.profileId);

        await act(async () => screen.getByText("Retry restore").click());
        await waitFor(() => expect(restores).toBe(2));
        // `isLoading` and `restoreError` are what swap the router for the splash (app.tsx).
        expect(screen.getByTestId("loading")).toHaveTextContent("false");
        await act(async () => fail("vault_unavailable"));
        expect(screen.getByTestId("loading")).toHaveTextContent("false");
        expect(screen.getByTestId("restore-error")).toHaveTextContent("none");
        expect(screen.getByTestId("session")).toHaveTextContent(session.profileId);
    });

    it("still shows progress, then the failure, while a failed boot restore is retried", async () => {
        let restores = 0;
        let fail: (error: unknown) => void = () => {};
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command !== "session_restore") return null;
            restores += 1;
            if (restores === 1) throw "vault_unavailable";
            return new Promise((_, reject) => (fail = reject));
        });
        renderAuth();
        await waitFor(() => expect(screen.getByTestId("restore-error")).toHaveTextContent("vault_unavailable"));

        await act(async () => screen.getByText("Retry restore").click());
        await waitFor(() => expect(screen.getByTestId("loading")).toHaveTextContent("true"));
        await act(async () => fail("vault_unavailable"));
        await restored();
        expect(screen.getByTestId("restore-error")).toHaveTextContent("vault_unavailable");
    });

    it("renders revoked restore as re-login-required", async () => {
        vi.mocked(invoke).mockRejectedValue("relogin_required");
        renderAuth();
        await restored();
        expect(screen.getByTestId("issue")).toHaveTextContent("relogin_required");
        expect(screen.getByTestId("session")).toHaveTextContent("none");
    });

    it("marks a rejected refresh as expired while keeping the session and the page's data", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => (command === "session_restore" ? session : null));
        renderAuth();
        await restored();
        activeQueryClient.setQueryData(["container-inventory", session.profileId], "cached canary");
        await act(async () => screen.getByText("Report inventory relogin").click());
        expect(screen.getByTestId("expired")).toHaveTextContent("true");
        expect(screen.getByTestId("session")).toHaveTextContent(session.profileId);
        expect(activeQueryClient.getQueryData(["container-inventory", session.profileId])).toBe("cached canary");
    });

    it("signing back in to the same profile clears the expiry and keeps the page's data", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => (command === "session_restore" || command === "session_login" ? session : null));
        renderAuth();
        await restored();
        activeQueryClient.setQueryData(["container-inventory", session.profileId], "cached canary");
        await act(async () => screen.getByText("Report inventory relogin").click());
        await act(async () => screen.getByText("Login").click());
        await waitFor(() => expect(screen.getByTestId("expired")).toHaveTextContent("false"));
        expect(activeQueryClient.getQueryData(["container-inventory", session.profileId])).toBe("cached canary");
    });

    it("clears the cache and the visible session after a current-profile custody failure", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => (command === "session_restore" ? session : null));
        renderAuth();
        await restored();
        activeQueryClient.setQueryData(["container-inventory", session.profileId], "cached canary");
        await act(async () => screen.getByText("Report vault failure").click());
        expect(screen.getByTestId("session")).toHaveTextContent("none");
        expect(screen.getByTestId("issue")).toHaveTextContent("vault_unavailable");
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

    it("rejects a wrong password with the failure details and no session", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_login") throw { code: "invalid_credentials", attemptsRemaining: 7, requestId: "req-1" };
            return null;
        });
        let failure: unknown;
        function Probe() {
            const auth = useAuth();
            return (
                <button
                    onClick={() => {
                        auth.login({ origin: "https://controller.example", email: "jane@example.com", password: "wrong horse battery" }).catch(
                            (error: unknown) => {
                                failure = error;
                            },
                        );
                    }}
                >
                    Wrong
                </button>
            );
        }
        render(
            <AuthProvider>
                <Probe />
                <TestConsumer />
            </AuthProvider>,
        );
        await restored();
        await act(async () => screen.getByText("Wrong").click());
        await waitFor(() => expect(failure).toEqual({ code: "invalid_credentials", attemptsRemaining: 7, requestId: "req-1" }));
        expect(screen.getByTestId("issue")).toHaveTextContent("none");
        expect(screen.getByTestId("session")).toHaveTextContent("none");
    });

    it("drops the password from mutation state after a sign-in fails and after it succeeds", async () => {
        let accept = false;
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command !== "session_login") return null;
            if (!accept) throw { code: "invalid_credentials", attemptsRemaining: 7 };
            return session;
        });
        renderAuth();
        await restored();

        for (const outcome of ["failure", "success"]) {
            accept = outcome === "success";
            vi.mocked(invoke).mockClear();
            await act(async () => screen.getByText("Login").click());
            await waitFor(() => expect(invoke).toHaveBeenCalledWith("session_login", expect.objectContaining({ password: "correct horse battery" })));
            await waitFor(() => expect(activeQueryClient.getMutationCache().getAll()).toEqual([]));
            expect(sessionCommands.login?.variables, outcome).toBeUndefined();
        }
        expect(screen.getByTestId("session")).toHaveTextContent(session.profileId);
    });

    it("drops the password and setup code from mutation state after a registration fails", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_register") throw { code: "validation_failed", fields: { setupCode: "invalid" } };
            return null;
        });
        let failure: unknown;
        function Register() {
            const auth = useAuth();
            return (
                <button
                    onClick={() => {
                        auth.register({
                            origin: "https://controller.example",
                            displayName: "Jane",
                            email: "jane@example.com",
                            password: "correct horse battery",
                            setupCode: "setup-code-canary",
                        }).catch((error: unknown) => {
                            failure = error;
                        });
                    }}
                >
                    Register
                </button>
            );
        }
        render(
            <AuthProvider>
                <CaptureQueryClient />
                <Register />
                <TestConsumer />
            </AuthProvider>,
        );
        await restored();
        await act(async () => screen.getByText("Register").click());
        await waitFor(() => expect(failure).toEqual({ code: "validation_failed", fields: { setupCode: "invalid" } }));
        expect(invoke).toHaveBeenCalledWith("session_register", expect.objectContaining({ setupCode: "setup-code-canary" }));
        await waitFor(() => expect(activeQueryClient.getMutationCache().getAll()).toEqual([]));
        expect(sessionCommands.register?.variables).toBeUndefined();
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

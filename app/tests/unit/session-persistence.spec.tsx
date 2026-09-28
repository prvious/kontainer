import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { useQueryClient } from "@tanstack/react-query";
import { AuthProvider } from "@/contexts/auth-context";
import { useAuth } from "@/hooks/use-auth";
import type { SessionView } from "@/types/session";

const session: SessionView = {
    profileId: "local-profile-id",
    origin: "https://controller.example/",
    clusterId: "cluster-id",
    principalId: "principal-id",
    email: "jane@example.com",
    displayName: "jane",
    label: "Production",
    accessExpiresAtUnixMs: 2_000_000_000_000,
};

function TestConsumer() {
    const { session: active, issue, isLoading, login } = useAuth();
    const queryClient = useQueryClient();
    return (
        <div>
            <span data-testid='profile'>{active?.profileId ?? "none"}</span>
            <span data-testid='origin'>{active?.origin ?? "none"}</span>
            <span data-testid='issue'>{issue ?? "none"}</span>
            <span data-testid='loading'>{String(isLoading)}</span>
            <span data-testid='query-cache'>
                {JSON.stringify(
                    queryClient
                        .getQueryCache()
                        .getAll()
                        .map((query) => ({ data: query.state.data, error: query.state.error ? String(query.state.error) : null })),
                )}
            </span>
            <button
                onClick={() => {
                    void login({ origin: "https://controller.example/", email: "jane@example.com", password: "correct horse battery" }).catch(() => {});
                }}
            >
                Login
            </button>
        </div>
    );
}

function renderWithProvider() {
    return render(
        <AuthProvider>
            <TestConsumer />
        </AuthProvider>,
    );
}

describe("Session Persistence", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_restore") return null;
            if (command === "session_login") return session;
            return null;
        });
    });

    it("restores through Rust custody without plaintext profile commands", async () => {
        renderWithProvider();
        await waitFor(() => expect(screen.getByTestId("loading")).toHaveTextContent("false"));
        expect(invoke).toHaveBeenCalledWith("session_restore", undefined);
        expect(vi.mocked(invoke).mock.calls.some(([command]) => ["load_profiles", "load_session", "save_profile"].includes(command))).toBe(false);
    });

    it("uses the restored non-secret session view", async () => {
        vi.mocked(invoke).mockResolvedValue({ ...session, accessToken: "credential-canary" });
        renderWithProvider();
        await waitFor(() => expect(screen.getByTestId("profile")).toHaveTextContent("local-profile-id"));
        expect(screen.getByTestId("origin")).toHaveTextContent("https://controller.example/");
        expect(screen.getByTestId("query-cache")).not.toHaveTextContent("credential-canary");
    });

    it("shows no session when Rust invalidates a stale local profile", async () => {
        renderWithProvider();
        await waitFor(() => expect(screen.getByTestId("loading")).toHaveTextContent("false"));
        expect(screen.getByTestId("profile")).toHaveTextContent("none");
        expect(invoke).not.toHaveBeenCalledWith("delete_profile", expect.anything());
    });

    it("maps restore failure to a closed code and keeps raw errors out of the query cache", async () => {
        vi.mocked(invoke).mockRejectedValue("native-secret-canary");
        renderWithProvider();
        await waitFor(() => expect(screen.getByTestId("issue")).toHaveTextContent("invalid_response"));
        expect(screen.getByTestId("query-cache").textContent).not.toContain("native-secret-canary");
    });

    it("sends the origin, email, password, and label on login", async () => {
        renderWithProvider();
        await userEvent.setup().click(screen.getByRole("button", { name: "Login" }));
        await waitFor(() =>
            expect(invoke).toHaveBeenCalledWith("session_login", {
                origin: "https://controller.example/",
                email: "jane@example.com",
                password: "correct horse battery",
                label: null,
            }),
        );
    });

    it("publishes the native local profile UUID after login", async () => {
        renderWithProvider();
        await userEvent.setup().click(screen.getByRole("button", { name: "Login" }));
        await waitFor(() => expect(screen.getByTestId("profile")).toHaveTextContent("local-profile-id"));
    });
});

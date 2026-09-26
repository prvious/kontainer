import { useState } from "react";
import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { createMemoryHistory, createRouter, RouterProvider } from "@tanstack/react-router";
import { routeTree } from "@/routeTree.gen";
import { AuthProvider } from "@/contexts/auth-context";
import { useAuth } from "@/hooks/use-auth";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import type { SessionView } from "@/types/session";

const session: SessionView = {
    profileId: "local-profile-id",
    origin: "https://controller.example/",
    clusterId: "cluster-id",
    principalId: "principal-id",
    githubLogin: "jane",
    label: "Production",
    accessExpiresAtUnixMs: 2_000_000_000_000,
};

function TestApp({ initialPath = "/" }: { initialPath?: string }) {
    const auth = useAuth();
    if (auth.isLoading) return null;
    return <TestRouter auth={auth} initialPath={initialPath} />;
}

function TestRouter({ auth, initialPath }: { auth: AuthContextValue; initialPath: string }) {
    const [router] = useState(() => createRouter({ routeTree, context: { auth }, history: createMemoryHistory({ initialEntries: [initialPath] }) }));
    return <RouterProvider router={router} context={{ auth }} />;
}

describe("authenticated custody shell", () => {
    it("shows the non-secret session without mounting direct-agent screens", async () => {
        vi.clearAllMocks();
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_restore") return session;
            if (command === "load_app_settings") return { refreshInterval: 5000 };
            return null;
        });
        render(
            <AuthProvider>
                <TestApp />
            </AuthProvider>,
        );
        await waitFor(() => expect(screen.getByText("Connected")).toBeInTheDocument());
        expect(screen.getByText(/Signed in as jane to https:\/\/controller.example\//)).toBeInTheDocument();
        expect(screen.getByText("Cluster resources are temporarily unavailable.")).toBeInTheDocument();
        expect(
            vi
                .mocked(invoke)
                .mock.calls.map(([command]) => command)
                .every((command) => ["session_restore", "load_app_settings"].includes(command)),
        ).toBe(true);
    });

    it("keeps local settings available without mounting resource screens", async () => {
        vi.clearAllMocks();
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_restore") return session;
            if (command === "load_app_settings") return { refreshInterval: 5000 };
            return null;
        });
        render(
            <AuthProvider>
                <TestApp />
            </AuthProvider>,
        );
        await userEvent.setup().click(await screen.findByRole("link", { name: "Settings" }));
        await waitFor(() => expect(screen.getByRole("heading", { name: "Settings" })).toBeInTheDocument());
        expect(screen.getByText(session.origin)).toBeInTheDocument();
        expect(
            vi
                .mocked(invoke)
                .mock.calls.map(([command]) => command)
                .every((command) => ["session_restore", "load_app_settings"].includes(command)),
        ).toBe(true);
    });

    it("hides an unported resource route reached by a deep link", async () => {
        vi.clearAllMocks();
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_restore") return session;
            if (command === "load_app_settings") return { refreshInterval: 5000 };
            return null;
        });
        render(
            <AuthProvider>
                <TestApp initialPath='/tasks' />
            </AuthProvider>,
        );
        await waitFor(() => expect(screen.getByText("Cluster resources are temporarily unavailable.")).toBeInTheDocument());
        expect(screen.queryByRole("heading", { name: /containers/i })).not.toBeInTheDocument();
        expect(
            vi
                .mocked(invoke)
                .mock.calls.map(([command]) => command)
                .every((command) => ["session_restore", "load_app_settings"].includes(command)),
        ).toBe(true);
    });

    it("signs out through Rust custody and returns to login", async () => {
        vi.clearAllMocks();
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_restore") return session;
            if (command === "load_app_settings") return { refreshInterval: 5000 };
            return null;
        });
        render(
            <AuthProvider>
                <TestApp />
            </AuthProvider>,
        );
        await userEvent.setup().click(await screen.findByRole("button", { name: /Production/ }));
        await userEvent.setup().click(screen.getByRole("button", { name: "Sign out" }));
        await waitFor(() => expect(screen.getByRole("button", { name: "Connect with GitHub" })).toBeInTheDocument());
        expect(invoke).toHaveBeenCalledWith("session_logout", undefined);
    });

    it("opens the custody login from a fresh start", async () => {
        vi.clearAllMocks();
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_login") return session;
            if (command === "load_app_settings") return { refreshInterval: 5000 };
            return null;
        });
        render(
            <AuthProvider>
                <TestApp initialPath='/login' />
            </AuthProvider>,
        );
        await userEvent.setup().click(await screen.findByRole("button", { name: "Connect with GitHub" }));
        await waitFor(() => expect(screen.getByText("Connected")).toBeInTheDocument());
        expect(invoke).toHaveBeenCalledWith("session_login", { origin: "http://127.0.0.1:5150", label: null });
    });
});

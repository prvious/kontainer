import { useState } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { createMemoryHistory, createRouter, RouterProvider } from "@tanstack/react-router";
import { routeTree } from "@/routeTree.gen";
import { AuthProvider } from "@/contexts/auth-context";
import { useAuth } from "@/hooks/use-auth";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import type { SessionView } from "@/types/session";
import nodesFixture from "../fixtures/nodes-ipc.json";

const session: SessionView = {
    profileId: "local-profile-id",
    origin: "https://controller.example/",
    clusterId: "cluster-id",
    principalId: "principal-id",
    email: "jane@example.com",
    displayName: "Jane",
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

type Handler = (command: string, args?: unknown) => unknown;

function mockCommands(overrides: Record<string, Handler> = {}, restored: SessionView | null = session) {
    vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
        if (command in overrides) return overrides[command](command, args);
        switch (command) {
            case "session_restore":
                return restored;
            case "load_app_settings":
                return { refreshInterval: 5000 };
            case "session_connections":
                return [];
            case "session_bootstrap":
                return { ownerExists: true };
            case "controller_health":
                return { status: "ready" };
            case "list_nodes":
                return nodesFixture.nodeList;
            case "get_cluster_summary":
                return nodesFixture.clusterSummary;
            default:
                return null;
        }
    });
}

function renderApp(initialPath = "/") {
    render(
        <AuthProvider>
            <TestApp initialPath={initialPath} />
        </AuthProvider>,
    );
}

describe("authenticated shell", () => {
    beforeEach(() => vi.clearAllMocks());

    it("lands on the Cluster Overview and renders other routes in the outlet", async () => {
        mockCommands();
        renderApp();
        await waitFor(() => expect(screen.getByRole("heading", { name: "CLUSTER OVERVIEW", level: 1 })).toBeInTheDocument());
        expect(screen.getByRole("link", { name: "Overview" })).toHaveAttribute("data-status", "active");
        await userEvent.setup().click(screen.getByRole("link", { name: /Containers/ }));
        await waitFor(() => expect(screen.getByRole("heading", { name: "CONTAINERS", level: 1 })).toBeInTheDocument());
    });

    it("keeps local settings available from the shell", async () => {
        mockCommands();
        renderApp();
        await userEvent.setup().click(await screen.findByRole("link", { name: "Settings" }));
        await waitFor(() => expect(screen.getByRole("heading", { name: "SETTINGS" })).toBeInTheDocument());
        expect(await screen.findByDisplayValue(session.origin)).toBeInTheDocument();
        expect(screen.getByDisplayValue(`${session.displayName} · ${session.email}`)).toBeInTheDocument();
    });

    it("fills the status bar CPU and MEM and the sidebar DOWN badge from the cluster summary", async () => {
        mockCommands();
        renderApp();
        // One online node sampled: 12.5 % CPU, 1 of 8 GB; one node degraded and one down. Only the
        // down one counts toward the badge: degraded includes a node that merely has no agent.
        await waitFor(() => expect(screen.getByText("CPU 13%")).toBeInTheDocument());
        expect(screen.getByText("MEM 1/8G")).toBeInTheDocument();
        await waitFor(() => expect(screen.getByText("Docker 29.8.0")).toBeInTheDocument());
        expect(screen.getByTestId("nav-badge-nodes")).toHaveTextContent("1 DOWN");
        expect(invoke).toHaveBeenCalledWith("get_cluster_summary", { profileId: session.profileId });
        expect(invoke).toHaveBeenCalledWith("list_nodes", { profileId: session.profileId });
    });

    it("turns the titlebar dot red on a failed probe and the status bar to RECONNECTING after three", async () => {
        mockCommands({ controller_health: () => ({ status: "unreachable", diagnostic: "tcp" }) });
        renderApp();
        await waitFor(() => expect(screen.getByTestId("controller-health-dot")).toHaveAttribute("data-tone", "destructive"));
        expect(screen.queryByText("RECONNECTING")).not.toBeInTheDocument();
        // Two retries, 1 s apart, before the probe query errors.
        await waitFor(() => expect(screen.getByText("RECONNECTING")).toBeInTheDocument(), { timeout: 4000 });
        expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "controller_health")).toHaveLength(3);
    });

    it("keeps the page mounted under the session-expired dialog when a refresh is rejected", async () => {
        mockCommands({ list_containers: () => Promise.reject("relogin_required") });
        renderApp("/containers");
        const dialog = await screen.findByRole("dialog");
        expect(within(dialog).getByText("Your session ended")).toBeInTheDocument();
        expect(within(dialog).getByText(session.email)).toBeInTheDocument();
        // The page is still there, behind the scrim.
        expect(screen.getByRole("heading", { name: "CONTAINERS", level: 1, hidden: true })).toBeInTheDocument();
        expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "list_containers")).toHaveLength(1);
    });

    it("signs back in from the expired dialog and resumes the same page", async () => {
        let rejected = false;
        mockCommands({
            list_containers: () => {
                if (!rejected) {
                    rejected = true;
                    return Promise.reject("relogin_required");
                }
                return { items: [], nodes: [], snapshotAt: "2026-09-28T12:00:00.000Z" };
            },
            session_login: () => session,
        });
        renderApp("/containers");
        const user = userEvent.setup();
        const dialog = await screen.findByRole("dialog");
        await user.type(within(dialog).getByLabelText("Password"), "correct horse battery");
        await user.click(within(dialog).getByRole("button", { name: /SIGN IN/ }));
        await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
        expect(invoke).toHaveBeenCalledWith("session_login", {
            origin: session.origin,
            email: session.email,
            password: "correct horse battery",
            label: "Production",
        });
        expect(screen.getByRole("heading", { name: "CONTAINERS", level: 1 })).toBeInTheDocument();
    });

    it("signs out through the switcher and returns to Connect", async () => {
        mockCommands();
        renderApp();
        const user = userEvent.setup();
        await user.click(await screen.findByRole("button", { name: /Switch cluster/ }));
        await user.click(await screen.findByRole("menuitem", { name: /Sign out/ }));
        await waitFor(() => expect(screen.getByRole("button", { name: /Sign in/ })).toBeInTheDocument());
        expect(invoke).toHaveBeenCalledWith("session_logout", { summary: null });
    });

    it("signs in with email and password from a fresh start", async () => {
        mockCommands({ session_login: () => session }, null);
        renderApp("/login");
        const user = userEvent.setup();
        await user.type(await screen.findByLabelText("Email"), "jane@example.com");
        await user.type(screen.getByLabelText("Password"), "correct horse battery");
        await user.click(screen.getByRole("button", { name: /Sign in/ }));
        await waitFor(() => expect(screen.getByRole("heading", { name: "CLUSTER OVERVIEW", level: 1 })).toBeInTheDocument());
        expect(invoke).toHaveBeenCalledWith("session_login", {
            origin: "http://127.0.0.1:5150",
            email: "jane@example.com",
            password: "correct horse battery",
            label: null,
        });
    });
});

import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, Outlet, RouterProvider } from "@tanstack/react-router";
import { AuthContext, type AuthContextValue } from "@/contexts/auth-context-value";
import { ClusterSwitcher } from "@/components/cluster-switcher";
import type { SavedConnection, SessionView } from "@/types/session";

const session: SessionView = {
    profileId: "p1",
    origin: "https://prod.example/",
    clusterId: "c",
    principalId: "u",
    email: "owner@example.com",
    displayName: "Owner",
    label: "prod-swarm-01",
    accessExpiresAtUnixMs: 2_000_000_000_000,
};

const connections: SavedConnection[] = [
    { id: "prod", origin: "https://prod.example/", label: "prod-swarm-01", lastUsedAtUnixMs: Date.now(), lastSummary: { nodes: 3, services: 12 } },
    { id: "staging", origin: "https://staging.example/", label: "staging", lastUsedAtUnixMs: Date.now() - 60_000, lastSummary: { nodes: 1, services: 8 } },
    { id: "dev", origin: "https://dev.example/", label: "dev-local", lastUsedAtUnixMs: Date.now() - 2 * 3_600_000, lastSummary: null },
];

function renderSwitcher(logout = vi.fn(async () => {})) {
    const auth = { session, logout, expired: false } as unknown as AuthContextValue;
    const rootRoute = createRootRoute({
        component: () => (
            <>
                <ClusterSwitcher health={{ status: "ready" }} />
                <Outlet />
            </>
        ),
    });
    const home = createRoute({ getParentRoute: () => rootRoute, path: "/", component: () => null });
    const router = createRouter({ routeTree: rootRoute.addChildren([home]), history: createMemoryHistory({ initialEntries: ["/"] }) });
    render(
        <AuthContext value={auth}>
            <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
                <RouterProvider router={router} />
            </QueryClientProvider>
        </AuthContext>,
    );
    return { logout };
}

describe("ClusterSwitcher", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
            if (command === "session_connections") return connections;
            if (command === "connection_probe")
                return (args as { origin: string }).origin.includes("dev") ? { status: "unreachable", diagnostic: "tcp" } : { status: "ready" };
            return null;
        });
    });

    it("lists agents with their summary, marks the active one, and dims offline ones", async () => {
        const user = userEvent.setup();
        renderSwitcher();
        await user.click(await screen.findByRole("button", { name: /Switch cluster \(current: prod-swarm-01\)/ }));
        expect(await screen.findByText("3 agents")).toBeInTheDocument();
        expect(screen.getByText("3 nodes · 12 services")).toBeInTheDocument();
        expect(screen.getByTestId("switch-status-prod")).toHaveTextContent("active");
        await waitFor(() => expect(screen.getByTestId("switch-status-dev")).toHaveTextContent("offline"));
        expect(screen.getByTestId("switch-status-staging")).toHaveTextContent("healthy");
        const offline = screen.getByRole("menuitem", { name: /dev-local/ });
        expect(offline).toHaveAttribute("aria-disabled", "true");
        expect(offline).toHaveClass("opacity-50");
    });

    // The signed-in shell's signed-out redirect opens Connect with that origin (settings-page.spec).
    it("signs out toward the chosen origin", async () => {
        const user = userEvent.setup();
        const { logout } = renderSwitcher();
        await user.click(await screen.findByRole("button", { name: /Switch cluster/ }));
        await waitFor(() => expect(screen.getByTestId("switch-status-staging")).toHaveTextContent("healthy"));
        await user.click(screen.getByRole("menuitem", { name: /staging/ }));
        await waitFor(() => expect(logout).toHaveBeenCalledTimes(1));
        expect(logout).toHaveBeenCalledWith(undefined, "https://staging.example/");
    });

    it("ADD NEW AGENT signs out toward Connect with an empty URL", async () => {
        const user = userEvent.setup();
        const { logout } = renderSwitcher();
        await user.click(await screen.findByRole("button", { name: /Switch cluster/ }));
        await user.click(await screen.findByRole("menuitem", { name: /Add new agent/ }));
        await waitFor(() => expect(logout).toHaveBeenCalledWith(undefined, ""));
    });
});

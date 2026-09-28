import { useState } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { createMemoryHistory, createRootRouteWithContext, createRoute, createRouter, RouterProvider, useNavigate } from "@tanstack/react-router";
import { AuthProvider } from "@/contexts/auth-context";
import { useAuth } from "@/hooks/use-auth";
import type { AuthContextValue } from "@/contexts/auth-context-value";
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

const rootRoute = createRootRouteWithContext<{ auth: AuthContextValue }>()({});
const indexRoute = createRoute({ getParentRoute: () => rootRoute, path: "/", component: () => <span data-testid='location'>home</span> });
const dashboardRoute = createRoute({ getParentRoute: () => rootRoute, path: "/dashboard", component: () => <span data-testid='location'>dashboard</span> });
const loginRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: "/login",
    validateSearch: (search: Record<string, unknown>) => ({ redirect: typeof search.redirect === "string" ? search.redirect : undefined }),
    component: function Login() {
        const { login } = useAuth();
        const navigate = useNavigate();
        const { redirect } = loginRoute.useSearch();
        return (
            <button
                onClick={() => {
                    void login({ origin: session.origin, email: session.email, password: "correct horse battery" }).then(() =>
                        navigate({ to: redirect || "/" }),
                    );
                }}
            >
                Login
            </button>
        );
    },
});
const routeTree = rootRoute.addChildren([indexRoute, loginRoute, dashboardRoute]);

function TestApp({ initialPath }: { initialPath: string }) {
    const auth = useAuth();
    const [router] = useState(() => createRouter({ routeTree, context: { auth }, history: createMemoryHistory({ initialEntries: [initialPath] }) }));
    return <RouterProvider router={router} context={{ auth }} />;
}

describe("Redirect After Login", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_restore") return null;
            if (command === "session_login") return session;
            return null;
        });
    });

    it("navigates to redirect param after custody login", async () => {
        render(
            <AuthProvider>
                <TestApp initialPath='/login?redirect=/dashboard' />
            </AuthProvider>,
        );
        await userEvent.setup().click(await screen.findByRole("button", { name: "Login" }));
        await waitFor(() => expect(screen.getByTestId("location")).toHaveTextContent("dashboard"));
    });

    it("navigates to / when no redirect param", async () => {
        render(
            <AuthProvider>
                <TestApp initialPath='/login' />
            </AuthProvider>,
        );
        await userEvent.setup().click(await screen.findByRole("button", { name: "Login" }));
        await waitFor(() => expect(screen.getByTestId("location")).toHaveTextContent("home"));
    });
});

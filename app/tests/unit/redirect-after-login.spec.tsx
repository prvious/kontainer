import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { createMemoryHistory, createRootRouteWithContext, createRoute, createRouter, redirect, RouterProvider } from "@tanstack/react-router";
import { AuthProvider } from "@/contexts/auth-context";
import { useAuth } from "@/hooks/use-auth";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import type { ParsedToken } from "@/types/auth";

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

function LoginComponent() {
    const { login } = useAuth();
    return (
        <div>
            <span data-testid='location'>login</span>
            <button onClick={() => login("http://localhost:8080")}>Login</button>
        </div>
    );
}

function HomeComponent() {
    return <span data-testid='location'>home</span>;
}

function DashboardComponent() {
    return <span data-testid='location'>dashboard</span>;
}

// Create a test router with beforeLoad redirect (matches real login route)
const rootRoute = createRootRouteWithContext<{ auth: AuthContextValue }>()({});

const indexRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: "/",
    component: HomeComponent,
});

const loginRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: "/login",
    component: LoginComponent,
    validateSearch: (search: Record<string, unknown>) => ({
        redirect: (search.redirect as string) || undefined,
    }),
    beforeLoad: ({ context, search }) => {
        if (context.auth.user) {
            throw redirect({ to: search.redirect || "/" });
        }
    },
});

const dashboardRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: "/dashboard",
    component: DashboardComponent,
});

const routeTree = rootRoute.addChildren([indexRoute, loginRoute, dashboardRoute]);

function TestApp({ initialPath }: { initialPath: string }) {
    const auth = useAuth();
    const testRouter = createRouter({
        routeTree,
        context: { auth },
        history: createMemoryHistory({ initialEntries: [initialPath] }),
    });
    return <RouterProvider router={testRouter} />;
}

describe("Redirect After Login", () => {
    beforeEach(() => {
        vi.clearAllMocks();

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return { profiles: [], activeProfileId: null };
            if (cmd === "parse_auth_token") return mockParsedToken;
            if (cmd === "save_profile") return undefined;
            if (cmd === "auth_listen") return "fake.jwt.token";
            return undefined;
        });
    });

    it("navigates to redirect param after authentication", async () => {
        render(
            <AuthProvider>
                <TestApp initialPath='/login?redirect=/dashboard' />
            </AuthProvider>,
        );
        await act(async () => {});

        // Should be on login page
        expect(screen.getByTestId("location")).toHaveTextContent("login");

        // Initiate login — auth_listen resolves immediately with token
        await act(async () => {
            screen.getByText("Login").click();
        });

        // Should redirect to /dashboard via beforeLoad guard
        expect(screen.getByTestId("location")).toHaveTextContent("dashboard");
    });

    it("navigates to / when no redirect param", async () => {
        render(
            <AuthProvider>
                <TestApp initialPath='/login' />
            </AuthProvider>,
        );
        await act(async () => {});

        // Should be on login page
        expect(screen.getByTestId("location")).toHaveTextContent("login");

        // Initiate login
        await act(async () => {
            screen.getByText("Login").click();
        });

        // Should redirect to / via beforeLoad guard
        expect(screen.getByTestId("location")).toHaveTextContent("home");
    });
});

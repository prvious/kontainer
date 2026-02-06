import { useEffect } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { createMemoryHistory, createRootRouteWithContext, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
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
    const { user, login } = useAuth();
    const navigate = Route_Login.useNavigate();
    const { redirect } = Route_Login.useSearch();

    useEffect(() => {
        if (user) {
            navigate({ to: redirect || "/" });
        }
    }, [user, redirect, navigate]);

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

// Create a test router
const rootRoute = createRootRouteWithContext<{ auth: AuthContextValue }>()({});

const indexRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: "/",
    component: HomeComponent,
});

const Route_Login = createRoute({
    getParentRoute: () => rootRoute,
    path: "/login",
    component: LoginComponent,
    validateSearch: (search: Record<string, unknown>) => ({
        redirect: (search.redirect as string) || undefined,
    }),
});

const dashboardRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: "/dashboard",
    component: DashboardComponent,
});

const routeTree = rootRoute.addChildren([indexRoute, Route_Login, dashboardRoute]);

function createTestRouter(initialPath: string) {
    return createRouter({
        routeTree,
        context: { auth: undefined! },
        history: createMemoryHistory({ initialEntries: [initialPath] }),
    });
}

function TestApp({ initialPath }: { initialPath: string }) {
    const auth = useAuth();
    const testRouter = createTestRouter(initialPath);
    return <RouterProvider router={testRouter} context={{ auth }} />;
}

describe("Redirect After Login", () => {
    let deepLinkCallback: ((urls: string[]) => void) | null = null;

    beforeEach(() => {
        vi.clearAllMocks();
        deepLinkCallback = null;

        vi.mocked(onOpenUrl).mockImplementation((cb) => {
            deepLinkCallback = cb;
            return Promise.resolve(() => {});
        });

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_session") return null;
            if (cmd === "parse_auth_token") return mockParsedToken;
            if (cmd === "save_session") return undefined;
            if (cmd === "delete_session") return undefined;
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

        // Authenticate
        await act(async () => {
            deepLinkCallback?.(["devoper://auth/callback?token=fake.jwt.token"]);
        });

        // Should redirect to /dashboard
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

        // Authenticate
        await act(async () => {
            deepLinkCallback?.(["devoper://auth/callback?token=fake.jwt.token"]);
        });

        // Should redirect to /
        expect(screen.getByTestId("location")).toHaveTextContent("home");
    });
});

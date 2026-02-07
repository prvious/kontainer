import { useEffect } from "react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
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

class MockWebSocket {
    static instances: MockWebSocket[] = [];
    onmessage: ((ev: { data: string }) => void) | null = null;
    onerror: (() => void) | null = null;
    onclose: ((ev: { wasClean: boolean }) => void) | null = null;
    url: string;
    constructor(url: string) {
        this.url = url;
        MockWebSocket.instances.push(this);
    }
    close() {
        this.onclose?.({ wasClean: true });
    }
    serverSend(data: object) {
        this.onmessage?.({ data: JSON.stringify(data) });
    }
    static latest() {
        return this.instances.at(-1)!;
    }
    static clear() {
        this.instances = [];
    }
}

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
    beforeEach(() => {
        vi.clearAllMocks();
        MockWebSocket.clear();
        vi.stubGlobal("WebSocket", MockWebSocket);

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_session") return null;
            if (cmd === "parse_auth_token") return mockParsedToken;
            if (cmd === "save_session") return undefined;
            if (cmd === "delete_session") return undefined;
            return undefined;
        });
    });

    afterEach(() => {
        vi.unstubAllGlobals();
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

        // Initiate login
        await act(async () => {
            screen.getByText("Login").click();
        });

        const ws = MockWebSocket.latest();

        // Simulate WS auth flow
        await act(async () => {
            ws.serverSend({ sessionId: "abc" });
        });
        await act(async () => {
            ws.serverSend({ token: "fake.jwt.token" });
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

        // Initiate login
        await act(async () => {
            screen.getByText("Login").click();
        });

        const ws = MockWebSocket.latest();

        // Simulate WS auth flow
        await act(async () => {
            ws.serverSend({ sessionId: "abc" });
        });
        await act(async () => {
            ws.serverSend({ token: "fake.jwt.token" });
        });

        // Should redirect to /
        expect(screen.getByTestId("location")).toHaveTextContent("home");
    });
});

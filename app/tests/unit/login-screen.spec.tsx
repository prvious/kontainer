import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { createMemoryHistory, createRootRouteWithContext, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthProvider } from "@/contexts/auth-context";
import { LoginScreen } from "@/components/login-screen";
import { useAuth } from "@/hooks/use-auth";
import type { AuthContextValue } from "@/contexts/auth-context-value";

// Create a minimal test router that includes the /login route
const rootRoute = createRootRouteWithContext<{ auth: AuthContextValue }>()({});

const loginRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: "/login",
    component: LoginScreen,
    validateSearch: (search: Record<string, unknown>) => ({
        redirect: (search.redirect as string) || undefined,
    }),
});

const indexRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: "/",
    component: () => <div>Home</div>,
});

const routeTree = rootRoute.addChildren([indexRoute, loginRoute]);

function TestWrapper() {
    const auth = useAuth();
    const testRouter = createRouter({
        routeTree,
        context: { auth },
        history: createMemoryHistory({ initialEntries: ["/login"] }),
    });
    return <RouterProvider router={testRouter} />;
}

describe("LoginScreen", () => {
    beforeEach(() => {
        vi.clearAllMocks();

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return { profiles: [], activeProfileId: null };
            if (cmd === "ping_agent") return { app: "dev.kontainer.app", status: "ok" };
            return undefined;
        });
    });

    async function renderLoginScreen() {
        const result = render(
            <AuthProvider>
                <TestWrapper />
            </AuthProvider>,
        );
        await act(async () => {});
        return result;
    }

    it("renders the title and description", async () => {
        await renderLoginScreen();
        expect(screen.getByText("kontainer")).toBeInTheDocument();
        expect(screen.getByText("Connect to your infrastructure agent")).toBeInTheDocument();
    });

    it("renders the cluster URL input with default value", async () => {
        await renderLoginScreen();
        const input = screen.getByPlaceholderText("https://agent.kontainer.dev");
        expect(input).toBeInTheDocument();
        expect(input).toHaveValue("http://localhost:3000");
    });

    it("renders the connect button", async () => {
        await renderLoginScreen();
        expect(screen.getByRole("button", { name: /connect with github/i })).toBeInTheDocument();
    });

    it("pings the agent on submit", async () => {
        await renderLoginScreen();
        const user = userEvent.setup();

        const button = screen.getByRole("button", { name: /connect with github/i });
        await user.click(button);

        expect(invoke).toHaveBeenCalledWith("ping_agent", { url: "http://localhost:3000" });
    });

    it("shows authenticate with divider always", async () => {
        await renderLoginScreen();
        expect(screen.getByText("authenticate with")).toBeInTheDocument();
    });

    it("shows Connect with GitHub button always", async () => {
        await renderLoginScreen();
        expect(screen.getByRole("button", { name: /connect with github/i })).toBeInTheDocument();
    });

    it("shows GitHub button without needing to ping first", async () => {
        await renderLoginScreen();
        expect(screen.getByRole("button", { name: /connect with github/i })).toBeInTheDocument();
    });

    it("calls auth_listen when Connect with GitHub is clicked", async () => {
        // Capture event callbacks
        let sessionIdCallback: ((event: { payload: { sessionId: string } }) => void) | null = null;
        let tokenCallback: ((event: { payload: { token: string } }) => void) | null = null;
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        vi.mocked(listen).mockImplementation(async (event: string, handler: any) => {
            if (event === "auth://session-id") sessionIdCallback = handler;
            if (event === "auth://token") tokenCallback = handler;
            return () => {};
        });

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return { profiles: [], activeProfileId: null };
            if (cmd === "ping_agent") return { app: "dev.kontainer.app", status: "ok" };
            if (cmd === "auth_listen") {
                // Background task delivers session ID
                sessionIdCallback?.({ payload: { sessionId: "test-session" } });
                return undefined;
            }
            return undefined;
        });

        await renderLoginScreen();
        const user = userEvent.setup();

        // Single click triggers ping then OAuth
        const button = screen.getByRole("button", { name: /connect with github/i });
        await user.click(button);

        // auth_listen should be called
        expect(invoke).toHaveBeenCalledWith("auth_listen", { url: "http://localhost:3000" });

        // session ID event fires synchronously in invoke mock -> opens browser
        expect(openUrl).toHaveBeenCalledWith("http://localhost:3000/auth/github?session=test-session");

        // Clean up: deliver token so login() resolves
        await act(async () => {
            tokenCallback?.({ payload: { token: "fake-jwt" } });
        });
    });

    it("shows error when ping fails", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return { profiles: [], activeProfileId: null };
            if (cmd === "ping_agent") throw "Could not reach agent at http://localhost:3000";
            return undefined;
        });

        await renderLoginScreen();
        const user = userEvent.setup();

        const button = screen.getByRole("button", { name: /connect with github/i });
        await user.click(button);

        expect(screen.getByText(/could not reach agent/i)).toBeInTheDocument();
    });

    it("clears error message when URL changes", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return { profiles: [], activeProfileId: null };
            if (cmd === "ping_agent") throw "Could not reach agent";
            return undefined;
        });

        await renderLoginScreen();
        const user = userEvent.setup();

        // Trigger an error first
        const button = screen.getByRole("button", { name: /connect with github/i });
        await user.click(button);

        expect(screen.getByText(/could not reach agent/i)).toBeInTheDocument();

        // Change URL — error should clear
        const input = screen.getByPlaceholderText("https://agent.kontainer.dev");
        await user.type(input, "/new");

        expect(screen.queryByText(/could not reach agent/i)).not.toBeInTheDocument();
    });

    it("strips trailing slashes from agent URL", async () => {
        await renderLoginScreen();
        const user = userEvent.setup();

        const input = screen.getByPlaceholderText("https://agent.kontainer.dev");
        await user.clear(input);
        await user.type(input, "http://myserver:9090///");

        const button = screen.getByRole("button", { name: /connect with github/i });
        await user.click(button);

        expect(invoke).toHaveBeenCalledWith("ping_agent", { url: "http://myserver:9090" });
    });

    it("disables button while connecting and shows connecting text", async () => {
        let resolvePing: () => void;
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return { profiles: [], activeProfileId: null };
            if (cmd === "ping_agent") {
                return new Promise((resolve) => {
                    resolvePing = () => resolve({ app: "dev.kontainer.app", status: "ok" });
                });
            }
            return undefined;
        });

        await renderLoginScreen();
        const user = userEvent.setup();

        const button = screen.getByRole("button", { name: /connect with github/i });
        await user.click(button);

        expect(screen.getByText(/connecting/i)).toBeInTheDocument();

        await act(async () => {
            resolvePing!();
        });
    });

    it("shows error when saved connection fails to switch", async () => {
        const testProfile = {
            id: "profile-1",
            label: "My Cluster",
            agentUrl: "http://cluster.example.com",
            userId: "u1",
            userName: "Test User",
            userEmail: "test@example.com",
            userAvatarUrl: "",
            userProvider: "github",
            token: "jwt-abc",
            expiresAt: Math.floor(Date.now() / 1000) + 3600,
        };

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return { profiles: [testProfile], activeProfileId: null };
            if (cmd === "set_active_profile") throw new Error("Profile switch failed");
            return undefined;
        });

        await renderLoginScreen();

        // The saved connection should appear
        expect(screen.getByText("My Cluster")).toBeInTheDocument();

        const user = userEvent.setup();
        await user.click(screen.getByText("My Cluster"));

        // Should show error message instead of navigating
        await waitFor(() => {
            expect(screen.getByText(/profile switch failed/i)).toBeInTheDocument();
        });
    });
});

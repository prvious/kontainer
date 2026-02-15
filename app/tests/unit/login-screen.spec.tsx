import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
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
            if (cmd === "ping_agent") return undefined;
            if (cmd === "fetch_providers") return ["github"];
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
        expect(screen.getByText("Connect to Cluster")).toBeInTheDocument();
        expect(screen.getByText("Enter your agent URL to get started.")).toBeInTheDocument();
    });

    it("renders the cluster URL input with default value", async () => {
        await renderLoginScreen();
        const input = screen.getByPlaceholderText("http://localhost:8080");
        expect(input).toBeInTheDocument();
        expect(input).toHaveValue("http://localhost:8080");
    });

    it("renders the connect button", async () => {
        await renderLoginScreen();
        expect(screen.getByRole("button", { name: /connect/i })).toBeInTheDocument();
    });

    it("pings the agent on submit", async () => {
        await renderLoginScreen();
        const user = userEvent.setup();

        const button = screen.getByRole("button", { name: /connect/i });
        await user.click(button);

        expect(invoke).toHaveBeenCalledWith("ping_agent", { url: "http://localhost:8080" });
    });

    it("shows auth providers after successful ping", async () => {
        await renderLoginScreen();
        const user = userEvent.setup();

        const button = screen.getByRole("button", { name: /connect/i });
        await user.click(button);

        expect(screen.getByText("Authenticate")).toBeInTheDocument();
    });

    it("fetches providers after successful ping", async () => {
        await renderLoginScreen();
        const user = userEvent.setup();

        const button = screen.getByRole("button", { name: /connect/i });
        await user.click(button);

        expect(invoke).toHaveBeenCalledWith("fetch_providers", { url: "http://localhost:8080" });
    });

    it("shows provider buttons when online", async () => {
        await renderLoginScreen();
        const user = userEvent.setup();

        const button = screen.getByRole("button", { name: /connect/i });
        await user.click(button);

        expect(screen.getByRole("button", { name: /github/i })).toBeInTheDocument();
    });

    it("calls auth_listen when provider button is clicked", async () => {
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
            if (cmd === "ping_agent") return undefined;
            if (cmd === "fetch_providers") return ["github"];
            if (cmd === "auth_listen") {
                // Background task delivers session ID
                sessionIdCallback?.({ payload: { sessionId: "test-session" } });
                return undefined;
            }
            return undefined;
        });

        await renderLoginScreen();
        const user = userEvent.setup();

        const connectButton = screen.getByRole("button", { name: /connect/i });
        await user.click(connectButton);

        const githubButton = screen.getByRole("button", { name: /github/i });
        await user.click(githubButton);

        // auth_listen should be called
        expect(invoke).toHaveBeenCalledWith("auth_listen", { url: "http://localhost:8080" });

        // session ID event fires synchronously in invoke mock → opens browser
        expect(openUrl).toHaveBeenCalledWith("http://localhost:8080/auth/github?session=test-session");

        // Clean up: deliver token so login() resolves
        await act(async () => {
            tokenCallback?.({ payload: { token: "fake-jwt" } });
        });
    });

    it("shows error when ping fails", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return { profiles: [], activeProfileId: null };
            if (cmd === "ping_agent") throw "Could not reach agent at http://localhost:8080";
            return undefined;
        });

        await renderLoginScreen();
        const user = userEvent.setup();

        const button = screen.getByRole("button", { name: /connect/i });
        await user.click(button);

        expect(screen.getByText(/could not reach agent/i)).toBeInTheDocument();
    });

    it("resets status when URL changes", async () => {
        await renderLoginScreen();
        const user = userEvent.setup();

        // Connect first
        const button = screen.getByRole("button", { name: /connect/i });
        await user.click(button);

        expect(screen.getByText("Authenticate")).toBeInTheDocument();

        // Change URL — should reset
        const input = screen.getByPlaceholderText("http://localhost:8080");
        await user.type(input, "/new");

        expect(screen.queryByText("Authenticate")).not.toBeInTheDocument();
        expect(screen.getByRole("button", { name: /connect/i })).toBeInTheDocument();
    });

    it("strips trailing slashes from agent URL", async () => {
        await renderLoginScreen();
        const user = userEvent.setup();

        const input = screen.getByPlaceholderText("http://localhost:8080");
        await user.clear(input);
        await user.type(input, "http://myserver:9090///");

        const button = screen.getByRole("button", { name: /connect/i });
        await user.click(button);

        expect(invoke).toHaveBeenCalledWith("ping_agent", { url: "http://myserver:9090" });
    });

    it("disables button while connecting and shows connecting text", async () => {
        let resolvePing: () => void;
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return { profiles: [], activeProfileId: null };
            if (cmd === "ping_agent") {
                return new Promise<void>((resolve) => {
                    resolvePing = resolve;
                });
            }
            if (cmd === "fetch_providers") return ["github"];
            return undefined;
        });

        await renderLoginScreen();
        const user = userEvent.setup();

        const button = screen.getByRole("button", { name: /connect/i });
        await user.click(button);

        expect(screen.getByText(/connecting/i)).toBeInTheDocument();

        await act(async () => {
            resolvePing!();
        });
    });
});

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { createMemoryHistory, createRootRouteWithContext, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthProvider } from "@/contexts/auth-context";
import { LoginScreen } from "@/components/login-screen";
import { useAuth } from "@/hooks/use-auth";
import type { AuthContextValue } from "@/contexts/auth-context-value";

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
        MockWebSocket.clear();
        vi.stubGlobal("WebSocket", MockWebSocket);

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_profiles") return { profiles: [], activeProfileId: null };
            if (cmd === "ping_agent") return undefined;
            if (cmd === "fetch_providers") return ["github"];
            return undefined;
        });
    });

    afterEach(() => {
        vi.unstubAllGlobals();
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

    it("renders the logo wordmark", async () => {
        await renderLoginScreen();
        expect(screen.getByText("devoper")).toBeInTheDocument();
    });

    it("renders the card title and description", async () => {
        await renderLoginScreen();
        expect(screen.getByText("Connect to Cluster")).toBeInTheDocument();
        expect(screen.getByText("Enter your agent's URL to get started.")).toBeInTheDocument();
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

    it("shows online status after successful ping", async () => {
        await renderLoginScreen();
        const user = userEvent.setup();

        const button = screen.getByRole("button", { name: /connect/i });
        await user.click(button);

        expect(screen.getByText("Cluster Online")).toBeInTheDocument();
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

        expect(screen.getByRole("button", { name: /continue with github/i })).toBeInTheDocument();
    });

    it("opens WebSocket when provider button is clicked", async () => {
        await renderLoginScreen();
        const user = userEvent.setup();

        const connectButton = screen.getByRole("button", { name: /connect/i });
        await user.click(connectButton);

        const githubButton = screen.getByRole("button", { name: /continue with github/i });
        await user.click(githubButton);

        // After clicking provider, a WebSocket should be opened
        const ws = MockWebSocket.latest();
        expect(ws.url).toBe("ws://localhost:8080/auth/listen");

        // Simulate session ID → opens browser with correct provider
        await act(async () => {
            ws.serverSend({ sessionId: "test-session" });
        });

        expect(openUrl).toHaveBeenCalledWith("http://localhost:8080/auth/github?session=test-session");
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
        expect(MockWebSocket.instances).toHaveLength(0);
    });

    it("resets status when URL changes", async () => {
        await renderLoginScreen();
        const user = userEvent.setup();

        // Connect first
        const button = screen.getByRole("button", { name: /connect/i });
        await user.click(button);

        expect(screen.getByText("Cluster Online")).toBeInTheDocument();

        // Change URL — should reset
        const input = screen.getByPlaceholderText("http://localhost:8080");
        await user.type(input, "/new");

        expect(screen.queryByText("Cluster Online")).not.toBeInTheDocument();
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

    it("disables button while connecting and shows reaching cluster text", async () => {
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

        expect(button).toBeDisabled();
        expect(screen.getByText(/reaching cluster/i)).toBeInTheDocument();

        await act(async () => {
            resolvePing!();
        });
    });
});

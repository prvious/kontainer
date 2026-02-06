import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
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

        vi.mocked(onOpenUrl).mockImplementation(() => {
            return Promise.resolve(() => {});
        });

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_session") return null;
            if (cmd === "ping_agent") return undefined;
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

    it("renders the logo wordmark", async () => {
        await renderLoginScreen();
        expect(screen.getByText("devoper")).toBeInTheDocument();
    });

    it("renders the agent URL input with default value", async () => {
        await renderLoginScreen();
        const input = screen.getByPlaceholderText("http://localhost:8080");
        expect(input).toBeInTheDocument();
        expect(input).toHaveValue("http://localhost:8080");
    });

    it("renders the connect button", async () => {
        await renderLoginScreen();
        expect(screen.getByRole("button", { name: /connect/i })).toBeInTheDocument();
    });

    it("pings the agent and calls openUrl on submit", async () => {
        await renderLoginScreen();
        const user = userEvent.setup();

        const button = screen.getByRole("button", { name: /connect/i });
        await user.click(button);

        expect(invoke).toHaveBeenCalledWith("ping_agent", { url: "http://localhost:8080" });
        expect(openUrl).toHaveBeenCalledWith("http://localhost:8080/auth/github");
    });

    it("allows changing the agent URL", async () => {
        await renderLoginScreen();
        const user = userEvent.setup();

        const input = screen.getByPlaceholderText("http://localhost:8080");
        await user.clear(input);
        await user.type(input, "http://myserver:9090");

        expect(input).toHaveValue("http://myserver:9090");
    });

    it("strips trailing slashes from agent URL before login", async () => {
        await renderLoginScreen();
        const user = userEvent.setup();

        const input = screen.getByPlaceholderText("http://localhost:8080");
        await user.clear(input);
        await user.type(input, "http://myserver:9090///");

        const button = screen.getByRole("button", { name: /connect/i });
        await user.click(button);

        expect(invoke).toHaveBeenCalledWith("ping_agent", { url: "http://myserver:9090" });
        expect(openUrl).toHaveBeenCalledWith("http://myserver:9090/auth/github");
    });

    it("shows error and does not proceed when ping fails", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_session") return null;
            if (cmd === "ping_agent") throw "Could not reach agent at http://localhost:8080";
            return undefined;
        });

        await renderLoginScreen();
        const user = userEvent.setup();

        const button = screen.getByRole("button", { name: /connect/i });
        await user.click(button);

        expect(screen.getByText(/could not reach agent/i)).toBeInTheDocument();
        expect(openUrl).not.toHaveBeenCalled();
    });

    it("disables button while pinging", async () => {
        let resolvePing: () => void;
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "load_session") return null;
            if (cmd === "ping_agent") {
                return new Promise<void>((resolve) => {
                    resolvePing = resolve;
                });
            }
            return undefined;
        });

        await renderLoginScreen();
        const user = userEvent.setup();

        const button = screen.getByRole("button", { name: /connect/i });
        await user.click(button);

        expect(button).toBeDisabled();
        expect(button).toHaveTextContent("Connecting...");

        await act(async () => {
            resolvePing!();
        });

        expect(button).not.toBeDisabled();
        expect(button).toHaveTextContent("Connect");
    });
});

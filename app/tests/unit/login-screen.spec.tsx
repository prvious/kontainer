import { describe, it, expect, vi, beforeEach } from "vitest";
import { useState } from "react";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { createMemoryHistory, createRootRouteWithContext, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthProvider } from "@/contexts/auth-context";
import { LoginScreen } from "@/components/login-screen";
import { SessionIssue } from "@/components/session-issue";
import { useAuth } from "@/hooks/use-auth";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import { SESSION_ERROR_CODES, type SessionView } from "@/types/session";

const session: SessionView = {
    profileId: "9e91e40b-09d0-44b8-9e1b-329455b25617",
    origin: "https://controller.example/",
    clusterId: "cluster-1",
    principalId: "0a749bf5-21a9-43ba-81b4-7065cddbb30f",
    githubLogin: "jane",
    label: "Production",
    accessExpiresAtUnixMs: 2_000_000_000_000,
};

const rootRoute = createRootRouteWithContext<{ auth: AuthContextValue }>()({});
const loginRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: "/login",
    component: LoginScreen,
    validateSearch: (search: Record<string, unknown>) => ({ redirect: typeof search.redirect === "string" ? search.redirect : undefined }),
});
const indexRoute = createRoute({ getParentRoute: () => rootRoute, path: "/", component: () => <div>Home</div> });
const routeTree = rootRoute.addChildren([indexRoute, loginRoute]);

function TestWrapper() {
    const auth = useAuth();
    const [router] = useState(() =>
        createRouter({
            routeTree,
            context: { auth },
            history: createMemoryHistory({ initialEntries: ["/login"] }),
        }),
    );
    return <RouterProvider router={router} context={{ auth }} />;
}

async function renderLoginScreen() {
    const result = render(
        <AuthProvider>
            <TestWrapper />
        </AuthProvider>,
    );
    await waitFor(() => expect(screen.getByRole("button", { name: /connect with github/i })).toBeInTheDocument());
    return result;
}

describe("LoginScreen custody cutover", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_restore" || command === "session_login_hint") return null;
            if (command === "session_login") return session;
            return undefined;
        });
    });

    it("renders the title and controller description", async () => {
        await renderLoginScreen();
        expect(screen.getByText("kontainer")).toBeInTheDocument();
        expect(screen.getByText("Connect to your controller")).toBeInTheDocument();
    });

    it("renders the development controller URL by default", async () => {
        await renderLoginScreen();
        expect(screen.getByPlaceholderText("https://controller.example")).toHaveValue("http://127.0.0.1:5150");
    });

    it("renders the Connect with GitHub button", async () => {
        await renderLoginScreen();
        expect(screen.getByRole("button", { name: /connect with github/i })).toBeEnabled();
    });

    it("loads the non-secret login hint", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_login_hint") return { origin: "https://saved.example/", label: "Saved" };
            return null;
        });
        await renderLoginScreen();
        await waitFor(() => expect(screen.getByPlaceholderText("https://controller.example")).toHaveValue("https://saved.example/"));
    });

    it("sends only origin and label to session_login", async () => {
        await renderLoginScreen();
        const user = userEvent.setup();
        const input = screen.getByPlaceholderText("https://controller.example");
        await user.clear(input);
        await user.type(input, "https://controller.example");
        await user.click(screen.getByRole("button", { name: /connect with github/i }));
        await waitFor(() => expect(invoke).toHaveBeenCalledWith("session_login", { origin: "https://controller.example", label: null }));
    });

    it("never calls the old agent ping or browser token listener", async () => {
        await renderLoginScreen();
        await userEvent.setup().click(screen.getByRole("button", { name: /connect with github/i }));
        await waitFor(() => expect(invoke).toHaveBeenCalledWith("session_login", expect.anything()));
        expect(vi.mocked(invoke).mock.calls.some(([command]) => ["ping_agent", "auth_listen", "parse_auth_token"].includes(command))).toBe(false);
    });

    it("uses the hint label in session_login", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_login_hint") return { origin: "https://saved.example/", label: "Saved" };
            if (command === "session_login") return session;
            return null;
        });
        await renderLoginScreen();
        await waitFor(() => expect(screen.getByPlaceholderText("https://controller.example")).toHaveValue("https://saved.example/"));
        await userEvent.setup().click(screen.getByRole("button", { name: /connect with github/i }));
        await waitFor(() => expect(invoke).toHaveBeenCalledWith("session_login", { origin: "https://saved.example/", label: "Saved" }));
    });

    it("trims surrounding whitespace from the origin", async () => {
        await renderLoginScreen();
        const user = userEvent.setup();
        const input = screen.getByPlaceholderText("https://controller.example");
        await user.clear(input);
        await user.type(input, "  https://controller.example/  ");
        await user.click(screen.getByRole("button", { name: /connect with github/i }));
        await waitFor(() => expect(invoke).toHaveBeenCalledWith("session_login", { origin: "https://controller.example/", label: null }));
    });

    it("shows native browser progress while login is pending", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_login") return new Promise(() => {});
            return null;
        });
        await renderLoginScreen();
        await userEvent.setup().click(screen.getByRole("button", { name: /connect with github/i }));
        await waitFor(() => expect(screen.getByText("Authenticating...")).toBeInTheDocument());
    });

    it("cancels through session_cancel_login", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_login") return new Promise(() => {});
            return null;
        });
        await renderLoginScreen();
        await userEvent.setup().click(screen.getByRole("button", { name: /connect with github/i }));
        await userEvent.setup().click(await screen.findByRole("button", { name: "Cancel" }));
        await waitFor(() => expect(invoke).toHaveBeenCalledWith("session_cancel_login", undefined));
    });

    it("shows a closed error message instead of native error text", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_login") throw "secret native error canary";
            return null;
        });
        await renderLoginScreen();
        await userEvent.setup().click(screen.getByRole("button", { name: /connect with github/i }));
        await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("invalid response"));
        expect(document.body.textContent).not.toContain("secret native error canary");
    });

    it("offers commit retry for vault_write_failed", async () => {
        vi.mocked(invoke).mockImplementation(async (command: string) => {
            if (command === "session_login") throw "vault_write_failed";
            if (command === "session_retry_commit") return session;
            return null;
        });
        await renderLoginScreen();
        await userEvent.setup().click(screen.getByRole("button", { name: /connect with github/i }));
        await waitFor(() => expect(screen.getByRole("button", { name: "Retry saving session" })).toBeInTheDocument());
        await act(async () => {
            screen.getByRole("button", { name: "Retry saving session" }).click();
        });
        await waitFor(() => expect(invoke).toHaveBeenCalledWith("session_retry_commit", undefined));
    });

    it("renders every custody failure as an explicit message", () => {
        for (const code of SESSION_ERROR_CODES) {
            const { unmount } = render(<SessionIssue code={code} />);
            expect(screen.getByRole("alert").textContent?.trim().length).toBeGreaterThan(0);
            unmount();
        }
    });
});

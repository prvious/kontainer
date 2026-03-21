import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import { StacksPage } from "@/routes/_authenticated/stacks/index";

function createTestQueryClient() {
    return new QueryClient({
        defaultOptions: {
            queries: { retry: false, gcTime: 0 },
        },
    });
}

const mockAuth: AuthContextValue = {
    user: { id: "1", name: "Test", email: "test@test.com", avatarUrl: "", provider: "github" },
    token: "test-jwt",
    expiresAt: Math.floor(Date.now() / 1000) + 9999,
    agentUrl: "http://localhost:3000",
    isLoading: false,
    login: vi.fn(),
    logout: vi.fn(),
};

function createTestRouter() {
    const rootRoute = createRootRoute({});
    const indexRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/",
        component: StacksPage,
    });
    const stackDetailRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/stacks/$stackName",
        component: () => <div>detail</div>,
    });
    const routeTree = rootRoute.addChildren([indexRoute, stackDetailRoute]);
    return createRouter({
        routeTree,
        history: createMemoryHistory({ initialEntries: ["/"] }),
    });
}

function renderWithProviders() {
    const queryClient = createTestQueryClient();
    const router = createTestRouter();

    return render(
        <AuthContext value={mockAuth}>
            <QueryClientProvider client={queryClient}>
                <RouterProvider router={router} />
            </QueryClientProvider>
        </AuthContext>,
    );
}

describe("List Stacks", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("renders stacks with names and service counts", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_stacks")
                return {
                    stacks: [
                        { name: "myapp", services: 3, runningReplicas: 5, desiredReplicas: 5, updatedAt: "2025-01-02T00:00:00Z" },
                        { name: "monitoring", services: 2, runningReplicas: 4, desiredReplicas: 4, updatedAt: "2025-01-01T00:00:00Z" },
                    ],
                };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("myapp")).toBeInTheDocument();
        });

        expect(screen.getByText("monitoring")).toBeInTheDocument();
        // service counts appear in the table rows
        const threes = screen.getAllByText("3");
        expect(threes.length).toBeGreaterThan(0);
        const twos = screen.getAllByText("2");
        expect(twos.length).toBeGreaterThan(0);
    });

    it("shows replica badges with correct counts", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_stacks")
                return {
                    stacks: [{ name: "myapp", services: 2, runningReplicas: 3, desiredReplicas: 5, updatedAt: "2025-01-01T00:00:00Z" }],
                };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("3/5")).toBeInTheDocument();
        });
    });

    it("shows empty state when no stacks exist", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_stacks") return { stacks: [] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("No stacks found")).toBeInTheDocument();
        });
    });

    it("shows error state on failure", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_stacks") throw new Error("connection refused");
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Failed to load stacks")).toBeInTheDocument();
        });
    });

    it("renders column headers", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_stacks")
                return {
                    stacks: [{ name: "myapp", services: 1, runningReplicas: 1, desiredReplicas: 1, updatedAt: "2025-01-01T00:00:00Z" }],
                };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("NAME")).toBeInTheDocument();
        });

        expect(screen.getByText("STATUS")).toBeInTheDocument();
        expect(screen.getByText("SERVICES")).toBeInTheDocument();
        expect(screen.getByText("REPLICAS")).toBeInTheDocument();
        expect(screen.getByText("LAST DEPLOY")).toBeInTheDocument();
    });

    it("renders page header with title", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_stacks") return { stacks: [] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("STACKS")).toBeInTheDocument();
        });

        expect(screen.getByText("DEPLOY STACK")).toBeInTheDocument();
    });
});

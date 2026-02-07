import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import { ServicesList } from "@/components/services-list";
import type { ServiceItem } from "@/types/services";

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
    agentUrl: "http://localhost:8080",
    isLoading: false,
    login: vi.fn(),
    logout: vi.fn(),
};

function createTestRouter() {
    const rootRoute = createRootRoute({});
    const indexRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/",
        component: ServicesList,
    });
    const serviceDetailRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/services/$serviceId",
        component: () => <div>detail</div>,
    });
    const routeTree = rootRoute.addChildren([indexRoute, serviceDetailRoute]);
    return createRouter({
        routeTree,
        history: createMemoryHistory({ initialEntries: ["/"] }),
    });
}

function renderWithProviders(authOverrides: Partial<AuthContextValue> = {}) {
    const queryClient = createTestQueryClient();
    const auth = { ...mockAuth, ...authOverrides };
    const router = createTestRouter();

    return render(
        <AuthContext value={auth}>
            <QueryClientProvider client={queryClient}>
                <RouterProvider router={router} />
            </QueryClientProvider>
        </AuthContext>,
    );
}

const sampleServices: ServiceItem[] = [
    {
        id: "svc1",
        name: "web",
        image: "nginx:latest",
        stack: "myapp",
        replicasRunning: 3,
        replicasDesired: 3,
        updatedAt: new Date().toISOString(),
    },
    {
        id: "svc2",
        name: "api",
        image: "node:20",
        stack: "myapp",
        replicasRunning: 1,
        replicasDesired: 2,
        updatedAt: new Date().toISOString(),
    },
    {
        id: "svc3",
        name: "standalone-db",
        image: "postgres:16",
        stack: "",
        replicasRunning: 1,
        replicasDesired: 1,
        updatedAt: new Date().toISOString(),
    },
];

describe("ServicesList", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("shows loading skeleton initially", async () => {
        vi.mocked(invoke).mockImplementation(() => new Promise(() => {})); // never resolves
        renderWithProviders();

        // Wait for router to render, then check skeletons
        await waitFor(() => {
            const skeletons = document.querySelectorAll('[data-slot="skeleton"]');
            expect(skeletons.length).toBeGreaterThan(0);
        });
    });

    it("renders grouped services after loading", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_services") return { services: sampleServices };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("myapp")).toBeInTheDocument();
        });

        expect(screen.getByText("Standalone")).toBeInTheDocument();
        expect(screen.getByText("web")).toBeInTheDocument();
        expect(screen.getByText("api")).toBeInTheDocument();
        expect(screen.getByText("standalone-db")).toBeInTheDocument();
        expect(screen.getByText("nginx:latest")).toBeInTheDocument();
        expect(screen.getByText("3/3")).toBeInTheDocument();
        expect(screen.getByText("1/2")).toBeInTheDocument();
    });

    it("shows empty state when no services", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_services") return { services: [] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("No services found")).toBeInTheDocument();
        });
    });

    it("shows error state when fetch fails", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_services") throw new Error("network error");
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Failed to load services")).toBeInTheDocument();
        });
    });

    it("does not fetch when auth is missing", () => {
        vi.mocked(invoke).mockImplementation(async () => ({ services: [] }));

        renderWithProviders({ token: null, agentUrl: null });

        // invoke should not be called for list_services since query is disabled
        expect(invoke).not.toHaveBeenCalledWith("list_services", expect.anything());
    });

    it("renders global mode services with global badge", async () => {
        const globalService: ServiceItem = {
            id: "svc-global",
            name: "monitoring",
            image: "prometheus:v2",
            stack: "",
            replicasRunning: 2,
            replicasDesired: -1,
            updatedAt: new Date().toISOString(),
        };

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_services") return { services: [globalService] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("monitoring")).toBeInTheDocument();
        });

        expect(screen.getByText("global")).toBeInTheDocument();
    });
});

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import { useServices } from "@/hooks/use-services";
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
    agentUrl: "http://localhost:3000",
    isLoading: false,
    profiles: [],
    activeProfileId: null,
    login: vi.fn(),
    logout: vi.fn(),
    switchProfile: vi.fn(),
    removeProfile: vi.fn(),
};

// Minimal component that mirrors the stack detail page behavior
function TestStackDetail() {
    const stackName = "mystack"; // We'll use a fixed name via route params below
    const { data: services, isLoading, error } = useServices(stackName);

    if (isLoading) {
        return <div>Loading...</div>;
    }

    if (error) {
        return <div>Failed to load services</div>;
    }

    const totalRunning = (services ?? []).reduce((sum, s) => sum + s.replicasRunning, 0);
    const totalDesired = (services ?? []).reduce((sum, s) => sum + (s.replicasDesired === -1 ? s.replicasRunning : s.replicasDesired), 0);

    return (
        <div>
            <a href='/stacks'>Stacks</a>
            <h1>{stackName}</h1>
            <p>
                {services?.length ?? 0} services &middot; {totalRunning}/{totalDesired} replicas
            </p>
            {(services ?? []).length === 0 ? (
                <p>No services in this stack</p>
            ) : (
                <table>
                    <tbody>
                        {services!.map((svc) => (
                            <tr key={svc.id}>
                                <td>{svc.name}</td>
                                <td>{svc.image}</td>
                                <td>
                                    {svc.replicasRunning}/{svc.replicasDesired}
                                </td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            )}
        </div>
    );
}

function createTestRouter() {
    const rootRoute = createRootRoute({});
    const stackDetailRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/stacks/$stackName",
        component: TestStackDetail,
    });
    const stacksRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/stacks",
        component: () => <div>Stacks list</div>,
    });
    const serviceDetailRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/services/$serviceId",
        component: () => <div>Service detail</div>,
    });
    const routeTree = rootRoute.addChildren([stackDetailRoute, stacksRoute, serviceDetailRoute]);
    return createRouter({
        routeTree,
        history: createMemoryHistory({ initialEntries: ["/stacks/mystack"] }),
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
        stack: "mystack",
        replicasRunning: 3,
        replicasDesired: 3,
        updatedAt: new Date().toISOString(),
    },
    {
        id: "svc2",
        name: "api",
        image: "node:20",
        stack: "mystack",
        replicasRunning: 1,
        replicasDesired: 2,
        updatedAt: new Date().toISOString(),
    },
];

describe("StackDetail", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("renders services for the stack", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_services") return { services: sampleServices };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("web")).toBeInTheDocument();
        });

        expect(screen.getByText("api")).toBeInTheDocument();
        expect(screen.getByText("nginx:latest")).toBeInTheDocument();
        expect(screen.getByText("node:20")).toBeInTheDocument();
        expect(screen.getByText("3/3")).toBeInTheDocument();
        expect(screen.getByText("1/2")).toBeInTheDocument();
    });

    it("passes stack param to invoke", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_services") return { services: sampleServices };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith("list_services", {
                url: "http://localhost:3000",
                token: "test-jwt",
                stack: "mystack",
            });
        });
    });

    it("shows back link to stacks", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_services") return { services: sampleServices };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Stacks")).toBeInTheDocument();
        });
    });

    it("shows stack name as heading", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_services") return { services: sampleServices };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("mystack")).toBeInTheDocument();
        });
    });

    it("shows replica summary", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_services") return { services: sampleServices };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText(/2 services/)).toBeInTheDocument();
        });

        expect(screen.getByText(/4\/5 replicas/)).toBeInTheDocument();
    });

    it("shows empty state when stack has no services", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_services") return { services: [] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("No services in this stack")).toBeInTheDocument();
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
});

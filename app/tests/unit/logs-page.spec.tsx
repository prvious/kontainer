import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import type { ServiceItem } from "@/types/services";

// --- Route component under test ---
import { Route as LogsRoute } from "@/routes/_authenticated/logs/index";

// --- Mock useLogStream to avoid WebSocket connections ---
vi.mock("@/hooks/use-log-stream", () => ({
    useLogStream: () => ({
        logs: [],
        status: "connected" as const,
        isPaused: false,
        clearLogs: vi.fn(),
        pause: vi.fn(),
        resume: vi.fn(),
        reconnect: vi.fn(),
    }),
}));

// --- Helpers ---

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
    profiles: [
        {
            id: "profile-1",
            label: "My Cluster",
            agentUrl: "http://localhost:3000",
            userId: "1",
            userName: "Test",
            userEmail: "test@test.com",
            userAvatarUrl: "",
            userProvider: "github",
            token: "test-jwt",
            expiresAt: Math.floor(Date.now() / 1000) + 9999,
        },
    ],
    activeProfileId: "profile-1",
    login: vi.fn(),
    logout: vi.fn(),
    switchProfile: vi.fn(),
    removeProfile: vi.fn(),
    updateProfile: vi.fn(),
};

const sampleServices: ServiceItem[] = [
    {
        id: "svc-1",
        name: "nginx-proxy",
        image: "nginx:latest",
        stack: "",
        replicasRunning: 3,
        replicasDesired: 3,
        updatedAt: new Date().toISOString(),
    },
    {
        id: "svc-2",
        name: "api-gateway",
        image: "node:18",
        stack: "backend",
        replicasRunning: 2,
        replicasDesired: 2,
        updatedAt: new Date().toISOString(),
    },
];

// Extract the component from the route module
const LogsPage = LogsRoute.options.component!;

function createTestRouter() {
    const rootRoute = createRootRoute({});
    const indexRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/",
        component: LogsPage,
    });
    const routeTree = rootRoute.addChildren([indexRoute]);
    return createRouter({
        routeTree,
        history: createMemoryHistory({ initialEntries: ["/"] }),
    });
}

let uuidCounter = 0;

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

// --- Tests ---

describe("LogsPage", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        uuidCounter = 0;
        vi.spyOn(crypto, "randomUUID").mockImplementation(() => `uuid-${++uuidCounter}` as `${string}-${string}-${string}-${string}-${string}`);
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_services") return { services: sampleServices };
            return undefined;
        });
    });

    it("renders page header with LOGS title", async () => {
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("LOGS")).toBeInTheDocument();
        });
    });

    it("shows empty state when no streams", async () => {
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("No log streams active")).toBeInTheDocument();
        });
    });

    it("opens stream picker when clicking New Stream", async () => {
        const user = userEvent.setup();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("New Stream")).toBeInTheDocument();
        });

        await user.click(screen.getByText("New Stream"));

        await waitFor(() => {
            expect(screen.getByText("Select Service")).toBeInTheDocument();
        });
    });

    it("adds a stream tab when selecting a service", async () => {
        const user = userEvent.setup();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("New Stream")).toBeInTheDocument();
        });

        await user.click(screen.getByText("New Stream"));

        await waitFor(() => {
            expect(screen.getByText("Select Service")).toBeInTheDocument();
        });

        // Wait for services to load in the picker
        await waitFor(() => {
            expect(screen.getByText("nginx-proxy")).toBeInTheDocument();
        });

        await user.click(screen.getByText("nginx-proxy"));

        // Tab should appear with service name
        await waitFor(() => {
            // The empty state should be gone
            expect(screen.queryByText("No log streams active")).not.toBeInTheDocument();
            // The tab with the service name should be visible
            expect(screen.getByText("nginx-proxy")).toBeInTheDocument();
        });
    });

    it("switches between tabs", async () => {
        const user = userEvent.setup();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("New Stream")).toBeInTheDocument();
        });

        // Add first stream
        await user.click(screen.getByText("New Stream"));
        await waitFor(() => {
            expect(screen.getByText("nginx-proxy")).toBeInTheDocument();
        });
        await user.click(screen.getByText("nginx-proxy"));

        // Add second stream via the + button in the tab bar
        // After adding first stream, the "New Stream" button is still in the header
        await user.click(screen.getByText("New Stream"));
        await waitFor(() => {
            expect(screen.getByText("Select Service")).toBeInTheDocument();
        });
        await waitFor(() => {
            expect(screen.getByText("api-gateway")).toBeInTheDocument();
        });
        await user.click(screen.getByText("api-gateway"));

        // Both tabs should exist
        await waitFor(() => {
            expect(screen.getByText("nginx-proxy")).toBeInTheDocument();
            expect(screen.getByText("api-gateway")).toBeInTheDocument();
        });

        // The second tab (api-gateway) should be active since it was just added.
        // Click the first tab to switch.
        await user.click(screen.getByText("nginx-proxy"));

        // After clicking, the first tab button should have the active class
        await waitFor(() => {
            const nginxTab = screen.getByText("nginx-proxy").closest("button");
            expect(nginxTab).toHaveClass("text-accent-green");
        });
    });

    it("closes a stream tab", async () => {
        const user = userEvent.setup();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("New Stream")).toBeInTheDocument();
        });

        // Add a stream
        await user.click(screen.getByText("New Stream"));
        await waitFor(() => {
            expect(screen.getByText("nginx-proxy")).toBeInTheDocument();
        });
        await user.click(screen.getByText("nginx-proxy"));

        // Verify the tab is present
        await waitFor(() => {
            expect(screen.queryByText("No log streams active")).not.toBeInTheDocument();
            expect(screen.getByText("nginx-proxy")).toBeInTheDocument();
        });

        // Find the close (X) button inside the tab — it is an SVG with class "size-3"
        const tabButton = screen.getByText("nginx-proxy").closest("button")!;
        const closeIcon = tabButton.querySelector(".size-3.text-muted-foreground");
        expect(closeIcon).toBeTruthy();
        await user.click(closeIcon!);

        // Tab should be removed and empty state should return
        await waitFor(() => {
            expect(screen.getByText("No log streams active")).toBeInTheDocument();
        });
    });
});

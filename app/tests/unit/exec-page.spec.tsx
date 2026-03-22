import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import type { TaskItem } from "@/types/tasks";

// --- Mock ghostty-web to avoid native WASM loading ---
vi.mock("ghostty-web", () => {
    const noop = () => {};
    return {
        Terminal: class {
            loadAddon = noop;
            open = noop;
            onData = noop;
            dispose = noop;
            clear = noop;
            cols = 80;
            rows = 24;
        },
        FitAddon: class {
            fit = noop;
        },
        init: () => Promise.resolve(),
    };
});

// --- Mock useExecStream to avoid WebSocket connections ---
vi.mock("@/hooks/use-exec-stream", () => ({
    useExecStream: () => ({
        status: "connected" as const,
        exitCode: null,
        attachTerminal: vi.fn(),
        sendInput: vi.fn(),
        sendResize: vi.fn(),
        connect: vi.fn(),
        disconnect: vi.fn(),
    }),
}));

// --- Route component under test ---
import { Route as ExecRoute } from "@/routes/_authenticated/exec/index";
import { TerminalProvider } from "@/contexts/terminal-context";

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

const sampleTasks: TaskItem[] = [
    {
        id: "task-1",
        serviceId: "svc-1",
        serviceName: "nginx-proxy",
        slot: 1,
        nodeId: "node-1",
        nodeName: "manager-1",
        state: "running",
        desiredState: "running",
        exitCode: null,
        error: "",
        timestamp: new Date().toISOString(),
        containerId: "abc123def456",
        image: "nginx:latest",
    },
    {
        id: "task-2",
        serviceId: "svc-2",
        serviceName: "api-gateway",
        slot: 1,
        nodeId: "node-2",
        nodeName: "worker-1",
        state: "running",
        desiredState: "running",
        exitCode: null,
        error: "",
        timestamp: new Date().toISOString(),
        containerId: "def456ghi789",
        image: "node:18",
    },
];

// Extract the component from the route module
const ExecPage = ExecRoute.options.component!;

// Wrap ExecPage with TerminalProvider inside the router so TerminalPanel has router context
function ExecPageWithTerminal() {
    return (
        <TerminalProvider>
            <ExecPage />
        </TerminalProvider>
    );
}

function createTestRouter() {
    const rootRoute = createRootRoute({});
    const indexRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/",
        component: ExecPageWithTerminal,
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

// Polyfill ResizeObserver for jsdom
if (typeof globalThis.ResizeObserver === "undefined") {
    globalThis.ResizeObserver = class ResizeObserver {
        observe() {}
        unobserve() {}
        disconnect() {}
    } as unknown as typeof globalThis.ResizeObserver;
}

// Polyfill Element.getAnimations for jsdom (used by @base-ui/react ScrollArea)
if (typeof Element.prototype.getAnimations === "undefined") {
    Element.prototype.getAnimations = () => [];
}

describe("ExecPage", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        uuidCounter = 0;
        vi.spyOn(crypto, "randomUUID").mockImplementation(() => `uuid-${++uuidCounter}` as `${string}-${string}-${string}-${string}-${string}`);
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: sampleTasks };
            return undefined;
        });
    });

    it("renders EXEC page header", async () => {
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("EXEC")).toBeInTheDocument();
        });
    });

    it("shows empty state when no sessions", async () => {
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("No terminal sessions")).toBeInTheDocument();
        });
    });

    it("opens container picker when clicking New Session", async () => {
        const user = userEvent.setup();
        renderWithProviders();

        // The empty state has both a header and body "New Session" button
        await waitFor(() => {
            expect(screen.getAllByText("New Session").length).toBeGreaterThanOrEqual(1);
        });

        // Click the first "New Session" button (header button)
        await user.click(screen.getAllByText("New Session")[0]);

        await waitFor(() => {
            expect(screen.getByText("Select Container")).toBeInTheDocument();
        });
    });

    it("adds a session tab when selecting a container", async () => {
        const user = userEvent.setup();
        renderWithProviders();

        await waitFor(() => {
            expect(screen.getAllByText("New Session").length).toBeGreaterThanOrEqual(1);
        });

        // Open the container picker
        await user.click(screen.getAllByText("New Session")[0]);

        await waitFor(() => {
            expect(screen.getByText("Select Container")).toBeInTheDocument();
        });

        // Wait for tasks to load in the picker
        await waitFor(() => {
            expect(screen.getByText("nginx-proxy")).toBeInTheDocument();
        });

        await user.click(screen.getByText("nginx-proxy"));

        // Tab should appear and empty state should be gone
        await waitFor(() => {
            expect(screen.queryByText("No terminal sessions")).not.toBeInTheDocument();
            expect(screen.getByText("nginx-proxy")).toBeInTheDocument();
        });
    });
});

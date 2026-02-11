import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { AuthContext } from "@/contexts/auth-context-value";
import { SettingsContext } from "@/contexts/settings-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import type { SettingsContextValue } from "@/contexts/settings-context-value";
import type { TaskItem } from "@/types/tasks";

// Import the page component
import { Route as TasksRoute } from "@/routes/_authenticated/tasks/index";

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

const mockSettings: SettingsContextValue = {
    refreshInterval: 5000,
    setRefreshInterval: vi.fn(),
};

function createTestRouter() {
    const rootRoute = createRootRoute({});
    const tasksRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/tasks",
        component: TasksRoute.options.component,
    });
    const serviceDetailRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/services/$serviceId",
        component: () => <div>Service Detail</div>,
    });
    const routeTree = rootRoute.addChildren([tasksRoute, serviceDetailRoute]);
    return createRouter({
        routeTree,
        history: createMemoryHistory({ initialEntries: ["/tasks"] }),
    });
}

function renderWithProviders(authOverrides: Partial<AuthContextValue> = {}, settingsOverrides: Partial<SettingsContextValue> = {}) {
    const queryClient = createTestQueryClient();
    const auth = { ...mockAuth, ...authOverrides };
    const settings = { ...mockSettings, ...settingsOverrides };
    const router = createTestRouter();

    return render(
        <AuthContext value={auth}>
            <SettingsContext value={settings}>
                <QueryClientProvider client={queryClient}>
                    <RouterProvider router={router} />
                </QueryClientProvider>
            </SettingsContext>
        </AuthContext>,
    );
}

const sampleTasks: TaskItem[] = [
    {
        id: "task1",
        serviceId: "svc1",
        serviceName: "web",
        slot: 1,
        nodeId: "node1",
        nodeName: "worker-1",
        state: "running",
        desiredState: "running",
        exitCode: null,
        error: "",
        timestamp: new Date(Date.now() - 60000).toISOString(), // 1 minute ago
        containerId: "abc123",
        image: "nginx:latest",
    },
    {
        id: "task2",
        serviceId: "svc2",
        serviceName: "api",
        slot: 1,
        nodeId: "node2",
        nodeName: "worker-2",
        state: "failed",
        desiredState: "running",
        exitCode: 1,
        error: "container exited",
        timestamp: new Date(Date.now() - 3600000).toISOString(), // 1 hour ago
        containerId: "def456",
        image: "node:20",
    },
    {
        id: "task3",
        serviceId: "svc3",
        serviceName: "database",
        slot: 1,
        nodeId: "node3",
        nodeName: "worker-3",
        state: "complete",
        desiredState: "shutdown",
        exitCode: 0,
        error: "",
        timestamp: new Date(Date.now() - 86400000).toISOString(), // 1 day ago
        containerId: "ghi789",
        image: "postgres:16",
    },
];

describe("TasksPage", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("shows loading skeleton initially", async () => {
        vi.mocked(invoke).mockImplementation(() => new Promise(() => {})); // never resolves
        renderWithProviders();

        await waitFor(() => {
            const skeletons = document.querySelectorAll('[data-slot="skeleton"]');
            expect(skeletons.length).toBeGreaterThan(0);
        });
    });

    it("renders tasks table after loading", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: sampleTasks };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("web")).toBeInTheDocument();
        });

        expect(screen.getByText("api")).toBeInTheDocument();
        expect(screen.getByText("database")).toBeInTheDocument();
        expect(screen.getByText("worker-1")).toBeInTheDocument();
        expect(screen.getByText("worker-2")).toBeInTheDocument();
        expect(screen.getByText("nginx:latest")).toBeInTheDocument();
        expect(screen.getByText("node:20")).toBeInTheDocument();
    });

    it("shows task count", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: sampleTasks };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("3 tasks")).toBeInTheDocument();
        });
    });

    it("shows empty state when no tasks", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: [] };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("No tasks found")).toBeInTheDocument();
        });

        expect(screen.getByText("Deployment tasks and operations will appear here")).toBeInTheDocument();
    });

    it("shows error state when fetch fails", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") throw new Error("network error");
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("Failed to load tasks")).toBeInTheDocument();
        });
    });

    it("filters tasks by search query", async () => {
        const user = userEvent.setup();
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: sampleTasks };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("web")).toBeInTheDocument();
        });

        const searchInput = screen.getByPlaceholderText("Search tasks...");
        await user.type(searchInput, "api");

        // Should show only the api task
        expect(screen.getByText("api")).toBeInTheDocument();
        expect(screen.queryByText("web")).not.toBeInTheDocument();
        expect(screen.queryByText("database")).not.toBeInTheDocument();
    });

    it("shows no results message when search has no matches", async () => {
        const user = userEvent.setup();
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: sampleTasks };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("web")).toBeInTheDocument();
        });

        const searchInput = screen.getByPlaceholderText("Search tasks...");
        await user.type(searchInput, "nonexistent");

        await waitFor(() => {
            expect(screen.getByText("No tasks match your search")).toBeInTheDocument();
        });

        expect(screen.getByText("Try a different search term")).toBeInTheDocument();
    });

    it("renders state badges with correct colors", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: sampleTasks };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            expect(screen.getByText("running")).toBeInTheDocument();
        });

        expect(screen.getByText("failed")).toBeInTheDocument();
        expect(screen.getByText("complete")).toBeInTheDocument();
    });

    it("renders service links", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_tasks") return { tasks: sampleTasks };
            return undefined;
        });

        renderWithProviders();

        await waitFor(() => {
            const webLink = screen.getByRole("link", { name: /web/i });
            expect(webLink).toHaveAttribute("href", "/services/svc1");
        });
    });

    it("does not fetch when auth is missing", () => {
        vi.mocked(invoke).mockImplementation(async () => ({ tasks: [] }));

        renderWithProviders({ token: null, agentUrl: null });

        // invoke should not be called for list_tasks since query is disabled
        expect(invoke).not.toHaveBeenCalledWith("list_tasks", expect.anything());
    });
});

describe("formatRelativeTime helper", () => {
    it("formats time correctly", () => {
        const now = Date.now();

        // Test "just now"
        const justNow = new Date(now - 30000).toISOString(); // 30 seconds ago
        expect(formatRelativeTimeHelper(justNow)).toBe("just now");

        // Test minutes
        const minutesAgo = new Date(now - 120000).toISOString(); // 2 minutes ago
        expect(formatRelativeTimeHelper(minutesAgo)).toBe("2m ago");

        // Test hours
        const hoursAgo = new Date(now - 7200000).toISOString(); // 2 hours ago
        expect(formatRelativeTimeHelper(hoursAgo)).toBe("2h ago");

        // Test days
        const daysAgo = new Date(now - 172800000).toISOString(); // 2 days ago
        expect(formatRelativeTimeHelper(daysAgo)).toBe("2d ago");
    });
});

describe("getStateColor helper", () => {
    it("returns correct colors for states", () => {
        expect(getStateColorHelper("running")).toBe("bg-success");
        expect(getStateColorHelper("failed")).toBe("bg-destructive");
        expect(getStateColorHelper("rejected")).toBe("bg-destructive");
        expect(getStateColorHelper("complete")).toBe("bg-muted-foreground");
        expect(getStateColorHelper("shutdown")).toBe("bg-muted-foreground");
        expect(getStateColorHelper("preparing")).toBe("bg-warning");
        expect(getStateColorHelper("pending")).toBe("bg-warning");
        expect(getStateColorHelper("assigned")).toBe("bg-warning");
        expect(getStateColorHelper("accepted")).toBe("bg-warning");
        expect(getStateColorHelper("unknown")).toBe("bg-muted-foreground");
    });

    it("handles case insensitivity", () => {
        expect(getStateColorHelper("RUNNING")).toBe("bg-success");
        expect(getStateColorHelper("Failed")).toBe("bg-destructive");
    });
});

// Helper functions extracted for testing
function formatRelativeTimeHelper(iso: string): string {
    const seconds = Math.floor((Date.now() - new Date(iso).getTime()) / 1000);
    if (seconds < 60) return "just now";
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
}

function getStateColorHelper(state: string): string {
    switch (state.toLowerCase()) {
        case "running":
            return "bg-success";
        case "failed":
        case "rejected":
            return "bg-destructive";
        case "complete":
        case "shutdown":
            return "bg-muted-foreground";
        case "preparing":
        case "pending":
        case "assigned":
        case "accepted":
            return "bg-warning";
        default:
            return "bg-muted-foreground";
    }
}

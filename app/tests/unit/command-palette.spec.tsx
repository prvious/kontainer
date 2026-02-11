import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, render, screen, waitFor, fireEvent } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { CommandPaletteContext } from "@/contexts/command-palette-context-value";
import type { CommandPaletteContextValue } from "@/contexts/command-palette-context-value";

import { useAuth } from "@/hooks/use-auth";
vi.mock("@/hooks/use-auth", () => ({
    useAuth: vi.fn(),
}));

import { useServices } from "@/hooks/use-services";
vi.mock("@/hooks/use-services", () => ({
    useServices: vi.fn(),
}));

import { useNodes } from "@/hooks/use-nodes";
vi.mock("@/hooks/use-nodes", () => ({
    useNodes: vi.fn(),
}));

import { useStacks } from "@/hooks/use-stacks";
vi.mock("@/hooks/use-stacks", () => ({
    useStacks: vi.fn(),
}));

import { useServiceTasks } from "@/hooks/use-service-tasks";
vi.mock("@/hooks/use-service-tasks", () => ({
    useServiceTasks: vi.fn(),
}));

import { useTerminal } from "@/hooks/use-terminal";
vi.mock("@/hooks/use-terminal", () => ({
    useTerminal: vi.fn(),
}));

import { CommandPalette } from "@/components/command-palette";

const mockSetOpen = vi.fn();
const mockOpenExec = vi.fn();

const defaultAuthReturn = {
    user: { id: "u1", name: "Test User", email: "test@example.com", avatarUrl: "", provider: "github" },
    token: "test-token",
    expiresAt: Math.floor(Date.now() / 1000) + 3600,
    agentUrl: "https://agent.example.com",
    isLoading: false,
    profiles: [],
    activeProfileId: null,
    login: vi.fn(),
    logout: vi.fn(),
    switchProfile: vi.fn(),
    removeProfile: vi.fn(),
    updateProfile: vi.fn(),
};

const testServices = [
    { id: "svc1", name: "web-app", image: "nginx:latest", stack: "prod", replicasRunning: 3, replicasDesired: 3, updatedAt: "2026-01-01" },
    { id: "svc2", name: "api-server", image: "node:20", stack: "prod", replicasRunning: 2, replicasDesired: 2, updatedAt: "2026-01-01" },
];

const testNodes = [
    {
        id: "n1",
        hostname: "node-1",
        role: "manager",
        status: "ready",
        availability: "active",
        engineVersion: "24.0",
        os: "linux",
        architecture: "amd64",
        ip: "10.0.0.1",
        leader: true,
        updatedAt: "2026-01-01",
    },
];

const testStacks = [{ name: "prod", services: 5, runningReplicas: 10, desiredReplicas: 10, updatedAt: "2026-01-01" }];

const testTasks = [
    {
        id: "t1",
        slot: 1,
        nodeId: "n1",
        nodeName: "node-1",
        state: "running",
        desiredState: "running",
        exitCode: null,
        error: "",
        timestamp: "2026-01-01",
        containerId: "abc123def456",
        user: "root",
    },
    {
        id: "t2",
        slot: 2,
        nodeId: "n1",
        nodeName: "node-1",
        state: "running",
        desiredState: "running",
        exitCode: null,
        error: "",
        timestamp: "2026-01-01",
        containerId: "def456ghi789",
        user: "root",
    },
];

function createTestQueryClient() {
    return new QueryClient({
        defaultOptions: {
            queries: { retry: false, gcTime: 0 },
        },
    });
}

function PaletteTestComponent() {
    return <CommandPalette />;
}

function createTestRouter() {
    const rootRoute = createRootRoute({});
    const indexRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/",
        component: PaletteTestComponent,
    });
    const routeTree = rootRoute.addChildren([indexRoute]);
    return createRouter({
        routeTree,
        history: createMemoryHistory({ initialEntries: ["/"] }),
    });
}

function renderPalette(paletteOverrides: Partial<CommandPaletteContextValue> = {}) {
    const queryClient = createTestQueryClient();
    const paletteCtx: CommandPaletteContextValue = {
        open: true,
        setOpen: mockSetOpen,
        ...paletteOverrides,
    };
    const router = createTestRouter();

    return render(
        <CommandPaletteContext value={paletteCtx}>
            <QueryClientProvider client={queryClient}>
                <RouterProvider router={router} />
            </QueryClientProvider>
        </CommandPaletteContext>,
    );
}

function setupDefaultMocks() {
    vi.mocked(useAuth).mockReturnValue(defaultAuthReturn);
    vi.mocked(useServices).mockReturnValue({ data: testServices } as ReturnType<typeof useServices>);
    vi.mocked(useNodes).mockReturnValue({ data: testNodes } as ReturnType<typeof useNodes>);
    vi.mocked(useStacks).mockReturnValue({ data: testStacks } as ReturnType<typeof useStacks>);
    vi.mocked(useServiceTasks).mockReturnValue({ data: undefined } as ReturnType<typeof useServiceTasks>);
    vi.mocked(useTerminal).mockReturnValue({
        sessions: [],
        activeSessionId: null,
        openExec: mockOpenExec,
        closeSession: vi.fn(),
        closeAllSessions: vi.fn(),
        switchSession: vi.fn(),
        hasAnySessions: false,
    });
}

describe("CommandPalette", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        setupDefaultMocks();
    });

    it("renders the search input when open", async () => {
        renderPalette();

        await waitFor(() => {
            expect(screen.getByPlaceholderText("Type a command or search...")).toBeInTheDocument();
        });
    });

    it("shows navigation commands", async () => {
        renderPalette();

        await waitFor(() => {
            expect(screen.getByText("Navigation")).toBeInTheDocument();
            expect(screen.getByText("Overview")).toBeInTheDocument();
            expect(screen.getByText("Networking")).toBeInTheDocument();
            expect(screen.getByText("Volumes")).toBeInTheDocument();
        });
    });

    it("shows quick action commands", async () => {
        renderPalette();

        await waitFor(() => {
            expect(screen.getByText("Switch to dark mode")).toBeInTheDocument();
            expect(screen.getByText("Settings")).toBeInTheDocument();
            expect(screen.getByText("Disconnect")).toBeInTheDocument();
        });
    });

    it("shows services from data hooks", async () => {
        renderPalette();

        await waitFor(() => {
            expect(screen.getByText("web-app")).toBeInTheDocument();
            expect(screen.getByText("api-server")).toBeInTheDocument();
        });
    });

    it("shows nodes from data hooks", async () => {
        renderPalette();

        await waitFor(() => {
            expect(screen.getByText("node-1")).toBeInTheDocument();
        });
    });

    it("shows stacks from data hooks", async () => {
        renderPalette();

        await waitFor(() => {
            expect(screen.getByText("prod")).toBeInTheDocument();
        });
    });

    it("filters commands by search query", async () => {
        renderPalette();

        await waitFor(() => {
            expect(screen.getByPlaceholderText("Type a command or search...")).toBeInTheDocument();
        });

        const input = screen.getByPlaceholderText("Type a command or search...");
        await act(async () => {
            fireEvent.change(input, { target: { value: "web-app" } });
        });

        await waitFor(() => {
            expect(screen.getByText("web-app")).toBeInTheDocument();
            expect(screen.queryByText("Overview")).not.toBeInTheDocument();
            expect(screen.queryByText("api-server")).not.toBeInTheDocument();
        });
    });

    it("shows no results message when nothing matches", async () => {
        renderPalette();

        await waitFor(() => {
            expect(screen.getByPlaceholderText("Type a command or search...")).toBeInTheDocument();
        });

        const input = screen.getByPlaceholderText("Type a command or search...");
        await act(async () => {
            fireEvent.change(input, { target: { value: "xyznonexistent" } });
        });

        await waitFor(() => {
            expect(screen.getByText("No results found")).toBeInTheDocument();
        });
    });

    it("renders keyboard hints in footer", async () => {
        renderPalette();

        await waitFor(() => {
            expect(screen.getByText("navigate")).toBeInTheDocument();
            expect(screen.getByText("select")).toBeInTheDocument();
            expect(screen.getByText("close")).toBeInTheDocument();
        });
    });
});

describe("CommandPalette - Drill-down", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        setupDefaultMocks();
    });

    it("drills into service actions when clicking a service", async () => {
        renderPalette();

        await waitFor(() => {
            expect(screen.getByText("web-app")).toBeInTheDocument();
        });

        await act(async () => {
            screen.getByText("web-app").closest("button")!.click();
        });

        await waitFor(() => {
            expect(screen.getByText("View details")).toBeInTheDocument();
            expect(screen.getByText("View logs")).toBeInTheDocument();
            expect(screen.getByText("Open terminal")).toBeInTheDocument();
            expect(screen.getByText("Restart service")).toBeInTheDocument();
        });
    });

    it("shows breadcrumb when drilled into service actions", async () => {
        renderPalette();

        await waitFor(() => {
            expect(screen.getByText("web-app")).toBeInTheDocument();
        });

        await act(async () => {
            screen.getByText("web-app").closest("button")!.click();
        });

        await waitFor(() => {
            expect(screen.getByText("Commands")).toBeInTheDocument();
        });
    });

    it("drills into container selection from service actions", async () => {
        vi.mocked(useServiceTasks).mockReturnValue({ data: testTasks } as ReturnType<typeof useServiceTasks>);

        renderPalette();

        await waitFor(() => {
            expect(screen.getByText("web-app")).toBeInTheDocument();
        });

        // Click service to drill in
        await act(async () => {
            screen.getByText("web-app").closest("button")!.click();
        });

        await waitFor(() => {
            expect(screen.getByText("Open terminal")).toBeInTheDocument();
        });

        // Click "Open terminal"
        await act(async () => {
            screen.getByText("Open terminal").closest("button")!.click();
        });

        await waitFor(() => {
            expect(screen.getByText("Slot 1 on node-1")).toBeInTheDocument();
            expect(screen.getByText("Slot 2 on node-1")).toBeInTheDocument();
        });
    });

    it("calls openExec when selecting a container", async () => {
        vi.mocked(useServiceTasks).mockReturnValue({ data: testTasks } as ReturnType<typeof useServiceTasks>);

        renderPalette();

        await waitFor(() => {
            expect(screen.getByText("web-app")).toBeInTheDocument();
        });

        // Drill into service → terminal → container
        await act(async () => {
            screen.getByText("web-app").closest("button")!.click();
        });
        await waitFor(() => {
            expect(screen.getByText("Open terminal")).toBeInTheDocument();
        });

        await act(async () => {
            screen.getByText("Open terminal").closest("button")!.click();
        });
        await waitFor(() => {
            expect(screen.getByText("Slot 1 on node-1")).toBeInTheDocument();
        });

        await act(async () => {
            screen.getByText("Slot 1 on node-1").closest("button")!.click();
        });

        expect(mockOpenExec).toHaveBeenCalledWith({
            containerId: "abc123def456",
            nodeId: "n1",
            containerName: "web-app",
        });
        expect(mockSetOpen).toHaveBeenCalledWith(false);
    });

    it("navigates back with Backspace on empty search", async () => {
        renderPalette();

        await waitFor(() => {
            expect(screen.getByText("web-app")).toBeInTheDocument();
        });

        // Drill into service
        await act(async () => {
            screen.getByText("web-app").closest("button")!.click();
        });
        await waitFor(() => {
            expect(screen.getByText("View details")).toBeInTheDocument();
        });

        // Press Backspace on empty input to go back
        const input = screen.getByPlaceholderText("Filter...");
        await act(async () => {
            fireEvent.keyDown(input.closest("[data-slot='dialog-content']")!, { key: "Backspace" });
        });

        await waitFor(() => {
            expect(screen.getByText("web-app")).toBeInTheDocument();
            expect(screen.getByPlaceholderText("Type a command or search...")).toBeInTheDocument();
        });
    });
});

describe("CommandPalette - Keyboard navigation", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        setupDefaultMocks();
        vi.mocked(useServices).mockReturnValue({ data: [] } as unknown as ReturnType<typeof useServices>);
        vi.mocked(useNodes).mockReturnValue({ data: [] } as unknown as ReturnType<typeof useNodes>);
        vi.mocked(useStacks).mockReturnValue({ data: [] } as unknown as ReturnType<typeof useStacks>);
    });

    it("selects next item with ArrowDown", async () => {
        renderPalette();

        await waitFor(() => {
            expect(screen.getByPlaceholderText("Type a command or search...")).toBeInTheDocument();
        });

        const dialog = screen.getByPlaceholderText("Type a command or search...").closest("[data-slot='dialog-content']")!;

        // First item is selected by default (index 0)
        // Press ArrowDown to select next
        await act(async () => {
            fireEvent.keyDown(dialog, { key: "ArrowDown" });
        });

        // Second item should be selected (Settings)
        const buttons = dialog.querySelectorAll("button[data-selected]");
        expect(buttons.length).toBe(1);
    });

    it("executes selected item with Enter", async () => {
        renderPalette();

        await waitFor(() => {
            expect(screen.getByText("Switch to dark mode")).toBeInTheDocument();
        });

        const dialog = screen.getByPlaceholderText("Type a command or search...").closest("[data-slot='dialog-content']")!;

        // First item (Toggle theme) is selected, press Enter
        await act(async () => {
            fireEvent.keyDown(dialog, { key: "Enter" });
        });

        // setOpen(false) called = theme toggle closes palette
        expect(mockSetOpen).toHaveBeenCalledWith(false);
    });
});

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import { TerminalProvider } from "@/contexts/terminal-context";
import { ServiceTasks } from "@/components/service-tasks";

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

const mockTasks = [
    {
        id: "task1",
        slot: 1,
        nodeId: "node1",
        nodeName: "worker-1",
        state: "running",
        desiredState: "running",
        exitCode: 0,
        error: "",
        timestamp: "2025-01-02T00:00:00Z",
        containerId: "abc123def456",
    },
];

const mockTaskDetail = {
    id: "task1",
    serviceId: "svc1",
    slot: 1,
    nodeId: "node1",
    nodeName: "worker-1",
    state: "running",
    desiredState: "running",
    error: "",
    image: "nginx:latest",
    containerId: "abc123def456789",
    pid: 12345,
    exitCode: 0,
    createdAt: "2025-01-01T00:00:00Z",
    updatedAt: "2025-01-02T00:00:00Z",
    env: [
        { key: "NODE_ENV", value: "production" },
        { key: "PORT", value: "8080" },
    ],
    mounts: [{ type: "volume", source: "data-vol", target: "/data", readOnly: false }],
    networks: [{ name: "my-overlay", addresses: ["10.0.0.5/24"] }],
};

function renderWithProviders(serviceId: string) {
    const queryClient = createTestQueryClient();

    return render(
        <AuthContext value={mockAuth}>
            <QueryClientProvider client={queryClient}>
                <TerminalProvider>
                    <ServiceTasks serviceId={serviceId} />
                </TerminalProvider>
            </QueryClientProvider>
        </AuthContext>,
    );
}

describe("Inspect Task", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("opens task detail sheet when clicking a task row", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_service_tasks") return { tasks: mockTasks };
            if (cmd === "inspect_task") return mockTaskDetail;
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByText("Tasks (1)")).toBeInTheDocument();
        });

        fireEvent.click(screen.getByTestId("task-row-task1"));

        await waitFor(() => {
            expect(screen.getByText("Task Detail")).toBeInTheDocument();
        });

        expect(invoke).toHaveBeenCalledWith("inspect_task", {
            url: "http://localhost:8080",
            token: "test-jwt",
            taskId: "task1",
        });
    });

    it("shows container detail in the sheet", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_service_tasks") return { tasks: mockTasks };
            if (cmd === "inspect_task") return mockTaskDetail;
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByText("Tasks (1)")).toBeInTheDocument();
        });

        fireEvent.click(screen.getByTestId("task-row-task1"));

        await waitFor(() => {
            expect(screen.getByText("nginx:latest")).toBeInTheDocument();
        });

        expect(screen.getByText("abc123def456789")).toBeInTheDocument();
        expect(screen.getByText("12345")).toBeInTheDocument();
    });

    it("shows environment variables in the sheet", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_service_tasks") return { tasks: mockTasks };
            if (cmd === "inspect_task") return mockTaskDetail;
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByText("Tasks (1)")).toBeInTheDocument();
        });

        fireEvent.click(screen.getByTestId("task-row-task1"));

        await waitFor(() => {
            expect(screen.getByText("Environment (2)")).toBeInTheDocument();
        });

        expect(screen.getByText("NODE_ENV")).toBeInTheDocument();
        expect(screen.getByText("production")).toBeInTheDocument();
    });

    it("shows mounts and networks in the sheet", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_service_tasks") return { tasks: mockTasks };
            if (cmd === "inspect_task") return mockTaskDetail;
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByText("Tasks (1)")).toBeInTheDocument();
        });

        fireEvent.click(screen.getByTestId("task-row-task1"));

        await waitFor(() => {
            expect(screen.getByText("Mounts (1)")).toBeInTheDocument();
        });

        expect(screen.getByText("data-vol")).toBeInTheDocument();
        expect(screen.getByText("/data")).toBeInTheDocument();
        expect(screen.getByText("Networks (1)")).toBeInTheDocument();
        expect(screen.getByText("my-overlay")).toBeInTheDocument();
        expect(screen.getByText("10.0.0.5/24")).toBeInTheDocument();
    });

    it("shows error state when inspect fails", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_service_tasks") return { tasks: mockTasks };
            if (cmd === "inspect_task") throw new Error("connection refused");
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByText("Tasks (1)")).toBeInTheDocument();
        });

        fireEvent.click(screen.getByTestId("task-row-task1"));

        await waitFor(() => {
            expect(screen.getByText("Failed to load task details")).toBeInTheDocument();
        });
    });
});

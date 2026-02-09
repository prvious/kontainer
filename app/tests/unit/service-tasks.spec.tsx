import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
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

describe("Service Tasks", () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it("renders task list with state badges and node names", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_service_tasks")
                return {
                    tasks: [
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
                            user: "appuser",
                        },
                        {
                            id: "task2",
                            slot: 2,
                            nodeId: "node2",
                            nodeName: "worker-2",
                            state: "failed",
                            desiredState: "running",
                            exitCode: 137,
                            error: "task: non-zero exit (137)",
                            timestamp: "2025-01-01T00:00:00Z",
                            containerId: "xyz789",
                            user: "",
                        },
                    ],
                };
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByText("Tasks (2)")).toBeInTheDocument();
        });

        expect(screen.getByText("worker-1")).toBeInTheDocument();
        expect(screen.getByText("worker-2")).toBeInTheDocument();
        expect(screen.getByText("137")).toBeInTheDocument();
        expect(screen.getByText("abc123def456")).toBeInTheDocument();
    });

    it("shows empty state when no tasks", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_service_tasks") return { tasks: [] };
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByText("No tasks found")).toBeInTheDocument();
        });
    });

    it("shows error state on failure", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_service_tasks") throw new Error("connection refused");
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByText("Failed to load tasks")).toBeInTheDocument();
        });
    });

    it("shows table headers", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_service_tasks")
                return {
                    tasks: [
                        {
                            id: "task1",
                            slot: 1,
                            nodeId: "n1",
                            nodeName: "worker-1",
                            state: "running",
                            desiredState: "running",
                            exitCode: null,
                            error: "",
                            timestamp: "2025-01-01T00:00:00Z",
                            containerId: "",
                            user: "",
                        },
                    ],
                };
            return undefined;
        });

        renderWithProviders("svc1");

        await waitFor(() => {
            expect(screen.getByText("Slot")).toBeInTheDocument();
        });

        expect(screen.getByText("State")).toBeInTheDocument();
        expect(screen.getByText("Node")).toBeInTheDocument();
        expect(screen.getByText("Exit Code")).toBeInTheDocument();
        expect(screen.getByText("Container")).toBeInTheDocument();
    });

    it("calls invoke with correct parameters", async () => {
        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_service_tasks") return { tasks: [] };
            return undefined;
        });

        renderWithProviders("svc-test-123");

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith("list_service_tasks", {
                url: "http://localhost:8080",
                token: "test-jwt",
                serviceId: "svc-test-123",
            });
        });
    });
});

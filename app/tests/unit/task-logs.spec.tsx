import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, act, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import { TerminalProvider } from "@/contexts/terminal-context";
import { LogViewer } from "@/components/log-viewer";
import { ServiceTasks } from "@/components/service-tasks";

// --- Mock WebSocket ---
type MockWSInstance = {
    url: string;
    onopen: ((ev: Event) => void) | null;
    onmessage: ((ev: MessageEvent) => void) | null;
    onclose: ((ev: CloseEvent) => void) | null;
    onerror: ((ev: Event) => void) | null;
    close: ReturnType<typeof vi.fn>;
    readyState: number;
};

let wsInstances: MockWSInstance[] = [];

class MockWebSocket {
    url: string;
    onopen: ((ev: Event) => void) | null = null;
    onmessage: ((ev: MessageEvent) => void) | null = null;
    onclose: ((ev: CloseEvent) => void) | null = null;
    onerror: ((ev: Event) => void) | null = null;
    close = vi.fn();
    readyState = 0;

    static CONNECTING = 0;
    static OPEN = 1;
    static CLOSING = 2;
    static CLOSED = 3;

    constructor(url: string) {
        this.url = url;
        wsInstances.push(this as unknown as MockWSInstance);
    }
}

function simulateOpen(ws: MockWSInstance) {
    ws.readyState = 1;
    ws.onopen?.(new Event("open"));
}

function simulateMessage(ws: MockWSInstance, data: object) {
    ws.onmessage?.(new MessageEvent("message", { data: JSON.stringify(data) }));
}

const OriginalWebSocket = globalThis.WebSocket;

beforeEach(() => {
    wsInstances = [];
    globalThis.WebSocket = MockWebSocket as unknown as typeof WebSocket;
});

afterEach(() => {
    globalThis.WebSocket = OriginalWebSocket;
});

// --- Auth context helper ---
const mockAuth: AuthContextValue = {
    user: { id: "1", name: "Test", email: "test@test.com", avatarUrl: "", provider: "github" },
    token: "test-jwt-token",
    expiresAt: Math.floor(Date.now() / 1000) + 9999,
    agentUrl: "http://localhost:8080",
    isLoading: false,
    login: vi.fn(),
    logout: vi.fn(),
};

function createTestQueryClient() {
    return new QueryClient({
        defaultOptions: {
            queries: { retry: false, gcTime: 0 },
        },
    });
}

describe("Task Log Streaming", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.useFakeTimers({ shouldAdvanceTime: true });
    });

    afterEach(() => {
        vi.useRealTimers();
    });

    it("LogViewer with taskId constructs correct WS URL", async () => {
        render(
            <AuthContext value={mockAuth}>
                <LogViewer taskId='task-abc-123' />
            </AuthContext>,
        );

        await waitFor(() => {
            expect(wsInstances.length).toBeGreaterThan(0);
        });

        const ws = wsInstances[wsInstances.length - 1];
        expect(ws.url).toContain("ws://localhost:8080/api/tasks/task-abc-123/logs");
        expect(ws.url).toContain("token=test-jwt-token");
        expect(ws.url).toContain("tail=100");
    });

    it("LogViewer with taskId displays task log lines", async () => {
        render(
            <AuthContext value={mockAuth}>
                <LogViewer taskId='task-abc-123' />
            </AuthContext>,
        );

        await waitFor(() => {
            expect(wsInstances.length).toBeGreaterThan(0);
        });

        const ws = wsInstances[wsInstances.length - 1];

        act(() => {
            simulateOpen(ws);
        });

        act(() => {
            simulateMessage(ws, { stream: "stdout", timestamp: "", line: "container output" });
        });

        await waitFor(() => {
            expect(screen.getByText("container output")).toBeInTheDocument();
        });
    });

    it("LogViewer compact mode uses shorter height", async () => {
        const { container } = render(
            <AuthContext value={mockAuth}>
                <LogViewer taskId='task-abc-123' compact />
            </AuthContext>,
        );

        await waitFor(() => {
            expect(wsInstances.length).toBeGreaterThan(0);
        });

        const logContainer = container.querySelector(".h-\\[300px\\]");
        expect(logContainer).toBeInTheDocument();
    });

    it("task detail sheet shows log toggle button and opens log viewer", async () => {
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
                containerId: "abc123",
                user: "",
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
            env: [],
            mounts: [],
            networks: [],
        };

        vi.mocked(invoke).mockImplementation(async (cmd: string) => {
            if (cmd === "list_service_tasks") return { tasks: mockTasks };
            if (cmd === "inspect_task") return mockTaskDetail;
            return undefined;
        });

        const queryClient = createTestQueryClient();

        render(
            <AuthContext value={mockAuth}>
                <QueryClientProvider client={queryClient}>
                    <TerminalProvider>
                        <ServiceTasks serviceId='svc1' />
                    </TerminalProvider>
                </QueryClientProvider>
            </AuthContext>,
        );

        // Wait for tasks to load
        await waitFor(() => {
            expect(screen.getByText("Tasks (1)")).toBeInTheDocument();
        });

        // Click task row to open sheet
        fireEvent.click(screen.getByTestId("task-row-task1"));

        // Wait for task detail to load
        await waitFor(() => {
            expect(screen.getByText("Task Detail")).toBeInTheDocument();
        });

        // Find and click Show logs button
        const toggleBtn = screen.getByTestId("toggle-task-logs");
        expect(toggleBtn).toHaveTextContent("Show");

        fireEvent.click(toggleBtn);

        // After clicking Show, a WebSocket connection should be initiated for task logs
        await waitFor(() => {
            const taskWs = wsInstances.find((ws) => ws.url.includes("/api/tasks/task1/logs"));
            expect(taskWs).toBeDefined();
        });
    });
});

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, act, fireEvent } from "@testing-library/react";
import { AuthContext } from "@/contexts/auth-context-value";
import type { AuthContextValue } from "@/contexts/auth-context-value";
import { LogViewer } from "@/components/log-viewer";

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
    readyState = 0; // CONNECTING

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
    agentUrl: "http://localhost:3000",
    isLoading: false,
    login: vi.fn(),
    logout: vi.fn(),
};

function renderLogViewer() {
    return render(
        <AuthContext value={mockAuth}>
            <LogViewer serviceId='svc-123' />
        </AuthContext>,
    );
}

describe("LogViewer", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.useFakeTimers({ shouldAdvanceTime: true });
    });

    afterEach(() => {
        vi.useRealTimers();
    });

    it("shows connecting state initially", async () => {
        renderLogViewer();

        await waitFor(() => {
            expect(screen.getByText("Connecting...")).toBeInTheDocument();
        });
        expect(screen.getByText("connecting")).toBeInTheDocument();
    });

    it("shows connected state after WS opens", async () => {
        renderLogViewer();

        await waitFor(() => {
            expect(wsInstances.length).toBeGreaterThan(0);
        });

        act(() => {
            simulateOpen(wsInstances[wsInstances.length - 1]);
        });

        await waitFor(() => {
            expect(screen.getByText("connected")).toBeInTheDocument();
        });
    });

    it("constructs correct WS URL with query params", async () => {
        renderLogViewer();

        await waitFor(() => {
            expect(wsInstances.length).toBeGreaterThan(0);
        });

        const ws = wsInstances[wsInstances.length - 1];
        expect(ws.url).toContain("ws://localhost:3000/api/services/svc-123/logs");
        expect(ws.url).toContain("token=test-jwt-token");
        expect(ws.url).toContain("tail=100");
        expect(ws.url).toContain("timestamps=true");
        expect(ws.url).toContain("stdout=true");
        expect(ws.url).toContain("stderr=true");
    });

    it("displays log lines when messages arrive", async () => {
        renderLogViewer();

        await waitFor(() => {
            expect(wsInstances.length).toBeGreaterThan(0);
        });

        const ws = wsInstances[wsInstances.length - 1];

        act(() => {
            simulateOpen(ws);
        });

        act(() => {
            simulateMessage(ws, { stream: "stdout", timestamp: "2025-01-15T10:00:00Z", line: "app started" });
            simulateMessage(ws, { stream: "stdout", timestamp: "2025-01-15T10:00:01Z", line: "listening on :8080" });
        });

        await waitFor(() => {
            expect(screen.getByText("app started")).toBeInTheDocument();
            expect(screen.getByText("listening on :8080")).toBeInTheDocument();
        });
    });

    it("styles stderr lines with text-destructive", async () => {
        renderLogViewer();

        await waitFor(() => {
            expect(wsInstances.length).toBeGreaterThan(0);
        });

        const ws = wsInstances[wsInstances.length - 1];

        act(() => {
            simulateOpen(ws);
        });

        act(() => {
            simulateMessage(ws, { stream: "stderr", timestamp: "", line: "error occurred" });
        });

        await waitFor(() => {
            const errorLine = screen.getByText("error occurred");
            expect(errorLine).toHaveClass("text-destructive");
        });
    });

    it("shows line count", async () => {
        renderLogViewer();

        await waitFor(() => {
            expect(wsInstances.length).toBeGreaterThan(0);
        });

        const ws = wsInstances[wsInstances.length - 1];

        act(() => {
            simulateOpen(ws);
        });

        // Initially 0 lines
        expect(screen.getByText("0 lines")).toBeInTheDocument();

        act(() => {
            simulateMessage(ws, { stream: "stdout", line: "line 1" });
            simulateMessage(ws, { stream: "stdout", line: "line 2" });
        });

        await waitFor(() => {
            expect(screen.getByText("2 lines")).toBeInTheDocument();
        });
    });

    it("clear button removes all logs", async () => {
        renderLogViewer();

        await waitFor(() => {
            expect(wsInstances.length).toBeGreaterThan(0);
        });

        const ws = wsInstances[wsInstances.length - 1];

        act(() => {
            simulateOpen(ws);
        });

        act(() => {
            simulateMessage(ws, { stream: "stdout", line: "some log" });
        });

        await waitFor(() => {
            expect(screen.getByText("some log")).toBeInTheDocument();
        });

        const clearBtn = screen.getByTitle("Clear logs");
        act(() => {
            fireEvent.click(clearBtn);
        });

        await waitFor(() => {
            expect(screen.queryByText("some log")).not.toBeInTheDocument();
            expect(screen.getByText("0 lines")).toBeInTheDocument();
        });
    });

    it("pause buffers incoming logs and resume flushes them", async () => {
        renderLogViewer();

        await waitFor(() => {
            expect(wsInstances.length).toBeGreaterThan(0);
        });

        const ws = wsInstances[wsInstances.length - 1];

        act(() => {
            simulateOpen(ws);
        });

        // Click pause
        const pauseBtn = screen.getByTitle("Pause");
        act(() => {
            fireEvent.click(pauseBtn);
        });

        await waitFor(() => {
            expect(screen.getByText("Paused")).toBeInTheDocument();
        });

        // Send messages while paused
        act(() => {
            simulateMessage(ws, { stream: "stdout", line: "buffered line" });
        });

        // Line should not be visible while paused
        expect(screen.queryByText("buffered line")).not.toBeInTheDocument();

        // Click resume
        const resumeBtn = screen.getByTitle("Resume");
        act(() => {
            fireEvent.click(resumeBtn);
        });

        // Buffered line should now appear
        await waitFor(() => {
            expect(screen.getByText("buffered line")).toBeInTheDocument();
        });
    });

    it("disconnects WebSocket on unmount", async () => {
        const { unmount } = renderLogViewer();

        await waitFor(() => {
            expect(wsInstances.length).toBeGreaterThan(0);
        });

        const ws = wsInstances[wsInstances.length - 1];

        act(() => {
            simulateOpen(ws);
        });

        unmount();

        expect(ws.close).toHaveBeenCalled();
    });
});

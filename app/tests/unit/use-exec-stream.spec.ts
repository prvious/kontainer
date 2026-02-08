import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";

vi.mock("@/hooks/use-auth", () => ({
    useAuth: () => ({
        user: null,
        token: "test-token",
        expiresAt: null,
        agentUrl: "http://localhost:8080",
        isLoading: false,
        profiles: [],
        activeProfileId: null,
        login: vi.fn(),
        logout: vi.fn(),
        switchProfile: vi.fn(),
        removeProfile: vi.fn(),
    }),
}));

class MockWebSocket {
    static instances: MockWebSocket[] = [];
    static readonly CONNECTING = 0;
    static readonly OPEN = 1;
    static readonly CLOSING = 2;
    static readonly CLOSED = 3;

    readonly CONNECTING = 0;
    readonly OPEN = 1;
    readonly CLOSING = 2;
    readonly CLOSED = 3;

    onopen: ((ev: Event) => void) | null = null;
    onmessage: ((ev: MessageEvent) => void) | null = null;
    onclose: ((ev: CloseEvent) => void) | null = null;
    onerror: ((ev: Event) => void) | null = null;
    url: string;
    binaryType: string = "blob";
    readyState: number = MockWebSocket.CONNECTING;

    constructor(url: string) {
        this.url = url;
        MockWebSocket.instances.push(this);
    }

    close() {
        this.readyState = MockWebSocket.CLOSED;
        this.onclose?.({ wasClean: true } as CloseEvent);
    }

    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    send(_data: unknown) {
        // no-op for tests
    }

    simulateOpen() {
        this.readyState = MockWebSocket.OPEN;
        this.onopen?.({} as Event);
    }

    simulateMessage(data: ArrayBuffer | string) {
        this.onmessage?.({ data } as MessageEvent);
    }

    simulateError() {
        this.onerror?.({} as Event);
    }

    simulateClose(wasClean = true) {
        this.readyState = MockWebSocket.CLOSED;
        this.onclose?.({ wasClean } as CloseEvent);
    }

    static latest(): MockWebSocket {
        return this.instances.at(-1)!;
    }

    static clear() {
        this.instances = [];
    }
}

describe("useExecStream", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        MockWebSocket.clear();
        vi.stubGlobal("WebSocket", MockWebSocket);
    });

    afterEach(() => {
        vi.unstubAllGlobals();
    });

    async function importHook() {
        const { useExecStream } = await import("@/hooks/use-exec-stream");
        return useExecStream;
    }

    it("starts in disconnected status", async () => {
        const useExecStream = await importHook();
        const { result } = renderHook(() => useExecStream("container1", "node1"));

        expect(result.current.status).toBe("disconnected");
        expect(result.current.exitCode).toBeNull();
    });

    it("connect transitions to connecting then connected", async () => {
        const useExecStream = await importHook();
        const { result } = renderHook(() => useExecStream("container1", "node1"));

        await act(async () => {
            result.current.connect(80, 24);
        });

        expect(result.current.status).toBe("connecting");

        const ws = MockWebSocket.latest();
        expect(ws).toBeDefined();
        expect(ws.url).toContain("/api/exec/container1");
        expect(ws.url).toContain("token=test-token");
        expect(ws.url).toContain("nodeID=node1");
        expect(ws.url).toContain("shell=%2Fbin%2Fsh");
        expect(ws.url).toContain("cols=80");
        expect(ws.url).toContain("rows=24");
        expect(ws.binaryType).toBe("arraybuffer");

        await act(async () => {
            ws.simulateOpen();
        });

        expect(result.current.status).toBe("connected");
    });

    it("handles binary message (terminal output)", async () => {
        const useExecStream = await importHook();
        const writeFn = vi.fn();
        const { result } = renderHook(() => useExecStream("container1", "node1"));

        // Attach a mock terminal before connecting
        act(() => {
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            result.current.attachTerminal({ write: writeFn } as any);
        });

        act(() => {
            result.current.connect();
        });

        const ws = MockWebSocket.latest();
        act(() => {
            ws.simulateOpen();
        });

        // Create a proper ArrayBuffer in the correct realm for jsdom instanceof check
        const text = "hello terminal";
        const buffer = new ArrayBuffer(text.length);
        const view = new Uint8Array(buffer);
        for (let i = 0; i < text.length; i++) {
            view[i] = text.charCodeAt(i);
        }

        act(() => {
            ws.simulateMessage(buffer);
        });

        expect(writeFn).toHaveBeenCalled();
        const written = writeFn.mock.calls[0][0] as Uint8Array;
        expect(new TextDecoder().decode(written)).toBe("hello terminal");
    });

    it("handles error control message", async () => {
        const useExecStream = await importHook();
        const writeFn = vi.fn();
        const { result } = renderHook(() => useExecStream("container1", "node1"));

        await act(async () => {
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            result.current.attachTerminal({ write: writeFn } as any);
        });

        await act(async () => {
            result.current.connect();
        });

        const ws = MockWebSocket.latest();
        await act(async () => {
            ws.simulateOpen();
        });

        // Send error control message as text frame
        await act(async () => {
            ws.simulateMessage(JSON.stringify({ type: "error", message: "test error" }));
        });

        expect(result.current.status).toBe("error");
        // Should have written the error to the terminal
        expect(writeFn).toHaveBeenCalledWith(expect.stringContaining("Error: test error"));
    });

    it("handles exit control message", async () => {
        const useExecStream = await importHook();
        const writeFn = vi.fn();
        const { result } = renderHook(() => useExecStream("container1", "node1"));

        await act(async () => {
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            result.current.attachTerminal({ write: writeFn } as any);
        });

        await act(async () => {
            result.current.connect();
        });

        const ws = MockWebSocket.latest();
        await act(async () => {
            ws.simulateOpen();
        });

        // Send exit control message as text frame
        await act(async () => {
            ws.simulateMessage(JSON.stringify({ type: "exit", code: 0 }));
        });

        expect(result.current.status).toBe("exited");
        expect(result.current.exitCode).toBe(0);
        // Should write exit message to terminal
        expect(writeFn).toHaveBeenCalledWith(expect.stringContaining("Process exited with code 0"));
    });

    it("disconnect closes WebSocket", async () => {
        const useExecStream = await importHook();
        const { result } = renderHook(() => useExecStream("container1", "node1"));

        await act(async () => {
            result.current.connect();
        });

        const ws = MockWebSocket.latest();
        const closeSpy = vi.spyOn(ws, "close");

        await act(async () => {
            ws.simulateOpen();
        });

        expect(result.current.status).toBe("connected");

        await act(async () => {
            result.current.disconnect();
        });

        expect(closeSpy).toHaveBeenCalled();
        expect(result.current.status).toBe("disconnected");
    });

    it("shows error message when connection fails before open", async () => {
        const useExecStream = await importHook();
        const writeFn = vi.fn();
        const { result } = renderHook(() => useExecStream("container1", "node1"));

        await act(async () => {
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            result.current.attachTerminal({ write: writeFn } as any);
        });

        await act(async () => {
            result.current.connect();
        });

        expect(result.current.status).toBe("connecting");

        const ws = MockWebSocket.latest();

        // Simulate error without having opened first
        await act(async () => {
            ws.simulateError();
        });

        expect(result.current.status).toBe("error");
        // Should write the connection failure message since didOpen is false
        expect(writeFn).toHaveBeenCalledWith(expect.stringContaining("Failed to connect"));
    });
});

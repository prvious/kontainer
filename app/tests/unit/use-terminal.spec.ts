import { describe, it, expect, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { createElement } from "react";
import { useTerminal } from "@/hooks/use-terminal";
import { TerminalProvider } from "@/contexts/terminal-context";

vi.mock("@/components/terminal-panel", () => ({
    TerminalPanel: () => null,
}));

const wrapper = ({ children }: { children: React.ReactNode }) => createElement(TerminalProvider, null, children);

describe("useTerminal", () => {
    it("throws when used outside TerminalProvider", () => {
        expect(() => {
            renderHook(() => useTerminal());
        }).toThrow("useTerminal must be used within a TerminalProvider");
    });

    it("starts with empty sessions and null activeSessionId", () => {
        const { result } = renderHook(() => useTerminal(), { wrapper });

        expect(result.current.sessions).toEqual([]);
        expect(result.current.activeSessionId).toBeNull();
        expect(result.current.hasAnySessions).toBe(false);
    });

    it("provides all expected functions", () => {
        const { result } = renderHook(() => useTerminal(), { wrapper });

        expect(result.current.openExec).toBeInstanceOf(Function);
        expect(result.current.closeSession).toBeInstanceOf(Function);
        expect(result.current.closeAllSessions).toBeInstanceOf(Function);
        expect(result.current.switchSession).toBeInstanceOf(Function);
    });

    it("openExec adds a session and sets it active", () => {
        const { result } = renderHook(() => useTerminal(), { wrapper });

        act(() => {
            result.current.openExec({ containerId: "abc123", nodeId: "node1", containerName: "web" });
        });

        expect(result.current.sessions).toHaveLength(1);
        expect(result.current.sessions[0].target.containerId).toBe("abc123");
        expect(result.current.sessions[0].target.containerName).toBe("web");
        expect(result.current.activeSessionId).toBe(result.current.sessions[0].id);
        expect(result.current.hasAnySessions).toBe(true);
    });

    it("openExec with same containerId switches to existing session (no duplicate)", () => {
        const { result } = renderHook(() => useTerminal(), { wrapper });

        act(() => {
            result.current.openExec({ containerId: "abc123", nodeId: "node1" });
        });
        const firstId = result.current.sessions[0].id;

        act(() => {
            result.current.openExec({ containerId: "def456", nodeId: "node2" });
        });
        expect(result.current.sessions).toHaveLength(2);
        expect(result.current.activeSessionId).toBe(result.current.sessions[1].id);

        // Re-open the first container — should switch, not create a third
        act(() => {
            result.current.openExec({ containerId: "abc123", nodeId: "node1" });
        });
        expect(result.current.sessions).toHaveLength(2);
        expect(result.current.activeSessionId).toBe(firstId);
    });

    it("closeSession removes session and switches active to last remaining", () => {
        const { result } = renderHook(() => useTerminal(), { wrapper });

        act(() => {
            result.current.openExec({ containerId: "a", nodeId: "n1" });
        });
        act(() => {
            result.current.openExec({ containerId: "b", nodeId: "n2" });
        });
        act(() => {
            result.current.openExec({ containerId: "c", nodeId: "n3" });
        });

        const sessionB = result.current.sessions[1];
        const sessionC = result.current.sessions[2];

        // Active is C (last opened). Close C → active should be B
        act(() => {
            result.current.closeSession(sessionC.id);
        });
        expect(result.current.sessions).toHaveLength(2);
        expect(result.current.activeSessionId).toBe(sessionB.id);
    });

    it("closeSession on last session returns to empty state", () => {
        const { result } = renderHook(() => useTerminal(), { wrapper });

        act(() => {
            result.current.openExec({ containerId: "solo", nodeId: "n1" });
        });
        const sessionId = result.current.sessions[0].id;

        act(() => {
            result.current.closeSession(sessionId);
        });
        expect(result.current.sessions).toEqual([]);
        expect(result.current.activeSessionId).toBeNull();
        expect(result.current.hasAnySessions).toBe(false);
    });

    it("closeAllSessions clears everything", () => {
        const { result } = renderHook(() => useTerminal(), { wrapper });

        act(() => {
            result.current.openExec({ containerId: "a", nodeId: "n1" });
        });
        act(() => {
            result.current.openExec({ containerId: "b", nodeId: "n2" });
        });

        act(() => {
            result.current.closeAllSessions();
        });
        expect(result.current.sessions).toEqual([]);
        expect(result.current.activeSessionId).toBeNull();
        expect(result.current.hasAnySessions).toBe(false);
    });

    it("switchSession changes activeSessionId", () => {
        const { result } = renderHook(() => useTerminal(), { wrapper });

        act(() => {
            result.current.openExec({ containerId: "a", nodeId: "n1" });
        });
        act(() => {
            result.current.openExec({ containerId: "b", nodeId: "n2" });
        });

        const firstId = result.current.sessions[0].id;
        expect(result.current.activeSessionId).toBe(result.current.sessions[1].id);

        act(() => {
            result.current.switchSession(firstId);
        });
        expect(result.current.activeSessionId).toBe(firstId);
    });

    it("closing a non-active session does not change activeSessionId", () => {
        const { result } = renderHook(() => useTerminal(), { wrapper });

        act(() => {
            result.current.openExec({ containerId: "a", nodeId: "n1" });
        });
        act(() => {
            result.current.openExec({ containerId: "b", nodeId: "n2" });
        });

        const sessionA = result.current.sessions[0];
        const sessionB = result.current.sessions[1];

        // Active is B. Close A → active should stay B
        act(() => {
            result.current.closeSession(sessionA.id);
        });
        expect(result.current.sessions).toHaveLength(1);
        expect(result.current.activeSessionId).toBe(sessionB.id);
    });
});

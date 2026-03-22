import { createContext } from "react";

export interface ExecTarget {
    containerId: string;
    nodeId: string;
    containerName?: string;
    shell?: string;
    user?: string;
}

export interface TerminalSession {
    id: string;
    target: ExecTarget;
    createdAt: number;
}

export interface TerminalContextValue {
    sessions: TerminalSession[];
    activeSessionId: string | null;
    openExec: (target: ExecTarget, options?: { showPanel?: boolean }) => void;
    closeSession: (sessionId: string) => void;
    closeAllSessions: () => void;
    switchSession: (sessionId: string) => void;
    hasAnySessions: boolean;
    isMinimized: boolean;
    setMinimized: (minimized: boolean) => void;
    showPanel: boolean;
    setShowPanel: (show: boolean) => void;
}

export const TerminalContext = createContext<TerminalContextValue | null>(null);

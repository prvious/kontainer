import { createContext } from "react";

export interface ExecTarget {
    containerId: string;
    nodeId: string;
    containerName?: string;
    shell?: string;
}

export interface TerminalContextValue {
    target: ExecTarget | null;
    openExec: (target: ExecTarget) => void;
    closeExec: () => void;
}

export const TerminalContext = createContext<TerminalContextValue | null>(null);

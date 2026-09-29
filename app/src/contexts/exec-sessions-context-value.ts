import { createContext } from "react";
import type { ExecBuffer } from "@/components/exec/exec-buffer";

/** What a shell runs in. */
export interface ExecTarget {
    nodeId: string;
    containerId: string;
    /** The container's name, for tabs and messages. */
    name: string;
}

/** One Exec tab: a container, the shell's buffer, and the session feeding it. */
export interface ExecTab extends ExecTarget {
    id: string;
    /** Set once `exec_open` answers. */
    sessionId: string | null;
    buffer: ExecBuffer;
    /** The shell asked for, or null for the node's default (bash, then sh). */
    shell: string | null;
}

export interface ExecSessionsContextValue {
    tabs: ExecTab[];
    activeId: string | null;
    select(id: string): void;
    open(target: ExecTarget): void;
    close(id: string): void;
    /** A fresh shell in the same container, optionally a given one. */
    reconnect(id: string, shell?: string | null): void;
    write(id: string, data: string): void;
    resize(id: string, cols: number, rows: number): void;
}

export const ExecSessionsContext = createContext<ExecSessionsContextValue | null>(null);

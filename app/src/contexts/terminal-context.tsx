import { useCallback, useState } from "react";
import { TerminalPanel } from "@/components/terminal-panel";
import { TerminalContext } from "@/contexts/terminal-context-value";
import type { ExecTarget, TerminalSession } from "@/contexts/terminal-context-value";

export function TerminalProvider({ children }: { children: React.ReactNode }) {
    const [sessions, setSessions] = useState<TerminalSession[]>([]);
    const [activeSessionId, setActiveSessionId] = useState<string | null>(null);

    const openExec = useCallback((target: ExecTarget) => {
        setSessions((prev) => {
            const existing = prev.find((s) => s.target.containerId === target.containerId);
            if (existing) {
                setActiveSessionId(existing.id);
                return prev;
            }
            const id = crypto.randomUUID();
            setActiveSessionId(id);
            return [...prev, { id, target, createdAt: Date.now() }];
        });
    }, []);

    const closeSession = useCallback((sessionId: string) => {
        setSessions((prev) => {
            const next = prev.filter((s) => s.id !== sessionId);
            setActiveSessionId((currentActive) => {
                if (currentActive !== sessionId) return currentActive;
                return next.length > 0 ? next[next.length - 1].id : null;
            });
            return next;
        });
    }, []);

    const closeAllSessions = useCallback(() => {
        setSessions([]);
        setActiveSessionId(null);
    }, []);

    const switchSession = useCallback((sessionId: string) => {
        setActiveSessionId(sessionId);
    }, []);

    const hasAnySessions = sessions.length > 0;

    return (
        <TerminalContext value={{ sessions, activeSessionId, openExec, closeSession, closeAllSessions, switchSession, hasAnySessions }}>
            {children}
            {hasAnySessions && <TerminalPanel />}
        </TerminalContext>
    );
}
